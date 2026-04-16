//! Beanfun / MapleStory TW + HK login and account-management client.
//!
//! Ports the legacy C# `BeanfunClient` surface (under
//! `Beanfun/Tools/BeanfunClient.*.cs`) into idiomatic async Rust.
//!
//! # Layers
//!
//! | Module       | Responsibility                                          |
//! |--------------|---------------------------------------------------------|
//! | [`client`]   | `BeanfunClient` — reqwest wrapper + cookie jar + config |
//! | [`error`]    | `LoginError` — typed error enum mapping WPF `errmsg`    |
//! | [`session`]  | `Credentials`, `Session` (zeroize'd where sensitive)    |
//! | [`login`]    | Login flows: session-key, TW/HK regular, TOTP, QRCode   |
//!
//! # Safety posture
//!
//! - Every outbound request is capped by [`ClientConfig::timeout`] and
//!   [`ClientConfig::max_body_size`] so a malicious / misbehaving server
//!   cannot hang the runtime or exhaust memory.
//! - TLS verification is **always** on (rustls + webpki-roots); we deliberately
//!   do not expose an `accept_invalid_certs` knob.
//! - Sensitive fields (password, bfWebToken, skey) are redacted by the `Debug`
//!   impls on [`session::Credentials`] / [`session::Session`]; the password is
//!   additionally [`zeroize`]'d on drop.
//! - Cookie jars are scoped **per [`BeanfunClient`]** instance — two concurrent
//!   sessions never share cookies, matching the WPF `WebClient` per-instance
//!   jar behaviour.

pub mod client;
pub mod error;
pub mod login;
pub mod session;

pub use client::{BeanfunClient, ClientConfig, Endpoints, LoginRegion};
pub use error::LoginError;
pub use session::{Credentials, Session};
