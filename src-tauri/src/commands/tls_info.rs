//! What the in-app browser's padlock shows: who answered, and with what
//! certificate.
//!
//! ## Why it is a fresh handshake
//!
//! WebView2 will not tell us the certificate of a page that loaded correctly —
//! `ServerCertificateErrorDetected` fires only when validation *fails*, and its
//! `ICoreWebView2Certificate` comes with it. There is no equivalent for the
//! ordinary case. So the padlock opens its own TLS connection to the same host
//! and reports what that one was handed.
//!
//! That is a real distinction and the panel says so: this is the certificate
//! the host is serving now, not a readback of the one this page arrived over.
//! For the question people actually ask it — *is something sitting between me
//! and beanfun?* — it answers well, because a machine-wide interceptor (the
//! game accelerators many users run are exactly this) reissues for every
//! connection, ours included.

use serde::Serialize;

/// How long we will wait on a host before giving up. The padlock is a panel the
/// user is staring at, so it must fail fast rather than hang.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

/// What the padlock panel draws.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    /// False for plain http, which the panel calls out rather than decorates.
    pub encrypted: bool,
    /// None when the handshake failed; `error` then says why.
    pub certificate: Option<CertificateInfo>,
    pub error: Option<String>,
}

/// The leaf certificate, reduced to what is worth reading.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CertificateInfo {
    pub subject: String,
    pub issuer: String,
    /// RFC 2822, which `Date.parse` accepts, so the toolbar can render it
    /// in the user's own locale.
    pub valid_from: String,
    pub valid_to: String,
    /// SHA-256 of the DER, grouped in pairs — the value to compare against
    /// what a second machine sees when something looks wrong.
    pub fingerprint: String,
    pub serial: String,
}

/// Inspect `url`'s host. Blocking; call it off the async workers.
pub fn inspect(url: &url::Url) -> ConnectionInfo {
    let host = url.host_str().unwrap_or_default().to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    let encrypted = url.scheme() == "https";

    if !encrypted || host.is_empty() {
        return ConnectionInfo {
            host,
            port,
            encrypted,
            certificate: None,
            error: None,
        };
    }

    match handshake(&host, port) {
        Ok(cert) => ConnectionInfo {
            host,
            port,
            encrypted,
            certificate: Some(cert),
            error: None,
        },
        Err(e) => ConnectionInfo {
            host,
            port,
            encrypted,
            certificate: None,
            error: Some(e),
        },
    }
}

/// Connect, complete a TLS handshake, and describe the leaf certificate.
fn handshake(host: &str, port: u16) -> Result<CertificateInfo, String> {
    use std::net::ToSocketAddrs;

    let address = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("could not resolve {host}: {e}"))?
        .next()
        .ok_or_else(|| format!("{host} resolved to nothing"))?;

    let stream = std::net::TcpStream::connect_timeout(&address, TIMEOUT)
        .map_err(|e| format!("could not reach {host}:{port}: {e}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();

    let connector =
        native_tls::TlsConnector::new().map_err(|e| format!("TLS setup failed: {e}"))?;
    // A handshake that fails validation still tells the user something useful,
    // but native-tls gives us no certificate in that case, so it stays an error.
    let tls = connector
        .connect(host, stream)
        .map_err(|e| format!("TLS handshake failed: {e}"))?;

    let der = tls
        .peer_certificate()
        .map_err(|e| format!("could not read the certificate: {e}"))?
        .ok_or_else(|| "the server sent no certificate".to_string())?
        .to_der()
        .map_err(|e| format!("could not encode the certificate: {e}"))?;

    describe(&der)
}

/// Pull the human-readable fields out of a DER certificate.
fn describe(der: &[u8]) -> Result<CertificateInfo, String> {
    use sha2::Digest;
    use x509_parser::prelude::*;

    let (_, cert) = X509Certificate::from_der(der)
        .map_err(|e| format!("could not parse the certificate: {e}"))?;

    let digest = sha2::Sha256::digest(der);
    let fingerprint = digest
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|pair| pair.join(""))
        .collect::<Vec<_>>()
        .join(" ");

    Ok(CertificateInfo {
        subject: common_name(cert.subject()).unwrap_or_else(|| cert.subject().to_string()),
        issuer: common_name(cert.issuer()).unwrap_or_else(|| cert.issuer().to_string()),
        valid_from: cert.validity().not_before.to_rfc2822().unwrap_or_default(),
        valid_to: cert.validity().not_after.to_rfc2822().unwrap_or_default(),
        fingerprint,
        serial: cert.tbs_certificate.raw_serial_as_string(),
    })
}

/// The CN if there is one — the whole DN is accurate but not readable.
fn common_name(name: &x509_parser::x509::X509Name) -> Option<String> {
    name.iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_is_reported_unencrypted_without_reaching_out() {
        let info = inspect(&"http://example.com/x".parse().unwrap());
        assert!(!info.encrypted);
        assert_eq!(info.host, "example.com");
        assert_eq!(info.port, 80);
        assert!(info.certificate.is_none());
        assert!(info.error.is_none());
    }

    #[test]
    fn a_certificate_is_reduced_to_readable_fields() {
        // A self-signed leaf is enough: nothing here validates a chain.
        let der = include_bytes!("../../tests/fixtures/example-cert.der");
        let cert = describe(der).expect("the fixture should parse");
        assert_eq!(cert.subject, "example.test");
        assert_eq!(cert.issuer, "example.test");
        assert!(cert.valid_from.contains("2020"));
        // Grouped in pairs: 32 bytes become 16 groups of four hex digits.
        assert_eq!(cert.fingerprint.split(' ').count(), 16);
        assert!(cert
            .fingerprint
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == ' '));
    }
}
