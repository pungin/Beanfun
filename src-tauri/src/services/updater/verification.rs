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
