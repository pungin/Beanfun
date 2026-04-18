//! Typed errors for [`services/process`][`super`].
//!
//! Declared up-front for chunk 9.1 so the enum shape is stable across
//! 9.1 / 9.2 / 9.3 / 10.3. 9.1 landed the first five variants, 9.2 added
//! [`PostMessage`][ProcessError::PostMessage], 9.3 added
//! [`NonAscii`][ProcessError::NonAscii] for the auto-paste Win32 wrappers,
//! and 10.3 D5d adds [`WindowNotFound`][ProcessError::WindowNotFound] so
//! orchestration call sites can distinguish "no matching top-level window"
//! from the backend-failure variants.
//!
//! # WPF mapping
//!
//! | Variant                | WPF origin                                                                                              |
//! | ---------------------- | ------------------------------------------------------------------------------------------------------- |
//! | [`WmiInit`]            | **beanfun-next exclusive** — `ManagementObjectSearcher` inits COM for us                                |
//! | [`WmiConnect`]         | **beanfun-next exclusive** — same                                                                       |
//! | [`WmiQuery`]           | `MainWindow.xaml.cs` L1775-1795 `ManagementObjectSearcher.Get()` throwing                               |
//! | [`OpenProcess`]        | `MainWindow.xaml.cs` L1823 `Process.GetProcessById(pid)` throwing                                       |
//! | [`TerminateProcess`]   | `MainWindow.xaml.cs` L1831 `Process.Kill()` throwing                                                    |
//! | [`PostMessage`]        | `MainWindow.xaml.cs` L2450 `WindowsAPI.PostMessage(hWnd, WM_CLOSE, …)`                                  |
//! | [`NonAscii`]           | **beanfun-next exclusive** — `WindowsAPI.cs:25` silently maps non-ASCII to `'?'` via `ASCIIEncoding`    |
//! | [`Win32Call`]          | **beanfun-next exclusive** — generic shape for "must-succeed" Win32 calls (D5+ `GetClientRect`, etc.)   |
//! | [`WindowNotFound`]     | `MainWindow.xaml.cs` L2158-2162 `FindWindow` returning `IntPtr.Zero` (P10.3 D5d auto-paste preflight)   |
//!
//! [`WmiInit`]: ProcessError::WmiInit
//! [`WmiConnect`]: ProcessError::WmiConnect
//! [`WmiQuery`]: ProcessError::WmiQuery
//! [`OpenProcess`]: ProcessError::OpenProcess
//! [`TerminateProcess`]: ProcessError::TerminateProcess
//! [`PostMessage`]: ProcessError::PostMessage
//! [`NonAscii`]: ProcessError::NonAscii
//! [`Win32Call`]: ProcessError::Win32Call
//! [`WindowNotFound`]: ProcessError::WindowNotFound

