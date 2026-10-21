//! One authoritative RAWX requirement directory for the current Ghostlight language.

use crate::governance::CapabilitySet;

use super::Operation;

const READ_WRITE: CapabilitySet = CapabilitySet::READ.union(CapabilitySet::WRITE);
const READ_ACTION: CapabilitySet = CapabilitySet::READ.union(CapabilitySet::ACTION);
const READ_WRITE_ACTION: CapabilitySet = READ_WRITE.union(CapabilitySet::ACTION);

/// One reachable capability variant of an advertised tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityVariant {
    /// Exact advertised tool name.
    pub tool: &'static str,
    /// Variant key when one tool has meaningfully different authority shapes.
    pub variant: Option<&'static str>,
    /// Complete independent RAWX requirement set.
    pub requirements: CapabilitySet,
    /// Human-plain explanation used by policy inspection.
    pub description: &'static str,
}

const fn variant(
    tool: &'static str,
    action: Option<&'static str>,
    requirements: CapabilitySet,
    description: &'static str,
) -> CapabilityVariant {
    CapabilityVariant {
        tool,
        variant: action,
        requirements,
        description,
    }
}

const TABS_LIST: CapabilityVariant = variant(
    "browser_tabs",
    Some("list"),
    CapabilitySet::READ,
    "List this workspace's bound tabs, read live from the browser.",
);

const TABS_FOCUS: CapabilityVariant = variant(
    "browser_tabs",
    Some("focus"),
    CapabilitySet::EMPTY,
    "Focus one controlled tab.",
);

const TABS_CLOSE: CapabilityVariant = variant(
    "browser_tabs",
    Some("close"),
    CapabilitySet::ACTION,
    "Close one controlled tab.",
);

const NAVIGATE: CapabilityVariant = variant(
    "browser_navigate",
    None,
    CapabilitySet::READ,
    "Retrieve a page in a controlled tab.",
);

const HISTORY: CapabilityVariant = variant(
    "browser_history",
    None,
    CapabilitySet::ACTION,
    "Traverse or reload browser history.",
);

const WINDOW_ZOOM: CapabilityVariant = variant(
    "browser_window",
    Some("zoom"),
    CapabilitySet::READ,
    "Change page zoom.",
);

const WINDOW_RESIZE: CapabilityVariant = variant(
    "browser_window",
    Some("resize"),
    CapabilitySet::EMPTY,
    "Resize browser chrome.",
);

const READ: CapabilityVariant = variant(
    "browser_read",
    None,
    CapabilitySet::READ,
    "Read bounded page text.",
);

const INSPECT: CapabilityVariant = variant(
    "browser_inspect",
    None,
    CapabilitySet::READ,
    "Inspect bounded page structure.",
);

const FIND: CapabilityVariant = variant(
    "browser_find",
    None,
    CapabilitySet::READ,
    "Find semantic page targets.",
);

const SCREENSHOT: CapabilityVariant = variant(
    "browser_screenshot",
    None,
    CapabilitySet::READ,
    "Capture a bounded page image.",
);

const CLICK: CapabilityVariant = variant(
    "browser_click",
    None,
    CapabilitySet::ACTION,
    "Activate a page target.",
);

const CLICK_SELECTOR: CapabilityVariant = variant(
    "browser_click",
    Some("selector"),
    READ_ACTION,
    "Find a named control and activate it.",
);

const CLICK_EXPECT: CapabilityVariant = variant(
    "browser_click",
    Some("expect"),
    READ_ACTION,
    "Activate a page target and observe the requested postcondition.",
);

const SCROLL: CapabilityVariant = variant(
    "browser_scroll",
    None,
    CapabilitySet::READ,
    "Scroll or reveal page content.",
);

const HOVER: CapabilityVariant = variant(
    "browser_hover",
    None,
    CapabilitySet::READ,
    "Hover over page content.",
);

const FILL: CapabilityVariant = variant(
    "browser_fill_form",
    Some("fill"),
    READ_WRITE,
    "Read targets and fill declared fields.",
);

const FILL_SUBMIT: CapabilityVariant = variant(
    "browser_fill_form",
    Some("submit"),
    READ_WRITE_ACTION,
    "Read targets, fill fields, and submit.",
);

const TYPE: CapabilityVariant = variant(
    "browser_type_text",
    None,
    CapabilitySet::ACTION,
    "Type through browser input events.",
);

