//! Bounds on what a surface may send the workbench, checked before anything parses it.
//!
//! The Tauri adapter and the loopback server share these, so every screen says the same thing
//! about the same bad input.

use crate::language::workbench as words;

/// The most characters a search may hold.
pub const SEARCH_QUERY_LIMIT: usize = 120;

/// Upper bound on a policy document arriving from the surface. A policy is bounded at 256
/// grants of bounded patterns; this leaves generous room and still refuses anything absurd.
pub const POLICY_DOCUMENT_LIMIT: usize = 256 * 1024;

/// Bound a search at the adapter boundary.
pub fn validate_search_query(query: &str) -> Result<(), String> {
    if query.chars().count() > SEARCH_QUERY_LIMIT {
        return Err(words::search_too_long(SEARCH_QUERY_LIMIT));
    }
    if query
        .chars()
        .any(|character| character.is_control() && !character.is_whitespace())
    {
        return Err(words::SEARCH_CONTROL_CHARACTER.into());
    }
    Ok(())
}

/// Bound a policy document at the adapter boundary before the parser ever sees it.
pub fn validate_policy_document(document: &str) -> Result<(), String> {
    if document.trim().is_empty() {
        return Err(words::POLICY_EMPTY.into());
    }
    if document.len() > POLICY_DOCUMENT_LIMIT {
        return Err(words::policy_too_large(POLICY_DOCUMENT_LIMIT));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_input_is_bounded_at_the_adapter() {
        assert!(validate_search_query("blocked browser").is_ok());
        assert!(validate_search_query(&"x".repeat(121)).is_err());
        assert!(validate_search_query("blocked\u{0007}").is_err());
    }

    #[test]
    fn policy_document_is_bounded_at_the_adapter() {
        // The facade tests call the facade directly and never cross this boundary, where the
        // untrusted bytes arrive. Only this test fails when the check is weakened or dropped.
        assert!(
            validate_policy_document(r#"{"schema":4,"name":"x","version":"1","grants":[]}"#)
                .is_ok()
        );
        assert!(validate_policy_document("").is_err());
        assert!(validate_policy_document("   \n\t  ").is_err());
        assert!(validate_policy_document(&"x".repeat(POLICY_DOCUMENT_LIMIT)).is_ok());
        assert!(validate_policy_document(&"x".repeat(POLICY_DOCUMENT_LIMIT + 1)).is_err());
    }
}
