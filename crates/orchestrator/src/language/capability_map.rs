//! One authoritative RAWX requirement directory for the current Ghostlight language.

use crate::governance::CapabilitySet;

use super::{Operation, SequenceStep};

const READ_WRITE: CapabilitySet = CapabilitySet::READ.union(CapabilitySet::WRITE);
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

/// The complete current action directory.
pub const DIRECTORY: &[CapabilityVariant] = &[
    variant(
        "browser_tabs",
        Some("list"),
        CapabilitySet::READ,
        "List controlled tabs.",
    ),
    variant(
        "browser_tabs",
        Some("focus"),
        CapabilitySet::EMPTY,
        "Focus one controlled tab.",
    ),
    variant(
        "browser_tabs",
        Some("close"),
        CapabilitySet::ACTION,
        "Close one controlled tab.",
    ),
    variant(
        "browser_navigate",
        None,
        CapabilitySet::READ,
        "Retrieve a page in a controlled tab.",
    ),
    variant(
        "browser_history",
        None,
        CapabilitySet::ACTION,
        "Traverse or reload browser history.",
    ),
    variant(
        "browser_window",
        Some("zoom"),
        CapabilitySet::READ,
        "Change page zoom.",
    ),
    variant(
        "browser_window",
        Some("resize"),
        CapabilitySet::EMPTY,
        "Resize browser chrome.",
    ),
    variant(
        "browser_read",
        None,
        CapabilitySet::READ,
        "Read bounded page text.",
    ),
    variant(
        "browser_inspect",
        None,
        CapabilitySet::READ,
        "Inspect bounded page structure.",
    ),
    variant(
        "browser_find",
        None,
        CapabilitySet::READ,
        "Find semantic page targets.",
    ),
    variant(
        "browser_screenshot",
        None,
        CapabilitySet::READ,
        "Capture a bounded page image.",
    ),
    variant(
        "browser_click",
        None,
        CapabilitySet::ACTION,
        "Activate a page target.",
    ),
    variant(
        "browser_scroll",
        None,
        CapabilitySet::READ,
        "Scroll or reveal page content.",
    ),
    variant(
        "browser_hover",
        None,
        CapabilitySet::READ,
        "Hover over page content.",
    ),
    variant(
        "browser_fill_form",
        Some("fill"),
        READ_WRITE,
        "Read targets and fill declared fields.",
    ),
    variant(
        "browser_fill_form",
        Some("submit"),
        READ_WRITE_ACTION,
        "Read targets, fill fields, and submit.",
    ),
    variant(
        "browser_type_text",
        None,
        CapabilitySet::ACTION,
        "Type through browser input events.",
    ),
    variant(
        "browser_press_key",
        None,
        CapabilitySet::ACTION,
        "Send one keyboard action.",
    ),
    variant(
        "browser_drag",
        None,
        CapabilitySet::ACTION,
        "Perform one pointer drag.",
    ),
    variant(
        "browser_upload",
        None,
        CapabilitySet::WRITE,
        "Place declared files into a page input.",
    ),
    variant(
        "browser_execute",
        None,
        CapabilitySet::EXECUTE,
        "Evaluate explicit page JavaScript.",
    ),
    variant(
        "browser_wait",
        None,
        CapabilitySet::READ,
        "Observe one bounded page condition.",
    ),
    variant(
        "browser_sequence",
        None,
        CapabilitySet::EMPTY,
        "Compose independently governed steps.",
    ),
    variant(
        "browser_dialog",
        Some("status"),
        CapabilitySet::READ,
        "Inspect JavaScript dialog state.",
    ),
    variant(
        "browser_dialog",
        Some("resolve"),
        CapabilitySet::ACTION,
        "Accept, dismiss, or respond to a dialog.",
    ),
    variant(
        "browser_record",
        Some("start"),
        CapabilitySet::READ,
        "Start capturing a controlled page.",
    ),
    variant(
        "browser_record",
        Some("inspect"),
        CapabilitySet::EMPTY,
        "Inspect, stop, or discard volatile capture.",
    ),
    variant(
        "browser_record",
        Some("save_client"),
        CapabilitySet::READ,
        "Return or download a captured page recording.",
    ),
    variant(
        "browser_record",
        Some("save_target"),
        CapabilitySet::WRITE,
        "Place a recording into a page target.",
    ),
    variant(
        "browser_diagnose",
        None,
        CapabilitySet::READ,
        "Read bounded opt-in browser diagnostics.",
    ),
];

