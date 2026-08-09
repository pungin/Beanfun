//! Who is allowed to use the loopback proxy.
//!
//! The listener binds `127.0.0.1` on an ephemeral port, so it is not
//! reachable off-machine — but every *local* process can still reach it,
//! and an open local proxy is a capability we do not want to hand out:
//! it would let any program on the box borrow `beanfun.exe`'s identity
//! for outbound traffic (which is precisely the thing that makes the
//! proxy useful to us, and precisely why someone else might want it).
//!
//! So each accepted connection is traced back to the process that owns
//! the client socket, and only the WebView2 runtime — or this process
//! itself, which already holds every capability the proxy could lend —
//! is served.
//!
//! # Fail-open, deliberately
//!
//! If the ownership lookup itself fails — the connection already closed
//! and left the table, an API error, an unexpected address family — the
//! connection is **allowed**. The check is defense-in-depth on a
//! loopback port that is off by default; a bug or a race in it must
//! never be able to take the user's webview offline. Only a *positive*
//! identification of some other process rejects.

use std::net::SocketAddr;

/// Image name of the WebView2 runtime's processes. Every socket the
/// webview opens is owned by one of these (the network service child,
/// in practice) — never by `beanfun.exe` itself, which is the whole
/// reason this proxy exists.
#[cfg(target_os = "windows")]
const WEBVIEW_IMAGE_NAME: &str = "msedgewebview2.exe";

/// Convert the port field of a `MIB_TCPROW_OWNER_PID` (network byte
/// order stashed in the low word of a `DWORD`) into a host-order port.
#[cfg(target_os = "windows")]
fn port_from_row(raw: u32) -> u16 {
    (((raw & 0xFF) as u16) << 8) | ((raw >> 8) & 0xFF) as u16
}

/// Is this peer one of our own webview processes?
///
/// `peer` is the client end of an accepted connection and `listen_port`
/// the port it connected to; together they identify one row of the TCP
/// table uniquely.
#[cfg(target_os = "windows")]
pub fn peer_is_webview(peer: SocketAddr, listen_port: u16) -> bool {
    let Some(pid) = owning_pid(peer, listen_port) else {
        // Could not attribute the socket — see the fail-open note above.
        return true;
    };
    // Our own process is trivially allowed: it already has every
    // capability the proxy could lend it, so refusing would buy nothing
    // and would make the relay untestable in-process.
    if pid == std::process::id() {
        return true;
    }
    match image_name(pid) {
        Some(name) => {
            let ours = name.eq_ignore_ascii_case(WEBVIEW_IMAGE_NAME);
            if !ours {
                tracing::warn!(pid, image = %name, "webview proxy: refused a connection from another process");
            }
            ours
        }
        None => true,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn peer_is_webview(_peer: SocketAddr, _listen_port: u16) -> bool {
    // The proxy is only ever wired up on Windows (it exists to give
    // WebView2's out-of-process traffic our own process identity). The
    // stub keeps the module compiling — and unit-testable — elsewhere.
    true
}

/// PID owning the loopback socket `peer` → `127.0.0.1:listen_port`.
#[cfg(target_os = "windows")]
fn owning_pid(peer: SocketAddr, listen_port: u16) -> Option<u32> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_CONNECTIONS,
    };
    use windows::Win32::Networking::WinSock::AF_INET;

    // We bind 127.0.0.1, so anything that reaches us is IPv4.
    let SocketAddr::V4(peer) = peer else {
        return None;
    };

    let mut size: u32 = 0;
    // First call sizes the buffer; a zero-size query is expected to
    // "fail" with ERROR_INSUFFICIENT_BUFFER, so the return value is
    // deliberately ignored here and validated on the real call.
    unsafe {
        GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        );
    }
    if size == 0 {
        return None;
    }

    let mut buf = vec![0u8; size as usize];
    let rc = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr().cast()),
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    if rc != 0 {
        return None;
    }

    // SAFETY: on success the buffer holds a MIB_TCPTABLE_OWNER_PID —
    // a row count followed by that many rows. `buf` outlives the reads
    // and is at least `size` bytes because the API just filled it.
    let table = buf.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
    let count = unsafe { (*table).dwNumEntries } as usize;
    let rows = unsafe { std::ptr::addr_of!((*table).table).cast::<MIB_TCPROW_OWNER_PID>() };

    let peer_ip = u32::from(*peer.ip()).to_be();
    for i in 0..count {
        let row = unsafe { &*rows.add(i) };
        if port_from_row(row.dwLocalPort) == peer.port()
            && port_from_row(row.dwRemotePort) == listen_port
            && row.dwLocalAddr == peer_ip
        {
            return Some(row.dwOwningPid);
        }
    }
    None
}

/// File name (no directory) of a PID's executable image.
#[cfg(target_os = "windows")]
fn image_name(pid: u32) -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: a plain Win32 handle round-trip. The handle is closed on
    // every path below before the value is returned.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut buf = [0u16; 260 * 2];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .is_ok();
    unsafe {
        let _ = CloseHandle(handle);
    }
    if !ok {
        return None;
    }

    let full = String::from_utf16_lossy(&buf[..len as usize]);
    Some(full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn port_field_is_read_as_network_byte_order() {
        // 0x1F90 == 8080; the API stores it byte-swapped in the low word.
        assert_eq!(port_from_row(0x0000_901F), 8080);
        assert_eq!(port_from_row(0x0000_BB01), 443);
        assert_eq!(port_from_row(0), 0);
    }

    #[test]
    fn the_high_word_of_the_port_field_is_ignored() {
        // Windows leaves the upper 16 bits unspecified — they must not
        // leak into the comparison.
        assert_eq!(port_from_row(0xDEAD_901F), 8080);
    }

    #[test]
    fn our_own_process_is_not_mistaken_for_the_webview() {
        let me = std::process::id();
        let name = image_name(me).expect("own image name is readable");
        assert!(!name.eq_ignore_ascii_case(WEBVIEW_IMAGE_NAME), "got {name}");
    }

    #[test]
    fn an_unattributable_peer_fails_open() {
        // Nothing is connected on this pair, so the table lookup finds
        // no row — the connection must still be allowed.
        let peer: SocketAddr = "127.0.0.1:1".parse().expect("addr");
        assert!(peer_is_webview(peer, 2));
    }
}
