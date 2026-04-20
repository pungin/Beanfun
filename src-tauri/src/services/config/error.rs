//! Typed error enum for the AppSettings XML config layer.
//!
//! # Design
//!
//! - `Io` carries plain file I/O failures (`std::fs::read` / `write` /
//!   `remove_file` / `create_dir_all`) on the `Config.xml` path. Kept
//!   distinct from `XmlParse` / `XmlWrite` so caller logs can pinpoint
//!   the failure surface.
//! - `XmlParse` and `XmlWrite` wrap [`quick_xml::Error`] for the
//!   on-disk serialize / deserialize round-trip. Note that
//!   [`get_value`] / [`get_value_or`] do **not** propagate these
//!   variants — they are caught internally and treated as
//!   "first-time run" matching WPF
//!   `ConfigAppSettings.GetValue`'s catch-all
//!   (`Beanfun/Helper/ConfigAppSettings.cs` L88-91).
//! - `set_value` *does* propagate `Io` / `XmlWrite` for true write
//!   failures (disk full, permission denied, encode errors). This is
//!   a **deliberate deviation from WPF** — `ConfigAppSettings.SetValue`
//!   silently swallows these via an empty `catch{}` block (L60),
//!   which means user settings can be lost without any signal. The
//!   typed surface lets the P10 service layer decide whether to
//!   surface a UI prompt or log + ignore.
//! - `AppDataMissing` is the documented signal that
//!   `std::env::var_os("APPDATA")` returned `None`, blocking
//!   [`default_config_xml_path`] from resolving the on-disk path.
//!   This should never happen on Windows under normal user contexts;
//!   it exists to keep the helper's contract honest.
//!
//! [`get_value`]: crate::services::config::xml::get_value
//! [`get_value_or`]: crate::services::config::xml::get_value_or
//! [`default_config_xml_path`]: crate::services::config::xml::default_config_xml_path

use thiserror::Error;

/// Typed failure surface for the config layer.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Generic file I/O failure on the `Config.xml` path — read /
    /// write / remove / `create_dir_all`.
    #[error("config I/O error: {0}")]
    Io(#[source] std::io::Error),

    /// On-disk XML failed to deserialize. `set_value` catches this
    /// internally (delete file + start from empty map) so it never
    /// surfaces from the IO-bearing API; `parse_app_settings` does
    /// propagate it for callers driving deserialization directly.
    #[error("config XML parse failed: {0}")]
    XmlParse(#[source] quick_xml::Error),

    /// XML serialization failed.
    ///
    /// `quick_xml::Writer` writes through `std::io::Write` and so
    /// surfaces failures as `std::io::Error`. In practice this is
    /// unreachable for the in-memory `Cursor<Vec<u8>>` writer
    /// `serialize_app_settings` uses, but the variant is kept
    /// distinct from [`Self::Io`] so callers / logs can tell encode
    /// failure apart from disk write failure.
    #[error("config XML write failed: {0}")]
    XmlWrite(#[source] std::io::Error),

    /// `%APPDATA%` environment variable was unset or empty, blocking
    /// [`default_config_xml_path`] from resolving the on-disk path.
    ///
    /// [`default_config_xml_path`]: crate::services::config::xml::default_config_xml_path
    #[error("APPDATA environment variable is missing or empty")]
    AppDataMissing,
}
