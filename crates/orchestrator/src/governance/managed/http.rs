//! Bounded conditional HTTPS transport for organization-hosted policy bundles.

use std::io::Read as _;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{pem::PemObject as _, CertificateDer};
use thiserror::Error;

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
    let mut request = agent.get(&bootstrap.source);
    if let Some(token) = &bootstrap.bearer_token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    if let Some(etag) = etag {
        request = request.set("If-None-Match", etag);
    }
    match request.call() {
        Ok(response) if response.status() == 304 => Ok(FetchOutcome::NotModified),
        Ok(response) if response.status() == 200 => {
            let etag = response.header("ETag").map(str::to_owned);
            let mut bytes = Vec::new();
            response
                .into_reader()
                .take(MAX_BUNDLE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| FetchError::Transport(error.to_string()))?;
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BUNDLE_BYTES {
                return Err(FetchError::TooLarge);
            }
            Ok(FetchOutcome::Modified { bytes, etag })
        }
        Ok(response) => Err(FetchError::Status(response.status())),
        Err(ureq::Error::Status(code, _)) => Err(FetchError::Status(code)),
        Err(ureq::Error::Transport(error)) => Err(FetchError::Transport(error.to_string())),
    }
}

fn agent(ca_cert_pem: Option<&str>) -> Result<ureq::Agent, FetchError> {
    let mut builder = ureq::builder()
        .redirects(0)
        .timeout_connect(REQUEST_TIMEOUT)
        .timeout_read(REQUEST_TIMEOUT)
        .timeout_write(REQUEST_TIMEOUT);
    if let Some(pem) = ca_cert_pem {
        builder = builder.tls_config(Arc::new(pinned_tls(pem)?));
    }
    Ok(builder.build())
}

fn pinned_tls(pem: &str) -> Result<rustls::ClientConfig, FetchError> {
    let mut roots = rustls::RootCertStore::empty();
    let mut count = 0_usize;
    for certificate in CertificateDer::pem_slice_iter(pem.as_bytes()) {
        roots
            .add(certificate.map_err(|error| FetchError::Pin(error.to_string()))?)
            .map_err(|error| FetchError::Pin(error.to_string()))?;
        count += 1;
    }
    if count == 0 {
        return Err(FetchError::Pin("no certificate was found".into()));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| FetchError::Pin(error.to_string()))
        .map(|builder| builder.with_root_certificates(roots).with_no_client_auth())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, Write as _};
    use std::net::TcpListener;

    use super::{fetch, pinned_tls, FetchOutcome};
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
        assert!(pinned_tls("not a certificate").is_err());
    }
}
