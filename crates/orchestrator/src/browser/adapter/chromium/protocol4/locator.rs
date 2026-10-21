//! Locators that name a document as well as an element.
//!
//! A page asset mints locators that mean something only inside its own document and world. The
//! service never shows the model a bare local locator. It binds each one to the document that
//! minted it, as `{frame_id}:{loader_id}:{local}`, so a locator survives only as long as its
//! document does: a replaced document has a new loader, and its old locators name nothing.

use super::wire::FrameRef;
use crate::browser::adapter::{ObservedTarget, StaleCandidate, StaleLookup};
use crate::browser::BrowserError;
use crate::workspace::StaleReason;

/// The separator between the parts of an encoded locator.
const SEPARATOR: char = ':';
/// The prefix every local locator carries.
const LOCAL_PREFIX: &str = "locator_";
/// The separator between a local locator's world epoch and its counter.
const LOCAL_SEPARATOR: char = '_';

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
    /// Returns [`BrowserError::StaleLocator`] with the reason `unknown` for a locator that is
    /// malformed or was not minted by a protocol 4 asset: nothing was sent, so nothing happened.
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
            _ => Err(BrowserError::stale(StaleReason::Unknown)),
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

/// Bind the candidates of a stale locator's lookup to the document of that locator.
///
/// A page looks only in its own document, so the controls it found belong to the document the
/// failed locator named. `subject` is the one locator the failed command named. A command with
/// no single locator cannot say which document looked, so its lookup is dropped. Any other
/// error passes through.
#[must_use]
pub fn bind_lookup(error: BrowserError, subject: Option<&str>) -> BrowserError {
    match error {
        BrowserError::StaleLocator {
            reason,
            lookup: Some(lookup),
        } => {
            let doc = subject
                .and_then(|text| Locator::decode(text).ok())
                .map(|locator| locator.doc);
            let lookup = doc.map(|doc| StaleLookup {
                candidates: lookup
                    .candidates
                    .into_iter()
                    .map(|candidate| StaleCandidate {
                        target: ObservedTarget {
                            locator: Locator::new(doc.clone(), candidate.target.locator).encode(),
                            ..candidate.target
                        },
                        same: candidate.same,
                    })
                    .collect(),
                ..lookup
            });
            BrowserError::StaleLocator { reason, lookup }
        }
        other => other,
    }
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
    use serde_json::Value;

    /// A world golden both suites read: it carries a handle in the form the page asset mints.
    const ACTIVATE_FIXTURE: &str =
        include_str!("../../../../../../../tests/golden/protocol4/world/activate.json");

    /// A handle the page minted round-trips through the frame-qualified form the model holds, and
    /// any other text is a stale `unknown` handle before anything is sent, so a malformed or
    /// foreign handle never reaches a page.
    #[test]
    fn a_handle_round_trips_and_anything_else_is_a_stale_handle() {
        let fixture: Value = serde_json::from_str(ACTIVATE_FIXTURE).expect("the fixture parses");
        let minted = fixture["locator"]
            .as_str()
            .expect("the fixture names a handle");
        let document = FrameRef {
            frame_id: "7F3A".into(),
            loader_id: "9C21".into(),
        };
        let locator = Locator::new(document, minted);
        assert_eq!(Locator::decode(&locator.encode()), Ok(locator));

        for text in [
            String::new(),
            minted.to_owned(),
            format!("7F3A:{minted}"),
            format!(":9C21:{minted}"),
            format!("7F3A::{minted}"),
            "7F3A:9C21:".into(),
            "7F3A:9C21:element_1".into(),
            "7F3A:9C21:locator_1".into(),
            "7F3A:9C21:locator__1".into(),
            "7F3A:9C21:locator_a1b2c3d4_".into(),
            "7F3A:9C21:locator_xyz_1".into(),
            format!("7F3A:9C21:{minted}:extra"),
            format!("7F 3A:9C21:{minted}"),
        ] {
            assert_eq!(
                Locator::decode(&text),
                Err(BrowserError::stale(StaleReason::Unknown)),
                "{text:?}"
            );
        }
    }
}
