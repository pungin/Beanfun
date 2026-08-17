//! Where the client-integrity values come from, newest source first.
//!
//! beanfun's TW credential endpoint asks the caller to state which build
//! of the Gamania Games Manager is asking: a version, and the SHA-256 of
//! one of its files. Both are constants for a given GGM release, so we
//! ship a known-good pair — but the day beanfun requires a newer one,
//! that pair stops working **for everyone at once**, and only for the
//! users who have no GGM installed to read real values from.
//!
//! Answering that with an emergency release means every affected user
//! has to notice, download and install one, while unable to play. So the
//! values are looked up in order:
//!
//! 1. a `ggm-client.json` the user pinned themselves — an explicit
//!    choice, so nothing overrides it;
//! 2. the GGM installed on this machine, which follows its own updates;
//! 3. a small file published alongside the app, cached here — one commit
//!    fixes every user without them doing anything;
//! 4. the pair compiled in, so a machine with none of the above works.
//!
//! Layer 3 is the hotfix lever. See `docs/GGM-CLIENT-HOTFIX.md` for the
//! runbook, including how to tell this failure apart from the ones that
//! need a code change instead.
//!
//! # Failing quietly is the point
//!
//! Every step here is best-effort. A fetch that times out, a file that
//! will not parse, a value that fails validation — each falls through to
//! the next source rather than surfacing. The alternative is a network
//! hiccup costing someone their password when a perfectly good compiled
//! -in pair was sitting right there.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

/// Where the published values live, and the mirrors that serve the same
/// file where GitHub itself is unreachable — which is most of the
/// mainland audience this exists for.
const HOTFIX_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/pungin/Beanfun/code/ggm-client.json",
    "https://cdn.jsdelivr.net/gh/pungin/Beanfun@code/ggm-client.json",
    "https://fastly.jsdelivr.net/gh/pungin/Beanfun@code/ggm-client.json",
    "https://ghproxy.net/https://raw.githubusercontent.com/pungin/Beanfun/code/ggm-client.json",
];

/// File name of both the published copy and the user's pin — the same
/// name on purpose: whatever is fetched can be edited in place, and an
/// edited file is simply one the fetch will not overwrite.
const CACHE_FILE: &str = "ggm-client.json";

/// How long a fetched copy is trusted before another fetch is tried.
///
/// Six hours is the lever's real latency: it is how long a bad published
/// value keeps hurting after it is reverted, and how long a good one
/// takes to reach everybody. Short enough to be a fix, long enough that
/// the app is not pulling a file on every password.
const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// Total budget for reaching a mirror. Missing the hotfix costs a
/// fallback; waiting on it costs the user their password.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Directory holding the cached / pinned file. Set once at boot.
static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// A published `CV` / `Hash` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedValues {
    pub cv: String,
    pub hash: String,
}

/// Record where the cached file lives (the storage root).
///
/// Called once during boot. Without it every layer here is skipped and
/// resolution falls through to the installed GGM or the compiled-in
/// pair, which is the pre-hotfix behaviour — degraded, never broken.
pub fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE_DIR.set(dir);
}

fn cache_path() -> Option<PathBuf> {
    Some(CACHE_DIR.get()?.join(CACHE_FILE))
}

/// Values the user pinned themselves, if any.
///
/// Told apart by an `override` flag rather than by living somewhere
/// else: editing the fetched file in place is then all it takes to pin
/// values — no second path to explain, and no way to "fix" the file and
/// have the next fetch quietly undo you.
pub fn pinned() -> Option<PublishedValues> {
    let body = std::fs::read_to_string(cache_path()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(strip_bom(&body)).ok()?;
    if value["override"].as_bool() != Some(true) {
        return None;
    }
    let values = parse(&body)?;
    tracing::info!(cv = %values.cv, "ggm-hotfix: using the pinned local values");
    Some(values)
}

/// The published values: the cached copy while it is fresh, otherwise a
/// fetch, otherwise whatever stale copy we still have.
///
/// A stale copy beats nothing: it was good enough to publish, and the
/// alternative is the compiled-in pair that is by definition older.
pub async fn published() -> Option<PublishedValues> {
    if let Some((values, fetched_at)) = cached() {
        if fetched_at.elapsed().unwrap_or(CACHE_TTL) < CACHE_TTL {
            return Some(values);
        }
        return match fetch().await {
            Some(fresh) => Some(fresh),
            None => {
                tracing::info!("ggm-hotfix: refresh failed; keeping the cached values");
                Some(values)
            }
        };
    }
    fetch().await
}

/// Read the cached file and when it was written.
fn cached() -> Option<(PublishedValues, SystemTime)> {
    let path = cache_path()?;
    let body = std::fs::read_to_string(&path).ok()?;
    let values = parse(&body)?;
    let written = std::fs::metadata(&path).ok()?.modified().ok()?;
    Some((values, written))
}

/// Try each mirror in turn, caching the first usable answer.
async fn fetch() -> Option<PublishedValues> {
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .ok()?;

    for url in HOTFIX_URLS {
        let Ok(resp) = client.get(*url).send().await else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(body) = resp.text().await else {
            continue;
        };
        let Some(values) = parse(&body) else {
            // Reachable but unusable: worth saying which mirror, since
            // a stale CDN copy looks exactly like a bad commit.
            tracing::warn!(url, "ggm-hotfix: published file did not validate");
            continue;
        };
        tracing::info!(url, cv = %values.cv, "ggm-hotfix: published values fetched");
        write_cache(&body);
        return Some(values);
    }
    tracing::info!("ggm-hotfix: no mirror answered; using local sources");
    None
}

fn write_cache(body: &str) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(&path, body) {
        tracing::warn!(%error, "ggm-hotfix: could not cache the published values");
    }
}

