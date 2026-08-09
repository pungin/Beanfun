//! HTTP request-head parsing for the loopback webview proxy.
//!
//! Deliberately hand-rolled and tiny rather than pulling in a full HTTP
//! stack: the proxy only ever needs to understand *the first request
//! head on a connection* well enough to decide where to dial. Once the
//! upstream socket is open the two halves are relayed byte-for-byte, so
//! nothing here has to model bodies, chunking, or trailers.
//!
//! Everything in this module is pure — no sockets, no async — so the
//! parsing rules are unit-testable on their own.

/// Hard cap on the request head we are willing to buffer before giving
/// up. Chromium's proxy requests are a few hundred bytes; anything past
/// this is a malformed or hostile peer, not a real request.
pub const MAX_HEAD_BYTES: usize = 32 * 1024;

/// Default port when a `CONNECT` authority omits one. `CONNECT` without
/// a port is not something Chromium emits, but the RFC allows the
/// abbreviated form and 443 is the only sane reading of it.
const DEFAULT_CONNECT_PORT: u16 = 443;

/// Default port for an absolute-form `http://` target.
const DEFAULT_HTTP_PORT: u16 = 80;

/// A parsed request line plus its headers, borrowed as owned strings
/// (the head is at most [`MAX_HEAD_BYTES`], so the copy is cheap and
/// spares every caller a lifetime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHead {
    pub method: String,
    /// Request target verbatim: an authority (`host:port`) for
    /// `CONNECT`, an absolute URI otherwise.
    pub target: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
}

impl RequestHead {
    /// True for the tunnel-establishing verb. Case-insensitive because
    /// the method token is technically case-sensitive per RFC but being
    /// lenient on input costs nothing here.
    pub fn is_connect(&self) -> bool {
        self.method.eq_ignore_ascii_case("CONNECT")
    }
}

/// Byte offset just past the `\r\n\r\n` that ends the head, or `None`
/// while the head is still incomplete.
pub fn head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// Parse the head bytes (everything up to and including the blank
/// line). Returns `None` for anything that isn't a plausible request.
pub fn parse_head(buf: &[u8]) -> Option<RequestHead> {
    // Heads are ASCII by spec; a non-UTF-8 head is malformed input we
    // are happy to reject outright.
    let text = std::str::from_utf8(buf).ok()?;
    let mut lines = text.split("\r\n");

    let mut request_line = lines.next()?.split(' ');
    let method = request_line.next()?.trim();
    let target = request_line.next()?.trim();
    let version = request_line.next()?.trim();
    if method.is_empty() || target.is_empty() || !version.starts_with("HTTP/") {
        return None;
    }

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        // A header without a colon is malformed; skip it rather than
        // rejecting the whole request — the relay is not a validator.
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }

    Some(RequestHead {
        method: method.to_string(),
        target: target.to_string(),
        version: version.to_string(),
        headers,
    })
}

/// Split a `CONNECT` authority into host and port.
///
/// Handles the bracketed IPv6 literal form (`[::1]:443`) because
/// Chromium emits it for IPv6 origins.
pub fn connect_authority(target: &str) -> Option<(String, u16)> {
    if let Some(rest) = target.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        if host.is_empty() {
            return None;
        }
        let port = match tail.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None => DEFAULT_CONNECT_PORT,
        };
        return Some((host.to_string(), port));
    }

    match target.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => Some((host.to_string(), port.parse().ok()?)),
        // No colon at all — bare hostname, assume the https port.
        None if !target.is_empty() => Some((target.to_string(), DEFAULT_CONNECT_PORT)),
        _ => None,
    }
}

/// Split an absolute-form `http://` target into host, port, and the
/// origin-form path to forward upstream.
///
/// `https://` is intentionally rejected: a proxy only ever sees an
/// absolute `https` URI from a client that expects us to terminate TLS,
/// which this proxy deliberately never does (see the module docs on
/// [`super`]).
pub fn absolute_target(target: &str) -> Option<(String, u16, String)> {
    let rest = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("HTTP://"))?;

    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    // Strip any userinfo — we never forward credentials we were not
    // asked to forward.
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);

    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, tail) = inner.split_once(']')?;
        let port = match tail.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None => DEFAULT_HTTP_PORT,
        };
        (host.to_string(), port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) if !h.is_empty() => (h.to_string(), p.parse().ok()?),
            None if !authority.is_empty() => (authority.to_string(), DEFAULT_HTTP_PORT),
            _ => return None,
        }
    };

    Some((host, port, path.to_string()))
}

/// Headers that belong to the client↔proxy hop and must never be
/// forwarded to the origin server.
fn is_hop_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("proxy-connection")
        || name.eq_ignore_ascii_case("proxy-authorization")
        || name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("keep-alive")
}

