//! Open a URL in the user's default browser / mailto client.
//!
//! Ports three WPF sites that all use
//! `Process.Start(new ProcessStartInfo { FileName = url,
//! UseShellExecute = true })`:
//!
//! - `Beanfun/Update/ApplicationUpdater.cs` L174-180 — "download the
//!   new version" button opens the release page in the default
//!   browser.
//! - `Beanfun/About.xaml.cs` `Github_Click` / `MailContact_Click`
//!   (L62-80) — project links + mailto: contact in the About dialog.
//! - `Beanfun/MainWindow.xaml.cs` `runGame` L1741-1743 — the update
//!   prompt's "open download URL" branch.
//!
//! All three use the same shell-execute semantics, so a single
//! `open_url` service function serves them uniformly. Under the hood
//! we call the [`open`] crate, which wraps `ShellExecuteW` on
//! Windows / `LSOpenCFURLRef` on macOS / `xdg-open` on Linux — the
//! exact OS surface `UseShellExecute = true` delegates to in .NET.
//!
//! # Scheme allowlist (P10.3-Q1 hardening)
//!
//! Only `http` / `https` / `mailto` schemes are permitted. This
//! rejects:
//!
//! - `file://` — information leak (would let the frontend force-open
//!   arbitrary local paths in Explorer / Finder).
//! - `javascript:` — not executed by `ShellExecuteW` anyway, but
//!   rejecting up front avoids a foot-gun if the frontend concatenates
//!   user input into a URL without escaping.
//! - `data:` — binary payload surface; could be used to stage a
//!   malicious file drop in the user's default download handler.
//! - Custom schemes (e.g. `steam://` / `spotify:`) — power-user
//!   convenience we can add later per-scheme when there's a concrete
//!   use case, rather than allowing them all implicitly.
//!
//! WPF does not restrict the scheme because its callers pass
//! hard-coded strings from `About.xaml.cs` / `ApplicationUpdater`;
//! our `open_url` Tauri command is callable by arbitrary frontend
//! code so the narrower surface is the right default. Loosening the
//! allowlist is a one-line constant edit if a future chunk needs it.

use crate::services::system::error::SystemError;

/// Schemes [`open_url`] is willing to pass through to the OS opener.
/// Compared case-insensitively (RFC 3986 §3.1 — scheme matching is
/// ASCII-case-insensitive).
const ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Validate a URL's scheme against [`ALLOWED_SCHEMES`].
///
/// Kept pure + sync so unit tests can exercise the allowlist without
/// touching the OS. Intentionally does **not** parse the full URL —
/// the `url` crate's strict parser would reject some real-world
/// mailto: URIs the OS opener still handles (e.g. embedded Unicode
/// in the local-part), and deeper validation happens inside
/// `ShellExecuteW` / `xdg-open` anyway.
fn validate_scheme(url: &str) -> Result<(), SystemError> {
    if url.is_empty() {
        return Err(SystemError::InvalidUrl {
            url: String::new(),
            reason: "URL must not be empty".into(),
        });
    }
    let Some(scheme_end) = url.find(':') else {
        return Err(SystemError::InvalidUrl {
            url: url.to_string(),
            reason: "URL must include a scheme (e.g. `https:`)".into(),
        });
    };
    let scheme = url[..scheme_end].to_ascii_lowercase();
    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        return Err(SystemError::InvalidUrl {
            url: url.to_string(),
            reason: format!("scheme `{scheme}` not in allowlist {ALLOWED_SCHEMES:?}"),
        });
    }
    Ok(())
}

/// Open `url` in the user's default handler for its scheme.
///
/// Async wrapper around the synchronous [`open::that`] via
/// [`tokio::task::spawn_blocking`] so the cross-process launch does
/// not stall the reactor (P10-Q5 = A blocking-isolation rule).
///
/// # Errors
///
/// - [`SystemError::InvalidUrl`] — URL failed the pre-flight
///   scheme-allowlist check. No OS call made.
/// - [`SystemError::OpenFailed`] — OS opener returned an I/O error.
/// - [`SystemError::SpawnBlockingFailed`] — the blocking task
///   panicked or was cancelled (should not happen in steady state).
pub async fn open_url(url: &str) -> Result<(), SystemError> {
    validate_scheme(url)?;
    let owned = url.to_string();
    tokio::task::spawn_blocking(move || {
        open::that(&owned).map_err(|source| SystemError::OpenFailed { url: owned, source })
    })
    .await
    .map_err(SystemError::SpawnBlockingFailed)?
}

