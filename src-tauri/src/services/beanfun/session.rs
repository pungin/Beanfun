//! Session-scoped data types: user credentials and the resulting login
//! session handle.
//!
//! # Sensitive-data policy
//!
//! - **Password** — held in [`Credentials::password`], zeroized on drop via
//!   the `zeroize` crate so that a memory dump taken **after** the login
//!   completes does not reveal the plaintext.
//! - **bfWebToken / skey** — present in [`Session`] and treated as session
//!   secrets: both are redacted by the `Debug` impl so they cannot leak
//!   into `tracing` / `println!` logs. We do not `Zeroize` the `Session`
//!   right now because it generally lives for the entire GUI lifetime
//!   (dropping it = logout), and premature zero-on-move would complicate
//!   Tauri command plumbing; revisit in P5+ if we add persistence.

use zeroize::{Zeroize, ZeroizeOnDrop};

use super::client::LoginRegion;

/// Plain user-supplied login credentials.
///
/// The [`Zeroize`] + [`ZeroizeOnDrop`] derives ensure the password buffer
/// is overwritten with zeroes when the struct goes out of scope; cloning
/// the credentials forward into a login call is cheap (a String clone) and
/// the clone is then zeroed at its own drop site.
///
/// Callers should keep `Credentials` instances short-lived — ideally one
/// per login attempt — to minimise the window in which the plaintext
/// password lives in process memory.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    /// Login account id. Not a secret in the same sense as the password,
    /// but we still zeroize it on drop so that a single memory scrub
    /// handles both fields.
    pub account: String,
    /// Plaintext password. Sent as-is to `AccountLogin` under TLS, which is
    /// how the WPF client has always done it (no client-side encryption).
    pub password: String,
}

impl Credentials {
    /// Convenience constructor that accepts any string-like input.
    pub fn new(account: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            account: account.into(),
            password: password.into(),
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The account id is considered non-secret (same as what appears on
        // the invoice / support ticket) so we show its length hint only;
        // the password is always fully redacted.
        f.debug_struct("Credentials")
            .field("account_len", &self.account.len())
            .field("password", &"***")
            .finish()
    }
}

/// Successful login result: everything subsequent Beanfun calls need to
/// authenticate on the user's behalf.
///
/// Mirrors the set of fields that the WPF `BeanfunClient` exposes after a
/// successful `Login(...)` call (`this.webtoken`, `this.SessionKey`, plus
/// the service code/region it was configured with).
#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    /// Which region this session belongs to (TW / HK). Cookies and portal
    /// URLs diverge by region, so every follow-up call needs this.
    pub region: LoginRegion,

    /// `pSKey` — the one-time session key minted by the portal entry page.
    /// Not a long-lived secret but treated as sensitive in logs.
    pub skey: String,

    /// `bfWebToken` cookie value — the actual session bearer. Leaking this
    /// is equivalent to leaking the session.
    pub web_token: String,

    /// The account id the user logged in as. Not a secret; useful for UI.
    pub account_id: String,

    /// MapleStory service code, e.g. `"610074"`. Matches the WPF default.
    pub service_code: String,

    /// MapleStory service region, e.g. `"T9"`. Matches the WPF default.
    pub service_region: String,
}

impl Session {
    /// Convenience constructor used by the login flow once every ingredient
    /// is gathered.
    pub fn new(
        region: LoginRegion,
        skey: impl Into<String>,
        web_token: impl Into<String>,
        account_id: impl Into<String>,
        service_code: impl Into<String>,
        service_region: impl Into<String>,
    ) -> Self {
        Self {
            region,
            skey: skey.into(),
            web_token: web_token.into(),
            account_id: account_id.into(),
            service_code: service_code.into(),
            service_region: service_region.into(),
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("region", &self.region)
            .field("skey", &"***")
            .field("web_token", &"***")
            .field("account_id", &self.account_id)
            .field("service_code", &self.service_code)
            .field("service_region", &self.service_region)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_debug_redacts_password_and_account() {
        let creds = Credentials::new("hunter2@example.com", "p@ssw0rd!");
        let rendered = format!("{:?}", creds);
        assert!(
            !rendered.contains("p@ssw0rd!"),
            "password must not appear in Debug output: {rendered}"
        );
        assert!(
            !rendered.contains("hunter2@example.com"),
            "account should not appear verbatim either: {rendered}"
        );
        assert!(
            rendered.contains("***"),
            "Debug output should mark the password as redacted: {rendered}"
        );
    }

    #[test]
    fn session_debug_redacts_skey_and_web_token() {
        let sess = Session::new(
            LoginRegion::TW,
            "SKEY_ABC",
            "BFWT_XYZ",
            "alice",
            "610074",
            "T9",
        );
        let rendered = format!("{:?}", sess);
        assert!(
            !rendered.contains("SKEY_ABC"),
            "skey must be redacted in Debug: {rendered}"
        );
        assert!(
            !rendered.contains("BFWT_XYZ"),
            "web_token must be redacted in Debug: {rendered}"
        );
        // Non-secret fields remain visible — essential for log usefulness.
        assert!(rendered.contains("alice"), "account_id should be visible");
        assert!(rendered.contains("610074"));
        assert!(rendered.contains("T9"));
    }

    #[test]
    fn credentials_zeroize_clears_password_on_drop() {
        // We can only observe zeroize indirectly: after an explicit
        // `.zeroize()` call the buffers should be empty. Drop is covered
        // by the `ZeroizeOnDrop` impl, which delegates to the same path.
        let mut creds = Credentials::new("alice", "p@ssw0rd!");
        creds.zeroize();
        assert!(creds.account.is_empty());
        assert!(creds.password.is_empty());
    }
}
