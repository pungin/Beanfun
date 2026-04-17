//! Entropy salt for DPAPI operations — 8-char `[A-Z0-9]` string persisted
//! in `HKCU\SOFTWARE\BEANFUN\ENTROPY`.
//!
//! Ports the entropy flow from WPF `AccountManager.writeRawData`
//! (`Beanfun/Helper/AccountManager.cs` L244-260) plus
//! `Helper/ModifyRegistry.cs`:
//!
//! 1. Every `save_records` call generates a **fresh** entropy via
//!    [`Entropy::generate`] (WPF used `new Random()` time-seeded PRNG,
//!    we upgrade to [`OsRng`]).
//! 2. The entropy is persisted to the registry via
//!    [`write_to_registry`] **before** the ciphertext is written, so the
//!    load-side can read the salt back out.
//! 3. The entropy is passed as the DPAPI `pOptionalEntropy` parameter to
//!    [`super::dpapi_protect`] / [`super::dpapi_unprotect`].
//!
//! # Registry location
//!
//! - Sub-key: `SOFTWARE\BEANFUN` (hard-coded, uppercase). WPF derives this
//!   from `Application.ResourceAssembly.GetName().Name.ToUpper()`; our
//!   Rust crate is named `beanfun-next` so we hard-code the constant
//!   to preserve byte-for-byte interop with a WPF-written registry value.
//! - Value name: `ENTROPY` (hard-coded, uppercase). WPF
//!   `ModifyRegistry.Read` / `Write` upper-case the key name before
//!   calling into the Win32 API.
//! - Value type: `REG_SZ` (UTF-16 string).
//!
//! # RNG upgrade vs WPF
//!
//! WPF used `new Random()` seeded with the current tick count — a
//! Mersenne Twister with ~32 bits of entropy in the seed, so two Beanfun
//! instances started in the same millisecond can end up with identical
//! entropy salts. DPAPI ciphertext itself already derives from strong OS
//! key material, so this weakness does not actively compromise the
//! encrypted `Users.dat`; we still upgrade because the registry salt is
//! user-controllable data and (1) OsRng has no downside in this code
//! path, (2) it closes a trivially predictable input to a crypto API.
//!
//! The on-disk wire format (registry `REG_SZ`, 8 `[A-Z0-9]` chars,
//! UTF-8 bytes passed to `CryptProtectData`) is **unchanged** — this is
//! a pure RNG-quality upgrade, not a protocol change.

use rand::rngs::OsRng;
use rand::Rng;

use super::error::StorageError;

/// Hard-coded uppercase sub-key path for the entropy value under
/// `HKEY_CURRENT_USER`.
///
/// See module docs for why this is hard-coded rather than derived from
/// the crate name. Crate-private — external callers should reach the
/// production location through [`read_from_registry`] /
/// [`write_to_registry`] rather than reproducing the constant.
pub(crate) const REGISTRY_SUBKEY: &str = "SOFTWARE\\BEANFUN";

/// Hard-coded uppercase value name for the entropy. Crate-private; see
/// [`REGISTRY_SUBKEY`] for rationale.
pub(crate) const REGISTRY_VALUE_NAME: &str = "ENTROPY";

/// Length of the entropy string in UTF-8 bytes. Each character is one
/// byte because the charset is a subset of ASCII. Crate-private —
/// external callers should treat the [`Entropy`] type as opaque.
pub(crate) const ENTROPY_LEN: usize = 8;

/// Character set used for entropy generation — same 36-char alphabet as
/// WPF `AccountManager.writeRawData`.
const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// Wraps the 8-char `[A-Z0-9]` entropy salt supplied to DPAPI as the
/// `pOptionalEntropy` parameter.
///
/// `Clone` / `PartialEq` are implemented so callers can persist and
/// compare the generated salt; `Debug` is deliberately redacted to avoid
/// leaking the value into logs even though it is a salt (not a secret
/// key).
#[derive(Clone, PartialEq, Eq)]
pub struct Entropy(String);

impl Entropy {
    /// Generate a fresh cryptographically-random 8-char `[A-Z0-9]`
    /// entropy using [`OsRng`].
    ///
    /// See module docs for the WPF parity discussion.
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let s: String = (0..ENTROPY_LEN)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();
        Self(s)
    }

    /// Parse an existing entropy string, validating the 8-char
    /// `[A-Z0-9]` shape.
    ///
    /// Returns `Err(StorageError::EntropyShape)` when the input does not
    /// match the expected grammar — callers should treat this identically
    /// to [`StorageError::EntropyMissing`] (regenerate + overwrite).
    pub fn parse(raw: impl Into<String>) -> Result<Self, StorageError> {
        let s = raw.into();
        if s.len() != ENTROPY_LEN
            || !s
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(StorageError::EntropyShape);
        }
        Ok(Self(s))
    }

    /// View the raw UTF-8 bytes — suitable for passing directly to
    /// [`super::dpapi_protect`] / [`super::dpapi_unprotect`] as the
    /// `entropy` parameter.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// View the raw string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Entropy {
    /// Redacted — even though entropy is a salt (not a key), we avoid
    /// leaking it into logs to stay consistent with the rest of the
    /// codebase's security posture (see
    /// [`crate::services::beanfun::session::Credentials`]).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Entropy(<redacted>)")
    }
}

