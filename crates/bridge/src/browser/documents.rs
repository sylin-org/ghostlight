//! Browser-observed document identity and closed physical execution constraints.

use serde::{Deserialize, Serialize};

/// Maximum document metadata retained or admitted in one physical tab observation.
pub const DOCUMENT_LIMIT: usize = 256;

/// One browser-observed document, independent of reusable frame routing ids.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentInventory {
    /// Stable ordered current documents, bounded by `DOCUMENT_LIMIT`.
    pub documents: Vec<PhysicalDocument>,
    /// Document identities of explicitly targeted locators, points, or current focus.
    pub subjects: Vec<String>,
    /// A requested subject could not be bound to a current document.
    pub unresolved: bool,
    /// Browser metadata omitted documents or was otherwise incomplete.
    pub incomplete: bool,
}

/// Application-supplied physical scope, with no policy rule or capability classification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentScope {
    /// Inventory against which this execution was admitted.
    pub documents: Vec<PhysicalDocument>,
    /// Exact document identities the primitive may access.
    pub allowed: Vec<String>,
    /// Exact routed subjects, when the primitive names particular documents.
    pub subjects: Vec<String>,
    /// Mask other document regions for this capture.
    pub mask: Option<String>,
    /// Stop retained recording when this admitted document set changes.
    pub watch_changes: bool,
}

/// Physical coverage facts; no model-facing interpretation or excluded host disclosure.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentObservation {
    /// Documents actually reached by the content mechanism.
    pub visited: Vec<String>,
    /// Admitted documents unavailable at the point of observation.
    pub unavailable: Vec<String>,
    /// The single output budget omitted supported content.
    pub limited_by_size: bool,
    /// Number of excluded regions masked before image capture.
    pub masked_regions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::{adapter_capability as capability, BrowserCommand, BrowserOutcome};

    #[test]
    fn document_envelopes_round_trip_under_their_own_capability() {
        let document = PhysicalDocument {
            id: "browser-document".into(),
            url: "https://example.com/".into(),
            parent: None,
            supported: true,
        };
        let command = BrowserCommand::InDocuments {
            scope: DocumentScope {
                documents: vec![document.clone()],
                allowed: vec![document.id.clone()],
                subjects: vec![],
                mask: Some("Excluded by policy".into()),
                watch_changes: true,
            },
            primitive: Box::new(BrowserCommand::ReadText {
                tab_id: 7,
                locator: None,
                max_chars: 500,
            }),
        };
        assert_eq!(command.required_capability(), capability::DOCUMENT_SCOPE);
        assert_eq!(
            serde_json::from_value::<BrowserCommand>(serde_json::to_value(&command).unwrap())
                .unwrap(),
            command
        );
        let result = BrowserOutcome::InDocuments {
            observation: DocumentObservation {
                visited: vec![document.id.clone()],
                ..DocumentObservation::default()
            },
            result: Box::new(BrowserOutcome::Text {
                tab_id: 7,
                text: "permitted".into(),
                truncated: false,
                title: "Page".into(),
                url: document.url.clone(),
            }),
        };
        assert_eq!(
            serde_json::from_value::<BrowserOutcome>(serde_json::to_value(&result).unwrap())
                .unwrap(),
            result
        );
        let described = BrowserOutcome::Documents {
            tab_id: 7,
            inventory: DocumentInventory {
                documents: vec![document],
                ..DocumentInventory::default()
            },
        };
        assert_eq!(
            serde_json::from_value::<BrowserOutcome>(serde_json::to_value(&described).unwrap())
                .unwrap(),
            described
        );
        assert!(serde_json::from_value::<DocumentObservation>(serde_json::json!({"visited":[], "unavailable":[], "limited_by_size":false, "masked_regions":0, "host":"unexpected"})).is_err());
    }
}
