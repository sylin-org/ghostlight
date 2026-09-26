//! Service-owned page runtime installed by capable browser adapters.
//!
//! The runtime contains page-local observation and presentation behavior. The adapter only
//! verifies and installs this versioned bundle; product semantics remain in the orchestrator.

use std::fmt::Write as _;
use std::sync::OnceLock;

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome};
use sha2::{Digest, Sha256};

/// Current page-runtime contract revision.
pub(crate) const REVISION: u16 = 1;

const SENSOR_SCRIPT: &str = include_str!("sensor.js");
const CONTENT_SCRIPT: &str = include_str!("content.js");
const PRESENTATION_SCRIPT: &str = include_str!("../../../../extension/lib/presentation.js");
const PRESENTATION_CSS_SCRIPT: &str = include_str!("../../../../extension/lib/presentation-css.js");
const SHARED_SCRIPT: &str = include_str!("../../../../extension/lib/shared.js");
const FORM_DIAGNOSTICS_SCRIPT: &str = include_str!("../../../../extension/lib/form-diagnostics.js");

/// One immutable runtime payload reused for every compatible adapter connection.
#[derive(Debug)]
struct PageRuntimeBundle {
    /// Closed runtime contract revision.
    revision: u16,
    /// Lowercase SHA-256 of `script`.
    sha256: String,
    /// Complete self-contained page script.
    script: String,
}

/// Return the process-wide immutable page-runtime bundle.
#[must_use]
fn bundle() -> &'static PageRuntimeBundle {
    static BUNDLE: OnceLock<PageRuntimeBundle> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let body = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            SHARED_SCRIPT,
            FORM_DIAGNOSTICS_SCRIPT,
            SENSOR_SCRIPT,
            PRESENTATION_CSS_SCRIPT,
            PRESENTATION_SCRIPT,
            CONTENT_SCRIPT
        );
        let fingerprint = sha256(&body);
        let script = format!(
            "(() => {{\nif (globalThis.__ghostlight_page_runtime__ === \"{fingerprint}\") return;\n{body}\nglobalThis.__ghostlight_page_runtime__ = \"{fingerprint}\";\n}})();"
        );
        PageRuntimeBundle {
            revision: REVISION,
            sha256: sha256(&script),
            script,
        }
    })
}

/// Build the closed browser mechanism that installs the shared runtime.
#[must_use]
pub(crate) fn install_command() -> BrowserCommand {
    let bundle = bundle();
    BrowserCommand::InstallPageRuntime {
        revision: bundle.revision,
        sha256: bundle.sha256.clone(),
        script: bundle.script.clone(),
    }
}

/// Verify that an adapter acknowledged the exact runtime it was sent.
#[must_use]
pub(crate) fn acknowledged(outcome: &BrowserOutcome) -> bool {
    let bundle = bundle();
    matches!(
        outcome,
        BrowserOutcome::PageRuntimeInstalled { revision, sha256 }
            if *revision == bundle.revision && sha256 == &bundle.sha256
    )
}

fn sha256(value: &str) -> String {
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(value.as_bytes()) {
        write!(&mut result, "{byte:02x}").expect("writing to a string cannot fail");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_reused_and_self_authenticating() {
        assert!(std::ptr::eq(bundle(), bundle()));
        assert_eq!(bundle().revision, REVISION);
        assert_eq!(bundle().sha256, sha256(&bundle().script));
        assert_eq!(bundle().sha256.len(), 64);
    }

    #[test]
    fn acknowledgement_must_name_the_exact_bundle() {
        assert!(acknowledged(&BrowserOutcome::PageRuntimeInstalled {
            revision: bundle().revision,
            sha256: bundle().sha256.clone(),
        }));
        assert!(!acknowledged(&BrowserOutcome::PageRuntimeInstalled {
            revision: bundle().revision + 1,
            sha256: bundle().sha256.clone(),
        }));
    }
}
