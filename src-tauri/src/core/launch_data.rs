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
//! scheme. `data` carries whatever that game's OTP request needs —
//! obfuscated, but present, so it can be recovered without the
//! launcher being installed. See `docs/OTP-PROTOCOL-CHANGE.md` for how
//! this was established.
//!
//! # Two payloads
//!
//! There are two, and both are live:
//!
//! - **`LaunchTicket=…`** — a ticket for `get_webstart_otp_v2.ashx`.
//!   Observed on MapleStory.
//! - **`ppppp=…`** — the query parameters for the pre-v2
//!   `get_webstart_otp.ashx`, alongside `ServiceCode`,
//!   `ServiceRegion`, `ServiceAccount` and `CreateTime`, joined by
//!   `&&&&`. Observed on CSO, Elsword and Mabinogi.
//!
//! Both kinds of page declare `m_objData`, so only the decoded
//! contents distinguish them.
//!
//! Treating a `ppppp` payload as a failed `LaunchTicket` decode is
//! what broke every game but MapleStory (upstream issue #376): the
//! decode was fine, the acceptance test was too narrow.
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

    #[error("decrypted launch data carries neither a LaunchTicket nor a ppppp payload")]
    MissingTicket,

    #[error("LaunchTicket field is present but empty")]
    MalformedTicket(String),
}

impl From<wcdes::WcdesError> for LaunchDataError {
    fn from(err: wcdes::WcdesError) -> Self {
        LaunchDataError::Decrypt(err.to_string())
    }
}

/// The two payloads a `m_objData.data` blob is known to carry.
///
/// Which one a page hands over is per game — whether that game has
/// been migrated to the v2 endpoint yet. **From the outside the two
/// pages look identical**: both declare `m_objData`, so the blob has to
/// be decoded before the route is known. That is why routing on the
/// mere presence of the literal sent every un-migrated game to an
/// endpoint with nothing to give it (upstream #376).
///
/// Independently measured across four titles by @ToooAir on that
/// issue: MapleStory (`610074_T9`) carries a ticket; CSO
/// (`610153_TN`), Elsword (`300148_AF`) and Mabinogi (`600309_A2`)
/// carry the pre-v2 parameters. All four declare `m_objData`.
#[derive(Debug, PartialEq, Eq)]
pub enum LaunchPayload {
    /// A ticket `get_webstart_otp_v2.ashx` takes directly.
    Ticket(String),
    /// The query parameters the pre-v2 `get_webstart_otp.ashx` wants.
    Legacy(Box<LegacyOtpParams>),
}

/// Pre-v2 OTP query parameters, as carried inside the blob.
///
/// `ppppp` is the interesting one. The WPF client hardcoded a
/// 64-character constant for it and nobody knew where the value came
/// from; it is in fact supplied here, and the current one is 96
/// characters. Reading it from the blob is correct whether it is
/// global, per-session or per-launch, which is why nothing here caches
/// or pins it.
#[derive(Debug, PartialEq, Eq)]
pub struct LegacyOtpParams {
    pub ppppp: String,
    pub service_code: String,
    pub service_region: String,
    pub service_account: String,
    pub create_time: String,
}

/// Decode a `m_objData.data` blob into whichever payload it carries.
pub fn decode_launch_data(data: &str) -> Result<LaunchPayload, LaunchDataError> {
    let plaintext = decode(data)?;
    let fields = parse_fields(&plaintext);
    let field = |name: &str| {
        fields
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| (*v).to_owned())
    };

    if let Some(ticket) = field("LaunchTicket") {
        // Presence decides the route; length does not. Pinning it to
        // the 64 characters seen so far would be the same over-narrow
        // acceptance that made every `ppppp` game fail, and #376
        // reports observed lengths moving elsewhere in this protocol.
        if ticket.is_empty() {
            return Err(LaunchDataError::MalformedTicket(ticket));
        }
        return Ok(LaunchPayload::Ticket(ticket));
    }

    match (
        field("ppppp"),
        field("ServiceCode"),
        field("ServiceRegion"),
        field("ServiceAccount"),
        field("CreateTime"),
    ) {
        (
            Some(ppppp),
            Some(service_code),
            Some(service_region),
            Some(service_account),
            Some(create_time),
        ) => Ok(LaunchPayload::Legacy(Box::new(LegacyOtpParams {
            ppppp,
            service_code,
            service_region,
            service_account,
            create_time,
        }))),
        _ => Err(LaunchDataError::MissingTicket),
    }
}

