//! Ghostlight's language: every word it accepts and every word it says.
//!
//! Model-facing: `catalog` advertises the tools, `operation` holds the typed call each decodes to,
//! `decode` turns a call's JSON into one or refuses it, and `outcome` says what a call did, with
//! `wait`, `choices`, `handle` and `bounds` holding the closed vocabularies and the limits they
//! share. Person-facing: `setup`, `deployment`, `workbench`, `readiness`, `policy` and
//! `environment` word the window, the installer, `doctor` and the policy surfaces. History and the
//! audit record speak through `history`, `composition`, `audit` and `audit_health`.

pub mod adapter;
pub mod agent_icon;
pub mod area;
pub mod audit;
pub mod audit_health;
mod bounds;
pub mod capability_map;
mod catalog;
pub mod channel_switch;
pub mod choices;
pub mod composition;
pub mod control;
pub mod coverage;
mod decode;
pub mod deployment;
pub mod environment;
pub mod field_echo;
pub mod field_problem;
pub mod handle;
pub mod history;
pub mod intake;
pub mod migration;
mod operation;
pub mod outcome;
pub mod page_refusal;
mod phrase;
pub mod policy;
pub mod presentation;
pub mod provenance;
pub mod readiness;
#[cfg(test)]
pub(crate) mod self_check;
pub(crate) mod settlement;
pub mod setup;
pub mod wait;
pub mod workbench;

pub use catalog::{catalog, catalog_for, CATALOG_TOOL_COUNT, EXPECTED_TOOL_NAMES};
pub use choices::{
    DiagnosticDetail, DiagnosticSource, DialogAction, FindScope, HistoryDirection, InspectScope,
    OnError, PointerButton, RecordAction, ScrollAmount, ScrollDirection,
};
pub use decode::{base64_decoded_len, decode};
pub(crate) use decode::{decode_flow_step, SCROLL_AMOUNT_DEFAULT, SCROLL_DIRECTION_DEFAULT};
use decode::{HISTORY_RELOAD, WHEEL_DIRECTIONS};
pub use handle::HandleKind;
pub use operation::*;
pub use wait::{PageCondition, WaitCondition};

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use bounds::{
    Range, CLICK_COUNT, COORDINATE_MAX, DEFAULT_BUTTON, DEFAULT_CLICK_COUNT,
    DEFAULT_DIAGNOSTIC_DETAIL, DEFAULT_DIAGNOSTIC_LIMIT, DEFAULT_DIAGNOSTIC_SOURCE,
    DEFAULT_FILL_TIMEOUT_MS, DEFAULT_FIND_RESULTS, DEFAULT_FIND_SCOPE, DEFAULT_INSPECT_ITEMS,
    DEFAULT_INSPECT_SCOPE, DEFAULT_KEY_REPEAT, DEFAULT_ON_ERROR, DEFAULT_READ_CHARS,
    DEFAULT_RECORD_TIMEOUT_MS, DEFAULT_SCRIPT_RESULT_CHARS, DIAGNOSTIC_LIMIT, DIALOG_TEXT_CHARS,
    DOCUMENT_DEPTH, DURATION_MS, FIELD_CHOICES, FIELD_VALUE_CHARS, FILE_NAME_CHARS, FILL_FIELDS,
    FIND_RESULTS, FLOW_STEP_ID_CHARS, HANDLE_CHARS, INLINE_FILE_LIMIT, INSPECT_ITEMS, KEY_REPEAT,
    KEY_STROKES, MAX_POSTCONDITION_VALUE_CHARS, MEDIA_TYPE_CHARS, READ_CHARS, SCRIPT_RESULT_CHARS,
    SEARCH_TEXT_CHARS, UPLOAD_PATHS, UPLOAD_PATH_CHARS, URL_MAX, WHEEL_TICKS, WINDOW_HEIGHT,
    WINDOW_WIDTH, ZOOM_PERCENT,
};
pub use bounds::{DEFAULT_DOCUMENT_DEPTH, MAX_INSPECT_ITEMS, SCRIPT_MAX_CHARS};
pub(crate) use bounds::{DEFAULT_TIMEOUT_MS, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS};