/// Return every reachable requirement variant for one advertised tool.
pub fn variants(tool: &str) -> impl Iterator<Item = &'static CapabilityVariant> + '_ {
    DIRECTORY.iter().filter(move |entry| entry.tool == tool)
}

/// Return the exact requirement set for one decoded operation.
#[must_use]
pub fn requirements(operation: &Operation) -> CapabilitySet {
    match operation {
        Operation::ListTabs(_) => CapabilitySet::READ,
        Operation::ActivateTab(_) => CapabilitySet::EMPTY,
        Operation::OpenPage(_) | Operation::NavigatePage(_) => CapabilitySet::READ,
        Operation::NavigateHistory(_) | Operation::ReloadPage(_) => CapabilitySet::ACTION,
        Operation::CloseTab(_) => CapabilitySet::ACTION,
        Operation::ReadPage(_)
        | Operation::InspectPage(_)
        | Operation::Find(_)
        | Operation::TakeScreenshot(_)
        | Operation::ScrollPage(_)
        | Operation::SetZoom(_)
        | Operation::Hover(_)
        | Operation::Wait(_)
        | Operation::Diagnose(_) => CapabilitySet::READ,
        Operation::ResizeWindow(_) => CapabilitySet::EMPTY,
        Operation::Click(_)
        | Operation::TypeText(_)
        | Operation::PressKey(_)
        | Operation::Drag(_) => CapabilitySet::ACTION,
        Operation::FillForm(value) if value.submit_target.is_some() => READ_WRITE_ACTION,
        Operation::FillForm(_) => READ_WRITE,
        Operation::UploadFiles(_) => CapabilitySet::WRITE,
        Operation::RunScript(_) => CapabilitySet::EXECUTE,
        Operation::RunSequence(_) => CapabilitySet::EMPTY,
        Operation::HandleDialog(value) if value.action == "status" => CapabilitySet::READ,
        Operation::HandleDialog(_) => CapabilitySet::ACTION,
        Operation::Record(value) if value.action == "start" => CapabilitySet::READ,
        Operation::Record(value) if value.action == "save" && value.target.is_some() => {
            CapabilitySet::WRITE
        }
        Operation::Record(value) if value.action == "save" => CapabilitySet::READ,
        Operation::Record(_) => CapabilitySet::EMPTY,
    }
}

