//! Content-free coverage language and a separate volatile local-human explanation.

use serde::{Deserialize, Serialize};

/// Coverage relative to the requested supported scope, independent of browser effects.
/// Multi-primitive operations retain each count's observed maximum, not a sum of repeated reads.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    /// The operation targets selected documents rather than the whole composed page.
    pub targeted: bool,
    /// Documents in scope actually reached by the adapter.
    pub inspected_documents: usize,
    /// Requested documents excluded by enforced policy.
    pub excluded_documents: usize,
    /// Exclusions elsewhere on the page, relevant to the person's notice preference.
    pub page_excluded_documents: usize,
    /// Requested documents unavailable or not supported by this observation mechanism.
    pub unavailable_documents: usize,
    /// An output ceiling omitted otherwise permitted content.
    pub limited_by_size: bool,
    /// Excluded regions visibly masked in a returned capture.
    pub masked_regions: usize,
}

impl Coverage {
    /// Preserve observed limitations across preparation and the eventual browser primitive.
    pub fn include(&mut self, previous: &Self) {
        self.targeted &= previous.targeted;
        self.inspected_documents = self.inspected_documents.max(previous.inspected_documents);
        self.excluded_documents = self.excluded_documents.max(previous.excluded_documents);
        self.page_excluded_documents = self
            .page_excluded_documents
            .max(previous.page_excluded_documents);
        self.unavailable_documents = self
            .unavailable_documents
            .max(previous.unavailable_documents);
        self.limited_by_size |= previous.limited_by_size;
        self.masked_regions = self.masked_regions.max(previous.masked_regions);
    }
    /// Whether the requested observation has an explicit coverage limitation.
    #[must_use]
    pub fn limited(&self) -> bool {
        self.excluded_documents > 0 || self.unavailable_documents > 0 || self.limited_by_size
    }

    /// One short qualification, authored without browser content or document identities.
    #[must_use]
    pub fn qualification(&self) -> String {
        let mut parts = Vec::new();
        if self.excluded_documents > 0 {
            parts.push("Content excluded by policy.");
        }
        if self.unavailable_documents > 0 {
            parts.push("Some document content was unavailable.");
        }
        if self.limited_by_size {
            parts.push("The result reached its size limit.");
        }
        parts.join(" ")
    }
}

/// Volatile local-human details. Never serialize this into a tool result or audit record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HumanCoverage {
    /// Human-facing coverage indication authored by the orchestrator.
    pub summary: String,
    /// Explanation of the visible-page versus Ghostlight-access distinction.
    pub explanation: String,
    /// The operation whose bounded history entry owns these details.
    pub invocation: String,
    /// Content-free coverage also supplied to the client.
    pub coverage: Coverage,
    /// Bounded excluded host names, available only in the local workbench.
    pub excluded_hosts: Vec<String>,
    /// Whether to expose the indication before the person expands details.
    pub proactive: bool,
}

/// Author the completion qualification consistently for model results and retained history.
#[must_use]
pub fn qualify(summary: &str, coverage: &Coverage) -> String {
    let qualification = coverage.qualification();
    if qualification.is_empty() {
        summary.into()
    } else {
        format!("{summary} {qualification}")
    }
}

/// Visible content-free replacement for an excluded screenshot region.
pub const MASK_LABEL: &str = "Excluded by policy";
/// Human-only disclosure explanation, separate from the completed operation sentence.
pub const HUMAN_EXPLANATION: &str =
    "Visible in your browser; excluded from Ghostlight access by policy.";

/// Author one bounded local-human coverage indication.
#[must_use]
pub fn human_summary(coverage: &Coverage) -> String {
    if coverage.page_excluded_documents > 0 {
        format!(
            "{} embedded {} excluded by policy.",
            coverage.page_excluded_documents,
            if coverage.page_excluded_documents == 1 {
                "document"
            } else {
                "documents"
            }
        )
    } else {
        coverage.qualification()
    }
}
