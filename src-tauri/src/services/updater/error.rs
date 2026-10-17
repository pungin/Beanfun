//! Typed failure surface for the updater pipeline.
//!
//! Network and parsing errors remain distinguishable from a successful check
//! without a newer release. The command layer maps them to IPC errors.
//!
//! # Variants
//!
//! | Variant                         | Upstream shape                    | Source in WPF                                 |
//! | ------------------------------- | --------------------------------- | --------------------------------------------- |
//! | [`UpdaterError::Probe`]         | `reqwest::Error` (HEAD fail)      | `TryProbe` catch (L40-43)                     |
//! | [`UpdaterError::Fetch`]         | `reqwest::Error` (GET fail)       | `WebClient.DownloadData` catch (L195-198)     |
//! | [`UpdaterError::JsonDecode`]    | `serde_json::Error` (parse fail)  | `JsonConvert.DeserializeObject` fail          |
//! | [`UpdaterError::UnsupportedTag`] | tag name not matching regex      | `Regex.Match` `match.Success == false` (L137) |

use thiserror::Error;

/// Typed error for the updater service.
#[derive(Debug, Error)]
pub enum UpdaterError {
    /// Network-level HEAD probe failed — connection refused, DNS error,
    /// TLS handshake aborted, 5s timeout, or the response came back with
    /// a non-2xx status (we treat 3xx / 4xx / 5xx as probe failure to
    /// match WPF `WebRequest.GetResponse()` which throws `WebException`
    /// on those codes).
    #[error("proxy probe failed: {0}")]
    Probe(#[source] reqwest::Error),

    /// Network-level GET against `.../releases` failed. Surfaces
    /// transport errors, non-2xx statuses (via `.error_for_status()`),
    /// and body-read I/O errors uniformly.
    #[error("GitHub release fetch failed: {0}")]
    Fetch(#[source] reqwest::Error),

    /// Response body came back OK but `serde_json` refused to decode
    /// it as `Vec<GitHubRelease>`. Separate from [`Self::Fetch`] so
    /// tests can tell a malformed payload apart from an actual
    /// transport fault.
    #[error("GitHub release JSON decode failed: {0}")]
    JsonDecode(#[source] serde_json::Error),

    /// Response body exceeded the hard ceiling. Defensive guard
    /// against an unexpectedly large payload exhausting memory.
    #[error("response body too large: {actual} bytes exceeds {limit} byte limit")]
    BodyTooLarge { limit: usize, actual: usize },

    /// Tag name did not match the `^v(\d+)\.(\d+)\.(\d+)\.(\d+)$`
    /// shape the updater understands (e.g. a very old
    /// pre-`5.8.X.timestamp` release, or a manually-pushed tag with
    /// an unexpected layout). Matches WPF `Regex.Match.Success ==
    /// false` L137 which silently bails out.
    #[error("tag name `{0}` does not match expected v<major>.<minor>.<patch>.<timestamp> format")]
    UnsupportedTag(String),
}
