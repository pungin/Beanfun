//! A loopback HTTP proxy that lends the webview our process identity.
//!
//! # The problem
//!
//! WebView2 is always out-of-process: every request the app's web
//! surfaces make — the GamaPass popup, the MapleStory Classic portal,
//! the in-app browser — leaves the machine on a socket owned by
//! `msedgewebview2.exe`, never by `beanfun.exe`. Most Taiwanese/HK game
//! accelerators and split-tunnel VPNs pick traffic up **by process**, so
//! they match `beanfun.exe`, see nothing, and those windows quietly run
//! unaccelerated (issue #356 follow-up). Users experience it as "the
//! accelerator doesn't work on the login popup", and there is nothing
//! they can configure to fix it: the app's regular login *is*
//! accelerated, because that path goes through `reqwest` in our own
//! process.
//!
//! # The fix
//!
//! Run a tiny proxy inside `beanfun.exe` and point the webview at it
//! with `--proxy-server=http://127.0.0.1:<port>`:
//!
//! ```text
//! before:  msedgewebview2.exe ─────────────────────────→ tw.beanfun.com
//! after:   msedgewebview2.exe ──→ 127.0.0.1:<port>
//!                                   (beanfun.exe) ─────→ tw.beanfun.com
//! ```
//!
//! The outbound socket is now opened by us, so a process-matching
//! accelerator sees it. Nothing outside the app changes: no system proxy,
//! no registry, no WinINET/WinHTTP settings, no effect on any other
//! program. The loopback hop is not intercepted (accelerator filters skip
//! loopback), and the user has nothing to fill in — the port is ours and
//! ephemeral.
//!
//! # What this is not
//!
//! `CONNECT` is served as a blind byte relay, so https traffic is
//! **tunnelled, never terminated**: no certificate is generated, no TLS
//! is intercepted, and the proxy cannot read what passes through it. The
//! absolute-form `http://` path exists only because a proxy is required
//! to answer it; it forwards the head and relays the rest.
//!
//! # Lifetime and failure
//!
//! The listener is bound *synchronously* at startup so the port is known
//! before the browser arguments are assembled, then served from a
//! dedicated thread for the life of the process. If binding fails the
//! caller simply omits `--proxy-server` and the webview goes direct —
//! the feature degrades to "as before", never to "no network".
//!
//! Access is restricted to our own webview processes; see [`peer`].

mod head;
mod peer;

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use head::{
    absolute_target, connect_authority, head_end, parse_head, rewrite_origin_form, RequestHead,
    MAX_HEAD_BYTES,
};

/// How long a client may take to send a complete request head.
const HEAD_TIMEOUT: Duration = Duration::from_secs(15);

/// How long to wait for the upstream TCP connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Bind the loopback proxy and start serving it on a dedicated thread.
///
/// Returns the port the webview should be pointed at. The listener is
/// bound before this returns, so a successful result means the port is
/// already accepting.
pub fn start() -> std::io::Result<u16> {
    // Bind synchronously: the caller needs the port *now*, to put into
    // the WebView2 browser arguments before any window is created.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    std::thread::Builder::new()
        .name("beanfun-webview-proxy".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(%error, "webview proxy: runtime build failed");
                    return;
                }
            };
            runtime.block_on(serve(listener, port));
        })?;

    Ok(port)
}

/// Accept loop. Per-connection failures are logged and dropped; only a
/// listener-level failure can end it, and even then it backs off rather
/// than spinning.
async fn serve(listener: std::net::TcpListener, port: u16) {
    let listener = match TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%error, "webview proxy: could not adopt the listener");
            return;
        }
    };
    tracing::info!(port, "webview proxy listening on loopback");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                tokio::spawn(async move {
                    if let Err(error) = handle(stream, peer_addr, port).await {
                        // Half-closed tunnels are the normal way a page
                        // navigation ends, so this stays at debug.
                        tracing::debug!(%error, "webview proxy: connection ended");
                    }
                });
            }
            Err(error) => {
                tracing::warn!(%error, "webview proxy: accept failed");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

/// Serve one client connection: read its head, dial upstream, relay.
async fn handle(
    mut client: TcpStream,
    peer_addr: SocketAddr,
    listen_port: u16,
) -> std::io::Result<()> {
    if !peer::peer_is_webview(peer_addr, listen_port) {
        return Ok(());
    }

    let (buf, end) = read_head(&mut client).await?;
    let Some(request) = parse_head(&buf[..end]) else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return Ok(());
    };

    let mut upstream = match dial(&request).await {
        Some(upstream) => upstream,
        None => {
            let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
            return Ok(());
        }
    };

    if request.is_connect() {
        // The tunnel is opened; from here we are a pipe and nothing more.
        client
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await?;
    } else {
        let Some((_, _, path)) = absolute_target(&request.target) else {
            return Ok(());
        };
        upstream
            .write_all(&rewrite_origin_form(&request, &path))
            .await?;
    }

    // Anything already buffered past the head belongs to the body /
    // tunnel and must not be dropped.
    if buf.len() > end {
        upstream.write_all(&buf[end..]).await?;
    }

    copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

/// Read until the head is complete, returning the buffer and the offset
/// just past the blank line.
async fn read_head(client: &mut TcpStream) -> std::io::Result<(Vec<u8>, usize)> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];

    loop {
        if let Some(end) = head_end(&buf) {
            return Ok((buf, end));
        }
        if buf.len() > MAX_HEAD_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
        let read = tokio::time::timeout(HEAD_TIMEOUT, client.read(&mut chunk))
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "head read timed out")
            })?;
        match read? {
            0 => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "client closed before the head was complete",
                ))
            }
            n => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

