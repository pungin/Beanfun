//! Local AppSettings XML config layer — reads / writes the
//! `%APPDATA%\Beanfun\Config.xml` store with self-healing on parse
//! failure.
//!
//! Ports the legacy C# config surface under
//! `Beanfun/Helper/ConfigAppSettings.cs`:
//!
//! - [`ConfigAppSettings.GetValue(key)`][wpf-cfg] /
//!   `GetValue(key, def)` → `xml::get_value` / `xml::get_value_or`
//!   (catch-all → default, matches WPF L88-91).
//! - [`ConfigAppSettings.SetValue(key, value)`][wpf-cfg] →
//!   `xml::set_value` (typed `Result` — see deviation note below).
//! - .NET `<configuration><appSettings><add key value />` schema →
//!   `xml::parse_app_settings` / `xml::serialize_app_settings`.
//!
//! [wpf-cfg]: ../../../../../Beanfun/Helper/ConfigAppSettings.cs
//!
//! # Platform
//!
//! All XML parsing / serialization and the IO-bearing async APIs are
//! cross-platform — the chunk does not touch any Win32 surface.
//! `xml::default_config_xml_path` is Windows-only because it
//! resolves `%APPDATA%`, matching WPF
//! `SpecialFolder.ApplicationData`.
//!
//! # Self-healing on parse failure
//!
//! `xml::set_value` mirrors WPF's recursive retry by internally
//! collapsing it into a single flow: read existing file (or empty
//! map if missing) → on read or parse failure log a warning + delete
//! the file + start from an empty map → modify map → write back.
//! The flow always converges in one pass, no recursion or retry
//! counter required.
//!
//! # Deviation from WPF: typed `set_value` errors
//!
//! WPF [`ConfigAppSettings.SetValue`][wpf-cfg] (L60) silently swallows
//! second-attempt write failures via an empty `catch{}` block, which
//! means user settings can be lost without any signal. The Rust port
//! intentionally surfaces [`ConfigError::Io`] /
//! [`ConfigError::XmlWrite`] to the caller so the P10 service layer
//! can decide whether to prompt the user or log + ignore. Read
//! failure self-heal still aligns with WPF — surfacing only the
//! second-stage write error matches the user-visible behaviour
//! (settings save or it doesn't).
//!
//! # Layers
//!
//! | Module    | Responsibility                                                                     |
//! |-----------|------------------------------------------------------------------------------------|
//! | [`error`] | `ConfigError` — typed failures (`Io` / `XmlParse` / `XmlWrite` / `AppDataMissing`) |
//! | `xml`     | `parse` / `serialize` / `get_value` / `get_value_or` / `set_value` / path helper   |

pub mod error;
pub mod xml;

pub use error::ConfigError;
pub use xml::{get_value, get_value_or, parse_app_settings, serialize_app_settings, set_value};

#[cfg(target_os = "windows")]
pub use xml::default_config_xml_path;
