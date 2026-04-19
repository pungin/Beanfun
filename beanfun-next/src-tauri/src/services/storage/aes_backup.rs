//! AES-128-CBC backup of `Users.dat` plaintext for cross-machine
//! portability — wire-format-compatible with WPF
//! `Beanfun/Windows/AccRecovery.xaml.cs` (`Export_Button_Click` /
//! `Recovery_Button_Click`).
//!
//! # Wire format (1:1 with WPF)
//!
//! ```text
//! key  = MD5(UTF-8(password))                  // 16 bytes → AES-128
//! iv   = MD5(UTF-8("pungin"))                  // 16 bytes — fixed seed
//! ct   = AES-128-CBC(key, iv).encrypt(PKCS7(UTF-8(plaintext)))
//! out  = base64_standard(ct)                   // RFC 4648 §4 — no URL-safe variant
//! ```
//!
//! `Aes.Create()` in .NET defaults to CBC + PKCS7, and although its
//! `KeySize` property defaults to 256 bits, supplying a 16-byte key
//! to `CreateEncryptor(key, iv)` implicitly selects AES-128.
//! Independently verified against the .NET reference implementation
//! via PowerShell `System.Security.Cryptography.Aes`; reference
//! vectors are pinned in [`tests::wpf_reference_vectors_decrypt`].
//!
//! # Threat model — portability, not confidentiality
//!
//! MD5 is cryptographically broken (collision resistance ≈2018,
//! pre-image ≈2024). We **do not** rely on MD5 for confidentiality.
//! We rely on it for **wire-format compatibility** with the legacy
//! WPF `AccRecovery` dialog so users migrating off the WPF launcher
//! can import their existing `.dat` backup blobs without re-export.
//!
//! Callers are expected to treat the resulting base64 ciphertext as
//! "obfuscated plaintext" and store it with the same care as a
//! plaintext password dump. The Q7 plaintext-export caveat in
//! [`crate::commands::storage`] applies here unchanged.
//!
//! # Why a separate module (SRP)
//!
//! `users_dat.rs` owns the DPAPI + `Users.dat` on-disk format.
//! `aes_backup.rs` owns the *transport* AES + base64 wrapper around
//! the JSON wire that `users_dat::export_records` already produces.
//! Keeping the two split means swapping the backup crypto (e.g.
//! AES-GCM in a future P-track) is a single-file change with no
//! `Users.dat` regression surface.
//!
//! # API shape
//!
//! [`encrypt_records`] is infallible (encryption with a known-good
//! key/IV cannot fail). [`decrypt_records`] surfaces three typed
//! failure modes via [`BackupError`] so callers can map them to
//! distinct UI messages (`MsgDecryptFailed` for wrong password,
//! `RecoveryFailed` for malformed input).
//!
//! Both functions are pure (no IO, no global state) so they unit-
//! test cross-platform without DPAPI. The Windows-only IO + DPAPI
//! steps live in [`crate::commands::storage::backup_export`] /
//! [`crate::commands::storage::backup_restore`].

use aes::Aes128;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cbc::{Decryptor, Encryptor};
use cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use md5::{Digest, Md5};
use thiserror::Error;
use zeroize::Zeroizing;

/// Type aliases keep the call-site signatures readable. The
/// `Aes128` cipher is `aes::Aes128` (16-byte key); `cbc::Encryptor`
/// / `cbc::Decryptor` wrap it with the CBC mode adapter from the
/// RustCrypto block-modes family.
type Aes128CbcEnc = Encryptor<Aes128>;
type Aes128CbcDec = Decryptor<Aes128>;

/// Fixed IV seed — UTF-8 bytes of the literal string `"pungin"`.
/// Hard-coded in WPF `AccRecovery.xaml.cs` line 38 / 56; we keep
/// the same constant so the wire format stays compatible across
/// implementations. The value itself has no semantic meaning
/// beyond "what the WPF author picked".
const IV_SEED: &[u8] = b"pungin";

