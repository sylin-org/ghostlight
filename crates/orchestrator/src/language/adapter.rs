//! What Ghostlight tells a browser extension it cannot serve.
//!
//! The extension shows these words as they arrive and never writes its own about the service, so
//! the sentence stays with the rest of Ghostlight's language.

/// The message of the `adapter_incompatible` error frame: the extension speaks an adapter
/// protocol this service does not serve.
pub const ADAPTER_INCOMPATIBLE: &str = "This Ghostlight extension and the Ghostlight app on this computer do not match. Update both to the latest version.";

#[cfg(test)]
mod tests {
    use super::ADAPTER_INCOMPATIBLE;

    #[test]
    fn the_incompatible_sentence_is_plain_ascii() {
        assert!(ADAPTER_INCOMPATIBLE.is_ascii());
        assert!(ADAPTER_INCOMPATIBLE.ends_with('.'));
    }
}
