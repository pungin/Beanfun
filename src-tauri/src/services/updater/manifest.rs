//! Bind an authenticated release manifest to the chosen release and distribution.

use std::collections::HashMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::install::InstallationKind;
use super::verification::VerifiedManifest;
use super::{is_newer_version, parse_tag, GitHubRelease};

pub const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024;

/// Routing details selected from the official release, never from a renderer URL.
#[derive(Debug, PartialEq, Eq)]
pub struct UpdateRequest {
    pub endpoint: String,
    pub bundle_url: String,
    pub download_url: String,
    pub filename: String,
    pub version: String,
    pub tag: String,
    pub target: &'static str,
}

/// This document is parsed only after its attestation has been authenticated.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub version: String,
    pub tag: String,
    pub source_commit: String,
    pub pub_date: String,
    pub notes: String,
    pub platforms: HashMap<String, ManifestArtifact>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestArtifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// A distribution whose bytes and release routing are bound to an authenticated manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub filename: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("The selected release changed or is no longer newer than this application")]
    ReleaseChanged,
    #[error("This release does not provide a complete automatic update for this distribution")]
    InvalidManifest,
    #[error("The package is not bound to the authenticated release manifest")]
    UnauthenticatedPackage,
}

pub fn request_for_release(
    release: &GitHubRelease,
    expected_tag: &str,
    local_version: &str,
    kind: InstallationKind,
) -> Result<UpdateRequest, ManifestError> {
    if release.tag_name != expected_tag {
        return Err(ManifestError::ReleaseChanged);
    }
    let request = request_from_tag(expected_tag, local_version, kind)?;
    for required in [
        &request.endpoint,
        &request.bundle_url,
        &request.download_url,
    ] {
        if !release
            .assets
            .iter()
            .any(|asset| &asset.browser_download_url == required)
        {
            return Err(ManifestError::InvalidManifest);
        }
    }
    Ok(request)
}

fn request_from_tag(
    tag: &str,
    local_version: &str,
    kind: InstallationKind,
) -> Result<UpdateRequest, ManifestError> {
    let version = parse_tag(tag).map_err(|_| ManifestError::InvalidManifest)?;
    if !is_newer_version(local_version, &version) {
        return Err(ManifestError::ReleaseChanged);
    }
    let version = format!("{}.{}.{}", version.major, version.minor, version.patch);
    let base = release_base(tag);
    let (filename, target) = match kind {
        InstallationKind::Portable => ("Beanfun.exe".to_owned(), "windows-x86_64-portable"),
        InstallationKind::Nsis => (format!("Beanfun_{version}_x64-setup.exe"), "windows-x86_64"),
    };
    let endpoint = format!("{base}latest.json");
    let bundle_url = format!("{base}latest.json.sigstore.json");
    let download_url = format!("{base}{filename}");
    Ok(UpdateRequest {
        endpoint,
        bundle_url,
        download_url,
        filename,
        version,
        tag: tag.to_owned(),
        target,
    })
}

/// Shared by preparation and the executable-replacement helper before either can execute a package.
pub fn validate_signed_package(
    manifest_bytes: &[u8],
    verified: &VerifiedManifest,
    kind: InstallationKind,
    local_version: &str,
) -> Result<Package, ManifestError> {
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ManifestError::InvalidManifest);
    }
    if format!("{:x}", Sha256::digest(manifest_bytes)) != verified.manifest_sha256() {
        return Err(ManifestError::UnauthenticatedPackage);
    }
    let manifest: ReleaseManifest =
        serde_json::from_slice(manifest_bytes).map_err(|_| ManifestError::InvalidManifest)?;
    let request = request_from_tag(&manifest.tag, local_version, kind)?;
    let artifact = request.validate(manifest_bytes, verified.source_commit())?;
    verified
        .verify_subject(&request.filename, &artifact.sha256)
        .map_err(|_| ManifestError::UnauthenticatedPackage)?;
    Ok(Package {
        url: artifact.url,
        sha256: artifact.sha256,
        size: artifact.size,
        filename: request.filename,
    })
}

fn release_base(tag: &str) -> String {
    format!("https://github.com/pungin/Beanfun/releases/download/{tag}/")
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

impl UpdateRequest {
    /// Bind all manifest routing to the approved release and authenticated workflow commit.
    pub fn validate(
        &self,
        bytes: &[u8],
        authenticated_source_commit: &str,
    ) -> Result<ManifestArtifact, ManifestError> {
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(ManifestError::InvalidManifest);
        }
        let manifest: ReleaseManifest =
            serde_json::from_slice(bytes).map_err(|_| ManifestError::InvalidManifest)?;
        if manifest.schema_version != 1
            || manifest.version != self.version
            || manifest.tag != self.tag
            || !lower_hex(&manifest.source_commit, 40)
            || manifest.source_commit != authenticated_source_commit
            || chrono::DateTime::parse_from_rfc3339(&manifest.pub_date).is_err()
            || manifest.notes.len() as u64 > MAX_MANIFEST_BYTES
            || manifest.platforms.len() != 2
        {
            return Err(ManifestError::InvalidManifest);
        }
        let base = release_base(&self.tag);
        for (target, filename) in [
            ("windows-x86_64-portable", "Beanfun.exe".to_owned()),
            (
                "windows-x86_64",
                format!("Beanfun_{}_x64-setup.exe", self.version),
            ),
        ] {
            let artifact = manifest
                .platforms
                .get(target)
                .ok_or(ManifestError::InvalidManifest)?;
            if artifact.url != format!("{base}{filename}")
                || !lower_hex(&artifact.sha256, 64)
                || artifact.size == 0
                || artifact.size > MAX_EXECUTABLE_BYTES
            {
                return Err(ManifestError::InvalidManifest);
            }
        }
        let artifact = manifest
            .platforms
            .get(self.target)
            .ok_or(ManifestError::InvalidManifest)?;
        if artifact.url != self.download_url {
            return Err(ManifestError::InvalidManifest);
        }
        Ok(artifact.clone())
    }
}