/// Typed failure surface for [`decrypt_records`].
///
/// Distinct variants let the IPC layer map each failure to a
/// distinct [`crate::commands::error::CommandError`] code so the
/// frontend can choose between `MsgDecryptFailed` (likely user-
/// fixable: wrong password) and `RecoveryFailed` (likely data-
/// corruption: paste truncated, file mangled in transit).
#[derive(Debug, Error)]
pub enum BackupError {
    /// The provided ciphertext is not valid base64. Almost always
    /// means the user pasted only part of the export string, or
    /// the export string was URL-mangled in transit (e.g. dropped
    /// `=` padding from a chat client).
    #[error("base64 decode failed: {0}")]
    InvalidCiphertext(#[source] base64::DecodeError),

    /// AES-CBC PKCS7 unpad failed. In practice this means **wrong
    /// password** — the deciphered bytes are random garbage with
    /// effectively zero chance of producing a valid PKCS7 trailer
    /// (1/256 per byte for length 1, lower for longer paddings).
    /// Note: the variant carries no inner error because RustCrypto
    /// `UnpadError` is intentionally opaque to prevent oracle-
    /// attack leakage.
    #[error("AES decrypt failed (wrong password or corrupted ciphertext)")]
    DecryptFailed,

    /// Decryption succeeded but the plaintext bytes are not valid
    /// UTF-8. This means the ciphertext+key combination produced
    /// arbitrary bytes that happened to PKCS7-validate; in
    /// practice it is also a "wrong password" symptom but rarer
    /// than [`Self::DecryptFailed`]. Surfaced separately so logs
    /// can distinguish "PKCS7 trailer wrong" from "PKCS7 trailer
    /// right, payload not UTF-8".
    #[error("decrypted bytes are not valid UTF-8: {0}")]
    InvalidUtf8(#[source] std::string::FromUtf8Error),
}

/// Derive `(key, iv)` from `password` using MD5 — matches WPF
/// `AccRecovery.xaml.cs` lines 33-39 / 50-57.
///
/// Pure helper, exposed `pub(super)` only to share between the
/// two public entry points; no caller outside this module should
/// need direct access (key/IV derivation is an implementation
/// detail of the wire format).
fn derive_key_iv(password: &str) -> ([u8; 16], [u8; 16]) {
    let key: [u8; 16] = Md5::digest(password.as_bytes()).into();
    let iv: [u8; 16] = Md5::digest(IV_SEED).into();
    (key, iv)
}

/// Encrypt `plaintext` (UTF-8) under `password` and return the
/// base64-encoded ciphertext.
///
/// Infallible — AES-CBC encryption with a fixed-size key/IV
/// cannot fail; `cbc::Encryptor::encrypt_padded_vec_mut` returns
/// `Vec<u8>` directly (not `Result`).
///
/// # Examples
///
/// ```ignore
/// let plaintext = r#"{"account_list":[]}"#;
/// let b64 = encrypt_records(plaintext, "my-passphrase");
/// // `b64` is a base64 string, e.g. "Rq98iSYdF...".
/// ```
pub fn encrypt_records(plaintext: &str, password: &str) -> String {
    let (key, iv) = derive_key_iv(password);
    let cipher = Aes128CbcEnc::new(&key.into(), &iv.into());
    let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(plaintext.as_bytes());
    BASE64.encode(ciphertext)
}

/// Decrypt a `ciphertext_b64` (base64) under `password` and return
/// the recovered UTF-8 plaintext.
///
/// Whitespace around the input is trimmed before base64 decoding —
/// users routinely paste from chat clients that wrap long lines or
/// add a trailing newline; rejecting those would be a UX paper-cut
/// that the WPF dialog also avoided (its `TextBox` strips trailing
/// `\r\n` before `Convert.FromBase64String` via the .NET parser's
/// own lenient handling).
///
/// # Errors
///
/// See [`BackupError`] for the three typed failure modes.
///
/// # Side-channel note
///
/// PKCS7 unpad timing varies with padding length; this is a
/// classic padding-oracle vector. We are not exposed to it here
/// because the API runs entirely server-side (no remote oracle)
/// and the threat model already accepts the legacy crypto
/// limitations — see module docs.
pub fn decrypt_records(ciphertext_b64: &str, password: &str) -> Result<String, BackupError> {
    let ciphertext = BASE64
        .decode(ciphertext_b64.trim().as_bytes())
        .map_err(BackupError::InvalidCiphertext)?;
    let (key, iv) = derive_key_iv(password);
    let cipher = Aes128CbcDec::new(&key.into(), &iv.into());
    let plain_bytes = Zeroizing::new(
        cipher
            .decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)
            .map_err(|_| BackupError::DecryptFailed)?,
    );
    String::from_utf8(plain_bytes.to_vec()).map_err(BackupError::InvalidUtf8)
}

// =====================================================================
// Tests — wire-format parity with WPF + roundtrip + error surface
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// IV is a fixed value derived from the literal `"pungin"`. If
    /// this changes, every legacy WPF backup becomes unreadable —
    /// pin the constant against accidental refactor.
    #[test]
    fn iv_seed_is_fixed_at_pungin() {
        let (_key, iv) = derive_key_iv("anything");
        // MD5("pungin") = ce3bccabee4e632aecd092d066ded535
        // (verified via PowerShell `System.Security.Cryptography.MD5`).
        let expected: [u8; 16] = [
            0xce, 0x3b, 0xcc, 0xab, 0xee, 0x4e, 0x63, 0x2a, 0xec, 0xd0, 0x92, 0xd0, 0x66, 0xde,
            0xd5, 0x35,
        ];
        assert_eq!(iv, expected, "IV seed drift would brick legacy backups");
    }

