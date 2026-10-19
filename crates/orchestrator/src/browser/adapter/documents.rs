//! Browser-observed document identity and closed physical execution constraints.

/// Maximum document metadata retained or admitted in one physical tab observation.
pub const DOCUMENT_LIMIT: usize = 256;

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
    /// Document identities of explicitly targeted locators, points, or current focus.
    pub subjects: Vec<String>,
    /// A requested point or the focus could not be bound to a current document. A locator never
    /// sets it: it binds, fails the description as stale, or is left to `incomplete`.
    pub unresolved: bool,
    /// Browser metadata omitted documents or was otherwise incomplete.
    pub incomplete: bool,
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
    /// Stop retained recording when this admitted document set changes.
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