/// Model-facing instructions supplied to every protocol edge by the orchestrator.
///
/// A client that defers loading tool schemas chooses its first calls from this text alone, so it
/// teaches the whole working loop in six short sentences: open, read, collect handles, act, know how
/// long handles last, and trust what each result says about its effect. It also carries the guidance
/// every tool shares, `tab` and `timeout_ms`, so no tool repeats it (ADR-0198 decision 26).
pub const SERVER_INSTRUCTIONS: &str = "Ghostlight controls the user's visible Chromium browser. \
Open or reuse a tab with browser_navigate, or list this session's tabs with browser_tabs; omit tab when one tab is unambiguous, but when other agents may share the session, open your own tab with browser_navigate new_tab:true and pass that tab on every call. \
Read a page with browser_read, and get handles for its controls from browser_find or browser_inspect, which also show what each form control accepts. \
Act with those handles, for example with browser_click or browser_fill_form; timeout_ms bounds the whole call. \
Handles expire when the page navigates; a handle follows its control through a re-render the page can prove, and a stale handle's refusal says why. \
Every result states its effect and whether repeating it is safe.";
/// Model-facing instructions of a connection that offers no tools because another connection of
/// the same agent already does, such as Claude Code with both the plugin and the installer's entry
/// (ADR-0203, amendment of 2026-10-19). The tools come later only when that connection ends.
pub const STANDBY_INSTRUCTIONS: &str = "Ghostlight is already connected to this agent through another server entry, so this one offers no tools. \
Use the Ghostlight tools you already have; if that connection ends, this one offers them.";
/// Closed role vocabulary a semantic selector may filter on, and the enum the catalog advertises.
///
/// Every entry is a role the page asset's `roleFor` can produce for an observed control, and the
/// page-asset test `every semantic role is producible` builds one element per entry, so the
/// two lists move together.
const SEMANTIC_ROLES: &[&str] = &[
    "button",
    "link",
    "checkbox",
    "radio",
    "textbox",
    "searchbox",
    "combobox",
    "listbox",
    "slider",
    "spinbutton",
    "tab",
    "menuitem",
    "option",
];

const NAMED_KEYS: &[&str] = &[
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Space",
];

/// A model-language validation failure before work starts.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LanguageError {
    /// The tool name is not in the catalog.
    #[error("unknown tool `{0}`")]
    UnknownTool(String),
    /// The input is not a valid object for this tool.
    #[error("invalid input: {0}")]
    Invalid(String),
}

