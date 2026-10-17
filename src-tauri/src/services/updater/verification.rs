//! Authenticate update manifests with Sigstore and a fixed GitHub release policy.
//!
//! The writable TUF cache is never a trust anchor. Both the application and the
//! helper verify it from Sigstore's embedded bootstrap root before using it.

use std::{collections::BTreeMap, path::Path, time::Duration};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
pub use sigstore_trust_root::TrustedRoot;
use sigstore_trust_root::TufConfig;
use sigstore_types::{Bundle, SignatureContent};
use sigstore_verify::{
    crypto::FulcioCiClaims, IdentityMatcher, VerificationPolicy, VerificationResult, Verifier,
};

use super::manifest::{MAX_BUNDLE_BYTES, MAX_MANIFEST_BYTES};

const ISSUER: &str = "https://token.actions.githubusercontent.com";
const REPOSITORY: &str = "https://github.com/pungin/Beanfun";
const OWNER: &str = "https://github.com/pungin";
const REPOSITORY_ID: &str = "155946625";
const OWNER_ID: &str = "11711288";
const SOURCE_REF: &str = "refs/heads/code";
const WORKFLOW_PATH: &str = ".github/workflows/build-and-release.yml";
const WORKFLOW: &str =
    "https://github.com/pungin/Beanfun/.github/workflows/build-and-release.yml@refs/heads/code";
const SOURCE_URI: &str = "git+https://github.com/pungin/Beanfun@refs/heads/code";
const BUILD_TYPE: &str = "https://actions.github.io/buildtypes/workflow/v1";

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct VerificationError {
    pub code: &'static str,
    pub message: String,
}

impl VerificationError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "update.signature_invalid",
            message: message.into(),
        }
    }

    fn trust(message: impl Into<String>) -> Self {
        Self {
            code: "update.trust_unavailable",
            message: message.into(),
        }
    }
}

/// Refresh the public-good TUF repository, including authenticated root rotation.
pub async fn fetch_trust_root(cache_dir: &Path) -> Result<TrustedRoot, VerificationError> {
    let client = sigstore_trust_root::reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .redirect(sigstore_trust_root::reqwest::redirect::Policy::none())
        .https_only(true)
        .user_agent("Beanfun updater")
        .build()
        .map_err(|error| VerificationError::trust(format!("Cannot initialise TUF: {error}")))?;
    let config = TufConfig::production()
        .with_cache_dir(cache_dir.to_path_buf())
        .with_http_client(client);
    tokio::time::timeout(Duration::from_secs(60), TrustedRoot::from_tuf(config))
        .await
        .map_err(|_| VerificationError::trust("The TUF refresh timed out"))?
        .map_err(|error| {
            VerificationError::trust(format!("Cannot authenticate TUF metadata: {error}"))
        })
}

/// Reauthenticate the cached chain and its expiry without network access.
///
/// This deliberately uses the embedded bootstrap, never `UnsafeCachedRoot` or
/// a raw cached `trusted_root.json`. Missing, altered or expired data fails shut.
pub async fn load_cached_trust_root(cache_dir: &Path) -> Result<TrustedRoot, VerificationError> {
    let config = TufConfig::production()
        .with_cache_dir(cache_dir.to_path_buf())
        .offline();
    TrustedRoot::from_tuf(config).await.map_err(|error| {
        VerificationError::trust(format!("Cannot authenticate cached TUF metadata: {error}"))
    })
}

/// Evidence created only after signature, transparency and release-policy checks.
#[derive(Debug)]
pub struct VerifiedManifest {
    source_commit: String,
    manifest_sha256: String,
    subjects: BTreeMap<String, String>,
}

impl VerifiedManifest {
    pub fn verify(
        manifest_bytes: &[u8],
        bundle_bytes: &[u8],
        root: &TrustedRoot,
    ) -> Result<Self, VerificationError> {
        if manifest_bytes.is_empty()
            || manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES
            || bundle_bytes.is_empty()
            || bundle_bytes.len() as u64 > MAX_BUNDLE_BYTES
        {
            return Err(VerificationError::invalid(
                "The attestation or manifest exceeds its size limit",
            ));
        }
        let bundle_text = std::str::from_utf8(bundle_bytes)
            .map_err(|_| VerificationError::invalid("The attestation is not UTF-8"))?;
        let bundle = Bundle::from_json(bundle_text).map_err(|error| {
            VerificationError::invalid(format!("Invalid Sigstore bundle: {error}"))
        })?;
        let result = verify_crypto(manifest_bytes, &bundle, root, &release_policy())?;
        let certificate = result.certificate().ok_or_else(|| {
            VerificationError::invalid("The attestation has no verified certificate")
        })?;

        // Only interpret the manifest after authenticating its exact downloaded bytes.
        #[derive(Deserialize)]
        struct ManifestSource {
            source_commit: String,
        }
        let manifest: ManifestSource = serde_json::from_slice(manifest_bytes)
            .map_err(|_| VerificationError::invalid("The authenticated manifest is invalid"))?;
        validate_claims(&certificate.ci_claims, &manifest.source_commit)?;
        let manifest_sha256 = format!("{:x}", Sha256::digest(manifest_bytes));
        let subjects = validate_statement(&bundle, &manifest.source_commit, &manifest_sha256)?;
        Ok(Self {
            source_commit: manifest.source_commit,
            manifest_sha256,
            subjects,
        })
    }