/// Reveal a local directory in the OS file manager.
///
/// Separate from [`open_url`] on purpose: the scheme allowlist there
/// exists to stop arbitrary strings reaching the shell, and a path is
/// not a URL. Callers must pass a path *the app computed itself*, never
/// one supplied by a page or a config value.
///
/// # Errors
///
/// - [`SystemError::OpenFailed`] — the OS opener returned an I/O error.
/// - [`SystemError::SpawnBlockingFailed`] — the blocking task panicked
///   or was cancelled.
pub async fn open_directory(dir: &std::path::Path) -> Result<(), SystemError> {
    let owned = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        open::that(&owned).map_err(|source| SystemError::OpenFailed {
            url: owned.display().to_string(),
            source,
        })
    })
    .await
    .map_err(SystemError::SpawnBlockingFailed)?
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // Scheme allowlist — pure, runs everywhere (no OS involvement).
    // -----------------------------------------------------------------

    #[test]
    fn validate_scheme_accepts_http() {
        assert!(validate_scheme("http://example.com").is_ok());
    }

    #[test]
    fn validate_scheme_accepts_https() {
        assert!(validate_scheme("https://example.com/path?q=1").is_ok());
    }

    #[test]
    fn validate_scheme_accepts_mailto() {
        assert!(validate_scheme("mailto:user@example.com").is_ok());
    }

    #[test]
    fn validate_scheme_is_case_insensitive() {
        // Scheme matching is ASCII-case-insensitive per RFC 3986 §3.1.
        assert!(validate_scheme("HTTPS://example.com").is_ok());
        assert!(validate_scheme("Mailto:user@example.com").is_ok());
    }

    #[test]
    fn validate_scheme_rejects_empty_url() {
        let err = validate_scheme("").expect_err("empty URL must fail");
        match err {
            SystemError::InvalidUrl { url, reason } => {
                assert_eq!(url, "");
                assert!(
                    reason.contains("empty"),
                    "reason should mention empty, got: {reason}"
                );
            }
            other => panic!("expected InvalidUrl, got {other:?}"),
        }
    }

    #[test]
    fn validate_scheme_rejects_missing_scheme() {
        let err = validate_scheme("example.com").expect_err("no scheme must fail");
        match err {
            SystemError::InvalidUrl { url, reason } => {
                assert_eq!(url, "example.com");
                assert!(
                    reason.contains("scheme"),
                    "reason should mention scheme, got: {reason}"
                );
            }
            other => panic!("expected InvalidUrl, got {other:?}"),
        }
    }

    #[test]
    fn validate_scheme_rejects_file_scheme() {
        // `file://` would let the frontend force-open arbitrary local
        // paths — explicitly rejected.
        let err = validate_scheme("file:///C:/Windows/System32/cmd.exe")
            .expect_err("file:// must be rejected");
        match err {
            SystemError::InvalidUrl { reason, .. } => {
                assert!(
                    reason.contains("file") && reason.contains("allowlist"),
                    "reason should mention rejected scheme + allowlist, got: {reason}"
                );
            }
            other => panic!("expected InvalidUrl, got {other:?}"),
        }
    }

    #[test]
    fn validate_scheme_rejects_javascript_scheme() {
        assert!(matches!(
            validate_scheme("javascript:alert(1)"),
            Err(SystemError::InvalidUrl { .. })
        ));
    }

    #[test]
    fn validate_scheme_rejects_data_scheme() {
        assert!(matches!(
            validate_scheme("data:text/plain,hello"),
            Err(SystemError::InvalidUrl { .. })
        ));
    }

    #[test]
    fn validate_scheme_rejects_custom_scheme() {
        // Future chunks can add `steam://` etc. per-need by extending
        // ALLOWED_SCHEMES; implicit allow-all is the wrong default.
        assert!(matches!(
            validate_scheme("steam://run/12345"),
            Err(SystemError::InvalidUrl { .. })
        ));
    }
}
