//! DES/ECB/NoPadding encryption — byte-compatible with the legacy C# WPF
//! `WCDESComp` class in `Beanfun/API/WCDESComp.cs`.
//!
//! Used by `GetOTP` (see `Beanfun/Tools/BeanfunClient.OTP.cs`) to decrypt the
//! OTP payload returned by `get_webstart_otp.ashx`: the first 8 bytes of the
//! response are an ASCII key, the rest is a hex-encoded DES-ECB ciphertext
//! without padding.
//!
//! Design decisions (match the C# reference exactly):
//! - Key and plaintext are encoded as ASCII. Non-ASCII code points map to
//!   `?` (0x3F), matching `System.Text.Encoding.ASCII`.
//! - No padding is applied; plaintext length must be a multiple of 8 bytes.
//! - Hex output is **uppercase**, matching `BitConverter.ToString(..).Replace("-","")`.
//! - Hex input is case-insensitive, matching `Convert.ToByte(s, 16)`.
//! - The decoder does **not** strip trailing NUL bytes; callers decide (the
//!   C# caller does `otp.Trim('\0')` after `DecryStrHex`).

use cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, BlockSizeUser, KeyInit};
use des::Des;
use thiserror::Error;

/// DES block size in bytes.
const BLOCK_SIZE: usize = 8;

/// Byte used by `System.Text.Encoding.ASCII` to replace code points > 0x7F.
const ASCII_REPLACEMENT: u8 = b'?';

/// Convenience alias for a DES input/output block (`GenericArray<u8, U8>`).
type DesBlock = GenericArray<u8, <Des as BlockSizeUser>::BlockSize>;

/// Errors surfaced by `encrypt_hex` / `decrypt_hex`.
///
/// The C# reference swallows every exception and returns `null`. We surface
/// a typed error instead so callers can give better diagnostics without
/// changing byte-level behaviour.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WcdesError {
    #[error("key must be exactly {BLOCK_SIZE} ASCII bytes, got {0}")]
    InvalidKeyLength(usize),

    #[error("plaintext length must be a multiple of {BLOCK_SIZE} bytes, got {0}")]
    InvalidPlaintextLength(usize),

    #[error(
        "ciphertext hex length must be a multiple of 16 characters (= one DES block), got {0}"
    )]
    InvalidCiphertextHexLength(usize),

    #[error("invalid hex character: {0:?}")]
    InvalidHexChar(char),
}

pub type Result<T> = std::result::Result<T, WcdesError>;

/// Encrypt an ASCII plaintext with DES/ECB/NoPadding and return uppercase hex.
///
/// Equivalent to C# `WCDESComp.EncryStrHex(str, key)`.
pub fn encrypt_hex(plaintext: &str, key: &str) -> Result<String> {
    let key_bytes = ascii_encode(key);
    let pt_bytes = ascii_encode(plaintext);

    if pt_bytes.len() % BLOCK_SIZE != 0 {
        return Err(WcdesError::InvalidPlaintextLength(pt_bytes.len()));
    }

    let cipher = des_from_key(&key_bytes)?;
    let out = process_blocks(&cipher, &pt_bytes, |c, b| c.encrypt_block(b));

    Ok(bytes_to_upper_hex(&out))
}

/// Decrypt a hex ciphertext with DES/ECB/NoPadding and return an ASCII string.
///
/// Equivalent to C# `WCDESComp.DecryStrHex(hex, key)`. The returned string may
/// contain trailing NUL (`\0`) bytes — callers typically trim those themselves.
pub fn decrypt_hex(hex_str: &str, key: &str) -> Result<String> {
    let key_bytes = ascii_encode(key);
    let ct_bytes = hex_decode(hex_str)?;

    if ct_bytes.len() % BLOCK_SIZE != 0 {
        return Err(WcdesError::InvalidCiphertextHexLength(hex_str.len()));
    }

    let cipher = des_from_key(&key_bytes)?;
    let out = process_blocks(&cipher, &ct_bytes, |c, b| c.decrypt_block(b));

    Ok(bytes_to_ascii_string(&out))
}

// -----------------------------------------------------------------------------
// Helpers (private)
// -----------------------------------------------------------------------------

/// Apply a per-block DES operation (encrypt or decrypt) across a slice whose
/// length is already known to be a multiple of [`BLOCK_SIZE`].
///
/// Centralises the block-by-block loop shared by [`encrypt_hex`] and
/// [`decrypt_hex`]: only the `op` closure differs between the two.
fn process_blocks(cipher: &Des, data: &[u8], mut op: impl FnMut(&Des, &mut DesBlock)) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    for (in_chunk, out_chunk) in data
        .chunks_exact(BLOCK_SIZE)
        .zip(out.chunks_exact_mut(BLOCK_SIZE))
    {
        let mut block = GenericArray::clone_from_slice(in_chunk);
        op(cipher, &mut block);
        out_chunk.copy_from_slice(&block);
    }
    out
}

