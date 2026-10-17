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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn accepts_only_official_https_hosts_without_credentials_or_custom_ports() {
        for url in [
            "https://github.com/pungin/Beanfun/releases/download/v6.3.0.2610180000/Beanfun.exe",
            "https://release-assets.githubusercontent.com/github-production-release-asset/example?token=example",
            "https://objects.githubusercontent.com/github-production-release-asset/example",
        ] {
            assert!(allowed_url(&url.parse().unwrap()), "{url}");
        }
        for url in [
            "http://github.com/pungin/Beanfun/asset",
            "https://github.com.attacker.test/asset",
            "https://attacker.test/asset",
            "https://user:password@github.com/asset",
            "https://github.com:444/asset",
            "file:///tmp/Beanfun.exe",
        ] {
            assert!(!allowed_url(&url.parse().unwrap()), "{url}");
        }
    }

    #[tokio::test]
    async fn rejects_disallowed_urls_before_starting_a_request() {
        assert!(matches!(
            download_bytes(
                "http://127.0.0.1/asset",
                100,
                None,
                Duration::from_secs(1),
                |_, _| {}
            )
            .await,
            Err(DownloadError::InvalidUrl),
        ));
    }

    #[tokio::test]
    async fn downloads_exact_bytes_with_progress_and_rejects_wrong_size() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/asset"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"executable".to_vec()))
            .mount(&server)
            .await;
        let client = Client::new();
        let url: Url = format!("{}/asset", server.uri()).parse().unwrap();
        let mut progress = Vec::new();
        let bytes = download_with_client(&client, url.clone(), 100, Some(10), |n, total| {
            progress.push((n, total))
        })
        .await
        .unwrap();
        assert_eq!(bytes, b"executable");
        assert_eq!(progress.last(), Some(&(10, Some(10))));
        assert!(matches!(
            download_with_client(&client, url.clone(), 100, Some(9), |_, _| {}).await,
            Err(DownloadError::SizeMismatch)
        ));
        assert!(matches!(
            download_with_client(&client, url, 9, None, |_, _| {}).await,
            Err(DownloadError::TooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn enforces_the_bound_on_chunked_responses_without_content_length() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await.unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n5\r\n67890\r\n0\r\n\r\n").await.unwrap();
        });
        let url = format!("http://{address}/asset").parse().unwrap();
        assert!(matches!(
            download_with_client(&Client::new(), url, 8, None, |_, _| {}).await,
            Err(DownloadError::TooLarge { .. })
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stalled_downloads_observe_the_client_deadline() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(1))
                    .set_body_bytes(b"executable".to_vec()),
            )
            .mount(&server)
            .await;
        let client = Client::builder()
            .timeout(Duration::from_millis(30))
            .build()
            .unwrap();
        let error =
            download_with_client(&client, server.uri().parse().unwrap(), 100, None, |_, _| {})
                .await
                .unwrap_err();
        assert!(matches!(error, DownloadError::Network(error) if error.is_timeout()));
    }

    #[test]
    fn rejects_tampered_bytes_and_does_not_accept_a_digest_of_another_artifact() {
        let digest = format!("{:x}", Sha256::digest(b"executable"));
        assert!(verify_digest(b"executable", &digest).is_ok());
        assert!(matches!(
            verify_digest(b"modified executable", &digest),
            Err(DownloadError::DigestMismatch)
        ));
    }
}