/// Return the exact requirement set for one sequence step.
#[must_use]
pub fn sequence_step_requirements(step: &SequenceStep) -> CapabilitySet {
    match step {
        SequenceStep::Wait { .. } | SequenceStep::Scroll { .. } | SequenceStep::Hover { .. } => {
            CapabilitySet::READ
        }
        SequenceStep::Fill { .. } => READ_WRITE,
        SequenceStep::Click { .. }
        | SequenceStep::TypeText { .. }
        | SequenceStep::PressKey { .. } => CapabilitySet::ACTION,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::governance::CapabilitySet;
    use serde_json::json;

    use super::{
        requirements, sequence_step_requirements, variants, CapabilityVariant, SequenceStep,
        DIRECTORY,
    };

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
            ("browser_scroll", None) => json!({}),
            ("browser_hover", None) => json!({"target":"target_1"}),
            ("browser_fill_form", Some("fill")) => {
                json!({"fields":[{"target":"target_1","value":"Ada"}]})
            }
            ("browser_fill_form", Some("submit")) => {
                json!({"fields":[{"target":"target_1","value":"Ada"}],"submit_target":"target_2"})
            }
            ("browser_type_text", None) => json!({"target":"target_1","text":"Ada"}),
            ("browser_press_key", None) => json!({"key":"Enter"}),
            ("browser_drag", None) => {
                json!({"source_target":"target_1","destination_target":"target_2"})
            }
            ("browser_upload", None) => json!({"target":"target_1","paths":[upload_path]}),
            ("browser_execute", None) => json!({"script":"1+1"}),
            ("browser_wait", None) => json!({"condition":"load_ready"}),
            ("browser_sequence", None) => json!({"steps":[
                {"action":"wait","condition":"load_ready"},
                {"action":"hover","target":"target_1"}
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
            "browser_sequence",
            "browser_dialog",
            "browser_record",
            "browser_diagnose",
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
        assert_eq!(
            decoded(
                "browser_sequence",
                json!({"steps":[
                    {"action":"fill","target":"target_1","value":"Ada"},
                    {"action":"wait","condition":"load_ready"}
                ]})
            ),
            CapabilitySet::EMPTY
        );
        let sequence = crate::language::decode(
            "browser_sequence",
            json!({"steps":[
                {"action":"fill","target":"target_1","value":"Ada"},
                {"action":"wait","condition":"load_ready"}
            ]}),
        )
        .expect("valid sequence");
        let crate::language::Operation::RunSequence(sequence) = sequence else {
            panic!("sequence decoded as a sequence")
        };
        assert_eq!(
            sequence_step_requirements(&sequence.steps[0]),
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        );
    }

    #[test]
    fn every_directory_entry_matches_what_decode_actually_requires() {
        for entry in DIRECTORY {
            let CapabilityVariant {
                tool,
                variant,
                requirements: expected,
                ..
            } = *entry;
            let input = fixture(tool, variant);
            let operation = crate::language::decode(tool, input.clone())
                .unwrap_or_else(|error| panic!("tool={tool} variant={variant:?} input={input}: fixture must decode, got {error:?}"));
            assert_eq!(
                requirements(&operation),
                expected,
                "tool={tool} variant={variant:?}: DIRECTORY's advertised requirement diverged \
                 from what requirements() actually returns for the decoded operation"
            );
        }
    }

    #[test]
    fn sequence_steps_carry_the_same_requirements_as_their_standalone_tool() {
        // Every SequenceStep variant has exactly one standalone-tool equivalent in DIRECTORY.
        // A drift here means composing an action inside browser_sequence would be governed
        // differently than calling the same action directly -- exactly the kind of mismatch
        // that would let a sequence step slip past a policy the standalone tool honors.
        let pairs: &[(&str, Option<&str>, SequenceStep)] = &[
            (
                "browser_click",
                None,
                SequenceStep::Click {
                    target: "target_1".into(),
                    button: "primary".into(),
                    click_count: 1,
                },
            ),
            (
                "browser_fill_form",
                Some("fill"),
                SequenceStep::Fill {
                    target: "target_1".into(),
                    value: "Ada".into(),
                },
            ),
            (
                "browser_type_text",
                None,
                SequenceStep::TypeText {
                    target: "target_1".into(),
                    text: "Ada".into(),
                    clear_first: false,
                },
            ),
            (
                "browser_press_key",
                None,
                SequenceStep::PressKey {
                    key: "Enter".into(),
                    target: None,
                    modifiers: Vec::new(),
                },
            ),
            (
                "browser_scroll",
                None,
                SequenceStep::Scroll {
                    target: None,
                    direction: None,
                    amount: None,
                },
            ),
            (
                "browser_hover",
                None,
                SequenceStep::Hover {
                    target: "target_1".into(),
                },
            ),
            (
                "browser_wait",
                None,
                SequenceStep::Wait {
                    condition: "load_ready".into(),
                    value: None,
                    target: None,
                },
            ),
        ];
        for (tool, variant, step) in pairs {
            let expected = DIRECTORY
                .iter()
                .find(|entry| entry.tool == *tool && entry.variant == *variant)
                .unwrap_or_else(|| panic!("no DIRECTORY entry for tool={tool} variant={variant:?}"))
                .requirements;
            assert_eq!(
                sequence_step_requirements(step),
                expected,
                "tool={tool} variant={variant:?}: sequence step requirements diverged from its \
                 standalone tool's DIRECTORY entry"
            );
        }
    }
}
