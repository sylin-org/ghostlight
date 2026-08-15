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
    use super::{signing, verification_key, verify, VerificationKey, MLDSA_SIGNATURE_BYTES};

    #[test]
    fn composite_verification_requires_both_signature_legs() {
        let ed_seed = [7_u8; 32];
        let mldsa_seed = [9_u8; 32];
        let message = b"signed policy claims";
        let context = b"ghostlight/policy";
        let key = verification_key(
            &signing::ed25519_public(&ed_seed),
            Some(&signing::mldsa_public(&mldsa_seed)),
        )
        .unwrap();
        let ed = signing::ed25519(&ed_seed, context, message);
        let mldsa = signing::mldsa(&mldsa_seed, context, message);

        assert!(verify(&key, context, message, &ed, Some(&mldsa)));
        assert!(!verify(&key, context, message, &ed, None));
        assert!(!verify(
            &key,
            context,
            message,
            &ed,
            Some(&[0_u8; MLDSA_SIGNATURE_BYTES])
        ));
        assert!(matches!(key, VerificationKey::Composite { .. }));
    }

    #[test]
    fn ed25519_only_rejects_an_unexpected_second_leg() {
        let seed = [3_u8; 32];
        let message = b"signed policy claims";
        let key = verification_key(&signing::ed25519_public(&seed), None).unwrap();
        let signature = signing::ed25519(&seed, b"ghostlight/policy", message);
        assert!(verify(
            &key,
            b"ghostlight/policy",
            message,
            &signature,
            None
        ));
        assert!(!verify(
            &key,
            b"ghostlight/policy",
            message,
            &signature,
            Some(&[0_u8; MLDSA_SIGNATURE_BYTES])
        ));
    }

    #[test]
    fn ed25519_leg_is_bound_to_its_context_not_just_the_message() {
        // Before this bound the context into what gets signed, a signature made for any context
        // verified successfully under every other context too, as long as the message bytes
        // matched -- silently discarding the one thing `context` exists to guarantee.
        let seed = [11_u8; 32];
        let message = b"signed policy claims";
        let key = verification_key(&signing::ed25519_public(&seed), None).unwrap();
        let signature = signing::ed25519(&seed, b"ghostlight/policy", message);

        assert!(verify(
            &key,
            b"ghostlight/policy",
            message,
            &signature,
            None
        ));
        assert!(!verify(
            &key,
            b"some/other/context",
            message,
            &signature,
            None
        ));
        assert!(!verify(&key, b"", message, &signature, None));

        // The length prefix also rules out `(context, message)` being reinterpreted as a
        // differently-split pair that happens to concatenate to the same bytes.
        let split_signature =
            signing::ed25519(&seed, b"ghostlight", b"/policysigned policy claims");
        assert!(!verify(
            &key,
            b"ghostlight/policy",
            message,
            &split_signature,
            None
        ));
    }
}
