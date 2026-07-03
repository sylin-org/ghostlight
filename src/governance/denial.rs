//! The stable denial id scheme (shared format doc section 7.1, ADR-0020 commitment 6).
//!
//! Introduced with the sacred-domains rule (g08) and reused verbatim by the manifest engine's
//! grant-denial rules (g13): every denial a policy decision produces is traceable to the exact
//! rule and policy version that produced it, without leaking the rule's content (the id is a
//! one-way hash, never a reversible encoding). Pure: no I/O, no policy decisions -- this module
//! only computes the id string.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// The stable denial id for a resolved rule: `"D-"` followed by the first 8 lowercase hex
/// characters of `SHA256(manifest_hash + "\n" + grant_id + "\n" + rule)`, all UTF-8, exactly
/// one LF between components (shared format doc section 7.1). `manifest_hash` is the empty
/// string when no manifest is active; `grant_id` is the empty string when no grant matched
/// (the sacred-domains rule, which has no grant, always passes `""`).
pub fn denial_id(manifest_hash: &str, grant_id: &str, rule: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(manifest_hash.as_bytes());
    hasher.update(b"\n");
    hasher.update(grant_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(rule.as_bytes());
    let digest = hasher.finalize();

    let mut hex = String::with_capacity(10);
    hex.push_str("D-");
    for byte in &digest[..4] {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denial_id_is_stable_and_pinned() {
        assert_eq!(denial_id("", "", "sacred/mybank.com"), "D-171052e3");
        assert_eq!(denial_id("", "", "sacred/*.mybank.com"), "D-af6633ec");
    }

    #[test]
    fn same_inputs_always_produce_the_same_id() {
        let a = denial_id("h1", "g1", "sacred/example.com");
        let b = denial_id("h1", "g1", "sacred/example.com");
        assert_eq!(a, b);
    }

    #[test]
    fn changing_any_one_component_changes_the_id() {
        let base = denial_id("h1", "g1", "sacred/example.com");
        assert_ne!(base, denial_id("h2", "g1", "sacred/example.com"));
        assert_ne!(base, denial_id("h1", "g2", "sacred/example.com"));
        assert_ne!(base, denial_id("h1", "g1", "sacred/other.com"));
    }

    #[test]
    fn format_is_d_dash_eight_lowercase_hex() {
        let id = denial_id("h1", "g1", "sacred/example.com");
        assert!(id.starts_with("D-"));
        let hex = &id[2..];
        assert_eq!(hex.len(), 8);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