/// Strip a UTF-8 byte-order mark.
///
/// Several Windows editors add one when saving, and a BOM makes the
/// document fail to parse — which here means every user silently drops
/// to the compiled-in pair. That is the worst possible failure for a
/// hotfix: it looks like the fix was published, and nobody is helped.
/// Cheaper to tolerate it than to document it away.
fn strip_bom(body: &str) -> &str {
    body.strip_prefix('\u{feff}').unwrap_or(body)
}

/// Parse and validate a published document.
///
/// Validation is not politeness: a malformed pair is sent to beanfun as
/// the caller's identity and gets everyone refused. Anything that is not
/// obviously a version and a SHA-256 is treated as if the file were
/// absent.
fn parse(body: &str) -> Option<PublishedValues> {
    let value: serde_json::Value = serde_json::from_str(strip_bom(body)).ok()?;
    let cv = value["cv"].as_str()?.trim().to_string();
    let hash = value["hash"].as_str()?.trim().to_ascii_lowercase();

    if !is_version(&cv) || !is_sha256(&hash) {
        tracing::warn!(cv = %cv, hash_len = hash.len(), "ggm-hotfix: values failed validation");
        return None;
    }
    Some(PublishedValues { cv, hash })
}

/// Digits and dots only, and at least one digit.
fn is_version(cv: &str) -> bool {
    !cv.is_empty()
        && cv.chars().any(|c| c.is_ascii_digit())
        && cv.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Exactly sixty-four hex characters.
fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_HASH: &str = "dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06";

    fn doc(cv: &str, hash: &str) -> String {
        format!(r#"{{"cv":"{cv}","hash":"{hash}"}}"#)
    }

    #[test]
    fn reads_a_published_pair() {
        let values = parse(&doc("1.5.0.2", GOOD_HASH)).expect("parses");
        assert_eq!(values.cv, "1.5.0.2");
        assert_eq!(values.hash, GOOD_HASH);
    }

    #[test]
    fn tolerates_a_byte_order_mark() {
        // The failure this prevents is invisible: a BOM would drop every
        // user to the compiled-in pair while the fix looks published.
        let body = format!("\u{feff}{}", doc("1.5.0.2", GOOD_HASH));
        assert!(parse(&body).is_some(), "a BOM must not defeat the hotfix");
    }

    #[test]
    fn accepts_an_uppercase_hash_by_normalising_it() {
        let values = parse(&doc("1.5.0.2", &GOOD_HASH.to_uppercase())).expect("parses");
        assert_eq!(values.hash, GOOD_HASH, "beanfun is given lowercase hex");
    }

    #[test]
    fn rejects_a_hash_that_is_not_a_sha256() {
        // Sent as our identity, a malformed hash gets everyone refused —
        // so it is treated as if the file were not there.
        assert!(parse(&doc("1.5.0.2", "abc")).is_none());
        assert!(parse(&doc("1.5.0.2", &"z".repeat(64))).is_none());
        assert!(parse(&doc("1.5.0.2", &format!("{GOOD_HASH}00"))).is_none());
    }

    #[test]
    fn rejects_a_version_that_is_not_one() {
        assert!(parse(&doc("", GOOD_HASH)).is_none());
        assert!(parse(&doc("1.5.0.2-beta", GOOD_HASH)).is_none());
        assert!(parse(&doc("...", GOOD_HASH)).is_none());
    }

    #[test]
    fn rejects_a_document_that_is_not_the_document() {
        assert!(parse("not json at all").is_none());
        assert!(parse(r#"{"version":"1.5.0.2"}"#).is_none());
    }

    #[test]
    fn the_shipped_file_validates() {
        // The published file lives in the repo; if it stops parsing, the
        // hotfix lever is broken before anyone needs to pull it.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(CACHE_FILE);
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()));
        assert!(
            parse(&body).is_some(),
            "the shipped {CACHE_FILE} must validate, or publishing it helps nobody"
        );
    }
}