/// Every failure that [`services/process`][`super`] can surface.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// `COMLibrary::new()` failed — another COM apartment mode was
    /// already active on this thread, or `CoInitializeEx` ran out of
    /// system resources. Rare in practice; callers retrying on a fresh
    /// thread usually recover.
    #[error("failed to initialize COM for WMI")]
    WmiInit(#[source] wmi::WMIError),

    /// `WMIConnection::new(com)` failed — typically "Windows Management
    /// Instrumentation" service is stopped/disabled, or the caller lacks
    /// permission on the `root\cimv2` namespace.
    #[error("failed to connect to WMI namespace")]
    WmiConnect(#[source] wmi::WMIError),

    /// A WQL query returned non-success. `query` is the exact WQL string
    /// sent to WMI (useful for diagnostics, no secrets inside — the
    /// input to WMI queries in this module is always a process name).
    #[error("WMI query failed: {query}")]
    WmiQuery {
        query: String,
        #[source]
        source: wmi::WMIError,
    },

    /// `OpenProcess(PROCESS_TERMINATE, _, pid)` failed — `pid` no longer
    /// exists, the calling process lacks SE_DEBUG_NAME / lacks
    /// permission, or `pid` points at a protected/critical system
    /// process (e.g. `System` = 4). `source` carries the raw
    /// `GetLastError` via [`windows::core::Error`].
    #[error("OpenProcess failed for pid {pid}")]
    OpenProcess {
        pid: u32,
        #[source]
        source: windows::core::Error,
    },

    /// `OpenProcess` succeeded but `TerminateProcess` failed before we
    /// could close the handle. Rare (the primary cause would be a
    /// critical-process mark set after `OpenProcess` returned).
    #[error("TerminateProcess failed for pid {pid}")]
    TerminateProcess {
        pid: u32,
        #[source]
        source: windows::core::Error,
    },

    /// `PostMessageW` returned failure after [`FindWindowW`][fw] found
    /// a window. The most common cause is the window being destroyed
    /// between the find and the post (race condition). `hwnd` is the
    /// raw window handle reinterpreted as `usize` for logging —
    /// `HWND` is pointer-sized and never semantically negative, so
    /// `usize` is the narrower, more faithful integer shape than a
    /// signed cast.
    ///
    /// [fw]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-findwindoww
    #[error("PostMessageW failed for HWND {hwnd:#x}")]
    PostMessage {
        hwnd: usize,
        #[source]
        source: windows::core::Error,
    },

    /// Auto-paste input contains a non-ASCII character. The Win32
    /// auto-paste path used by chunk 9.3 is byte-oriented (`WM_CHAR`
    /// with a single `u8` payload per message), so codepoints outside
    /// `0x00..=0x7F` cannot be expressed.
    ///
    /// WPF (`WindowsAPI.cs:25`) silently replaces non-ASCII with `'?'`
    /// via `ASCIIEncoding.ASCII.GetBytes`. This crate surfaces the
    /// failure instead — silently corrupting credential input fails
    /// the user too quietly. `offset` is the byte index of the first
    /// offending character within the original `&str` (consistent with
    /// `Utf8Error::valid_up_to()`); `s[..offset]` slices the
    /// ASCII-safe prefix if a partial flush is desired.
    #[error("input contains non-ASCII character {ch:?} at byte offset {offset}")]
    NonAscii { offset: usize, ch: char },

    /// Generic shape for "must-succeed" Win32 calls in chunk 9.3 whose
    /// failure modes are uniformly "the underlying handle just became
    /// invalid" or "system refused for security reasons" — the family
    /// of [`GetClientRect`][gcr] / [`ClientToScreen`][cts] used by the
    /// click-positioning portion of the auto-paste flow.
    ///
    /// `name` is the Win32 function name as a string literal so log
    /// records can pinpoint the call site without keeping the full
    /// stack frame. WPF discards these failures entirely and uses the
    /// resulting garbage values (`Size.Empty` / unconverted `Point`),
    /// which sends the synthetic mouse click to the wrong screen
    /// coordinates — surfacing instead lets P10 recover (re-find the
    /// window) or warn the user.
    ///
    /// "Best-effort" companions like
    /// [`get_cursor_pos`][crate::services::process::get_cursor_pos]
    /// (D6) intentionally use `Option`/`bool` rather than this
    /// variant — see chunk 9.3 D5/D6 design notes.
    ///
    /// [gcr]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect
    /// [cts]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-clienttoscreen
    #[error("Win32 call {name} failed")]
    Win32Call {
        name: &'static str,
        #[source]
        source: windows::core::Error,
    },

    /// The target window for an orchestration (chunk 10.3 D5d auto-
    /// paste) could not be located. `primary_class` is the class
    /// name the orchestrator looked up first; `fallback_class` is
    /// the secondary class name it also tried (if any). Both come
    /// back in the error payload so the command layer can surface
    /// "tried `MapleStoryClass`, then `MapleStoryClassTW`, still no
    /// match" to the frontend for a clipboard-copy fallback.
    ///
    /// Distinct from [`WmiQuery`] / [`OpenProcess`] (which describe
    /// backend failures) and from `find_window` returning `None`
    /// (which, standalone, is a routine non-error outcome) — this
    /// variant marks a call site where "no matching window" means
    /// the orchestration cannot proceed and callers must surface
    /// the failure.
    ///
    /// [`WmiQuery`]: ProcessError::WmiQuery
    /// [`OpenProcess`]: ProcessError::OpenProcess
    #[error(
        "target window not found (primary class: {primary_class:?}, fallback class: {fallback_class:?})"
    )]
    WindowNotFound {
        primary_class: String,
        fallback_class: Option<String>,
    },
}
