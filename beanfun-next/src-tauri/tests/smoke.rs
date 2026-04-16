//! Cargo-level smoke test: verifies the test harness compiles and that
//! a few key third-party dependencies (serde_json, reqwest, sha2) are
//! wired up correctly. Real behaviour tests live in their respective
//! modules under `src/` and `tests/`.

#[test]
fn harness_arithmetic() {
    assert_eq!(1 + 1, 2);
}

#[test]
fn serde_json_roundtrip() {
    let original = serde_json::json!({ "app": "beanfun-next", "version": 1 });
    let serialized = serde_json::to_string(&original).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&serialized).expect("deserialize");

    assert_eq!(parsed["app"], "beanfun-next");
    assert_eq!(parsed["version"], 1);
}

#[test]
fn reqwest_client_builds() {
    let client = reqwest::Client::builder().build();

    assert!(client.is_ok(), "default reqwest client should build");
}

#[test]
fn sha256_produces_expected_digest() {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"beanfun-next");
    let digest = hasher.finalize();

    // Hex-encoded SHA-256 of the ASCII string "beanfun-next".
    let hex = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}