/// Open the upstream socket for a parsed request. This is the call whose
/// process identity the whole module exists for.
async fn dial(request: &RequestHead) -> Option<TcpStream> {
    let (host, port) = if request.is_connect() {
        connect_authority(&request.target)?
    } else {
        let (host, port, _) = absolute_target(&request.target)?;
        (host, port)
    };

    match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(stream)) => Some(stream),
        Ok(Err(error)) => {
            tracing::debug!(%host, port, %error, "webview proxy: upstream connect failed");
            None
        }
        Err(_) => {
            tracing::debug!(%host, port, "webview proxy: upstream connect timed out");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// End-to-end through the real listener: a `CONNECT` tunnel must
    /// reach an origin server and relay both directions verbatim.
    #[test]
    fn connect_tunnels_bytes_to_the_origin_in_both_directions() {
        // A trivial origin that echoes one line back, uppercased.
        let origin = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind origin");
        let origin_port = origin.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            let (mut socket, _) = origin.accept().expect("accept");
            let mut got = [0u8; 5];
            socket.read_exact(&mut got).expect("read");
            socket
                .write_all(got.to_ascii_uppercase().as_slice())
                .expect("write");
        });

        let port = start().expect("proxy starts");

        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect proxy");
        client
            .write_all(format!("CONNECT 127.0.0.1:{origin_port} HTTP/1.1\r\n\r\n").as_bytes())
            .expect("send CONNECT");

        let mut response = [0u8; 39];
        client.read_exact(&mut response).expect("read response");
        assert!(
            String::from_utf8_lossy(&response).starts_with("HTTP/1.1 200"),
            "got {:?}",
            String::from_utf8_lossy(&response)
        );

        client.write_all(b"hello").expect("send payload");
        let mut echoed = [0u8; 5];
        client.read_exact(&mut echoed).expect("read echo");
        assert_eq!(&echoed, b"HELLO");
    }

    /// The plain-HTTP path must arrive at the origin in origin form,
    /// with the proxy hop headers gone.
    #[test]
    fn absolute_form_requests_are_rewritten_to_origin_form() {
        let origin = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind origin");
        let origin_port = origin.local_addr().expect("addr").port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let seen_writer = std::sync::Arc::clone(&seen);
        std::thread::spawn(move || {
            let (mut socket, _) = origin.accept().expect("accept");
            let mut buf = [0u8; 512];
            let n = socket.read(&mut buf).expect("read");
            *seen_writer.lock().expect("lock") = String::from_utf8_lossy(&buf[..n]).into_owned();
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
                .expect("write");
        });

        let port = start().expect("proxy starts");
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect proxy");
        client
            .write_all(
                format!(
                    "GET http://127.0.0.1:{origin_port}/probe HTTP/1.1\r\n\
                     Host: 127.0.0.1:{origin_port}\r\n\
                     Proxy-Connection: keep-alive\r\n\r\n"
                )
                .as_bytes(),
            )
            .expect("send request");

        let mut response = Vec::new();
        client.read_to_end(&mut response).expect("read response");
        assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 204"));

        let request = seen.lock().expect("lock").clone();
        assert!(
            request.starts_with("GET /probe HTTP/1.1\r\n"),
            "got {request}"
        );
        assert!(!request.to_lowercase().contains("proxy-connection"));
    }

    /// A head that never terminates must not be buffered without bound.
    #[test]
    fn an_unterminated_head_is_rejected_rather_than_buffered_forever() {
        let port = start().expect("proxy starts");
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect proxy");

        let filler = vec![b'x'; 4096];
        // Once past MAX_HEAD_BYTES the proxy drops us, which surfaces
        // here as a write error or a closed read — either is the point.
        for _ in 0..(MAX_HEAD_BYTES / filler.len() + 4) {
            if client.write_all(&filler).is_err() {
                return;
            }
        }
        let mut sink = Vec::new();
        let _ = client.read_to_end(&mut sink);
        assert!(sink.is_empty(), "the proxy must not answer a junk head");
    }
}
