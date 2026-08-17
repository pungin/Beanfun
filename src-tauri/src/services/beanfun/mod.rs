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
//! | [`account`]  | Account list + JSON management (gamezone.ashx) + WebForms add-account / change-password |
//! | [`otp`]      | OTP retrieval (5 HTTP + WCDES decrypt)                  |
//! | [`client_integrity`] | `CV`/`Hash`/`arch` GGM fingerprint the TW OTP endpoint requires |
//! | [`verify`]   | Advance-check captcha re-auth (3 HTTP, TW only)         |
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

pub mod account;
pub mod client;
pub mod client_integrity;
pub mod error;
pub mod games;
pub mod ggm_hotfix;
pub mod login;
pub mod otp;
pub mod session;
pub mod verify;

pub use account::{
    add_service_account, change_service_account_display_name, get_accounts, get_email,
    get_remain_point, get_service_contract, unconnected_game_add_account,
    unconnected_game_add_account_check, unconnected_game_add_account_check_nickname,
    unconnected_game_change_password, unconnected_game_init_add_account_payload, AccountListResult,
    AddAccountInit, AddAccountOutcome, AddAccountSession, AmountLimitNotice, ChangePasswordOutcome,
    CheckOutcome, ServiceAccount,
};
pub use client::{BeanfunClient, ClientConfig, Endpoints, LoginRegion};
pub use client_integrity::ClientIntegrity;
pub use error::LoginError;
pub use games::{
    image_base_url, list_games, parse_service_ini, parse_service_list, GameInfoBundle,
    GameIniEntry, GameService,
};
pub use otp::get_otp;
pub use session::{Credentials, Session};
pub use verify::{
    get_verify_captcha, get_verify_page_info, submit_verify, VerifyOutcome, VerifyPageInfo,
};
