//! Browser-observed document identity and closed physical execution constraints.

use crate::browser::BrowserError;

/// Maximum document metadata retained or admitted in one physical tab observation.
pub const DOCUMENT_LIMIT: usize = 256;
/// The longest document url an inventory may carry, in bytes.
pub const DOCUMENT_URL_LIMIT: usize = 8192;
/// The longest document id an inventory may carry, in bytes.
const DOCUMENT_ID_LIMIT: usize = 160;
/// The protocol failure of an inventory that is not one bounded tree.
const NOT_ONE_TREE: &str = "document inventory is not one bounded tree";

/// One browser-observed document, independent of reusable frame routing ids.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalDocument {
    /// Browser-issued identity that changes when the document is replaced.
    pub id: String,
    /// Browser-observed URL used only by the application authority.
    pub url: String,
    /// Parent document identity; absent only for the top document.
    pub parent: Option<String>,
    /// Whether the adapter can address this document with its observation mechanism.
    pub supported: bool,
}

/// Content-free document inventory and the subjects of an exact physical request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DocumentInventory {
    /// Stable ordered current documents, bounded by `DOCUMENT_LIMIT`.
    pub documents: Vec<PhysicalDocument>,
    /// Document identities of the request's locators. A point and the focus are not here: the
    /// adapter binds them when it sends the input, once the page shows where they are.
    pub subjects: Vec<String>,
    /// The tab may hold a document this list does not show: the browser could not complete its
    /// inventory, or listed a frame the adapter could not place in the tree.
    pub incomplete: bool,
}

impl DocumentInventory {
    /// Check that the inventory is one bounded tree whose subjects are documents of it.
    ///
    /// An adapter owns the shape of the tree it reports, so a list that is not one is a breach
    /// by the adapter, not a state of the page, and nothing the model could act on.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::Protocol`] for an empty or oversized list, an empty, oversized or
    /// repeated id, an oversized url, no top or several, a parent that is not listed, a loop,
    /// and a subject that is not a listed document.
    pub fn validate(&self) -> Result<(), BrowserError> {
        use std::collections::HashSet;
        let breach = || BrowserError::Protocol(NOT_ONE_TREE.into());
        let mut ids = HashSet::new();
        if self.documents.is_empty()
            || self.documents.len() > DOCUMENT_LIMIT
            || self.subjects.len() > DOCUMENT_LIMIT
            || self.documents.iter().any(|document| {
                document.id.is_empty()
                    || document.id.len() > DOCUMENT_ID_LIMIT
                    || document.url.len() > DOCUMENT_URL_LIMIT
                    || !ids.insert(document.id.as_str())
            })
            || self
                .documents
                .iter()
                .filter(|document| document.parent.is_none())
                .count()
                != 1
            || self.subjects.iter().any(|id| !ids.contains(id.as_str()))
        {
            return Err(breach());
        }
        for document in &self.documents {
            let mut ancestry = HashSet::new();
            let mut current = document;
            while let Some(parent) = &current.parent {
                if !ancestry.insert(current.id.as_str()) {
                    return Err(breach());
                }
                current = self
                    .documents
                    .iter()
                    .find(|item| &item.id == parent)
                    .ok_or_else(breach)?;
            }
        }
        Ok(())
    }
}

/// Application-supplied physical scope, with no policy rule or capability classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentScope {
    /// Inventory against which this execution was admitted.
    pub documents: Vec<PhysicalDocument>,
    /// Exact document identities the primitive may access.
    pub allowed: Vec<String>,
    /// Exact routed subjects, when the primitive names particular documents.
    pub subjects: Vec<String>,
    /// Watch this admitted document set: a recording stops when it changes, and a diagnostics
    /// read is bound to it, so retained entries reset when it changes.
    pub watch_changes: bool,
    /// Require the complete admitted tree to stay fixed for a broad restricted operation.
    pub strict_tree: bool,
    /// Remaining shared preparation budget for local collectors in milliseconds. A spent budget
    /// is zero; it does not say the agent declined settling.
    pub settle_ms: u64,
    /// The agent's intent: false only when the call turned settling off.
    pub settle_requested: bool,
}

/// Physical coverage facts; no model-facing interpretation or excluded host disclosure.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DocumentObservation {
    /// Documents actually reached by the content mechanism.
    pub visited: Vec<String>,
    /// Admitted documents unavailable at the point of observation.
    pub unavailable: Vec<String>,
    /// The single output budget omitted supported content.
    pub limited_by_size: bool,
}