/// Encode a `&str` as ASCII bytes, replacing any code point > 0x7F with `?`.
///
/// Matches the lossy behaviour of `System.Text.Encoding.ASCII.GetBytes`.
fn ascii_encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if (c as u32) <= 0x7F {
                c as u8
            } else {
                ASCII_REPLACEMENT
            }
        })
        .collect()
}

/// Decode bytes as an ASCII string, replacing bytes > 0x7F with `?`.
///
/// Matches `System.Text.Encoding.ASCII.GetString` lossy fallback.
fn bytes_to_ascii_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b <= 0x7F {
                b as char
            } else {
                ASCII_REPLACEMENT as char
            }
        })
        .collect()
}

/// Uppercase-hex encode, matching `BitConverter.ToString(..).Replace("-","")`.
fn bytes_to_upper_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(hex_nibble_upper(b >> 4));
        out.push(hex_nibble_upper(b & 0x0F));
    }
    out
}

#[inline]
fn hex_nibble_upper(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'A' + (n - 10)) as char,
        _ => unreachable!("nibble out of range: {n}"),
    }
}

/// Decode a case-insensitive hex string. Errors on odd length or non-hex chars.
fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let bytes = s.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err(WcdesError::InvalidCiphertextHexLength(bytes.len()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let hi = hex_value(chunk[0])?;
        let lo = hex_value(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

#[inline]
fn hex_value(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(WcdesError::InvalidHexChar(b as char)),
    }
}

/// Construct a DES cipher from an ASCII-encoded key, enforcing an 8-byte length.
fn des_from_key(key_bytes: &[u8]) -> Result<Des> {
    if key_bytes.len() != BLOCK_SIZE {
        return Err(WcdesError::InvalidKeyLength(key_bytes.len()));
    }
    Des::new_from_slice(key_bytes).map_err(|_| WcdesError::InvalidKeyLength(key_bytes.len()))
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Fixtures — pre-computed with Node's `crypto.createCipheriv('des-ecb')`
    // with `autoPadding = false`, which is byte-equal to .NET's
    // `DES.Create() + CipherMode.ECB + PaddingMode.None`. These serve as
    // the ground-truth "functional parity with WPF WCDESComp" check.
    // -------------------------------------------------------------------------

    /// `(key, plaintext, expected_hex)` tuples.
    const WPF_FIXTURES: &[(&str, &str, &str)] = &[
        ("12345678", "PLAINTXT", "0309B843D74E1A40"),
        (
            "12345678",
            "MAPLESTORY123456",
            "3FFCE1682ADB96B9A5BA42853018BFF3",
        ),
        (
            "12345678",
            "ABCDEFGH12345678HELLOTHX",
            "96DE603EAED6256F96D0028878D58C89DA4A75D69D63A29C",
        ),
        ("abcdefgh", "Now is t", "27176663304B9404"),
        ("12345678", "OTP:1234", "5495D9041D7E149B"),
        ("KEYONE89", "123456\0\0", "16D42698743EB312"),
    ];

    #[test]
    fn encrypt_matches_wpf_fixtures() {
        for &(key, plaintext, expected) in WPF_FIXTURES {
            let got = encrypt_hex(plaintext, key).expect("encrypt should succeed");
            assert_eq!(
                got, expected,
                "key={key:?}, plaintext={plaintext:?} produced {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn decrypt_matches_wpf_fixtures() {
        for &(key, expected_plaintext, hex) in WPF_FIXTURES {
            let got = decrypt_hex(hex, key).expect("decrypt should succeed");
            assert_eq!(
                got, expected_plaintext,
                "key={key:?}, hex={hex} produced {got:?}, expected {expected_plaintext:?}"
            );
        }
    }

    #[test]
    fn hex_input_is_case_insensitive() {
        // Same ciphertext, just lowercase — must decode to the same plaintext.
        let got = decrypt_hex("0309b843d74e1a40", "12345678").expect("decrypt should succeed");
        assert_eq!(got, "PLAINTXT");
    }

    #[test]
    fn encrypt_output_is_uppercase_hex() {
        let hex = encrypt_hex("PLAINTXT", "12345678").expect("encrypt should succeed");
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)));
    }

    // -------------------------------------------------------------------------
    // Roundtrip
    // -------------------------------------------------------------------------

    #[test]
    fn roundtrip_8_bytes() {
        roundtrip_case("12345678", "ABCDEFGH");
    }

    #[test]
    fn roundtrip_16_bytes() {
        roundtrip_case("KEYONE89", "SIXTEENBYTEINPUT");
    }

    #[test]
    fn roundtrip_24_bytes() {
        roundtrip_case("abcdefgh", "TWENTYFOUR12345678901234");
    }

    #[test]
    fn roundtrip_preserves_trailing_nulls() {
        // Mimics the OTP response shape (WPF caller trims \0 itself).
        let pt = "OTP123\0\0";
        roundtrip_case("12345678", pt);
    }

    fn roundtrip_case(key: &str, plaintext: &str) {
        let hex = encrypt_hex(plaintext, key).expect("encrypt should succeed");
        let decrypted = decrypt_hex(&hex, key).expect("decrypt should succeed");
        assert_eq!(decrypted, plaintext, "roundtrip mismatch for {plaintext:?}");
    }

    // -------------------------------------------------------------------------
    // Error paths
    // -------------------------------------------------------------------------

    #[test]
    fn encrypt_rejects_short_key() {
        assert_eq!(
            encrypt_hex("PLAINTXT", "1234567"),
            Err(WcdesError::InvalidKeyLength(7))
        );
    }

    #[test]
    fn encrypt_rejects_long_key() {
        assert_eq!(
            encrypt_hex("PLAINTXT", "123456789"),
            Err(WcdesError::InvalidKeyLength(9))
        );
    }

    #[test]
    fn encrypt_rejects_empty_key() {
        assert_eq!(
            encrypt_hex("PLAINTXT", ""),
            Err(WcdesError::InvalidKeyLength(0))
        );
    }

    #[test]
    fn encrypt_rejects_non_block_plaintext() {
        assert_eq!(
            encrypt_hex("SHORT", "12345678"),
            Err(WcdesError::InvalidPlaintextLength(5))
        );
    }

    #[test]
    fn encrypt_allows_empty_plaintext() {
        // C# `TransformFinalBlock` with zero-length input returns an empty byte array;
        // our impl mirrors that (0 is a valid multiple of 8).
        assert_eq!(encrypt_hex("", "12345678"), Ok(String::new()));
    }

    #[test]
    fn decrypt_rejects_odd_length_hex() {
        assert!(matches!(
            decrypt_hex("0309B843D74E1A4", "12345678"),
            Err(WcdesError::InvalidCiphertextHexLength(15))
        ));
    }

    #[test]
    fn decrypt_rejects_non_block_hex() {
        // 4 bytes (= 8 hex chars), not a full DES block.
        assert!(matches!(
            decrypt_hex("03090309", "12345678"),
            Err(WcdesError::InvalidCiphertextHexLength(_))
        ));
    }

    #[test]
    fn decrypt_rejects_invalid_hex_char() {
        assert_eq!(
            decrypt_hex("XYZ9B843D74E1A40", "12345678"),
            Err(WcdesError::InvalidHexChar('X'))
        );
    }

    #[test]
    fn decrypt_rejects_short_key() {
        assert_eq!(
            decrypt_hex("0309B843D74E1A40", "short"),
            Err(WcdesError::InvalidKeyLength(5))
        );
    }

    // -------------------------------------------------------------------------
    // ASCII encoding behaviour (match C# Encoding.ASCII lossy fallback)
    // -------------------------------------------------------------------------

    #[test]
    fn non_ascii_chars_are_replaced_with_question_mark_in_plaintext() {
        // 'é' (0xE9) and '中' (0x4E2D) both exceed 0x7F → both become '?'.
        // "é中23ABCD" encodes to "??23ABCD" (8 bytes), which is a valid DES block.
        let via_non_ascii =
            encrypt_hex("\u{00E9}\u{4E2D}23ABCD", "12345678").expect("encrypt should succeed");
        let via_question_marks =
            encrypt_hex("??23ABCD", "12345678").expect("encrypt should succeed");
        assert_eq!(via_non_ascii, via_question_marks);
    }

    #[test]
    fn non_ascii_chars_are_replaced_with_question_mark_in_key() {
        // Key "é2345678" (1 non-ASCII char + 7 ASCII) → 8 ASCII bytes after replacement.
        let via_non_ascii =
            encrypt_hex("PLAINTXT", "\u{00E9}2345678").expect("encrypt should succeed");
        let via_question_marks =
            encrypt_hex("PLAINTXT", "?2345678").expect("encrypt should succeed");
        assert_eq!(via_non_ascii, via_question_marks);
    }
}