const TYPE_SELECTOR: CapabilityVariant = variant(
    "browser_type_text",
    Some("selector"),
    READ_ACTION,
    "Find a named control and type through browser input events.",
);

const TYPE_EXPECT: CapabilityVariant = variant(
    "browser_type_text",
    Some("expect"),
    READ_ACTION,
    "Type through browser input events and observe the requested postcondition.",
);

const PRESS_KEY: CapabilityVariant = variant(
    "browser_press_key",
    None,
    CapabilitySet::ACTION,
    "Send one keyboard action.",
);

const PRESS_KEY_EXPECT: CapabilityVariant = variant(
    "browser_press_key",
    Some("expect"),
    READ_ACTION,
    "Send one keyboard action and observe the requested postcondition.",
);

const DRAG: CapabilityVariant = variant(
    "browser_drag",
    None,
    CapabilitySet::ACTION,
    "Perform one pointer drag.",
);

const UPLOAD: CapabilityVariant = variant(
    "browser_upload",
    None,
    CapabilitySet::WRITE,
    "Place declared files into a page input.",
);

const UPLOAD_SELECTOR: CapabilityVariant = variant(
    "browser_upload",
    Some("selector"),
    READ_WRITE,
    "Find a named file input and place declared files into it.",
);

const EXECUTE: CapabilityVariant = variant(
    "browser_execute",
    None,
    CapabilitySet::EXECUTE,
    "Evaluate explicit page JavaScript.",
);

const WAIT: CapabilityVariant = variant(
    "browser_wait",
    None,
    CapabilitySet::READ,
    "Observe one bounded page condition.",
);

const FLOW: CapabilityVariant = variant(
    "browser_flow",
    None,
    CapabilitySet::EMPTY,
    "Compose result-aware governed steps; each child authorizes normally.",
);

const DIALOG_STATUS: CapabilityVariant = variant(
    "browser_dialog",
    Some("status"),
    CapabilitySet::READ,
    "Inspect JavaScript dialog state.",
);

const DIALOG_RESOLVE: CapabilityVariant = variant(
    "browser_dialog",
    Some("resolve"),
    CapabilitySet::ACTION,
    "Accept, dismiss, or respond to a dialog.",
);

const RECORD_START: CapabilityVariant = variant(
    "browser_record",
    Some("start"),
    CapabilitySet::READ,
    "Start capturing a controlled page.",
);

const RECORD_INSPECT: CapabilityVariant = variant(
    "browser_record",
    Some("inspect"),
    CapabilitySet::EMPTY,
    "Inspect, stop, or discard volatile capture.",
);

const RECORD_SAVE_CLIENT: CapabilityVariant = variant(
    "browser_record",
    Some("save_client"),
    CapabilitySet::READ,
    "Return or download a captured page recording.",
);

const RECORD_SAVE_TARGET: CapabilityVariant = variant(
    "browser_record",
    Some("save_target"),
    READ_WRITE,
    "Read captured sources and place a recording into a page target.",
);

const DIAGNOSE: CapabilityVariant = variant(
    "browser_diagnose",
    None,
    CapabilitySet::READ,
    "Read bounded opt-in browser diagnostics.",
);

const POLICY_EXPLAIN: CapabilityVariant = variant(
    "policy_explain",
    None,
    CapabilitySet::EMPTY,
    "Explain the authority in force.",
);

/// The complete current action directory, in the order `ghostlight policy` prints it.
pub const DIRECTORY: &[CapabilityVariant] = &[
    TABS_LIST,
    TABS_FOCUS,
    TABS_CLOSE,
    NAVIGATE,
    HISTORY,
    WINDOW_ZOOM,
    WINDOW_RESIZE,
    READ,
    INSPECT,
    FIND,
    SCREENSHOT,
    CLICK,
    CLICK_SELECTOR,
    CLICK_EXPECT,
    SCROLL,
    HOVER,
    FILL,
    FILL_SUBMIT,
    TYPE,
    TYPE_SELECTOR,
    TYPE_EXPECT,
    PRESS_KEY,
    PRESS_KEY_EXPECT,
    DRAG,
    UPLOAD,
    UPLOAD_SELECTOR,
    EXECUTE,
    WAIT,
    FLOW,
    DIALOG_STATUS,
    DIALOG_RESOLVE,
    RECORD_START,
    RECORD_INSPECT,
    RECORD_SAVE_CLIENT,
    RECORD_SAVE_TARGET,
    DIAGNOSE,
    POLICY_EXPLAIN,
];

