//! Transport-independent signed managed policy bundle format.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::crypto::{self, VerificationKey};

const POLICY_CONTEXT: &[u8] = b"ghostlight/policy";
const ARMOR_BEGIN: &str = "-----BEGIN GHOSTLIGHT POLICY-----";
const ARMOR_END: &str = "-----END GHOSTLIGHT POLICY-----";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct Presentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) org_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) contacts: Vec<Contact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct Contact {
    pub(super) kind: String,
    pub(super) value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) label: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    #[serde(default = "policy_kind")]
    kind: String,
    seq: u64,
    manifest: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    presentation: Option<Presentation>,
}

fn policy_kind() -> String {
    "policy".into()
}

#[derive(Debug, Deserialize, Serialize)]
struct Envelope {
    v: u32,
    claims: String,
    sig: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sig_mldsa: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct VerifiedBundle {
    pub(super) sequence: u64,
    pub(super) manifest: Value,
    pub(super) presentation: Option<Presentation>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(super) enum BundleError {
    #[error("invalid policy bundle envelope: {0}")]
    Envelope(String),
    #[error("unsupported policy bundle version {0}")]
    Version(u32),
    #[error("policy bundle field '{0}' is not valid base64")]
    Base64(&'static str),
    #[error("policy bundle signature has the wrong length")]
    SignatureLength,
    #[error("policy bundle signature verification failed")]
    Signature,
    #[error("invalid policy bundle claims: {0}")]
    Claims(String),
    #[error("unsupported policy bundle kind '{0}'")]
    Kind(String),
    #[error("invalid policy presentation: {0}")]
    Presentation(String),
    #[error("malformed armored policy bundle")]
    Armor,
}

pub(super) fn verify(bytes: &[u8], key: &VerificationKey) -> Result<VerifiedBundle, BundleError> {
    let envelope_bytes = dearmor_if_needed(bytes)?;
    let envelope: Envelope = serde_json::from_slice(&envelope_bytes)
        .map_err(|error| BundleError::Envelope(error.to_string()))?;
    if envelope.v != 1 {
        return Err(BundleError::Version(envelope.v));
    }
    let claims_bytes = BASE64
        .decode(envelope.claims)
        .map_err(|_| BundleError::Base64("claims"))?;
    let ed25519 = BASE64
        .decode(envelope.sig)
        .map_err(|_| BundleError::Base64("sig"))?;
    if ed25519.len() != crypto::ED_SIGNATURE_BYTES {
        return Err(BundleError::SignatureLength);
    }
    let mldsa = envelope
        .sig_mldsa
        .map(|value| {
            BASE64
                .decode(value)
                .map_err(|_| BundleError::Base64("sig_mldsa"))
        })
        .transpose()?;
    if mldsa
        .as_ref()
        .is_some_and(|value| value.len() != crypto::MLDSA_SIGNATURE_BYTES)
    {
        return Err(BundleError::SignatureLength);
    }
    if !crypto::verify(
        key,
        POLICY_CONTEXT,
        &claims_bytes,
        &ed25519,
        mldsa.as_deref(),
    ) {
        return Err(BundleError::Signature);
    }
    let claims: Claims = serde_json::from_slice(&claims_bytes)
        .map_err(|error| BundleError::Claims(error.to_string()))?;
    if claims.kind != "policy" {
        return Err(BundleError::Kind(claims.kind));
    }
    validate_presentation(claims.presentation.as_ref())?;
    Ok(VerifiedBundle {
        sequence: claims.seq,
        manifest: claims.manifest,
        presentation: claims.presentation,
    })
}

pub(super) fn sign(
    ed25519_seed: &[u8; 32],
    mldsa_seed: Option<&[u8; 32]>,
    sequence: u64,
    manifest: Value,
    presentation: Option<Presentation>,
) -> Vec<u8> {
    let claims = serde_json::to_vec(&Claims {
        kind: policy_kind(),
        seq: sequence,
        manifest,
        presentation,
    })
    .expect("policy claims serialize");
    let envelope = Envelope {
        v: 1,
        claims: BASE64.encode(&claims),
        sig: BASE64.encode(crypto::signing::ed25519(ed25519_seed, &claims)),
        sig_mldsa: mldsa_seed
            .map(|seed| BASE64.encode(crypto::signing::mldsa(seed, POLICY_CONTEXT, &claims))),
    };
    serde_json::to_vec_pretty(&envelope).expect("policy envelope serializes")
}

pub(super) fn armor(bytes: &[u8]) -> String {
    let encoded = BASE64.encode(bytes);
    let mut output = String::from(ARMOR_BEGIN);
    output.push('\n');
    for chunk in encoded.as_bytes().chunks(64) {
        output.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        output.push('\n');
    }
    output.push_str(ARMOR_END);
    output.push('\n');
    output
}

fn dearmor_if_needed(bytes: &[u8]) -> Result<Vec<u8>, BundleError> {
    let text = String::from_utf8_lossy(bytes);
    if !text.contains(ARMOR_BEGIN) {
        return Ok(bytes.to_vec());
    }
    let body = text
        .split_once(ARMOR_BEGIN)
        .and_then(|(_, rest)| rest.split_once(ARMOR_END).map(|(body, _)| body))
        .ok_or(BundleError::Armor)?;
    let compact: String = body
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    BASE64.decode(compact).map_err(|_| BundleError::Armor)
}

pub(super) fn validate_presentation(
    presentation: Option<&Presentation>,
) -> Result<(), BundleError> {
    let Some(presentation) = presentation else {
        return Ok(());
    };
    bounded(presentation.org_name.as_deref(), 100, "org_name")?;
    bounded(presentation.rationale.as_deref(), 500, "rationale")?;
    if presentation.contacts.len() > 8 {
        return Err(BundleError::Presentation(
            "contacts has more than 8 entries".into(),
        ));
    }
    for contact in &presentation.contacts {
        bounded(Some(&contact.kind), 32, "contact kind")?;
        bounded(Some(&contact.value), 240, "contact value")?;
        bounded(contact.label.as_deref(), 80, "contact label")?;
    }
    Ok(())
}

fn bounded(value: Option<&str>, max: usize, field: &str) -> Result<(), BundleError> {
    if value.is_some_and(|value| value.trim().is_empty() || value.len() > max) {
        return Err(BundleError::Presentation(format!(
            "{field} must be non-empty and at most {max} bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;
    use serde_json::json;

    use super::{armor, sign, verify, Envelope};
    use crate::governance::managed::crypto::{self, verification_key};

    fn manifest() -> serde_json::Value {
        serde_json::json!({"schema":3,"name":"org","version":"1","grants":[]})
    }

    #[test]
    fn ed25519_and_composite_bundles_round_trip() {
        let ed_seed = [21_u8; 32];
        let mldsa_seed = [22_u8; 32];
        let ed_key = verification_key(&crypto::signing::ed25519_public(&ed_seed), None).unwrap();
        let composite_key = verification_key(
            &crypto::signing::ed25519_public(&ed_seed),
            Some(&crypto::signing::mldsa_public(&mldsa_seed)),
        )
        .unwrap();

        assert_eq!(
            verify(&sign(&ed_seed, None, 4, manifest(), None), &ed_key)
                .unwrap()
                .sequence,
            4
        );
        assert_eq!(
            verify(
                &sign(&ed_seed, Some(&mldsa_seed), 5, manifest(), None),
                &composite_key
            )
            .unwrap()
            .sequence,
            5
        );
        assert!(verify(&sign(&ed_seed, None, 6, manifest(), None), &composite_key).is_err());
    }

    #[test]
    fn armor_preserves_the_exact_signed_envelope() {
        let seed = [23_u8; 32];
        let bytes = sign(&seed, None, 7, manifest(), None);
        let key = verification_key(&crypto::signing::ed25519_public(&seed), None).unwrap();
        assert_eq!(verify(armor(&bytes).as_bytes(), &key).unwrap().sequence, 7);
    }

    #[test]
    fn tampering_is_rejected() {
        let seed = [24_u8; 32];
        let mut bytes = sign(&seed, None, 8, manifest(), None);
        let key = verification_key(&crypto::signing::ed25519_public(&seed), None).unwrap();
        let index = bytes.len() / 2;
        bytes[index] ^= 1;
        assert!(verify(&bytes, &key).is_err());
    }

    #[test]
    fn version_08_claims_without_a_kind_remain_compatible() {
        let seed = [25_u8; 32];
        let claims = serde_json::to_vec(&json!({
            "seq": 9,
            "manifest": manifest()
        }))
        .unwrap();
        let envelope = Envelope {
            v: 1,
            claims: BASE64.encode(&claims),
            sig: BASE64.encode(crypto::signing::ed25519(&seed, &claims)),
            sig_mldsa: None,
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let key = verification_key(&crypto::signing::ed25519_public(&seed), None).unwrap();
        assert_eq!(verify(&bytes, &key).unwrap().sequence, 9);
    }

    #[test]
    fn a_valid_signature_cannot_turn_an_unknown_bundle_kind_into_policy() {
        let seed = [26_u8; 32];
        let claims = serde_json::to_vec(&json!({
            "kind": "break_glass",
            "seq": 10,
            "manifest": manifest()
        }))
        .unwrap();
        let envelope = Envelope {
            v: 1,
            claims: BASE64.encode(&claims),
            sig: BASE64.encode(crypto::signing::ed25519(&seed, &claims)),
            sig_mldsa: None,
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let key = verification_key(&crypto::signing::ed25519_public(&seed), None).unwrap();
        assert!(verify(&bytes, &key).is_err());
    }
}
