//! The Chromium page assets and the bundle that installs them.
//!
//! Every file under `assets/` runs in a Chromium page: page-local observation and presentation
//! behavior. This module assembles them into one immutable, hashed bundle. The extension only
//! verifies and installs that bundle; product semantics remain in the orchestrator.
//!
//! Level 3 injects the bundle into the page's own JavaScript world. The bundle bytes are pinned
//! by test so a move or an edit of an asset can never change what a published extension runs
//! without the change being deliberate.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use crate::browser::adapter::chromium::level3::{
    adapter_capability, BrowserCommand, BrowserOutcome,
};
use sha2::{Digest, Sha256};

/// Current page-runtime contract revision.
const REVISION: u16 = 1;

const SENSOR_SCRIPT: &str = include_str!("assets/sensor.js");
const CONTENT_SCRIPT: &str = include_str!("assets/content.js");
const PRESENTATION_SCRIPT: &str = include_str!("assets/presentation.js");
const PRESENTATION_CSS_SCRIPT: &str = include_str!("assets/presentation-css.js");
const SHARED_SCRIPT: &str = include_str!("assets/shared.js");
const FORM_DIAGNOSTICS_SCRIPT: &str = include_str!("assets/form-diagnostics.js");

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

/// Whether an adapter that advertised these capabilities must receive the bundle.
#[must_use]
pub(crate) fn installs(capabilities: &HashMap<String, u16>) -> bool {
    capabilities
        .get(adapter_capability::PAGE_RUNTIME)
        .copied()
        .unwrap_or_default()
        >= REVISION
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

    /// The level 3 bundle as published extensions receive it. Moving or reformatting an asset
    /// must not change a byte of it; change this value only when the bundle changes on purpose.
    const LEVEL3_BUNDLE_SHA256: &str =
        "9eb521d0cab493adeda19cef6091c6513de27e06c59d509dcf2c3869ac79d8b4";

    #[test]
    fn level3_bundle_sha256_is_pinned() {
        assert_eq!(bundle().sha256, LEVEL3_BUNDLE_SHA256);
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
