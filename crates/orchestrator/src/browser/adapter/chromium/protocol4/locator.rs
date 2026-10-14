//! Locators that name a document as well as an element.
//!
//! A page asset mints locators that mean something only inside its own document and world. The
//! service never shows the model a bare local locator. It binds each one to the document that
//! minted it, as `{frame_id}:{loader_id}:{local}`, so a locator survives only as long as its
//! document does: a replaced document has a new loader, and its old locators name nothing.

use super::wire::FrameRef;
use crate::browser::adapter::ObservedTarget;
use crate::browser::BrowserError;

/// The separator between the parts of an encoded locator.
const SEPARATOR: char = ':';
/// The prefix every local locator carries.
const LOCAL_PREFIX: &str = "locator_";
/// The separator between a local locator's world epoch and its counter.
const LOCAL_SEPARATOR: char = '_';
/// The failure for a locator that names no live element.
const STALE_LOCATOR: &str = "stale browser locator";

/// An element locator bound to the document that minted it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Locator {
    /// The document the element lives in.
    pub doc: FrameRef,
    /// The locator the document's own asset understands.
    pub local: String,
}

impl Locator {
    /// Bind a document-local locator to its document.
    #[must_use]
    pub fn new(doc: FrameRef, local: impl Into<String>) -> Self {
        Self {
            doc,
            local: local.into(),
        }
    }

    /// The form the model carries: `{frame_id}:{loader_id}:{local}`.
    #[must_use]
    pub fn encode(&self) -> String {
        format!(
            "{}{SEPARATOR}{}{SEPARATOR}{}",
            self.doc.frame_id, self.doc.loader_id, self.local
        )
    }

    /// Read an encoded locator.
    ///
    /// # Errors
    ///
    /// Returns a plain primitive failure for a locator that is malformed or was not minted by a
    /// protocol 4 asset: nothing was sent, so nothing happened.
    pub fn decode(text: &str) -> Result<Self, BrowserError> {
        let mut parts = text.splitn(3, SEPARATOR);
        match (parts.next(), parts.next(), parts.next()) {
            (Some(frame_id), Some(loader_id), Some(local))
                if is_browser_id(frame_id) && is_browser_id(loader_id) && is_local(local) =>
            {
                Ok(Self::new(
                    FrameRef {
                        frame_id: frame_id.to_owned(),
                        loader_id: loader_id.to_owned(),
                    },
                    local,
                ))
            }
            _ => Err(BrowserError::Primitive(STALE_LOCATOR.to_owned())),
        }
    }
}

/// Bind every target a document observed to that document.
#[must_use]
pub fn rescope(doc: &FrameRef, targets: Vec<ObservedTarget>) -> Vec<ObservedTarget> {
    targets
        .into_iter()
        .map(|target| ObservedTarget {
            locator: Locator::new(doc.clone(), target.locator).encode(),
            ..target
        })
        .collect()
}

/// Whether text is a frame or loader identity: letters, digits and hyphens.
pub(super) fn is_browser_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// Whether text is `locator_{epoch}_{counter}`, the form a protocol 4 asset mints.
fn is_local(text: &str) -> bool {
    text.strip_prefix(LOCAL_PREFIX)
        .and_then(|rest| rest.split_once(LOCAL_SEPARATOR))
        .is_some_and(|(epoch, counter)| {
            !epoch.is_empty()
                && epoch.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !counter.is_empty()
                && counter.bytes().all(|byte| byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(frame: &str, loader: &str) -> FrameRef {
        FrameRef {
            frame_id: frame.into(),
            loader_id: loader.into(),
        }
    }

    #[test]
    fn a_locator_round_trips_through_its_encoded_form() {
        let locator = Locator::new(doc("7F3A", "9C21"), "locator_a1b2c3d4_12");
        let encoded = locator.encode();
        assert_eq!(encoded, "7F3A:9C21:locator_a1b2c3d4_12");
        assert_eq!(Locator::decode(&encoded), Ok(locator));
    }

    #[test]
    fn a_malformed_or_foreign_locator_is_a_stale_locator() {
        for text in [
            "",
            "locator_a1b2c3d4_1",
            "7F3A:locator_a1b2c3d4_1",
            ":9C21:locator_a1b2c3d4_1",
            "7F3A::locator_a1b2c3d4_1",
            "7F3A:9C21:",
            "7F3A:9C21:element_1",
            "7F3A:9C21:locator_1",
            "7F3A:9C21:locator__1",
            "7F3A:9C21:locator_a1b2c3d4_",
            "7F3A:9C21:locator_xyz_1",
            "7F3A:9C21:locator_a1b2c3d4_1:extra",
            "0:doc_6B1F:locator_3",
            "7F 3A:9C21:locator_a1b2c3d4_1",
        ] {
            assert_eq!(
                Locator::decode(text),
                Err(BrowserError::Primitive("stale browser locator".into())),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rescoping_binds_every_observed_target_to_its_document() {
        let target = |locator: &str| ObservedTarget {
            locator: locator.into(),
            role: "button".into(),
            name: "Save".into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        };
        let scoped = rescope(
            &doc("F1", "L1"),
            vec![target("locator_a1b2c3d4_1"), target("locator_a1b2c3d4_2")],
        );
        assert_eq!(
            scoped
                .iter()
                .map(|t| t.locator.as_str())
                .collect::<Vec<_>>(),
            ["F1:L1:locator_a1b2c3d4_1", "F1:L1:locator_a1b2c3d4_2"]
        );
        assert!(scoped.iter().all(|t| t.name == "Save"));
    }

    #[test]
    fn rescoping_keeps_what_a_control_holds_and_accepts() {
        let select = ObservedTarget {
            locator: "locator_a1b2c3d4_1".into(),
            role: "combobox".into(),
            name: "Role".into(),
            value: Some(serde_json::json!("Other")),
            options: Some(serde_json::json!(["Developer", "Other"])),
            hint: Some("Choose one...".into()),
            ..Default::default()
        };
        let scoped = rescope(&doc("F1", "L1"), vec![select.clone()]);
        assert_eq!(scoped[0].value, select.value);
        assert_eq!(scoped[0].options, select.options);
        assert_eq!(scoped[0].hint, select.hint);
    }
}