/// Split the decoded plaintext into `key=value` pairs.
///
/// The separator is `&&&&`, but splitting on a single `&` and dropping
/// the empty runs handles that and the single-`&` form alike, so one
/// parser covers both payloads. Everything from the first `;` on is a
/// trailer, not a field.
fn parse_fields(plaintext: &str) -> Vec<(&str, &str)> {
    plaintext
        .split(';')
        .next()
        .unwrap_or_default()
        .split('&')
        .filter(|segment| !segment.is_empty())
        .filter_map(|pair| pair.split_once('='))
        .collect()
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
            // Either marker means the table was right. Testing only for
            // `LaunchTicket=` silently discarded a perfectly good
            // decode of the other payload as "wrong table", which is
            // what made every `ppppp` game fail with a decryption
            // error while MapleStory worked.
            Ok(plaintext)
                if plaintext.contains("LaunchTicket=") || plaintext.contains("ppppp=") =>
            {
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
                decode_launch_data(&blob).unwrap(),
                LaunchPayload::Ticket(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned()
                ),
                "selector {selector} failed"
            );
        }
    }

    /// The payload every non-MapleStory game was observed to hand over.
    /// Field separator is `&&&&` and the `ppppp` is 96 characters, not
    /// the 64-character constant the WPF client pinned — both shapes
    /// taken from live blobs.
    #[test]
    fn legacy_payload_round_trips_through_every_selector() {
        let ppppp = "F".repeat(96);
        let plaintext = format!(
            "ppppp={ppppp}&&&&ServiceCode=600309&&&&ServiceRegion=A2&&&&ServiceAccount=A205b371011500171132&&&&CreateTime=2010-08-18 20:28:29&&&&BeanfunUrl=https://tw.beanfun.com/&&&&WebStartPatch=http://tw.patch.beanfun.gamania.com/beanfun05/;"
        );
        let padded = format!("{plaintext}{}", "\0".repeat((8 - plaintext.len() % 8) % 8));
        let key = "b034e744";
        let cipher_hex = encrypt_hex(&padded, key).unwrap().to_lowercase();

        for selector in 0..16 {
            let blob = encode(selector, key, &cipher_hex);
            let LaunchPayload::Legacy(params) = decode_launch_data(&blob).unwrap() else {
                panic!("selector {selector}: expected the pre-v2 payload");
            };
            assert_eq!(params.ppppp, ppppp, "selector {selector}");
            assert_eq!(params.service_code, "600309");
            assert_eq!(params.service_region, "A2");
            assert_eq!(params.service_account, "A205b371011500171132");
            assert_eq!(params.create_time, "2010-08-18 20:28:29");
        }
    }

    /// A decode that yields neither marker is a wrong table, and every
    /// table being wrong is the one case that is genuinely a failure.
    #[test]
    fn a_payload_with_neither_marker_is_rejected() {
        let plaintext = "SomethingElse=1&Other=2;xx";
        let padded = format!("{plaintext}{}", "\0".repeat((8 - plaintext.len() % 8) % 8));
        let key = "a1b2c3d4";
        let cipher_hex = encrypt_hex(&padded, key).unwrap().to_lowercase();
        let blob = encode(3, key, &cipher_hex);
        assert_eq!(
            decode_launch_data(&blob).unwrap_err(),
            LaunchDataError::MissingTicket
        );
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
        assert_eq!(decode_launch_data("").unwrap_err(), LaunchDataError::Empty);
    }

    #[test]
    fn non_hex_selector_is_rejected() {
        assert_eq!(
            decode_launch_data("zzzz").unwrap_err(),
            LaunchDataError::BadSelector('z')
        );
    }

    #[test]
    fn character_outside_the_table_is_rejected() {
        // Selector 0 → table 0, which has no 'z'.
        let err = decode_launch_data("0zzz").unwrap_err();
        assert!(
            matches!(err, LaunchDataError::UnmappableChar('z', 0)),
            "got {err:?}"
        );
    }

    #[test]
    fn blob_too_short_for_a_key_is_rejected() {
        // Table 0 characters only, but far fewer than offset + 8.
        let err = decode_launch_data("0bac").unwrap_err();
        assert!(
            matches!(err, LaunchDataError::TooShort { .. }),
            "got {err:?}"
        );
    }
}
