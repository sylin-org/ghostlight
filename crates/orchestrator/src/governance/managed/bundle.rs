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
        sig: BASE64.encode(crypto::signing::ed25519(
            ed25519_seed,
            POLICY_CONTEXT,
            &claims,
        )),
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
    use crate::governance::manifest::organization_bounds::{
        CONTACTS, CONTACT_KIND_BYTES, CONTACT_LABEL_BYTES, CONTACT_VALUE_BYTES, NAME_BYTES,
        STATEMENT_BYTES,
    };
    bounded(presentation.org_name.as_deref(), NAME_BYTES, "org_name")?;
    bounded(
        presentation.rationale.as_deref(),
        STATEMENT_BYTES,
        "rationale",
    )?;
    if presentation.contacts.len() > CONTACTS {
        return Err(BundleError::Presentation(format!(
            "contacts has more than {CONTACTS} entries"
        )));
    }
    for contact in &presentation.contacts {
        bounded(Some(&contact.kind), CONTACT_KIND_BYTES, "contact kind")?;
        bounded(Some(&contact.value), CONTACT_VALUE_BYTES, "contact value")?;
        bounded(
            contact.label.as_deref(),
            CONTACT_LABEL_BYTES,
            "contact label",
        )?;
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

    use super::{sign, verify, BundleError, Envelope, POLICY_CONTEXT};
    use crate::governance::managed::crypto::{signing, verification_key};

    /// The sibling signing domain ADR-0055 reserves beside policy.
    const LICENSE_CONTEXT: &[u8] = b"ghostlight/license";

    /// An envelope carrying `claims` under the Ed25519 signature `sig`.
    fn envelope(claims: &[u8], sig: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&Envelope {
            v: 1,
            claims: BASE64.encode(claims),
            sig: BASE64.encode(sig),
            sig_mldsa: None,
        })
        .unwrap()
    }

    /// docs/trust/security-overview.md, the questionnaire's CEK answers, ADR-0055 Decisions 3 and
    /// 9(a) and 6c138338: a bundle verifies as policy only when the bootstrap's key signed these
    /// exact claims in the ghostlight/policy domain and the claims are policy. Tampered claims,
    /// another key, another domain and a context/message split that concatenates to the same
    /// bytes are refused as a bad signature. A validly signed claim of any other kind is refused
    /// as that kind. Claims signed before the kind field existed are policy.
    #[test]
    fn a_bundle_is_policy_only_when_the_bootstraps_key_signed_these_exact_policy_claims() {
        const SEED: [u8; 32] = [24; 32];
        const OTHER_SEED: [u8; 32] = [25; 32];
        const SEQUENCE: u64 = 8;
        const OTHER_KIND: &str = "break_glass";
        let key = verification_key(&signing::ed25519_public(&SEED), None).unwrap();
        let manifest = json!({"schema": 4, "name": "org", "version": "1", "grants": []});
        let claims = |seq: u64| {
            serde_json::to_vec(&json!({"kind": "policy", "seq": seq, "manifest": manifest}))
                .unwrap()
        };
        let signed = claims(SEQUENCE);
        let sig = signing::ed25519(&SEED, POLICY_CONTEXT, &signed);

        assert_eq!(
            verify(&sign(&SEED, None, SEQUENCE, manifest.clone(), None), &key)
                .map(|bundle| bundle.sequence),
            Ok(SEQUENCE)
        );
        assert_eq!(
            verify(&envelope(&signed, &sig), &key).map(|bundle| bundle.sequence),
            Ok(SEQUENCE),
            "the control envelope verifies"
        );

        let (split_context, split_prefix) = POLICY_CONTEXT.split_at(b"ghostlight".len());
        let split_message = [split_prefix, signed.as_slice()].concat();
        for (row, bytes) in [
            ("tampered claims", envelope(&claims(SEQUENCE + 1), &sig)),
            (
                "another key",
                envelope(
                    &signed,
                    &signing::ed25519(&OTHER_SEED, POLICY_CONTEXT, &signed),
                ),
            ),
            (
                "another domain",
                envelope(&signed, &signing::ed25519(&SEED, LICENSE_CONTEXT, &signed)),
            ),
            (
                "the same bytes split differently",
                envelope(
                    &signed,
                    &signing::ed25519(&SEED, split_context, &split_message),
                ),
            ),
        ] {
            assert_eq!(
                verify(&bytes, &key).map(|bundle| bundle.sequence),
                Err(BundleError::Signature),
                "{row}"
            );
        }

        let other_kind =
            serde_json::to_vec(&json!({"kind": OTHER_KIND, "seq": SEQUENCE, "manifest": manifest}))
                .unwrap();
        assert_eq!(
            verify(
                &envelope(
                    &other_kind,
                    &signing::ed25519(&SEED, POLICY_CONTEXT, &other_kind)
                ),
                &key
            )
            .map(|bundle| bundle.sequence),
            Err(BundleError::Kind(OTHER_KIND.into()))
        );
        let kindless = serde_json::to_vec(&json!({"seq": SEQUENCE, "manifest": manifest})).unwrap();
        assert_eq!(
            verify(
                &envelope(
                    &kindless,
                    &signing::ed25519(&SEED, POLICY_CONTEXT, &kindless)
                ),
                &key
            )
            .map(|bundle| bundle.sequence),
            Ok(SEQUENCE),
            "claims signed before the kind field existed are policy"
        );
    }
}
