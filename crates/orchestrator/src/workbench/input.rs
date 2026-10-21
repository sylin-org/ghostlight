//! Bounds on what a surface may send the workbench, checked before anything parses it.
//!
//! Every request the loopback server reads is checked with these, so every screen says the same
//! thing about the same bad input.

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

    /// The page, whose search field must stop at the length this adapter accepts.
    const PAGE: &str = include_str!("../../ui/index.html");

    /// The page's search field, by its id.
    const SEARCH_FIELD: &str = "id=\"palette-query\"";

    /// The attribute that bounds the field's length.
    const MAX_LENGTH: &str = "maxlength=\"";

    /// The `maxlength` the page gives its search field.
    fn page_search_limit() -> usize {
        let field = &PAGE[PAGE
            .find(SEARCH_FIELD)
            .expect("the page has a search field")..];
        let field = &field[..field.find('>').expect("the field's tag ends")];
        let value =
            &field[field.find(MAX_LENGTH).expect("the field is bounded") + MAX_LENGTH.len()..];
        value[..value.find('"').expect("the value ends")]
            .parse()
            .expect("the bound is a number")
    }

    /// Protects desktop gate 3 ("Search input is bounded at the adapter"): a search longer than
    /// `SEARCH_QUERY_LIMIT` characters, or holding a control character, is refused here, the limit
    /// counts characters as the page does, not bytes, and the page's search field stops at the same
    /// length (a limit on both sides, the P-D4 pattern).
    #[test]
    fn search_input_is_bounded_at_the_adapter() {
        assert!(validate_search_query("blocked browser").is_ok());
        assert!(validate_search_query(&"x".repeat(SEARCH_QUERY_LIMIT)).is_ok());
        assert!(validate_search_query(&"x".repeat(SEARCH_QUERY_LIMIT + 1)).is_err());
        assert!(validate_search_query(&"\u{e9}".repeat(SEARCH_QUERY_LIMIT)).is_ok());
        assert!(validate_search_query("blocked\u{0007}").is_err());
        assert_eq!(page_search_limit(), SEARCH_QUERY_LIMIT);
    }
}
