//! Login flows for the Beanfun service.
//!
//! Every submodule here is an `async` function (or small family of
//! functions) that drives one discrete HTTP round-trip of the WPF login
//! sequence. The flows are composed by higher-level orchestrators (added
//! in later chunks) that call each step in order, handling branching
//! (TOTP required / advance-check required / QR-code polling) as typed
//! [`super::LoginError`] variants.
//!
//! # Why split per step?
//!
//! The WPF source inlines the entire flow inside one giant `try` block per
//! method (`TwRegularLogin`, `HkRegularLogin`, `QRCodeLogin`). That makes
//! unit-testing any individual step difficult without mocking the whole
//! sequence. Splitting each HTTP call into its own function lets us:
//!
//! - Test each step against a tiny wiremock expectation.
//! - Reuse shared steps (e.g. `get_session_key`) across all three login
//!   methods without duplicating code.
//! - Surface per-step errors via the typed [`super::LoginError`] enum.

pub mod session_key;

pub use session_key::get_session_key;