    pub fn source_commit(&self) -> &str {
        &self.source_commit
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// Require the selected executable to be a subject of the same attestation.
    pub fn verify_subject(&self, name: &str, sha256: &str) -> Result<(), VerificationError> {
        if self.subjects.get(name).map(String::as_str) != Some(sha256) {
            return Err(VerificationError::invalid(
                "The executable is not a subject of this release attestation",
            ));
        }
        Ok(())
    }
}

pub fn verify_manifest(
    manifest_bytes: &[u8],
    bundle_bytes: &[u8],
    root: &TrustedRoot,
) -> Result<VerifiedManifest, VerificationError> {
    VerifiedManifest::verify(manifest_bytes, bundle_bytes, root)
}

fn release_policy() -> VerificationPolicy {
    VerificationPolicy::new(IdentityMatcher::Uri(WORKFLOW.to_owned()), ISSUER)
}

fn verify_crypto(
    artifact: &[u8],
    bundle: &Bundle,
    root: &TrustedRoot,
    policy: &VerificationPolicy,
) -> Result<VerificationResult, VerificationError> {
    let verifier = Verifier::new(root).map_err(|error| {
        VerificationError::trust(format!("Invalid Sigstore trust material: {error}"))
    })?;
    let result = verifier.verify(artifact, bundle, policy).map_err(|error| {
        VerificationError::invalid(format!("Sigstore verification failed: {error}"))
    })?;
    if !result.certificate_verified()
        || !result.sct_verified()
        || !result.tlog_verified()
        || !result.identity_policy_checked()
    {
        return Err(VerificationError::invalid(
            "The attestation lacks required verification evidence",
        ));
    }
    Ok(result)
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_claims(claims: &FulcioCiClaims, source_commit: &str) -> Result<(), VerificationError> {
    if !is_lower_hex(source_commit, 40) {
        return Err(VerificationError::invalid("The source commit is invalid"));
    }
    let expected = [
        (claims.build_signer_uri.as_deref(), WORKFLOW),
        (claims.build_signer_digest.as_deref(), source_commit),
        (claims.build_config_uri.as_deref(), WORKFLOW),
        (claims.build_config_digest.as_deref(), source_commit),
        (claims.source_repository_uri.as_deref(), REPOSITORY),
        (
            claims.source_repository_identifier.as_deref(),
            REPOSITORY_ID,
        ),
        (claims.source_repository_owner_uri.as_deref(), OWNER),
        (
            claims.source_repository_owner_identifier.as_deref(),
            OWNER_ID,
        ),
        (claims.source_repository_ref.as_deref(), SOURCE_REF),
        (claims.source_repository_digest.as_deref(), source_commit),
        (claims.runner_environment.as_deref(), "github-hosted"),
        (claims.build_trigger.as_deref(), "workflow_dispatch"),
        (
            claims.source_repository_visibility_at_signing.as_deref(),
            "public",
        ),
    ];
    if expected
        .iter()
        .any(|(actual, expected)| *actual != Some(*expected))
    {
        return Err(VerificationError::invalid(
            "The certificate does not identify the authorised release workflow and source",
        ));
    }
    Ok(())
}

fn validate_statement(
    bundle: &Bundle,
    source_commit: &str,
    manifest_sha256: &str,
) -> Result<BTreeMap<String, String>, VerificationError> {
    let SignatureContent::DsseEnvelope(envelope) = &bundle.content else {
        return Err(VerificationError::invalid(
            "The release requires a DSSE attestation",
        ));
    };
    if envelope.payload_type != "application/vnd.in-toto+json" {
        return Err(VerificationError::invalid(
            "The attestation payload type is invalid",
        ));
    }
    let statement: Value = serde_json::from_slice(envelope.payload.as_bytes())
        .map_err(|_| VerificationError::invalid("The attestation statement is invalid"))?;
    validate_provenance(&statement, source_commit)?;
    let entries = statement["subject"]
        .as_array()
        .filter(|subjects| subjects.len() == 3)
        .ok_or_else(|| {
            VerificationError::invalid("The release must attest its manifest and both executables")
        })?;
    let mut subjects = BTreeMap::new();
    for subject in entries {
        let name = subject["name"]
            .as_str()
            .ok_or_else(|| VerificationError::invalid("Missing subject name"))?;
        let digest = subject["digest"]["sha256"]
            .as_str()
            .filter(|value| is_lower_hex(value, 64))
            .ok_or_else(|| VerificationError::invalid("Invalid subject digest"))?;
        if name.is_empty()
            || name.contains(['/', '\\'])
            || subjects
                .insert(name.to_owned(), digest.to_owned())
                .is_some()
        {
            return Err(VerificationError::invalid(
                "Invalid or duplicate attestation subject",
            ));
        }
    }
    if subjects.get("latest.json").map(String::as_str) != Some(manifest_sha256)
        || !subjects.contains_key("Beanfun.exe")
        || !subjects
            .keys()
            .any(|name| name.starts_with("Beanfun_") && name.ends_with("_x64-setup.exe"))
    {
        return Err(VerificationError::invalid(
            "The attestation does not bind the expected release files",
        ));
    }
    Ok(subjects)
}

fn validate_provenance(statement: &Value, source_commit: &str) -> Result<(), VerificationError> {
    let fields = [
        ("/_type", "https://in-toto.io/Statement/v1"),
        ("/predicateType", "https://slsa.dev/provenance/v1"),
        ("/predicate/buildDefinition/buildType", BUILD_TYPE),
        (
            "/predicate/buildDefinition/externalParameters/workflow/ref",
            SOURCE_REF,
        ),
        (
            "/predicate/buildDefinition/externalParameters/workflow/repository",
            REPOSITORY,
        ),
        (
            "/predicate/buildDefinition/externalParameters/workflow/path",
            WORKFLOW_PATH,
        ),
        (
            "/predicate/buildDefinition/internalParameters/github/event_name",
            "workflow_dispatch",
        ),
        (
            "/predicate/buildDefinition/internalParameters/github/repository_id",
            REPOSITORY_ID,
        ),
        (
            "/predicate/buildDefinition/internalParameters/github/repository_owner_id",
            OWNER_ID,
        ),
        (
            "/predicate/buildDefinition/internalParameters/github/runner_environment",
            "github-hosted",
        ),
        ("/predicate/runDetails/builder/id", WORKFLOW),
    ];
    if fields
        .iter()
        .any(|(path, expected)| statement.pointer(path).and_then(Value::as_str) != Some(*expected))
    {
        return Err(VerificationError::invalid(
            "The provenance does not identify the authorised release build",
        ));
    }
    let dependencies = statement
        .pointer("/predicate/buildDefinition/resolvedDependencies")
        .and_then(Value::as_array)
        .ok_or_else(|| VerificationError::invalid("The provenance has no source dependency"))?;
    let matching: Vec<_> = dependencies
        .iter()
        .filter(|entry| entry["uri"].as_str() == Some(SOURCE_URI))
        .collect();
    if matching.len() != 1 || matching[0]["digest"]["gitCommit"].as_str() != Some(source_commit) {
        return Err(VerificationError::invalid(
            "The provenance source commit does not match the manifest",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_BUNDLE: &str = include_str!("fixtures/conda-attestation.sigstore.json");
    const FIXTURE_ARTIFACT: &[u8] =
        include_bytes!("fixtures/signed-package-2.1.0-hb0f4dca_0.conda");
    const COMMIT: &str = "1234567890abcdef1234567890abcdef12345678";

    fn fixture_root() -> TrustedRoot {
        // Immutable upstream fixtures use their embedded historical public roots.
        // Production never falls back to these bytes when TUF is unavailable.
        TrustedRoot::from_json(sigstore_trust_root::SIGSTORE_PRODUCTION_TRUSTED_ROOT).unwrap()
    }

    fn fixture_policy() -> VerificationPolicy {
        VerificationPolicy::new(
            IdentityMatcher::Uri("https://github.com/prefix-dev/sigstore-example/.github/workflows/action.yaml@refs/heads/main".to_owned()),
            ISSUER,
        )
    }

    #[test]
    fn authentic_upstream_attestation_passes_all_cryptographic_checks() {
        let bundle = Bundle::from_json(FIXTURE_BUNDLE).unwrap();
        let result = verify_crypto(
            FIXTURE_ARTIFACT,
            &bundle,
            &fixture_root(),
            &fixture_policy(),
        )
        .unwrap();
        assert_eq!(
            result
                .certificate()
                .unwrap()
                .ci_claims
                .source_repository_digest
                .as_deref(),
            Some("193b5bd7d3985809503963ae400594ea16df31cf")
        );
    }

    #[test]
    fn changed_artifact_is_rejected() {
        let bundle = Bundle::from_json(FIXTURE_BUNDLE).unwrap();
        let mut artifact = FIXTURE_ARTIFACT.to_vec();
        artifact[0] ^= 1;
        assert!(verify_crypto(&artifact, &bundle, &fixture_root(), &fixture_policy()).is_err());
    }

    #[test]
    fn changed_transparency_proof_is_rejected() {
        let mut bundle = Bundle::from_json(FIXTURE_BUNDLE).unwrap();
        let proof = bundle.verification_material.tlog_entries[0]
            .inclusion_proof
            .as_mut()
            .unwrap();
        proof.hashes[0] = sigstore_types::Sha256Hash::new([0; 32]);
        assert!(verify_crypto(
            FIXTURE_ARTIFACT,
            &bundle,
            &fixture_root(),
            &fixture_policy()
        )
        .is_err());
    }

    #[test]
    fn changed_dsse_payload_is_rejected_even_when_its_json_is_equivalent() {
        let mut bundle = Bundle::from_json(FIXTURE_BUNDLE).unwrap();
        let SignatureContent::DsseEnvelope(envelope) = &mut bundle.content else {
            panic!("DSSE fixture");
        };
        let mut payload = envelope.payload.as_bytes().to_vec();
        payload.push(b' ');
        envelope.payload = payload.into();
        assert!(verify_crypto(
            FIXTURE_ARTIFACT,
            &bundle,
            &fixture_root(),
            &fixture_policy()
        )
        .is_err());
    }

    #[test]
    fn authentic_attestation_from_another_repository_is_rejected() {
        let bundle = Bundle::from_json(FIXTURE_BUNDLE).unwrap();
        assert!(verify_crypto(
            FIXTURE_ARTIFACT,
            &bundle,
            &fixture_root(),
            &release_policy()
        )
        .is_err());
    }

    fn authorised_claims() -> FulcioCiClaims {
        let mut claims = FulcioCiClaims::default();
        claims.build_signer_uri = Some(WORKFLOW.into());
        claims.build_signer_digest = Some(COMMIT.into());
        claims.build_config_uri = Some(WORKFLOW.into());
        claims.build_config_digest = Some(COMMIT.into());
        claims.source_repository_uri = Some(REPOSITORY.into());
        claims.source_repository_identifier = Some(REPOSITORY_ID.into());
        claims.source_repository_owner_uri = Some(OWNER.into());
        claims.source_repository_owner_identifier = Some(OWNER_ID.into());
        claims.source_repository_ref = Some(SOURCE_REF.into());
        claims.source_repository_digest = Some(COMMIT.into());
        claims.runner_environment = Some("github-hosted".into());
        claims.build_trigger = Some("workflow_dispatch".into());
        claims.source_repository_visibility_at_signing = Some("public".into());
        claims
    }

    #[test]
    fn release_claim_policy_rejects_wrong_source_repository_runner_and_event() {
        let claims = authorised_claims();
        validate_claims(&claims, COMMIT).unwrap();
        let mut invalid = Vec::new();
        let mut value = claims.clone();
        value.source_repository_identifier = Some("1".into());
        invalid.push(value);
        let mut value = claims.clone();
        value.source_repository_owner_identifier = Some("1".into());
        invalid.push(value);
        let mut value = claims.clone();
        value.source_repository_ref = Some("refs/pull/1/merge".into());
        invalid.push(value);
        let mut value = claims.clone();
        value.source_repository_digest = Some("0".repeat(40));
        invalid.push(value);
        let mut value = claims.clone();
        value.build_trigger = Some("pull_request".into());
        invalid.push(value);
        let mut value = claims.clone();
        value.runner_environment = Some("self-hosted".into());
        invalid.push(value);
        let mut value = claims;
        value.build_signer_uri = None;
        invalid.push(value);
        for value in invalid {
            assert!(validate_claims(&value, COMMIT).is_err());
        }
    }

    fn authorised_statement() -> Value {
        serde_json::json!({
            "_type": "https://in-toto.io/Statement/v1",
            "predicateType": "https://slsa.dev/provenance/v1",
            "subject": [
                {"name": "latest.json", "digest": {"sha256": "1".repeat(64)}},
                {"name": "Beanfun.exe", "digest": {"sha256": "2".repeat(64)}},
                {"name": "Beanfun_6.2.0_x64-setup.exe", "digest": {"sha256": "3".repeat(64)}}
            ],
            "predicate": {
                "buildDefinition": {
                    "buildType": BUILD_TYPE,
                    "externalParameters": {"workflow": {"ref": SOURCE_REF, "repository": REPOSITORY, "path": WORKFLOW_PATH}},
                    "internalParameters": {"github": {"event_name": "workflow_dispatch", "repository_id": REPOSITORY_ID, "repository_owner_id": OWNER_ID, "runner_environment": "github-hosted"}},
                    "resolvedDependencies": [{"uri": SOURCE_URI, "digest": {"gitCommit": COMMIT}}]
                },
                "runDetails": {"builder": {"id": WORKFLOW}}
            }
        })
    }

    #[test]
    fn provenance_requires_bound_source_and_workflow() {
        let statement = authorised_statement();
        validate_provenance(&statement, COMMIT).unwrap();
        let mut changed = statement.clone();
        changed["predicate"]["buildDefinition"]["resolvedDependencies"][0]["digest"]["gitCommit"] =
            Value::String("0".repeat(40));
        assert!(validate_provenance(&changed, COMMIT).is_err());
        let mut changed = statement;
        changed["predicate"]["buildDefinition"]["externalParameters"]["workflow"]["ref"] =
            Value::String("refs/heads/other".into());
        assert!(validate_provenance(&changed, COMMIT).is_err());
    }

    #[test]
    fn statement_requires_manifest_digest_and_both_unique_executables() {
        // These are policy-only mutations; no test treats them as signed evidence.
        let mut bundle = Bundle::from_json(FIXTURE_BUNDLE).unwrap();
        let SignatureContent::DsseEnvelope(envelope) = &mut bundle.content else {
            panic!("DSSE fixture");
        };
        let statement = authorised_statement();
        envelope.payload = serde_json::to_vec(&statement).unwrap().into();
        validate_statement(&bundle, COMMIT, &"1".repeat(64)).unwrap();
        assert!(validate_statement(&bundle, COMMIT, &"0".repeat(64)).is_err());
        let mut duplicate = statement;
        duplicate["subject"][2]["name"] = Value::String("Beanfun.exe".into());
        let SignatureContent::DsseEnvelope(envelope) = &mut bundle.content else {
            unreachable!();
        };
        envelope.payload = serde_json::to_vec(&duplicate).unwrap().into();
        assert!(validate_statement(&bundle, COMMIT, &"1".repeat(64)).is_err());
    }

    #[tokio::test]
    async fn arbitrary_cached_root_cannot_establish_trust() {
        let directory = tempfile::tempdir().unwrap();
        let targets = directory.path().join("targets");
        std::fs::create_dir(&targets).unwrap();
        std::fs::write(directory.path().join("root.json"), b"{}").unwrap();
        std::fs::write(
            targets.join("trusted_root.json"),
            sigstore_trust_root::SIGSTORE_PRODUCTION_TRUSTED_ROOT,
        )
        .unwrap();
        let error = load_cached_trust_root(directory.path()).await.unwrap_err();
        assert_eq!(error.code, "update.trust_unavailable");
    }

    #[tokio::test]
    #[ignore = "Requires the public Sigstore TUF service; ordinary tests remain offline"]
    async fn live_tuf_cache_roundtrip_rejects_tampering() {
        let directory = tempfile::tempdir().unwrap();
        fetch_trust_root(directory.path()).await.unwrap();
        load_cached_trust_root(directory.path()).await.unwrap();

        let target = directory.path().join("targets/trusted_root.json");
        let original = std::fs::read(&target).unwrap();
        std::fs::write(&target, b"{}").unwrap();
        assert_eq!(
            load_cached_trust_root(directory.path())
                .await
                .unwrap_err()
                .code,
            "update.trust_unavailable"
        );

        std::fs::write(&target, original).unwrap();
        load_cached_trust_root(directory.path()).await.unwrap();
        std::fs::write(directory.path().join("timestamp.json"), b"{}").unwrap();
        assert_eq!(
            load_cached_trust_root(directory.path())
                .await
                .unwrap_err()
                .code,
            "update.trust_unavailable"
        );
    }
}
