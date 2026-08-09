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
//! # Two checks, because a name is not an identity
//!
//! 1. The owning process's **image name** is `msedgewebview2.exe`.
//! 2. That process **descends from us**.
//!
//! The first alone would be spoofable: any local program can name its
//! executable `msedgewebview2.exe`. The second is not, because a process
//! cannot choose its parent — WebView2's browser process is spawned by
//! us and the renderer / network-service children hang off that, so a
//! genuine webview socket always has our PID somewhere up its ancestry.
//! Requiring both means an impostor would have to be a process we
//! ourselves launched, which is not something an outsider can arrange.
//!
//! Ancestry alone is not enough either: we do spawn other children (the
//! game client, NGM, LocaleRemulator), and none of them has any business
//! borrowing this tunnel.
//!
//! # Fail-open, deliberately
//!
//! If a lookup itself fails — the connection already closed and left the
//! table, a snapshot error, an unexpected address family — the
//! connection is **allowed**. The check is defense-in-depth on a
//! loopback port that is off by default; a bug or a race in it must
//! never be able to take the user's webview offline. Only a *positive*
//! identification of something that isn't our webview rejects.
//!
//! Note this is not a hole in practice: to *use* the proxy a caller
//! needs a live connection, and a live connection always has a row in
//! the TCP table. The lookup realistically only misses for sockets that
//! are already gone, which have nothing left to relay.

#[cfg(target_os = "windows")]
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
#[cfg(target_os = "windows")]
use std::sync::Mutex;

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

    // Cheap check first: the image name filters out every ordinary
    // caller (a script, a curl, another app) without touching the
    // process snapshot.
    let Some(name) = image_name(pid) else {
        return true;
    };
    if !name.eq_ignore_ascii_case(WEBVIEW_IMAGE_NAME) {
        tracing::warn!(pid, image = %name, "webview proxy: refused a connection from another process");
        return false;
    }

    // Expensive check second: a matching name only counts if the
    // process is actually one we launched.
    match descends_from_us(pid) {
        Some(true) => true,
        Some(false) => {
            tracing::warn!(
                pid,
                image = %name,
                "webview proxy: refused a look-alike process that is not our webview"
            );
            false
        }
        None => true,
    }
}

/// Verified descendants, so a busy page load pays for the process
/// snapshot once rather than once per connection.
///
/// Caching a PID is safe against PID reuse because the image-name check
/// above runs on *every* connection and is not cached: a recycled PID
/// only reaches this cache if the new occupant is itself named
/// `msedgewebview2.exe`.
#[cfg(target_os = "windows")]
static VERIFIED_DESCENDANTS: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

/// Longest ancestry chain we will walk before giving up. WebView2 sits
/// two hops from us (network service → browser process → us); the extra
/// room costs nothing and the bound guarantees termination even if the
/// snapshot ever contains a cycle.
#[cfg(target_os = "windows")]
const MAX_ANCESTRY_HOPS: usize = 16;

/// Does `pid` have our own process somewhere up its parent chain?
///
/// `None` means the question could not be answered (snapshot failure) —
/// callers treat that as "allow", per the fail-open policy above.
#[cfg(target_os = "windows")]
fn descends_from_us(pid: u32) -> Option<bool> {
    if let Ok(cache) = VERIFIED_DESCENDANTS.lock() {
        if cache.as_ref().is_some_and(|seen| seen.contains(&pid)) {
            return Some(true);
        }
    }

    let parents = parent_map()?;
    let me = std::process::id();

    let mut current = pid;
    for _ in 0..MAX_ANCESTRY_HOPS {
        // A link we cannot resolve means the process (or an ancestor)
        // has exited, so the chain cannot reach us. That is a definite
        // "not ours" — unlike a snapshot failure, which is unknowable
        // and handled by the `?` on `parent_map` above.
        let Some(&parent) = parents.get(&current) else {
            return Some(false);
        };
        if parent == me {
            if let Ok(mut cache) = VERIFIED_DESCENDANTS.lock() {
                cache.get_or_insert_with(HashSet::new).insert(pid);
            }
            return Some(true);
        }
        // PID 0 is the idle process and a self-parent is corrupt data;
        // either way the chain is over and we never found ourselves.
        if parent == 0 || parent == current {
            return Some(false);
        }
        current = parent;
    }
    Some(false)
}

/// Snapshot every live process as a `pid -> parent pid` map.
#[cfg(target_os = "windows")]
fn parent_map() -> Option<HashMap<u32, u32>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: the snapshot handle is closed on every path below, and
    // `entry` is initialised with the size field the API requires.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    let mut map = HashMap::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    unsafe {
        let _ = CloseHandle(snapshot);
    }

    if map.is_empty() {
        return None;
    }
    Some(map)
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
    fn the_process_snapshot_sees_this_process_and_its_parent() {
        let parents = parent_map().expect("snapshot");
        assert!(parents.contains_key(&std::process::id()));
    }

    #[test]
    fn a_process_we_launched_is_recognised_as_our_descendant() {
        // The real guarantee: an impostor cannot fake this, because it
        // cannot arrange for us to be its parent.
        let mut child = std::process::Command::new("cmd.exe")
            .args(["/c", "ping -n 6 127.0.0.1 > nul"])
            .spawn()
            .expect("spawn child");

        let verdict = descends_from_us(child.id());
        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(verdict, Some(true));
    }

    #[test]
    fn a_process_outside_our_tree_is_rejected() {
        // PID 4 is the Windows System process — parented by the idle
        // process, so the walk terminates without ever reaching us.
        assert_eq!(descends_from_us(4), Some(false));
    }

    #[test]
    fn an_unknown_pid_is_rejected_rather_than_walked_forever() {
        assert_eq!(descends_from_us(u32::MAX), Some(false));
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
