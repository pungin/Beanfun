//! Bounded HTTPS downloads for authenticated update metadata and executable assets.

use std::time::Duration;

use reqwest::{Client, Url};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("The update URL is outside the official release hosts")]
    InvalidUrl,
    #[error("The update download failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("The update response exceeds its {limit}-byte limit")]
    TooLarge { limit: u64 },
    #[error("The download size does not match the authenticated release manifest")]
    SizeMismatch,
    #[error("The download digest does not match the authenticated release manifest")]
    DigestMismatch,
}

fn allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn client(timeout: Duration) -> Result<Client, DownloadError> {
    Ok(Client::builder()
        .user_agent("Beanfun automatic updater")
        .https_only(true)
        .connect_timeout(Duration::from_secs(20))
        .timeout(timeout)
        .no_gzip()
        .no_deflate()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !allowed_url(attempt.url()) {
                attempt.error("Update redirect is outside the official HTTPS release hosts")
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

/// Download without trusting Content-Length: enforce the bound after every chunk.
pub async fn download_bytes<F: FnMut(u64, Option<u64>)>(
    url: &str,
    limit: u64,
    exact_size: Option<u64>,
    timeout: Duration,
    on_progress: F,
) -> Result<Vec<u8>, DownloadError> {
    let url = Url::parse(url).map_err(|_| DownloadError::InvalidUrl)?;
    if !allowed_url(&url) {
        return Err(DownloadError::InvalidUrl);
    }
    download_with_client(&client(timeout)?, url, limit, exact_size, on_progress).await
}

async fn download_with_client<F: FnMut(u64, Option<u64>)>(
    client: &Client,
    url: Url,
    limit: u64,
    exact_size: Option<u64>,
    mut on_progress: F,
) -> Result<Vec<u8>, DownloadError> {
    if exact_size.is_some_and(|size| size > limit) {
        return Err(DownloadError::TooLarge { limit });
    }
    let mut response = client.get(url).send().await?.error_for_status()?;
    let content_length = response.content_length();
    if content_length.is_some_and(|length| length > limit) {
        return Err(DownloadError::TooLarge { limit });
    }
    if let (Some(expected), Some(length)) = (exact_size, content_length) {
        if expected != length {
            return Err(DownloadError::SizeMismatch);
        }
    }
    let total = exact_size.or(content_length);
    on_progress(0, total);
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let length = (bytes.len() as u64).saturating_add(chunk.len() as u64);
        if length > limit {
            return Err(DownloadError::TooLarge { limit });
        }
        if exact_size.is_some_and(|expected| length > expected) {
            return Err(DownloadError::SizeMismatch);
        }
        bytes.extend_from_slice(&chunk);
        on_progress(length, total);
    }
    if exact_size.is_some_and(|expected| bytes.len() as u64 != expected) {
        return Err(DownloadError::SizeMismatch);
    }
    Ok(bytes)
}

/// The expected digest comes only from the authenticated release manifest.
pub fn verify_digest(bytes: &[u8], expected: &str) -> Result<(), DownloadError> {
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err(DownloadError::DigestMismatch);
    }
    Ok(())
}
