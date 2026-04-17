//! Typed errors for [`services/process`][`super`].
//!
//! Declared up-front for chunk 9.1 so the enum shape is stable across
//! 9.1 / 9.2 / 9.3. Variants that the auto-paste Win32 wrappers (9.3)
//! will add land here when that chunk opens; 9.1 landed the first five,
//! 9.2 adds [`PostMessage`][ProcessError::PostMessage].
//!
//! # WPF mapping
//!
//! | Variant                | WPF origin                                                                |
//! | ---------------------- | ------------------------------------------------------------------------- |
//! | [`WmiInit`]            | **beanfun-next exclusive** — `ManagementObjectSearcher` inits COM for us  |
//! | [`WmiConnect`]         | **beanfun-next exclusive** — same                                         |
//! | [`WmiQuery`]           | `MainWindow.xaml.cs` L1775-1795 `ManagementObjectSearcher.Get()` throwing |
//! | [`OpenProcess`]        | `MainWindow.xaml.cs` L1823 `Process.GetProcessById(pid)` throwing         |
//! | [`TerminateProcess`]   | `MainWindow.xaml.cs` L1831 `Process.Kill()` throwing                      |
//! | [`PostMessage`]        | `MainWindow.xaml.cs` L2450 `WindowsAPI.PostMessage(hWnd, WM_CLOSE, …)`    |
//!
//! [`WmiInit`]: ProcessError::WmiInit
//! [`WmiConnect`]: ProcessError::WmiConnect
//! [`WmiQuery`]: ProcessError::WmiQuery
//! [`OpenProcess`]: ProcessError::OpenProcess
//! [`TerminateProcess`]: ProcessError::TerminateProcess
//! [`PostMessage`]: ProcessError::PostMessage

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
}
