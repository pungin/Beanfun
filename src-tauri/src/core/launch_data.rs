//! Decoder for the `Data` blob on Beanfun's game-start page.
//!
//! `game_start_step2.aspx` embeds a `m_objData` literal:
//!
//! ```javascript
//! var m_objData = {
//!     "region": "TW;Production",
//!     "sn": "<36-char GUID>",
//!     "data": "<obfuscated blob>"
//! };
//! ```
//!
//! and hands it to the native launcher over the `gamaniagames://` URL
//! scheme. `data` carries the `LaunchTicket` that
//! `get_webstart_otp_v2.ashx` requires — obfuscated, but present, so
//! the value can be recovered without the launcher being installed.
//! See `docs/OTP-PROTOCOL-CHANGE.md` for how this was established.
//!
//! # Format
//!
//! 1. The first character is a hex digit `n`, selecting substitution
//!    table `n % 4`.
//! 2. Every remaining character is mapped to its **index** in that
//!    table and re-emitted as a hex digit — the *normalized hex*.
//! 3. The 8 characters at offset `n + 1` of the normalized hex are the
//!    DES key, as ASCII.
//! 4. Removing those 8 leaves the ciphertext hex.
//! 5. DES-ECB, no padding, then trailing NULs trimmed.
//! 6. The plaintext is `key=value` pairs joined by `&`, terminated by
//!    `;` and a short trailer.
//!
//! Steps 3-5 are the same construction the pre-v2 OTP envelope used
//! (an 8-character ASCII key followed by hex ciphertext), so
//! [`crate::core::wcdes::decrypt_hex`] handles them unchanged. Only the
//! substitution layer and the field parse are specific to this blob.

use thiserror::Error;

use crate::core::wcdes::{self, decrypt_hex};

/// The four substitution alphabets, lifted from the launcher's
/// `Command.DecryptParam()`. Each is a permutation of the 16 hex
/// digits — a precondition for step 2 to be reversible, asserted in
/// the tests.
/// Public so a wire test can build a payload the way the page does,
/// rather than pasting one from a capture.
pub const TABLES: [&str; 8] = [
    "bac987d65e432f10",
    "3bc4d5e6f2a79108",
    "cdbeaf9012456378",
    "4e6fb81a3c5d7092",
    "bdef1246789ac530",
    "5f82cb4093e71d6a",
    "df1468ace0357b92",
    "b50c61a4f93e82d7",
];

/// Length of the ASCII DES key embedded in the normalized hex.
const KEY_LEN: usize = 8;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LaunchDataError {
    #[error("launch data is empty")]
    Empty,

    #[error("launch data does not start with a hex digit, got {0:?}")]
    BadSelector(char),

    #[error("launch data contains {0:?}, which is absent from substitution table {1}")]
    UnmappableChar(char, usize),

    #[error("launch data is too short to hold a key at offset {offset} (have {len} characters)")]
    TooShort { offset: usize, len: usize },

    #[error("DES decryption of launch data failed: {0}")]
    Decrypt(String),

    #[error("decrypted launch data has no LaunchTicket field")]
    MissingTicket,

    #[error("LaunchTicket is not 64 hex characters (got {0})")]
    MalformedTicket(String),
}

impl From<wcdes::WcdesError> for LaunchDataError {
    fn from(err: wcdes::WcdesError) -> Self {
        LaunchDataError::Decrypt(err.to_string())
    }
}

/// Recover the `LaunchTicket` from a `m_objData.data` blob.
///
/// The other decoded fields (`ServiceCode`, `ServiceRegion`,
/// `ServiceAccount`, `BeanfunUrl`, `WebStartPatch`) are discarded —
/// the v2 OTP request needs none of them, and the caller already holds
/// its own copies.
//
// ponytail: returns the one field the caller uses. Widen to the full
// field map if a future call site needs more than the ticket.
pub fn decode_launch_ticket(data: &str) -> Result<String, LaunchDataError> {
    let plaintext = decode(data)?;
    let ticket = plaintext
        .split(';')
        .next()
        .unwrap_or_default()
        .split('&')
        .find_map(|pair| pair.strip_prefix("LaunchTicket="))
        .ok_or(LaunchDataError::MissingTicket)?;

    if ticket.len() != 64 || !ticket.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(LaunchDataError::MalformedTicket(ticket.to_owned()));
    }
    Ok(ticket.to_owned())
}

