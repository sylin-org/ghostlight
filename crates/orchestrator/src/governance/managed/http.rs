//! Bounded conditional HTTPS transport for organization-hosted policy bundles.
//!
//! A fetch is one GET that follows no redirect, gives each phase of the exchange 15 seconds and
//! reads at most 8 MiB. TLS is rustls with the ring provider, against Mozilla's roots or, when the
//! bootstrap pins an organization CA, against that CA alone.

use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{pem::PemObject as _, CertificateDer};
use thiserror::Error;
use ureq::tls::{Certificate, RootCerts, TlsConfig};

use super::Bootstrap;

const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) enum FetchOutcome {
    Modified {
        bytes: Vec<u8>,
        etag: Option<String>,
    },
    NotModified,
}

#[derive(Debug, Error)]
pub(super) enum FetchError {
    #[error("transport failed: {0}")]
    Transport(String),
    #[error("invalid organization CA pin: {0}")]
    Pin(String),
    #[error("policy endpoint returned HTTP {0}")]
    Status(u16),
    #[error("policy endpoint returned more than 8 MiB")]
    TooLarge,
}

pub(super) fn fetch(bootstrap: &Bootstrap, etag: Option<&str>) -> Result<FetchOutcome, FetchError> {
    let agent = agent(bootstrap.ca_cert_pem.as_deref())?;
    let mut request = agent.get(bootstrap.source.as_str());
    if let Some(token) = &bootstrap.bearer_token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    if let Some(etag) = etag {
        request = request.header("If-None-Match", etag);
    }
    let response = request.call().map_err(transport)?;
    match response.status().as_u16() {
        304 => Ok(FetchOutcome::NotModified),
        200 => {
            let etag = response
                .headers()
                .get("ETag")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let mut body = response.into_body();
            let bytes = body
                .with_config()
                .limit(MAX_BUNDLE_BYTES)
                .read_to_vec()
                .map_err(|error| match error {
                    ureq::Error::BodyExceedsLimit(_) => FetchError::TooLarge,
                    other => transport(other),
                })?;
            Ok(FetchOutcome::Modified { bytes, etag })
        }
        code => Err(FetchError::Status(code)),
    }
}

/// A failure before a status arrived, or while the body was read.
fn transport(error: ureq::Error) -> FetchError {
    FetchError::Transport(error.to_string())
}

fn agent(ca_cert_pem: Option<&str>) -> Result<ureq::Agent, FetchError> {
    let roots = match ca_cert_pem {
        Some(pem) => pinned_roots(pem)?,
        None => RootCerts::WebPki,
    };
    let tls = TlsConfig::builder()
        .root_certs(roots)
        .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .build();
    Ok(ureq::Agent::config_builder()
        // A redirect comes back as its own status and is never followed.
        .max_redirects(0)
        // Every status is an answer this module reads itself.
        .http_status_as_error(false)
        .timeout_connect(Some(REQUEST_TIMEOUT))
        .timeout_send_request(Some(REQUEST_TIMEOUT))
        .timeout_recv_response(Some(REQUEST_TIMEOUT))
        .timeout_recv_body(Some(REQUEST_TIMEOUT))
        .tls_config(tls)
        .build()
        .new_agent())
}

/// The organization CA pin as the only roots. Each certificate is checked as a trust anchor here,
/// so a bad pin is refused before any network work rather than at the first connection.
fn pinned_roots(pem: &str) -> Result<RootCerts, FetchError> {
    let mut anchors = rustls::RootCertStore::empty();
    let mut certificates = Vec::new();
    for certificate in CertificateDer::pem_slice_iter(pem.as_bytes()) {
        let der = certificate.map_err(|error| FetchError::Pin(error.to_string()))?;
        anchors
            .add(der.clone())
            .map_err(|error| FetchError::Pin(error.to_string()))?;
        certificates.push(Certificate::from_der(der.as_ref()).to_owned());
    }
    if certificates.is_empty() {
        return Err(FetchError::Pin("no certificate was found".into()));
    }
    Ok(RootCerts::Specific(Arc::new(certificates)))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, Write as _};
    use std::net::TcpListener;

    use super::{fetch, pinned_roots, FetchError, FetchOutcome};
    use crate::governance::managed::Bootstrap;

    #[test]
    fn conditional_fetch_carries_bearer_and_etag_without_following_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut bearer = false;
            let mut etag = false;
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
                let lowercase = line.to_ascii_lowercase();
                bearer |= lowercase.starts_with("authorization: bearer test-secret");
                etag |= lowercase.starts_with("if-none-match: \"v4\"");
            }
            let body = b"signed-bundle";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v5\"\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
            (bearer, etag)
        });
        let bootstrap = Bootstrap {
            source: format!("http://127.0.0.1:{port}/policy"),
            pubkey_ed25519: "00".repeat(32),
            bearer_token: Some("test-secret".into()),
            ..Bootstrap::default()
        };

        let outcome = fetch(&bootstrap, Some("\"v4\"")).unwrap();
        assert_eq!(server.join().unwrap(), (true, true));
        assert!(matches!(
            outcome,
            FetchOutcome::Modified { bytes, etag }
                if bytes == b"signed-bundle" && etag.as_deref() == Some("\"v5\"")
        ));
    }

    #[test]
    fn malformed_ca_pin_is_rejected_before_network_work() {
        assert!(pinned_roots("not a certificate").is_err());
    }

    /// Serve one request on a fresh port with `response`, and return the port and the request
    /// head the server read. A write the client cuts short is not an error here.
    fn serve_once(response: Vec<u8>) -> (u16, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let _ = stream.write_all(&response);
            head
        });
        (port, server)
    }

    fn bootstrap_at(port: u16) -> Bootstrap {
        Bootstrap {
            source: format!("http://127.0.0.1:{port}/policy"),
            pubkey_ed25519: "00".repeat(32),
            ..Bootstrap::default()
        }
    }

    #[test]
    fn a_redirect_reads_as_its_status_and_is_not_followed() {
        let (port, server) = serve_once(
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
        );
        let outcome = fetch(&bootstrap_at(port), None);
        server.join().unwrap();
        assert!(matches!(outcome, Err(FetchError::Status(302))));
    }

    #[test]
    fn not_modified_reads_as_not_modified() {
        let (port, server) = serve_once(
            b"HTTP/1.1 304 Not Modified\r\nETag: \"v4\"\r\nConnection: close\r\n\r\n".to_vec(),
        );
        let outcome = fetch(&bootstrap_at(port), Some("\"v4\""));
        assert!(server
            .join()
            .unwrap()
            .to_ascii_lowercase()
            .contains("if-none-match: \"v4\""));
        assert!(matches!(outcome, Ok(FetchOutcome::NotModified)));
    }

    #[test]
    fn a_body_over_eight_mebibytes_is_too_large() {
        let over = usize::try_from(super::MAX_BUNDLE_BYTES).unwrap() + 1;
        let mut response =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {over}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        response.resize(response.len() + over, b"x"[0]);
        let (port, server) = serve_once(response);
        let outcome = fetch(&bootstrap_at(port), None);
        server.join().unwrap();
        assert!(matches!(outcome, Err(FetchError::TooLarge)));
    }
}