/// Rebuild the head in origin form for the upstream socket.
///
/// # Why `Connection: close`
///
/// In non-`CONNECT` mode Chromium is free to reuse one proxy connection
/// for requests to *different* origins. Our upstream socket is pinned to
/// the host of the first request, so a reused connection would deliver
/// request #2 to the wrong server. Closing after one request removes
/// that whole class of bug. The cost is negligible in practice: the
/// beanfun portals are https, so effectively everything goes through the
/// `CONNECT` path, which keeps its tunnel open normally.
pub fn rewrite_origin_form(head: &RequestHead, path: &str) -> Vec<u8> {
    let mut out = format!("{} {} {}\r\n", head.method, path, head.version);
    for (name, value) in &head.headers {
        if is_hop_header(name) {
            continue;
        }
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    out.push_str("Connection: close\r\n\r\n");
    out.into_bytes()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_end_waits_for_the_blank_line() {
        assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: a\r\n"), None);
        assert_eq!(head_end(b"GET / HTTP/1.1\r\n\r\nbody"), Some(18));
    }

    #[test]
    fn parses_a_connect_request() {
        let head =
            parse_head(b"CONNECT tw.beanfun.com:443 HTTP/1.1\r\nHost: tw.beanfun.com:443\r\n\r\n")
                .expect("parses");
        assert!(head.is_connect());
        assert_eq!(head.target, "tw.beanfun.com:443");
        assert_eq!(
            head.headers,
            vec![("Host".into(), "tw.beanfun.com:443".into())]
        );
    }

    #[test]
    fn rejects_a_head_without_a_version() {
        assert!(parse_head(b"GET /\r\n\r\n").is_none());
        assert!(parse_head(b"\r\n\r\n").is_none());
    }

    #[test]
    fn skips_a_malformed_header_line_instead_of_failing() {
        let head =
            parse_head(b"GET http://a/ HTTP/1.1\r\ngarbage\r\nHost: a\r\n\r\n").expect("parses");
        assert_eq!(head.headers, vec![("Host".into(), "a".into())]);
    }

    #[test]
    fn connect_authority_defaults_to_the_https_port() {
        assert_eq!(
            connect_authority("a.example"),
            Some(("a.example".into(), 443))
        );
        assert_eq!(
            connect_authority("a.example:8443"),
            Some(("a.example".into(), 8443))
        );
    }

    #[test]
    fn connect_authority_handles_bracketed_ipv6() {
        assert_eq!(connect_authority("[::1]:443"), Some(("::1".into(), 443)));
        assert_eq!(connect_authority("[::1]"), Some(("::1".into(), 443)));
    }

    #[test]
    fn connect_authority_rejects_junk() {
        assert_eq!(connect_authority(""), None);
        assert_eq!(connect_authority(":443"), None);
        assert_eq!(connect_authority("a.example:not-a-port"), None);
    }

    #[test]
    fn absolute_target_splits_host_port_and_path() {
        assert_eq!(
            absolute_target("http://tw.beanfun.com/game/index.aspx?x=1"),
            Some(("tw.beanfun.com".into(), 80, "/game/index.aspx?x=1".into()))
        );
        assert_eq!(
            absolute_target("http://a.example:8080"),
            Some(("a.example".into(), 8080, "/".into()))
        );
    }

    #[test]
    fn absolute_target_drops_userinfo() {
        assert_eq!(
            absolute_target("http://user:pw@a.example/x"),
            Some(("a.example".into(), 80, "/x".into()))
        );
    }

    #[test]
    fn absolute_target_refuses_a_non_http_scheme() {
        // We never terminate TLS, so an absolute https URI is not ours
        // to serve — it must arrive as CONNECT.
        assert_eq!(absolute_target("https://a.example/"), None);
        assert_eq!(absolute_target("/relative"), None);
    }

    #[test]
    fn rewrite_strips_hop_headers_and_closes_the_connection() {
        let head = parse_head(
            b"GET http://a.example/x HTTP/1.1\r\n\
              Host: a.example\r\n\
              Proxy-Connection: keep-alive\r\n\
              Proxy-Authorization: Basic zzz\r\n\
              Connection: keep-alive\r\n\
              User-Agent: probe\r\n\r\n",
        )
        .expect("parses");

        let out = String::from_utf8(rewrite_origin_form(&head, "/x")).expect("utf8");

        assert!(out.starts_with("GET /x HTTP/1.1\r\n"));
        assert!(out.contains("Host: a.example\r\n"));
        assert!(out.contains("User-Agent: probe\r\n"));
        assert!(!out.to_lowercase().contains("proxy-connection"));
        assert!(!out.to_lowercase().contains("proxy-authorization"));
        assert!(!out.contains("keep-alive"));
        assert!(out.ends_with("Connection: close\r\n\r\n"));
    }
}