/// Undo the substitution layer and decrypt, returning the raw
/// plaintext with trailing NULs trimmed.
fn decode(data: &str) -> Result<String, LaunchDataError> {
    let mut chars = data.chars();
    let selector_char = chars.next().ok_or(LaunchDataError::Empty)?;
    let selector = selector_char
        .to_digit(16)
        .ok_or(LaunchDataError::BadSelector(selector_char))? as usize;

    // Which table the selector names is not settled. `n % 4` decodes
    // every payload seen so far, but there are eight tables, and the one
    // sample available (selector 12) cannot tell `n % 4` apart from the
    // table order simply differing from the launcher's own indexing.
    //
    // So rather than commit to a rule and be wrong for some accounts,
    // each table is tried until one yields a plaintext carrying a
    // `LaunchTicket`. A wrong table gives noise, and noise does not
    // spell a field name by accident, so the signal is sound. Eight DES
    // passes over ~272 bytes costs nothing measurable, and it keeps
    // working if beanfun adds a ninth table.
    //
    // Most-likely first, so the diagnostic usually names the same one.
    let rest: String = chars.collect();
    let mut order: Vec<usize> = vec![selector % 4, selector % TABLES.len()];
    order.extend(0..TABLES.len());

    let mut tried: Vec<usize> = Vec::with_capacity(TABLES.len());
    let mut first_error: Option<LaunchDataError> = None;
    for table_index in order {
        if tried.contains(&table_index) {
            continue;
        }
        tried.push(table_index);
        match decode_with(&rest, selector, table_index) {
            Ok(plaintext) if plaintext.contains("LaunchTicket=") => {
                tracing::debug!(selector, table = table_index, "launch data table");
                return Ok(plaintext);
            }
            // Decoded to something, but not to our payload — that is a
            // wrong table, not a broken one.
            Ok(_) => {}
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    Err(first_error.unwrap_or(LaunchDataError::MissingTicket))
}

/// One decode attempt with the table already chosen.
fn decode_with(body: &str, selector: usize, table_index: usize) -> Result<String, LaunchDataError> {
    let table = TABLES[table_index];
    let chars = body.chars();

    let normalized = chars
        .map(|c| {
            table
                .find(c)
                // Tables are ASCII, so the byte offset `find` returns
                // is also the character index we want.
                .map(|idx| char::from_digit(idx as u32, 16).expect("index < 16 is a hex digit"))
                .ok_or(LaunchDataError::UnmappableChar(c, table_index))
        })
        .collect::<Result<String, _>>()?;

    let offset = selector + 1;
    if normalized.len() < offset + KEY_LEN {
        return Err(LaunchDataError::TooShort {
            offset,
            len: normalized.len(),
        });
    }

    let key = &normalized[offset..offset + KEY_LEN];
    let cipher_hex = format!(
        "{}{}",
        &normalized[..offset],
        &normalized[offset + KEY_LEN..]
    );

    let plaintext = decrypt_hex(&cipher_hex, key)?;
    Ok(plaintext.trim_matches('\0').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::wcdes::encrypt_hex;

    /// Re-apply the substitution layer, so a test can build a blob the
    /// decoder should accept. Mirrors `decode`'s inverse exactly.
    fn encode(selector: usize, key: &str, cipher_hex: &str) -> String {
        let table: Vec<char> = TABLES[selector % TABLES.len()].chars().collect();
        let offset = selector + 1;
        let normalized = format!("{}{}{}", &cipher_hex[..offset], key, &cipher_hex[offset..]);
        let body: String = normalized
            .chars()
            .map(|c| table[c.to_digit(16).expect("normalized hex") as usize])
            .collect();
        format!(
            "{}{}",
            char::from_digit(selector as u32, 16).expect("selector < 16"),
            body
        )
    }

    /// Step 2 is only reversible if every table is a permutation of the
    /// hex alphabet. Pinned so a future transcription slip is caught
    /// here rather than as an unexplained decode failure.
    #[test]
    fn tables_are_permutations_of_the_hex_alphabet() {
        for (i, table) in TABLES.iter().enumerate() {
            let mut chars: Vec<char> = table.chars().collect();
            chars.sort_unstable();
            chars.dedup();
            assert_eq!(chars.len(), 16, "table {i} has duplicate characters");
            assert!(
                chars
                    .iter()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "table {i} holds a non-lowercase-hex character"
            );
        }
    }

    #[test]
    fn round_trips_through_every_selector() {
        let plaintext = "LaunchTicket=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&ServiceCode=610074&ServiceRegion=T9;abcde";
        // `encrypt_hex` refuses a plaintext that is not a whole number
        // of DES blocks, and the real blob is NUL-padded the same way.
        let padded = format!("{plaintext}{}", "\0".repeat((8 - plaintext.len() % 8) % 8));
        let key = "a1b2c3d4";
        let cipher_hex = encrypt_hex(&padded, key).unwrap().to_lowercase();

        for selector in 0..16 {
            let blob = encode(selector, key, &cipher_hex);
            assert_eq!(
                decode_launch_ticket(&blob).unwrap(),
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "selector {selector} failed"
            );
        }
    }

    /// The two real blob lengths observed in the wild — 553 from the
    /// upstream report, 537 from our own capture — must both leave a
    /// ciphertext that is a whole number of DES blocks. This is the
    /// cheapest check that the format is understood correctly.
    #[test]
    fn observed_blob_lengths_yield_whole_des_blocks() {
        for total in [553usize, 537] {
            let cipher_hex_len = total - 1 - KEY_LEN;
            assert_eq!(cipher_hex_len % 2, 0, "len {total}: odd hex length");
            assert_eq!(
                (cipher_hex_len / 2) % 8,
                0,
                "len {total}: ciphertext is not a whole number of DES blocks"
            );
        }
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(
            decode_launch_ticket("").unwrap_err(),
            LaunchDataError::Empty
        );
    }

    #[test]
    fn non_hex_selector_is_rejected() {
        assert_eq!(
            decode_launch_ticket("zzzz").unwrap_err(),
            LaunchDataError::BadSelector('z')
        );
    }

    #[test]
    fn character_outside_the_table_is_rejected() {
        // Selector 0 → table 0, which has no 'z'.
        let err = decode_launch_ticket("0zzz").unwrap_err();
        assert!(
            matches!(err, LaunchDataError::UnmappableChar('z', 0)),
            "got {err:?}"
        );
    }

    #[test]
    fn blob_too_short_for_a_key_is_rejected() {
        // Table 0 characters only, but far fewer than offset + 8.
        let err = decode_launch_ticket("0bac").unwrap_err();
        assert!(
            matches!(err, LaunchDataError::TooShort { .. }),
            "got {err:?}"
        );
    }
}