/// Return every reachable requirement variant for one advertised tool.
pub fn variants(tool: &str) -> impl Iterator<Item = &'static CapabilityVariant> + '_ {
    DIRECTORY.iter().filter(move |entry| entry.tool == tool)
}

/// Return the exact requirement set for one decoded operation.
#[must_use]
pub fn requirements(operation: &Operation) -> CapabilitySet {
    entry(operation).requirements
}

/// The directory entry a decoded operation reaches. The match is exhaustive, so every operation
/// has an entry and no requirement set is written anywhere but the directory.
fn entry(operation: &Operation) -> &'static CapabilityVariant {
    match operation {
        Operation::ListTabs(_) => &TABS_LIST,
        Operation::ActivateTab(_) => &TABS_FOCUS,
        Operation::CloseTab(_) => &TABS_CLOSE,
        Operation::OpenPage(_) | Operation::NavigatePage(_) => &NAVIGATE,
        Operation::NavigateHistory(_) | Operation::ReloadPage(_) => &HISTORY,
        Operation::SetZoom(_) => &WINDOW_ZOOM,
        Operation::ResizeWindow(_) => &WINDOW_RESIZE,
        Operation::ReadPage(_) => &READ,
        Operation::InspectPage(_) => &INSPECT,
        Operation::Find(_) => &FIND,
        Operation::TakeScreenshot(_) => &SCREENSHOT,
        Operation::Click(value) if value.selector.is_some() => &CLICK_SELECTOR,
        Operation::Click(value) if value.expect.is_some() => &CLICK_EXPECT,
        Operation::Click(_) => &CLICK,
        Operation::ScrollPage(_) => &SCROLL,
        Operation::Hover(_) => &HOVER,
        Operation::FillForm(value) if value.submit_target.is_some() => &FILL_SUBMIT,
        Operation::FillForm(_) => &FILL,
        Operation::TypeText(value) if value.selector.is_some() => &TYPE_SELECTOR,
        Operation::TypeText(value) if value.expect.is_some() => &TYPE_EXPECT,
        Operation::TypeText(_) => &TYPE,
        Operation::PressKey(value) if value.expect.is_some() => &PRESS_KEY_EXPECT,
        Operation::PressKey(_) => &PRESS_KEY,
        Operation::Drag(_) => &DRAG,
        Operation::UploadFiles(value) if value.selector.is_some() => &UPLOAD_SELECTOR,
        Operation::UploadFiles(_) => &UPLOAD,
        Operation::RunScript(_) => &EXECUTE,
        Operation::Wait(_) => &WAIT,
        // A flow classifies nothing itself; every child step authorizes normally.
        Operation::RunFlow(_) => &FLOW,
        Operation::HandleDialog(value) if value.action == "status" => &DIALOG_STATUS,
        Operation::HandleDialog(_) => &DIALOG_RESOLVE,
        Operation::Record(value) if value.action == "start" => &RECORD_START,
        Operation::Record(value) if value.action == "save" && value.target.is_some() => {
            &RECORD_SAVE_TARGET
        }
        Operation::Record(value) if value.action == "save" => &RECORD_SAVE_CLIENT,
        Operation::Record(_) => &RECORD_INSPECT,
        Operation::Diagnose(_) => &DIAGNOSE,
        Operation::ExplainPolicy(_) => &POLICY_EXPLAIN,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::governance::CapabilitySet;
    use serde_json::json;

    use super::{entry, requirements, variants, CapabilityVariant, DIRECTORY};

    fn decoded(tool: &str, input: serde_json::Value) -> CapabilitySet {
        requirements(&crate::language::decode(tool, input).expect("valid operation"))
    }

    /// One minimal valid decode input for every entry in `DIRECTORY`, keyed by exactly the same
    /// (tool, variant) pair. A `DIRECTORY` entry with no arm here is a compile-time-silent, test-
    /// time-loud gap: the match below is exhaustive over what this test knows to check, and an
    /// unmatched entry panics with its own tool/variant rather than being skipped.
    fn fixture(tool: &str, variant: Option<&str>) -> serde_json::Value {
        let upload_path = if cfg!(windows) {
            r"C:\ghostlight\upload.txt"
        } else {
            "/tmp/ghostlight/upload.txt"
        };
        match (tool, variant) {
            ("browser_tabs", Some("list")) => json!({"action":"list"}),
            ("browser_tabs", Some("focus")) => json!({"action":"focus","tab":"tab_1"}),
            ("browser_tabs", Some("close")) => json!({"action":"close","tab":"tab_1"}),
            ("browser_navigate", None) => json!({"url":"https://example.com"}),
            ("browser_history", None) => json!({"action":"back"}),
            ("browser_window", Some("zoom")) => json!({"action":"zoom","percent":100}),
            ("browser_window", Some("resize")) => {
                json!({"action":"resize","width":1280,"height":720})
            }
            ("browser_read", None) => json!({}),
            ("browser_inspect", None) => json!({}),
            ("browser_find", None) => json!({"text":"Login"}),
            ("browser_screenshot", None) => json!({}),
            ("browser_click", None) => json!({"target":"target_1"}),
            ("browser_click", Some("selector")) => json!({"selector":{"name":"Save"}}),
            ("browser_click", Some("expect")) => {
                json!({"target":"target_1","expect":{"condition":"load_ready"}})
            }
            ("browser_scroll", None) => json!({}),
            ("browser_hover", None) => json!({"target":"target_1"}),
            ("browser_fill_form", Some("fill")) => {
                json!({"fields":[{"target":"target_1","value":"Ada"}]})
            }
            ("browser_fill_form", Some("submit")) => {
                json!({"fields":[{"target":"target_1","value":"Ada"}],"submit_target":"target_2"})
            }
            ("browser_type_text", None) => json!({"target":"target_1","text":"Ada"}),
            ("browser_type_text", Some("selector")) => {
                json!({"selector":{"name":"Draft"},"text":"Ada"})
            }
            ("browser_type_text", Some("expect")) => {
                json!({"focused":true,"text":"Ada","expect":{"condition":"load_ready"}})
            }
            ("browser_press_key", None) => json!({"key":"Enter"}),
            ("browser_press_key", Some("expect")) => {
                json!({"key":"Enter","expect":{"condition":"load_ready"}})
            }
            ("browser_drag", None) => {
                json!({"source_target":"target_1","destination_target":"target_2"})
            }
            ("browser_upload", None) => json!({"target":"target_1","paths":[upload_path]}),
            ("browser_upload", Some("selector")) => {
                json!({"selector":{"name":"Attachment"},"paths":[upload_path]})
            }
            ("browser_execute", None) => json!({"script":"1+1"}),
            ("browser_wait", None) => json!({"condition":"load_ready"}),
            ("browser_flow", None) => json!({"steps":[
                {"id":"list","tool":"browser_tabs","arguments":{"action":"list"}}
            ]}),
            ("browser_dialog", Some("status")) => json!({"action":"status"}),
            ("browser_dialog", Some("resolve")) => json!({"action":"accept"}),
            ("browser_record", Some("start")) => json!({"action":"start"}),
            ("browser_record", Some("inspect")) => json!({"action":"status"}),
            ("browser_record", Some("save_client")) => json!({"action":"save"}),
            ("browser_record", Some("save_target")) => {
                json!({"action":"save","target":"target_1"})
            }
            ("browser_diagnose", None) => json!({}),
            ("policy_explain", None) => json!({}),
            (tool, variant) => panic!(
                "DIRECTORY grew a (tool={tool}, variant={variant:?}) entry with no fixture in \
                 this test; add one so its requirements stay cross-checked against decode()"
            ),
        }
    }

    #[test]
    fn every_catalog_tool_has_at_least_one_policy_variant() {
        let expected: HashSet<_> = [
            "browser_tabs",
            "browser_navigate",
            "browser_history",
            "browser_window",
            "browser_read",
            "browser_inspect",
            "browser_find",
            "browser_screenshot",
            "browser_click",
            "browser_scroll",
            "browser_hover",
            "browser_fill_form",
            "browser_type_text",
            "browser_press_key",
            "browser_drag",
            "browser_upload",
            "browser_execute",
            "browser_wait",
            "browser_flow",
            "browser_dialog",
            "browser_record",
            "browser_diagnose",
            "policy_explain",
        ]
        .into_iter()
        .collect();
        let actual: HashSet<_> = DIRECTORY.iter().map(|entry| entry.tool).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn compound_and_empty_variants_are_explicit() {
        let submitted = variants("browser_fill_form")
            .find(|entry| entry.variant == Some("submit"))
            .expect("submit variant");
        assert_eq!(
            submitted.requirements,
            CapabilitySet::READ
                .union(CapabilitySet::WRITE)
                .union(CapabilitySet::ACTION)
        );
        assert!(variants("browser_tabs")
            .find(|entry| entry.variant == Some("focus"))
            .expect("focus variant")
            .requirements
            .is_empty());
        assert!(variants("browser_window")
            .find(|entry| entry.variant == Some("resize"))
            .expect("resize variant")
            .requirements
            .is_empty());
    }

    #[test]
    fn decoded_operations_use_the_exact_rawx_map() {
        assert_eq!(
            decoded("browser_navigate", json!({"url":"https://example.com"})),
            CapabilitySet::READ
        );
        assert_eq!(
            decoded("browser_tabs", json!({"action":"focus","tab":"tab_1"})),
            CapabilitySet::EMPTY
        );
        assert_eq!(
            decoded(
                "browser_window",
                json!({"action":"resize","width":1280,"height":720})
            ),
            CapabilitySet::EMPTY
        );
        assert_eq!(
            decoded(
                "browser_fill_form",
                json!({"fields":[{"target":"target_1","value":"Ada"}]})
            ),
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        );
        assert_eq!(
            decoded(
                "browser_fill_form",
                json!({"fields":[{"target":"target_1","value":"Ada"}],"submit_target":"target_2"})
            ),
            CapabilitySet::READ
                .union(CapabilitySet::WRITE)
                .union(CapabilitySet::ACTION)
        );
        assert_eq!(
            decoded(
                "browser_type_text",
                json!({"target":"target_1","text":"Ada"})
            ),
            CapabilitySet::ACTION
        );
        assert_eq!(
            decoded("browser_dialog", json!({"action":"respond","text":"Ada"})),
            CapabilitySet::ACTION
        );
        assert_eq!(
            decoded("browser_record", json!({"action":"discard"})),
            CapabilitySet::EMPTY
        );
        let flow = crate::language::decode("browser_flow", json!({"steps":[
            {"tool":"browser_fill_form","arguments":{"fields":[{"target":"target_1","value":"Ada"}]}},
            {"tool":"browser_wait","arguments":{"condition":"load_ready"}}
        ]})).unwrap();
        assert_eq!(requirements(&flow), CapabilitySet::EMPTY);
        let crate::language::Operation::RunFlow(flow) = flow else {
            panic!("flow")
        };
        let child =
            crate::language::decode(&flow.steps[0].tool, flow.steps[0].arguments.clone()).unwrap();
        assert_eq!(
            requirements(&child),
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        );
    }

    #[test]
    fn every_directory_entry_matches_what_decode_actually_requires() {
        for listed in DIRECTORY {
            let CapabilityVariant {
                tool,
                variant,
                requirements: expected,
                ..
            } = *listed;
            let input = fixture(tool, variant);
            let operation = crate::language::decode(tool, input.clone())
                .unwrap_or_else(|error| panic!("tool={tool} variant={variant:?} input={input}: fixture must decode, got {error:?}"));
            assert_eq!(
                requirements(&operation),
                expected,
                "tool={tool} variant={variant:?}: DIRECTORY's advertised requirement diverged \
                 from what requirements() actually returns for the decoded operation"
            );
            assert_eq!(
                entry(&operation),
                entry_of(tool, variant),
                "tool={tool} variant={variant:?}: the decoded operation reaches another entry"
            );
        }
    }

    fn entry_of(tool: &str, variant: Option<&str>) -> &'static CapabilityVariant {
        DIRECTORY
            .iter()
            .find(|entry| entry.tool == tool && entry.variant == variant)
            .expect("the entry is in the directory")
    }

    #[test]
    fn each_tool_and_variant_is_one_directory_entry() {
        let mut seen = HashSet::new();
        for entry in DIRECTORY {
            assert!(
                seen.insert((entry.tool, entry.variant)),
                "{} {:?} appears twice",
                entry.tool,
                entry.variant
            );
        }
    }
}