/// Read the entropy value from `HKCU\SOFTWARE\BEANFUN\ENTROPY`.
///
/// Returns:
/// - `Ok(entropy)` when the value is present and shape-valid.
/// - `Err(StorageError::EntropyMissing)` when the sub-key or value does
///   not exist (typical first-time run).
/// - `Err(StorageError::EntropyShape)` when the value is present but
///   does not match `[A-Z0-9]{8}` — caller should regenerate.
/// - `Err(StorageError::Registry)` on other I/O errors.
#[cfg(target_os = "windows")]
pub fn read_from_registry() -> Result<Entropy, StorageError> {
    read_from_registry_at(REGISTRY_SUBKEY, REGISTRY_VALUE_NAME)
}

/// Lower-level variant that reads from an arbitrary sub-key / value
/// name. Exposed publicly for integration tests so they can avoid
/// polluting the production `SOFTWARE\BEANFUN\ENTROPY` location.
///
/// Production callers should prefer [`read_from_registry`], which fixes
/// both arguments to the WPF-compatible constants.
#[cfg(target_os = "windows")]
pub fn read_from_registry_at(subkey: &str, value_name: &str) -> Result<Entropy, StorageError> {
    use std::io;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = match hkcu.open_subkey_with_flags(subkey, KEY_READ) {
        Ok(k) => k,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(StorageError::EntropyMissing);
        }
        Err(e) => return Err(StorageError::Registry(e)),
    };

    let raw: String = match key.get_value(value_name) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(StorageError::EntropyMissing);
        }
        Err(e) => return Err(StorageError::Registry(e)),
    };

    Entropy::parse(raw)
}

/// Write `entropy` into `HKCU\SOFTWARE\BEANFUN\ENTROPY`, creating the
/// sub-key if necessary.
#[cfg(target_os = "windows")]
pub fn write_to_registry(entropy: &Entropy) -> Result<(), StorageError> {
    write_to_registry_at(REGISTRY_SUBKEY, REGISTRY_VALUE_NAME, entropy)
}

/// Lower-level variant — see [`read_from_registry_at`] for rationale.
#[cfg(target_os = "windows")]
pub fn write_to_registry_at(
    subkey: &str,
    value_name: &str,
    entropy: &Entropy,
) -> Result<(), StorageError> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(subkey).map_err(StorageError::Registry)?;
    key.set_value(value_name, &entropy.0)
        .map_err(StorageError::Registry)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_has_correct_length() {
        let e = Entropy::generate();
        assert_eq!(e.as_str().len(), ENTROPY_LEN);
    }

    #[test]
    fn generate_uses_only_uppercase_and_digits() {
        for _ in 0..100 {
            let e = Entropy::generate();
            assert!(
                e.as_str()
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
                "entropy {} contains invalid char",
                e.as_str()
            );
        }
    }

    #[test]
    fn generate_produces_high_uniqueness_across_many_samples() {
        // 50 samples in a 36^8 (~2.8e12) space — birthday-paradox
        // collision probability is on the order of 1e-10. Allow one
        // collision to keep the test deterministic against pathological
        // RNG behaviour without being meaningfully looser than "all
        // unique"; OsRng in any sane build will hit `N` unique values.
        use std::collections::HashSet;
        const N: usize = 50;
        let unique: HashSet<String> = (0..N)
            .map(|_| Entropy::generate().as_str().to_owned())
            .collect();
        assert!(
            unique.len() >= N - 1,
            "OsRng generated {N} entropies with only {} unique — RNG misbehaving?",
            unique.len()
        );
    }

    #[test]
    fn parse_accepts_valid_shape() {
        let e = Entropy::parse("AB12CD34").expect("8-char upper+digit must parse");
        assert_eq!(e.as_str(), "AB12CD34");
        assert_eq!(e.as_bytes(), b"AB12CD34");
    }

    #[test]
    fn parse_rejects_lowercase() {
        let err = Entropy::parse("ab12cd34").expect_err("lowercase must fail");
        assert!(matches!(err, StorageError::EntropyShape));
    }

    #[test]
    fn parse_rejects_wrong_length() {
        for bad in ["", "A", "ABCDEFG", "ABCDEFGHI", "ABCDEFGHIJ"] {
            let err = Entropy::parse(bad).expect_err("wrong length must fail");
            assert!(
                matches!(err, StorageError::EntropyShape),
                "expected EntropyShape for {bad:?}"
            );
        }
    }

    #[test]
    fn parse_rejects_special_chars() {
        for bad in ["AB12!@CD", "ABCD 123", "AB-12-CD", "AB_12_CD"] {
            let err = Entropy::parse(bad).expect_err("special chars must fail");
            assert!(
                matches!(err, StorageError::EntropyShape),
                "expected EntropyShape for {bad:?}"
            );
        }
    }

    #[test]
    fn debug_is_redacted() {
        let e = Entropy::parse("AB12CD34").unwrap();
        let debug = format!("{:?}", e);
        assert_eq!(debug, "Entropy(<redacted>)");
        assert!(!debug.contains("AB12CD34"));
    }

    #[test]
    fn registry_constants_match_wpf() {
        // WPF: Application.ResourceAssembly.GetName().Name = "Beanfun"
        //      .ToUpper() = "BEANFUN", prefix "SOFTWARE\\"
        assert_eq!(REGISTRY_SUBKEY, "SOFTWARE\\BEANFUN");

        // WPF ModifyRegistry.Read does `KeyName.ToUpper()` on the value name.
        assert_eq!(REGISTRY_VALUE_NAME, "ENTROPY");
    }

    #[test]
    fn charset_matches_wpf_literal() {
        assert_eq!(CHARSET, b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789");
        assert_eq!(CHARSET.len(), 36);
    }
}
