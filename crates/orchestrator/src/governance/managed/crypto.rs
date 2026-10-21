//! Customer-owned composite signatures for managed policy bundles.

use ed25519_dalek::{Signature as EdSignature, VerifyingKey};
use fips204::ml_dsa_65;
use fips204::traits::{SerDes as _, Verifier as _};

pub(super) const ED_SIGNATURE_BYTES: usize = 64;
pub(super) const MLDSA_PUBLIC_BYTES: usize = 1952;
pub(super) const MLDSA_SIGNATURE_BYTES: usize = 3309;

pub(super) enum VerificationKey {
    Ed25519(VerifyingKey),
    Composite {
        ed25519: VerifyingKey,
        mldsa: Box<ml_dsa_65::PublicKey>,
    },
}

pub(super) fn verification_key(
    ed25519: &[u8; 32],
    mldsa: Option<&[u8; MLDSA_PUBLIC_BYTES]>,
) -> Option<VerificationKey> {
    let ed25519 = VerifyingKey::from_bytes(ed25519).ok()?;
    match mldsa {
        Some(bytes) => Some(VerificationKey::Composite {
            ed25519,
            mldsa: Box::new(ml_dsa_65::PublicKey::try_from_bytes(*bytes).ok()?),
        }),
        None => Some(VerificationKey::Ed25519(ed25519)),
    }
}

/// Pure Ed25519 (RFC 8032) has no context argument of its own, unlike the ML-DSA leg's `context`
/// parameter. Binding `context` into the signed bytes ourselves, with an explicit length prefix
/// so `(context, message)` cannot be reinterpreted as a different split, gives both legs the same
/// domain separation: a signature made for one context is not a valid signature under another.
fn bind_context(context: &[u8], message: &[u8]) -> Vec<u8> {
    let mut bound = Vec::with_capacity(8 + context.len() + message.len());
    bound.extend_from_slice(&(context.len() as u64).to_le_bytes());
    bound.extend_from_slice(context);
    bound.extend_from_slice(message);
    bound
}

pub(super) fn verify(
    key: &VerificationKey,
    context: &[u8],
    message: &[u8],
    ed25519_signature: &[u8],
    mldsa_signature: Option<&[u8]>,
) -> bool {
    let ed25519_valid = |key: &VerifyingKey| {
        let Ok(bytes) = <[u8; ED_SIGNATURE_BYTES]>::try_from(ed25519_signature) else {
            return false;
        };
        key.verify_strict(
            &bind_context(context, message),
            &EdSignature::from_bytes(&bytes),
        )
        .is_ok()
    };
    match key {
        VerificationKey::Ed25519(key) => mldsa_signature.is_none() && ed25519_valid(key),
        VerificationKey::Composite { ed25519, mldsa } => {
            let Some(signature) = mldsa_signature else {
                return false;
            };
            let Ok(signature) = <[u8; MLDSA_SIGNATURE_BYTES]>::try_from(signature) else {
                return false;
            };
            ed25519_valid(ed25519) && mldsa.verify(message, &signature, context)
        }
    }
}

pub(super) mod signing {
    use ed25519_dalek::{Signer as _, SigningKey};
    use fips204::ml_dsa_65;
    use fips204::traits::{KeyGen as _, SerDes as _, Signer as _};

    use super::{bind_context, ED_SIGNATURE_BYTES, MLDSA_PUBLIC_BYTES, MLDSA_SIGNATURE_BYTES};

    pub(in crate::governance::managed) fn ed25519(
        seed: &[u8; 32],
        context: &[u8],
        message: &[u8],
    ) -> [u8; ED_SIGNATURE_BYTES] {
        SigningKey::from_bytes(seed)
            .sign(&bind_context(context, message))
            .to_bytes()
    }

    pub(in crate::governance::managed) fn ed25519_public(seed: &[u8; 32]) -> [u8; 32] {
        SigningKey::from_bytes(seed).verifying_key().to_bytes()
    }

    pub(in crate::governance::managed) fn mldsa(
        seed: &[u8; 32],
        context: &[u8],
        message: &[u8],
    ) -> [u8; MLDSA_SIGNATURE_BYTES] {
        let (_, secret) = ml_dsa_65::KG::keygen_from_seed(seed);
        secret
            .try_sign_with_seed(&[0_u8; 32], message, context)
            .expect("bounded in-memory ML-DSA signing succeeds")
    }

    pub(in crate::governance::managed) fn mldsa_public(
        seed: &[u8; 32],
    ) -> [u8; MLDSA_PUBLIC_BYTES] {
        let (public, _) = ml_dsa_65::KG::keygen_from_seed(seed);
        public.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::{signing, verification_key, verify, MLDSA_SIGNATURE_BYTES};

    /// ACCEPTANCE Governance 18 and docs/trust/security-overview.md: Ed25519 is required, and
    /// with an ML-DSA-65 key configured both legs must verify. With Ed25519 alone, a second leg is
    /// refused rather than ignored.
    #[test]
    fn a_configured_ml_dsa_key_makes_both_legs_mandatory() {
        const ED_SEED: [u8; 32] = [7; 32];
        const OTHER_ED_SEED: [u8; 32] = [8; 32];
        const MLDSA_SEED: [u8; 32] = [9; 32];
        const CONTEXT: &[u8] = b"ghostlight/policy";
        const MESSAGE: &[u8] = b"signed policy claims";
        let composite = verification_key(
            &signing::ed25519_public(&ED_SEED),
            Some(&signing::mldsa_public(&MLDSA_SEED)),
        )
        .unwrap();
        let ed25519_only = verification_key(&signing::ed25519_public(&ED_SEED), None).unwrap();
        let ed = signing::ed25519(&ED_SEED, CONTEXT, MESSAGE);
        let other_ed = signing::ed25519(&OTHER_ED_SEED, CONTEXT, MESSAGE);
        let mldsa = signing::mldsa(&MLDSA_SEED, CONTEXT, MESSAGE);
        let zeros = [0_u8; MLDSA_SIGNATURE_BYTES];

        for (row, key, ed, mldsa, verifies) in [
            (
                "composite, both legs",
                &composite,
                &ed,
                Some(&mldsa[..]),
                true,
            ),
            ("composite, no second leg", &composite, &ed, None, false),
            (
                "composite, a forged second leg",
                &composite,
                &ed,
                Some(&zeros[..]),
                false,
            ),
            (
                "composite, another Ed25519 key",
                &composite,
                &other_ed,
                Some(&mldsa[..]),
                false,
            ),
            ("Ed25519 alone", &ed25519_only, &ed, None, true),
            (
                "Ed25519 alone, a second leg",
                &ed25519_only,
                &ed,
                Some(&mldsa[..]),
                false,
            ),
        ] {
            assert_eq!(verify(key, CONTEXT, MESSAGE, ed, mldsa), verifies, "{row}");
        }
    }
}