    /// MD5(UTF-8("test")) is a well-known test vector. Pinning it
    /// here catches a future RustCrypto `md-5` behaviour change
    /// (extremely unlikely but cheaper to assert than to debug).
    #[test]
    fn key_derives_md5_of_utf8_password() {
        let (key, _iv) = derive_key_iv("test");
        // MD5("test") = 098f6bcd4621d373cade4e832627b4f6 (RFC 1321 test set adjacent).
        let expected: [u8; 16] = [
            0x09, 0x8f, 0x6b, 0xcd, 0x46, 0x21, 0xd3, 0x73, 0xca, 0xde, 0x4e, 0x83, 0x26, 0x27,
            0xb4, 0xf6,
        ];
        assert_eq!(key, expected);
    }

    /// Round-trip soundness — encrypted payload always decrypts to
    /// the same plaintext under the same password.
    #[test]
    fn encrypt_then_decrypt_recovers_plaintext() {
        let plaintext = r#"{"account_list":[{"region":"TW","account_id":"u1"}]}"#;
        let ciphertext = encrypt_records(plaintext, "passphrase");
        let recovered = decrypt_records(&ciphertext, "passphrase").expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// Empty plaintext is a legitimate edge case — a fresh
    /// `Users.dat` after the user wipes every entry. Ciphertext
    /// for empty input is one PKCS7 padding block (16 bytes ⇒
    /// 24-character base64).
    #[test]
    fn encrypt_empty_plaintext_produces_one_block() {
        let ciphertext = encrypt_records("", "test");
        let bytes = BASE64.decode(&ciphertext).expect("valid b64");
        assert_eq!(bytes.len(), 16, "PKCS7 forces a full padding block");
        let recovered = decrypt_records(&ciphertext, "test").expect("decrypt empty");
        assert_eq!(recovered, "");
    }

    /// Wrong password ⇒ PKCS7 unpad failure ⇒ `DecryptFailed`.
    /// Distinct from `InvalidCiphertext` (which means malformed b64)
    /// and `InvalidUtf8` (which would mean PKCS7 trailer happened
    /// to be valid by coincidence — vanishingly rare).
    #[test]
    fn wrong_password_returns_decrypt_failed() {
        let ciphertext = encrypt_records("hello", "right-password");
        let err = decrypt_records(&ciphertext, "wrong-password").expect_err("must fail");
        assert!(
            matches!(err, BackupError::DecryptFailed),
            "wrong password must surface as DecryptFailed, got {err:?}"
        );
    }

    /// Malformed base64 (missing `=` padding, illegal chars) ⇒
    /// `InvalidCiphertext` not `DecryptFailed`. Matters for the UI:
    /// "your paste is incomplete" vs. "your password is wrong" are
    /// very different remediation paths.
    #[test]
    fn malformed_base64_returns_invalid_ciphertext() {
        let err = decrypt_records("not!valid@base64", "test").expect_err("must fail");
        assert!(
            matches!(err, BackupError::InvalidCiphertext(_)),
            "malformed b64 must surface as InvalidCiphertext, got {err:?}"
        );
    }

    /// Whitespace tolerance — users paste from chat clients that
    /// wrap lines or add trailing newlines; the WPF dialog accepted
    /// these silently via .NET's lenient `Convert.FromBase64String`.
    #[test]
    fn surrounding_whitespace_is_tolerated() {
        let ciphertext = encrypt_records("hello", "test");
        let padded = format!("\n  {ciphertext}  \r\n");
        let recovered = decrypt_records(&padded, "test").expect("decrypt with whitespace");
        assert_eq!(recovered, "hello");
    }

    // -----------------------------------------------------------------
    // WPF reference vectors — base64 ciphertexts produced by the
    // .NET `System.Security.Cryptography.Aes` reference implementation
    // via PowerShell on this machine (P12.2 D10.0 pre-flight).
    //
    // Asserting *decrypt* against pinned WPF outputs proves byte-for-
    // byte wire-format compatibility: a WPF user can export their
    // backup, paste the b64 into the new app, and recover their data.
    // The reverse direction is covered by the round-trip test above.
    //
    // To regenerate: run `powershell` with the snippet preserved in
    // the P12.2 D10.0 transcript (computes MD5(pw) → AES-128-CBC +
    // PKCS7 → base64 via System.Security.Cryptography APIs).
    // -----------------------------------------------------------------

    #[test]
    fn wpf_reference_vector_test_password_hello_world() {
        let plain =
            decrypt_records("Rq98iSYdFHJxBNaVSCy4AA==", "test").expect("decrypt WPF vector");
        assert_eq!(plain, "hello world");
    }

    #[test]
    fn wpf_reference_vector_test_password_empty_plaintext() {
        let plain =
            decrypt_records("Nud6MJ/pDwrsdydP/XU3qA==", "test").expect("decrypt WPF vector");
        assert_eq!(plain, "");
    }

    #[test]
    fn wpf_reference_vector_empty_password_abc_plaintext() {
        let plain = decrypt_records("cNdPqpwEz7l+yAJYl20oPw==", "").expect("decrypt WPF vector");
        assert_eq!(plain, "abc");
    }

    #[test]
    fn wpf_reference_vector_realistic_users_dat_payload() {
        let plain = decrypt_records(
            "yQH1nmMZHt7R3cPE90ZZQF12iH0/fSEPYqWM6rJ93Nbf68qvn0wGb9JoZgdGFpzL\
             EjLcl+wxlAMcbE+jekhjlSpufhOLt3UxBx7LM1wMVg9KQsAk6ywdyLs24069LC4r",
            "p@ssw0rd",
        )
        .expect("decrypt realistic vector");
        assert_eq!(
            plain,
            r#"{"account_list":[{"region":"TW","account_id":"u1","account_name":"alice","password":"pw1"}]}"#
        );
    }

    /// Round-trip property — encrypt under a password, decrypt the
    /// result under the same password, recover the original. Tests
    /// the full PKCS7 padding range (16 → 31 bytes triggers padding
    /// from 16-byte block to 1-byte trailer).
    #[test]
    fn round_trip_covers_pkcs7_padding_boundary() {
        for len in 0..=31 {
            let plaintext: String = "x".repeat(len);
            let ciphertext = encrypt_records(&plaintext, "boundary-test");
            let recovered =
                decrypt_records(&ciphertext, "boundary-test").expect("recover boundary");
            assert_eq!(recovered, plaintext, "round-trip failed at length {len}");
        }
    }
}
