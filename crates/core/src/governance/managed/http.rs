// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! managed:// network fetch (ADR-0055 Phase 3), behind the `managed-fetch` feature.
//!
//! The ENTIRE HTTP/TLS dependency (ureq + rustls) lives here and is reached only through
//! [`super::fetch_bytes`], so the rest of the managed module stays network-agnostic and testable
//! without a server (ADR-0055 Implementation Decision 4). A blocking client is fine: the caller runs
//! the periodic poll on a blocking task. CA pinning is a one-root rustls trust store (trust exactly
//! the org's provisioned CA); with no pin the bundled webpki roots are used. TLS is never the trust
//! anchor -- the bundle signature is -- so a fetch or TLS failure is simply "unreachable", handled by
//! the cache reconcile.

use std::io::Read as _;
use std::sync::Arc;

use super::ManagedBootstrap;

/// A sane ceiling on a policy bundle download (bundles are small; this bounds a hostile response).
const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024;

/// The outcome of a conditional fetch.
pub enum FetchOutcome {
    /// The source returned a body (200), with its ETag if any (for the next conditional request).
    Modified { bytes: Vec<u8>, etag: Option<String> },
    /// The source answered 304 Not Modified: keep enforcing what we already hold.
    NotModified,
}

/// Why a network fetch failed. All map to `FreshError::Unreachable` at the reconcile boundary (TLS is
/// never load-bearing); `PinMismatch` is called out for the Phase 5 guardian door.
#[derive(Debug)]
pub enum FetchError {
    /// Could not connect, the TLS handshake failed, or the request timed out.
    Transport(String),
    /// The presented certificate did not chain to the pinned CA (possible interception).
    PinMismatch(String),
    /// The pinned CA PEM in the bootstrap could not be parsed.
    BadPin(String),
    /// The server answered with a non-success, non-304 status.
    Status(u16),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Transport(m) => write!(f, "transport error: {m}"),
            FetchError::PinMismatch(m) => write!(f, "certificate did not match the pinned CA: {m}"),
            FetchError::BadPin(m) => write!(f, "invalid pinned CA: {m}"),
            FetchError::Status(c) => write!(f, "unexpected HTTP status {c}"),
        }
    }
}

/// Perform one conditional GET of the bootstrap's `source`, honoring the bearer token, the pinned CA,
/// and `if_none_match` (the last ETag). Blocking.
pub fn fetch(b: &ManagedBootstrap, if_none_match: Option<&str>) -> Result<FetchOutcome, FetchError> {
    let agent = build_agent(b.ca_cert_pem.as_deref())?;
    let mut req = agent.get(&b.source);
    if let Some(bearer) = &b.bearer_token {
        req = req.set("Authorization", &format!("Bearer {bearer}"));
    }
    if let Some(etag) = if_none_match {
        req = req.set("If-None-Match", etag);
    }
    match req.call() {
        // ureq returns any status below 400 as Ok, so 304 Not Modified arrives here, not as an error.
        Ok(resp) if resp.status() == 304 => Ok(FetchOutcome::NotModified),
        Ok(resp) => {
            let etag = resp.header("ETag").map(|s| s.to_string());
            let mut bytes = Vec::new();
            resp.into_reader()
                .take(MAX_BUNDLE_BYTES)
                .read_to_end(&mut bytes)
                .map_err(|e| FetchError::Transport(e.to_string()))?;
            Ok(FetchOutcome::Modified { bytes, etag })
        }
        Err(ureq::Error::Status(code, _)) => Err(FetchError::Status(code)),
        Err(ureq::Error::Transport(t)) => {
            let msg = t.to_string();
            let low = msg.to_lowercase();
            if low.contains("certificate") || low.contains("cert ") || low.contains("tls") {
                Err(FetchError::PinMismatch(msg))
            } else {
                Err(FetchError::Transport(msg))
            }
        }
    }
}

/// Build a ureq agent. With no pin, ureq's default (rustls + webpki roots). With a pin, a rustls
/// config whose ONLY trusted root is the org's provisioned CA.
fn build_agent(ca_pem: Option<&str>) -> Result<ureq::Agent, FetchError> {
    match ca_pem {
        None => Ok(ureq::agent()),
        Some(pem) => {
            let config = pinned_client_config(pem)?;
            Ok(ureq::builder().tls_config(Arc::new(config)).build())
        }
    }
}

/// A rustls client config that trusts EXACTLY the certificate(s) in `pem` (the org's pinned CA) and
/// nothing else. Server-auth only (bearer token is the v1 client auth, ADR-0055 D4).
fn pinned_client_config(pem: &str) -> Result<rustls::ClientConfig, FetchError> {
    let mut store = rustls::RootCertStore::empty();
    let mut reader = std::io::BufReader::new(pem.as_bytes());
    let mut added = 0usize;
    for cert in rustls_pemfile::certs(&mut reader) {
        let cert = cert.map_err(|e| FetchError::BadPin(e.to_string()))?;
        store
            .add(cert)
            .map_err(|e| FetchError::BadPin(e.to_string()))?;
        added += 1;
    }
    if added == 0 {
        return Err(FetchError::BadPin("no certificates found in the pinned CA PEM".into()));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| FetchError::BadPin(e.to_string()))?
        .with_root_certificates(store)
        .with_no_client_auth();
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_malformed_pin_is_rejected() {
        assert!(matches!(
            pinned_client_config("not a pem"),
            Err(FetchError::BadPin(_))
        ));
    }

    #[test]
    fn fetch_over_plain_http_reads_body_bearer_and_etag() {
        use std::io::{BufRead as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut saw_bearer = false;
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                if line.to_lowercase().starts_with("authorization: bearer opensesame") {
                    saw_bearer = true;
                }
                if line == "\r\n" {
                    break;
                }
            }
            let body = b"POLICY-BUNDLE-BYTES";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v7\"\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(resp.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
            saw_bearer
        });

        let b = ManagedBootstrap {
            source: format!("http://127.0.0.1:{port}/policy.bundle"),
            bearer_token: Some("opensesame".into()),
            ..Default::default()
        };
        let outcome = fetch(&b, None).expect("fetch ok");
        let saw_bearer = server.join().unwrap();
        assert!(saw_bearer, "the bearer credential was sent");
        match outcome {
            FetchOutcome::Modified { bytes, etag } => {
                assert_eq!(bytes, b"POLICY-BUNDLE-BYTES");
                assert_eq!(etag.as_deref(), Some("\"v7\""));
            }
            FetchOutcome::NotModified => panic!("expected a body"),
        }
    }

    #[test]
    fn a_304_is_not_modified() {
        use std::io::{BufRead as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n")
                .unwrap();
            stream.flush().unwrap();
        });

        let b = ManagedBootstrap {
            source: format!("http://127.0.0.1:{port}/policy.bundle"),
            ..Default::default()
        };
        let outcome = fetch(&b, Some("\"v7\"")).expect("fetch ok");
        server.join().unwrap();
        assert!(matches!(outcome, FetchOutcome::NotModified));
    }
}