impl LanguageError {
    /// The bare expectation or problem, phrased for use inside a next-step sentence.
    #[must_use]
    pub fn guidance(&self) -> String {
        match self {
            Self::UnknownTool(name) => {
                format!("`{name}` is not a Ghostlight tool; pick one from the advertised catalog.")
            }
            Self::Invalid(message) => message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decode_flow_step;
    use serde_json::json;

    use super::{
        base64_decoded_len, catalog, decode, LanguageError, Operation, ReadMode, ReusePolicy,
        CATALOG_TOOL_COUNT, DEFAULT_FILL_TIMEOUT_MS, DEFAULT_RECORD_TIMEOUT_MS, DEFAULT_TIMEOUT_MS,
        EXPECTED_TOOL_NAMES, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, SCROLL_AMOUNT_DEFAULT,
        SCROLL_DIRECTION_DEFAULT, SERVER_INSTRUCTIONS,
    };

    /// Collect every `role` enum the catalog advertises, wherever a selector sits in a schema.
    fn role_enums(schema: &serde_json::Value, found: &mut Vec<serde_json::Value>) {
        match schema {
            serde_json::Value::Object(map) => {
                if let Some(role) = map.get("role").and_then(|role| role.get("enum")) {
                    found.push(role.clone());
                }
                map.values().for_each(|value| role_enums(value, found));
            }
            serde_json::Value::Array(items) => {
                items.iter().for_each(|item| role_enums(item, found))
            }
            _ => {}
        }
    }

    /// The role enum a tool advertises is the vocabulary the decoder accepts, and nothing else.
    #[test]
    fn every_advertised_selector_role_is_in_the_semantic_role_vocabulary() {
        let mut found = Vec::new();
        for tool in catalog() {
            role_enums(&tool.input_schema, &mut found);
        }
        assert!(!found.is_empty(), "no tool advertises a selector role");
        for advertised in found {
            assert_eq!(advertised, json!(super::SEMANTIC_ROLES));
        }
    }

    /// Every tool advertises `timeout_ms` with the default its decoder applies, and a call may
    /// carry any value in range, so a deadline refusal that suggests a longer `timeout_ms`
    /// always names a field the tool has.
    #[test]
    fn every_tool_accepts_timeout_ms_with_its_advertised_default() {
        for tool in catalog() {
            let advertised = &tool.input_schema["properties"]["timeout_ms"];
            assert_eq!(advertised["minimum"], MIN_TIMEOUT_MS, "{}", tool.name);
            assert_eq!(advertised["maximum"], MAX_TIMEOUT_MS, "{}", tool.name);
            let mut bare = tool.input_schema["examples"][0].clone();
            bare.as_object_mut().unwrap().remove("timeout_ms");
            let operation = decode(&tool.name, bare.clone())
                .unwrap_or_else(|error| panic!("{} example: {error}", tool.name));
            assert_eq!(
                advertised["default"],
                operation.timeout_ms(),
                "{} timeout_ms default drifted from its schema",
                tool.name
            );
            for ms in [MIN_TIMEOUT_MS, MAX_TIMEOUT_MS] {
                let mut input = bare.clone();
                input["timeout_ms"] = json!(ms);
                let operation = decode(&tool.name, input).unwrap_or_else(|error| {
                    panic!("{} rejects timeout_ms {ms}: {error}", tool.name)
                });
                assert_eq!(
                    operation.timeout_ms(),
                    ms,
                    "{} ignored timeout_ms",
                    tool.name
                );
            }
            for ms in [MIN_TIMEOUT_MS - 1, MAX_TIMEOUT_MS + 1] {
                let mut input = bare.clone();
                input["timeout_ms"] = json!(ms);
                assert!(
                    decode(&tool.name, input).is_err(),
                    "{} took {ms}",
                    tool.name
                );
            }
        }
    }

    /// A call may be told to raise `timeout_ms` exactly when a call with a longer value decodes:
    /// below the maximum, and never at it. Filling a form and recording start at the maximum, so
    /// their default calls cannot grow.
    #[test]
    fn timeout_can_grow_exactly_when_a_longer_call_decodes() {
        for tool in catalog() {
            let mut bare = tool.input_schema["examples"][0].clone();
            bare.as_object_mut().unwrap().remove("timeout_ms");
            let mut inputs = vec![bare.clone()];
            for ms in [
                MIN_TIMEOUT_MS,
                DEFAULT_TIMEOUT_MS,
                MAX_TIMEOUT_MS - 1,
                MAX_TIMEOUT_MS,
            ] {
                let mut input = bare.clone();
                input["timeout_ms"] = json!(ms);
                inputs.push(input);
            }
            for input in inputs {
                let Ok(operation) = decode(&tool.name, input.clone()) else {
                    continue;
                };
                let mut longer = input.clone();
                longer["timeout_ms"] = json!(operation.timeout_ms() + 1);
                assert_eq!(
                    operation.timeout_can_grow(),
                    decode(&tool.name, longer).is_ok(),
                    "{} with {input}",
                    tool.name
                );
            }
        }
        for (tool, default) in [
            ("browser_fill_form", DEFAULT_FILL_TIMEOUT_MS),
            ("browser_record", DEFAULT_RECORD_TIMEOUT_MS),
        ] {
            let mut input = catalog()
                .into_iter()
                .find(|candidate| candidate.name == tool)
                .unwrap()
                .input_schema["examples"][0]
                .clone();
            input.as_object_mut().unwrap().remove("timeout_ms");
            let operation = decode(tool, input).unwrap();
            assert_eq!(operation.timeout_ms(), default);
            assert!(!operation.timeout_can_grow());
        }
    }

    /// The instructions a deferred-loading client reads first stay short and name only tools
    /// the catalog actually offers.
    #[test]
    fn server_instructions_teach_the_loop_with_real_tools() {
        assert!(SERVER_INSTRUCTIONS.is_ascii());
        let sentences = SERVER_INSTRUCTIONS
            .split(". ")
            .filter(|sentence| !sentence.trim().is_empty())
            .count();
        assert!(sentences <= 6, "{sentences} sentences");
        for shared in [
            "omit tab when one tab is unambiguous",
            "when other agents may share the session, open your own tab with browser_navigate new_tab:true",
            "pass that tab on every call",
            "timeout_ms bounds",
        ] {
            assert!(
                SERVER_INSTRUCTIONS.contains(shared),
                "instructions lost the shared guidance \"{shared}\""
            );
        }
        let named: Vec<&str> = SERVER_INSTRUCTIONS
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .filter(|word| word.starts_with("browser_") || word.starts_with("policy_"))
            .collect();
        for tool in [
            "browser_navigate",
            "browser_read",
            "browser_find",
            "browser_inspect",
        ] {
            assert!(named.contains(&tool), "instructions never name {tool}");
        }
        for tool in named {
            assert!(EXPECTED_TOOL_NAMES.contains(&tool), "{tool} is not a tool");
        }
    }

    #[test]
    fn catalog_has_unique_exact_tools_and_typo_closed_schemas() {
        let catalog = catalog();
        assert_eq!(catalog.len(), CATALOG_TOOL_COUNT);
        let mut names: Vec<_> = catalog.iter().map(|tool| tool.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CATALOG_TOOL_COUNT);
        for tool in catalog {
            assert!(tool.input_schema.is_object());
            assert!(tool.annotations.is_some());
        }
    }

    #[test]
    fn short_flows_assign_ids_and_reject_ambiguous_or_obsolete_input() {
        let Operation::RunFlow(flow) = decode(
            "browser_flow",
            json!({"steps":[
                {"tool":"browser_tabs"},
                {"id":"named","tool":"browser_read","arguments":{}}
            ]}),
        )
        .unwrap() else {
            panic!("flow")
        };
        assert_eq!(flow.steps[0].id, "step_1");
        assert_eq!(flow.steps[0].arguments, json!({}));
        assert_eq!(flow.steps[1].id, "named");
        for steps in [
            json!([{"id":"","tool":"browser_tabs"}]),
            json!([{"tool":"browser_tabs"},{"id":"step_1","tool":"browser_tabs"}]),
            json!([{"tool":"browser_tabs","typo":true}]),
            json!([{"tool":"browser_read","arguments":{"restrict_hosts":["example.com"]}}]),
        ] {
            assert!(decode("browser_flow", json!({"steps":steps})).is_err());
        }
    }

    #[test]
    fn a_flow_tab_defaults_only_tab_scoped_children_and_preserves_explicit_choices() {
        let Operation::ReadPage(read) =
            decode_flow_step("browser_read", json!({}), Some("tab_batch")).unwrap()
        else {
            panic!("read")
        };
        assert_eq!(read.tab.as_deref(), Some("tab_batch"));
        let Operation::ReadPage(read) = decode_flow_step(
            "browser_read",
            json!({"tab":"tab_other"}),
            Some("tab_batch"),
        )
        .unwrap() else {
            panic!("read")
        };
        assert_eq!(read.tab.as_deref(), Some("tab_other"));
        for (tool, input) in [
            ("policy_explain", json!({})),
            ("browser_tabs", json!({"action":"list"})),
            ("browser_record", json!({"action":"status"})),
            (
                "browser_navigate",
                json!({"new_tab":true,"url":"https://example.com"}),
            ),
        ] {
            assert!(decode_flow_step(tool, input, Some("tab_batch")).is_ok());
        }
        let Operation::ActivateTab(focus) =
            decode_flow_step("browser_tabs", json!({"action":"focus"}), Some("tab_batch")).unwrap()
        else {
            panic!("focus")
        };
        assert_eq!(focus.tab, "tab_batch");
    }

    #[test]
    fn a_flow_tab_is_not_injected_into_a_step_whose_handle_names_its_tab() {
        let tab = Some("tab_batch");
        let Operation::Click(click) =
            decode_flow_step("browser_click", json!({"target":"target_other"}), tab).unwrap()
        else {
            panic!("click")
        };
        assert_eq!(click.tab, None, "a target says which tab it is in");
        let Operation::Click(click) = decode_flow_step(
            "browser_click",
            json!({"view":"view_other","x":1,"y":1}),
            tab,
        )
        .unwrap() else {
            panic!("click")
        };
        assert_eq!(click.tab, None, "a view says which tab it is in");
        let Operation::FillForm(fill) = decode_flow_step(
            "browser_fill_form",
            json!({"fields":[{"target":"target_other","value":"a"}]}),
            tab,
        )
        .unwrap() else {
            panic!("fill")
        };
        assert_eq!(fill.tab, None, "a field's target says which tab it is in");
        let Operation::Drag(drag) = decode_flow_step(
            "browser_drag",
            json!({"source_target":"target_one","destination_target":"target_two"}),
            tab,
        )
        .unwrap() else {
            panic!("drag")
        };
        assert_eq!(drag.tab, None, "drag targets say which tab they are in");
        // A step whose handles name no tab, a selector fill included, still works in the flow's tab.
        let Operation::FillForm(fill) = decode_flow_step(
            "browser_fill_form",
            json!({"fields":[{"selector":{"name":"Email"},"value":"a"}]}),
            tab,
        )
        .unwrap() else {
            panic!("fill")
        };
        assert_eq!(fill.tab.as_deref(), Some("tab_batch"));
        // A selector names no tab, so a fill that has one keeps the flow's tab even when its
        // submit_target or another field is a handle.
        for mixed in [
            json!({"fields":[{"selector":{"name":"Email"},"value":"a"}],"submit_target":"target_submit"}),
            json!({"fields":[
                {"selector":{"name":"Email"},"value":"a"},
                {"target":"target_other","value":"b"}
            ]}),
        ] {
            let Operation::FillForm(fill) =
                decode_flow_step("browser_fill_form", mixed.clone(), tab).unwrap()
            else {
                panic!("fill")
            };
            assert_eq!(fill.tab.as_deref(), Some("tab_batch"), "{mixed}");
        }
        let Operation::TypeText(typed) = decode_flow_step(
            "browser_type_text",
            json!({"focused":true,"text":"target_lookalike"}),
            tab,
        )
        .unwrap() else {
            panic!("type")
        };
        assert_eq!(
            typed.tab.as_deref(),
            Some("tab_batch"),
            "typed text is not a handle"
        );
    }

    #[test]
    fn shortest_calls_receive_executable_defaults() {
        let Operation::NavigatePage(navigate) =
            decode("browser_navigate", json!({"url":"https://example.com"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(navigate.timeout_ms, 8_000);
        assert_eq!(navigate.reuse, ReusePolicy::Domain);
        let Operation::FillForm(fill) = decode(
            "browser_fill_form",
            json!({"fields":[{"target":"target_example","value":"Ada"}]}),
        )
        .unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(fill.timeout_ms, 30_000);
        let Operation::ReadPage(read) = decode("browser_read", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(read.max_chars, 8_000);
        assert_eq!(read.mode, None);
        let Operation::ReadPage(article) =
            decode("browser_read", json!({"mode":"article"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(article.mode, Some(ReadMode::Article));
        let Operation::InspectPage(inspect) = decode("browser_inspect", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(inspect.scope, crate::language::InspectScope::Controls);
        assert_eq!(inspect.max_items, 80);
    }

    #[test]
    fn navigate_reuse_follows_the_documented_ladder() {
        let reuse_of = |input: serde_json::Value| match decode("browser_navigate", input).unwrap() {
            Operation::NavigatePage(value) => value.reuse,
            Operation::OpenPage(value) => value.reuse,
            other => panic!("wrong operation {other:?}"),
        };
        assert_eq!(
            reuse_of(json!({"url":"https://example.com","reuse":"never"})),
            ReusePolicy::Never
        );
        assert_eq!(
            reuse_of(json!({"url":"https://example.com","new_tab":true})),
            ReusePolicy::Never
        );
        assert!(matches!(
            decode(
                "browser_navigate",
                json!({"url":"https://example.com","reuse":"sometimes"})
            ),
            Err(LanguageError::Invalid(_))
        ));
        // A client that fills in the advertised default sends reuse "domain" beside new_tab.
        assert_eq!(
            reuse_of(json!({"url":"https://example.com","new_tab":true,"reuse":"domain"})),
            ReusePolicy::Never
        );
    }

    #[test]
    fn execute_replaces_the_unreleased_evaluate_name_without_an_alias() {
        let Operation::RunScript(script) =
            decode("browser_execute", json!({"script":"document.title"})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(script.script, "document.title");
        assert!(matches!(
            decode("browser_evaluate", json!({"script":"document.title"})),
            Err(LanguageError::UnknownTool(name)) if name == "browser_evaluate"
        ));
    }

    #[test]
    fn unknown_fields_and_ambiguous_waits_fail() {
        let error = decode("browser_read", json!({"max_chars":8000,"max_char":1})).unwrap_err();
        assert!(matches!(error, LanguageError::Invalid(message) if message.contains("max_char")));
        assert!(decode("browser_wait", json!({"condition":"text_present"})).is_err());
        assert!(decode(
            "browser_wait",
            json!({"condition":"target_present","value":"x"})
        )
        .is_err());
    }

    #[test]
    fn validation_messages_teach_the_expected_shape() {
        let error = decode("browser_tabs", json!({"action":"nope"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid input: action must be one of list, focus, close: got `nope`"
        );
        let error = decode("browser_window", json!({"action":"zoom","percent":900})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid input: percent must be between 25 and 500; got 900"
        );
        let error = decode("browser_tabs", json!({"action":"close","tab":"nope"})).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid input: handles start with tab_; pass one exactly as a previous result issued it"
        );
        let error = decode(
            "browser_press_key",
            json!({"key":"a","tab":"tab_x","oops":1}),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .starts_with("invalid input: unknown field `oops`:"));
        let error = decode("browser_press_key", json!({"key":"PageLeft"})).unwrap_err();
        assert!(error
            .to_string()
            .starts_with("invalid input: key must be one character or a supported named key: "));
    }

    #[test]
    fn screenshot_target_and_full_page_are_mutually_exclusive() {
        assert!(decode(
            "browser_screenshot",
            json!({"target":"target_x","full_page":true})
        )
        .is_err());
    }

    #[test]
    fn invalid_input_guidance_teaches_the_expectation_without_the_diagnostic_prefix() {
        let error = decode(
            "browser_screenshot",
            json!({"tab":"tab_x","x":0,"y":0,"width":300,"height":200}),
        )
        .unwrap_err();
        assert_eq!(
            error.guidance(),
            "region work needs a current view handle: take a screenshot, then pass its view with these coordinates"
        );
        assert!(error.to_string().starts_with("invalid input: "));
        assert_eq!(
            LanguageError::UnknownTool("browser_evaluate".into()).guidance(),
            "`browser_evaluate` is not a Ghostlight tool; pick one from the advertised catalog."
        );
    }

    #[test]
    fn screenshot_region_uses_one_complete_current_view_rectangle() {
        let Operation::TakeScreenshot(capture) = decode(
            "browser_screenshot",
            json!({"view":"view_x","x":10.5,"y":20.25,"width":300,"height":200}),
        )
        .unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(capture.view.as_deref(), Some("view_x"));
        assert_eq!(capture.width, Some(300.0));
        for invalid in [
            json!({"view":"view_x","x":0,"y":0,"width":300}),
            json!({"view":"view_x","x":0,"y":0,"width":0,"height":200}),
            json!({"view":"view_x","x":0,"y":0,"width":300,"height":200,"full_page":true}),
            json!({"view":"view_x","x":0,"y":0,"width":300,"height":200,"target":"target_x"}),
        ] {
            assert!(decode("browser_screenshot", invalid).is_err());
        }
    }

    #[test]
    fn pointer_locations_are_exactly_semantic_or_view_bound() {
        assert!(decode(
            "browser_click",
            json!({"target":"target_x","view":"view_x","x":1,"y":1})
        )
        .is_err());
        assert!(decode("browser_click", json!({"x":1,"y":1})).is_err());
        let Operation::Click(click) =
            decode("browser_click", json!({"view":"view_x","x":10.5,"y":20.25})).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(click.view.as_deref(), Some("view_x"));
    }

    #[test]
    fn scroll_defaults_are_contextual_and_upload_paths_are_absolute() {
        let Operation::ScrollPage(scroll) = decode("browser_scroll", json!({})).unwrap() else {
            panic!("wrong operation")
        };
        assert!(scroll.direction.is_none());
        assert!(scroll.amount.is_none());
        // The advertised defaults beside a target are what a defaults-filling client sends.
        assert!(decode(
            "browser_scroll",
            json!({"target":"target_x","direction":"down","amount":"medium"})
        )
        .is_ok());
        for page in [json!({"direction":"up"}), json!({"amount":"page"})] {
            let mut call = json!({"target":"target_x"});
            call.as_object_mut()
                .unwrap()
                .extend(page.as_object().unwrap().clone());
            assert!(decode("browser_scroll", call).is_err());
        }
        assert!(decode(
            "browser_upload",
            json!({"target":"target_x","paths":["relative.txt"]})
        )
        .is_err());
    }

    #[test]
    fn base64_decoded_len_counts_exactly() {
        use base64::Engine as _;
        for length in 0..=12 {
            let encoded = base64::engine::general_purpose::STANDARD.encode(vec![7u8; length]);
            assert_eq!(base64_decoded_len(&encoded), length as u64, "{encoded}");
        }
    }

    #[test]
    fn inline_uploads_are_bounded_at_exactly_five_million_bytes() {
        let upload = |data_base64: String| {
            decode(
                "browser_upload",
                json!({"target":"target_x","files":[{"name":"a.bin","data_base64":data_base64}]}),
            )
        };
        // 5,000,000 bytes: 1,666,666 full groups and a last group of two bytes, "AAA=".
        let at_ceiling = format!("{}=", "A".repeat(6_666_667));
        assert_eq!(base64_decoded_len(&at_ceiling), 5_000_000);
        assert!(upload(at_ceiling).is_ok());
        let over = "A".repeat(6_666_668);
        assert_eq!(base64_decoded_len(&over), 5_000_001);
        assert!(upload(over).is_err());
    }

    #[test]
    fn a_wheel_scroll_takes_the_advertised_direction_and_checks_its_amount() {
        let wheel = json!({"view":"view_x","x":10,"y":20,"ticks":2});
        let Operation::ScrollPage(scroll) = decode("browser_scroll", wheel.clone()).unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(scroll.direction, Some(SCROLL_DIRECTION_DEFAULT));
        let with = |field: &str, value: &str| {
            let mut call = wheel.clone();
            call[field] = json!(value);
            decode("browser_scroll", call)
        };
        let Operation::ScrollPage(scroll) = with("direction", "up").unwrap() else {
            panic!("wrong operation")
        };
        assert_eq!(scroll.direction, Some(crate::language::ScrollDirection::Up));
        assert!(with("direction", "left").is_err());
        // The advertised amount default is what a defaults-filling client sends beside ticks.
        assert!(with("amount", SCROLL_AMOUNT_DEFAULT.as_str()).is_ok());
        for amount in ["page", "small", "huge", ""] {
            assert!(with("amount", amount).is_err(), "{amount}");
        }
        let Operation::ScrollPage(step) =
            decode_flow_step("browser_scroll", wheel, Some("tab_x")).unwrap()
        else {
            panic!("wrong operation")
        };
        assert_eq!(step.direction, Some(SCROLL_DIRECTION_DEFAULT));
    }

    #[test]
    fn action_families_reject_impossible_shapes() {
        assert!(decode("browser_tabs", json!({"action":"list","tab":"tab_x"})).is_err());
        assert!(decode(
            "browser_history",
            json!({"action":"back","bypass_cache":true})
        )
        .is_err());
        assert!(decode(
            "browser_history",
            json!({"action":"back","bypass_cache":false})
        )
        .is_ok());
        assert!(decode(
            "browser_window",
            json!({"action":"zoom","width":800,"height":600})
        )
        .is_err());
        assert!(decode("browser_dialog", json!({"action":"accept","text":"yes"})).is_err());
        assert!(decode(
            "browser_record",
            json!({"action":"stop","target":"target_x"})
        )
        .is_err());
    }

    #[test]
    fn flows_validate_steps_and_references_before_dispatch() {
        let ok = decode(
            "browser_flow",
            json!({"steps":[
                {"id":"list","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"read","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"list","pointer":"/facts/tab"}}}}
            ]}),
        );
        assert!(ok.is_ok(), "backward references are accepted: {ok:?}");
        for invalid in [
            json!({"steps":[
                {"id":"a","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"b","pointer":"/x"}}}},
                {"id":"b","tool":"browser_tabs","arguments":{"action":"list"}}
            ]}),
            json!({"steps":[
                {"id":"a","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"ghost","pointer":"/x"}}}}
            ]}),
            json!({"steps":[{"id":"a","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"a","pointer":"no-slash"}}}}]}),
            json!({"steps":[{"id":"a","tool":"browser_flow","arguments":{}}]}),
            json!({"steps":[
                {"id":"dup","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"dup","tool":"browser_read","arguments":{}}
            ]}),
            json!({"steps":[
                {"id":"a","tool":"browser_read","arguments":{"restrict_hosts":["example.com"]}}
            ]}),
        ] {
            assert!(
                decode("browser_flow", invalid.clone()).is_err(),
                "expected rejection: {invalid}"
            );
        }
    }
}
