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
pub mod tool;
pub mod wait;
pub mod workbench;

pub use catalog::{catalog, catalog_for};
pub use choices::{
    DiagnosticDetail, DiagnosticSource, DialogAction, FindScope, HistoryDirection, InspectScope,
    OnError, PointerButton, RecordAction, ScrollAmount, ScrollDirection,
};
pub use decode::{base64_decoded_len, decode};
pub(crate) use decode::{decode_flow_step, SCROLL_AMOUNT_DEFAULT, SCROLL_DIRECTION_DEFAULT};
use decode::{HISTORY_RELOAD, WHEEL_DIRECTIONS};
pub use handle::HandleKind;
pub use operation::*;
pub use tool::Tool;
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
    SEARCH_TEXT_CHARS, UPLOAD_AGGREGATE_BYTES, UPLOAD_PATHS, UPLOAD_PATH_CHARS, URL_MAX,
    WHEEL_TICKS, WINDOW_HEIGHT, WINDOW_WIDTH, ZOOM_PERCENT,
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
    use base64::Engine as _;
    use serde_json::{json, Value};

    use super::{
        base64_decoded_len, bounds::UPLOAD_AGGREGATE_BYTES, catalog, decode, decode_flow_step,
        LanguageError, Operation, ScrollAmount, ScrollDirection, SCROLL_AMOUNT_DEFAULT,
        SCROLL_DIRECTION_DEFAULT, WHEEL_DIRECTIONS,
    };

    /// What every refusal of a retired tool or argument tells a caller holding an old catalog
    /// (LANGUAGE.md: "Obsolete calls fail before browser work with catalog-refresh guidance").
    const CATALOG_REFRESH: &str = "Refresh the tool catalog";

    /// The tab a decoded flow child works in, for the children the flow-tab table reads; none for
    /// a child that works in no tab of its own.
    fn tab_of(operation: &Operation) -> Option<&str> {
        match operation {
            Operation::ReadPage(read) => read.tab.as_deref(),
            Operation::ActivateTab(focus) => Some(&focus.tab),
            Operation::Click(click) => click.tab.as_deref(),
            Operation::FillForm(fill) => fill.tab.as_deref(),
            Operation::Drag(drag) => drag.tab.as_deref(),
            Operation::TypeText(typed) => typed.tab.as_deref(),
            Operation::Record(record) => record.tab.as_deref(),
            Operation::ListTabs(_) | Operation::OpenPage(_) | Operation::ExplainPolicy(_) => None,
            other => panic!("the flow-tab table reads no tab of {other:?}"),
        }
    }

    /// ACCEPTANCE Consolidated inputs 2: a flow is decoded whole before any step runs. Step ids
    /// default to `step_N`, explicit ids are kept, and a backward result reference decodes; an
    /// empty or duplicate id, a child typo, a forward or unknown reference, a pointer without a
    /// leading slash and a nested flow are refused.
    #[test]
    fn a_flow_is_validated_whole_before_dispatch() {
        let Operation::RunFlow(flow) = decode(
            "browser_flow",
            json!({"steps":[
                {"tool":"browser_tabs"},
                {"id":"named","tool":"browser_read","arguments":{}}
            ]}),
        )
        .expect("a short flow decodes") else {
            panic!("browser_flow decodes to a flow")
        };
        assert_eq!(flow.steps[0].id, "step_1");
        assert_eq!(flow.steps[0].arguments, json!({}));
        assert_eq!(flow.steps[1].id, "named");

        let backward = decode(
            "browser_flow",
            json!({"steps":[
                {"id":"list","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"read","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"list","pointer":"/facts/tab"}}}}
            ]}),
        );
        assert!(
            backward.is_ok(),
            "a backward reference decodes: {backward:?}"
        );

        let reference = |step: &str, pointer: &str| json!({"max_chars":{"flow_ref":{"step":step,"pointer":pointer}}});
        for steps in [
            json!([{"id":"","tool":"browser_tabs"}]),
            json!([{"tool":"browser_tabs"},{"id":"step_1","tool":"browser_tabs"}]),
            json!([
                {"id":"dup","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"dup","tool":"browser_read","arguments":{}}
            ]),
            json!([{"tool":"browser_tabs","typo":true}]),
            json!([
                {"id":"a","tool":"browser_read","arguments":reference("b", "/x")},
                {"id":"b","tool":"browser_tabs","arguments":{"action":"list"}}
            ]),
            json!([{"id":"a","tool":"browser_read","arguments":reference("ghost", "/x")}]),
            json!([{"id":"a","tool":"browser_read","arguments":reference("a", "no-slash")}]),
            json!([{"id":"a","tool":"browser_flow","arguments":{}}]),
        ] {
            assert!(
                decode("browser_flow", json!({"steps":steps.clone()})).is_err(),
                "a flow with these steps decodes: {steps}"
            );
        }
    }

    /// ACCEPTANCE Consolidated inputs 3 and e2d6b413 (ADR-0203 5.3): a flow's tab reaches a
    /// tab-scoped child that names no tab, an explicit child tab wins, a target or view handle
    /// keeps the tab it names, a selector names no tab so a fill that holds one keeps the flow's
    /// tab even beside a handle, typed text is never a handle, and a child that works in no tab
    /// decodes without one.
    #[test]
    fn a_flow_tab_reaches_exactly_the_children_that_name_no_tab() {
        let flow_tab = "tab_batch";
        let rows: [(&str, Value, Option<&str>); 15] = [
            ("browser_read", json!({}), Some(flow_tab)),
            (
                "browser_read",
                json!({"tab":"tab_other"}),
                Some("tab_other"),
            ),
            ("browser_tabs", json!({"action":"focus"}), Some(flow_tab)),
            ("browser_click", json!({"target":"target_other"}), None),
            (
                "browser_click",
                json!({"view":"view_other","x":1,"y":1}),
                None,
            ),
            (
                "browser_fill_form",
                json!({"fields":[{"target":"target_other","value":"a"}]}),
                None,
            ),
            (
                "browser_drag",
                json!({"source_target":"target_one","destination_target":"target_two"}),
                None,
            ),
            (
                "browser_fill_form",
                json!({"fields":[{"selector":{"name":"Email"},"value":"a"}]}),
                Some(flow_tab),
            ),
            (
                "browser_fill_form",
                json!({"fields":[{"selector":{"name":"Email"},"value":"a"}],"submit_target":"target_submit"}),
                Some(flow_tab),
            ),
            (
                "browser_fill_form",
                json!({"fields":[
                    {"selector":{"name":"Email"},"value":"a"},
                    {"target":"target_other","value":"b"}
                ]}),
                Some(flow_tab),
            ),
            (
                "browser_type_text",
                json!({"focused":true,"text":"target_lookalike"}),
                Some(flow_tab),
            ),
            ("policy_explain", json!({}), None),
            ("browser_tabs", json!({"action":"list"}), None),
            ("browser_record", json!({"action":"status"}), None),
            (
                "browser_navigate",
                json!({"new_tab":true,"url":"https://example.com"}),
                None,
            ),
        ];
        for (tool, arguments, expected) in rows {
            let operation = decode_flow_step(tool, arguments.clone(), Some(flow_tab))
                .unwrap_or_else(|error| panic!("{tool} {arguments}: {error}"));
            assert_eq!(tab_of(&operation), expected, "{tool} {arguments}");
        }
    }

    /// Reproduces L-D5 (b5135a14): `base64_decoded_len` counts the bytes standard base64 decodes
    /// to exactly, so inline files of exactly `UPLOAD_AGGREGATE_BYTES` are accepted and one byte
    /// more is refused.
    #[test]
    fn an_inline_upload_of_exactly_the_ceiling_is_accepted_and_one_byte_more_is_refused() {
        let encode =
            |bytes: usize| base64::engine::general_purpose::STANDARD.encode(vec![7u8; bytes]);
        for length in 0..=12 {
            let encoded = encode(length);
            assert_eq!(base64_decoded_len(&encoded), length as u64, "{encoded}");
        }
        let upload = |bytes: u64| {
            let data = encode(usize::try_from(bytes).expect("the ceiling fits in memory"));
            decode(
                "browser_upload",
                json!({"target":"target_x","files":[{"name":"a.bin","data_base64":data}]}),
            )
        };
        assert!(upload(UPLOAD_AGGREGATE_BYTES).is_ok());
        assert!(upload(UPLOAD_AGGREGATE_BYTES + 1).is_err());
    }

    /// Reproduces L-D1 (59c67621): a wheel scroll that leaves out its direction gets
    /// `SCROLL_DIRECTION_DEFAULT`, directly and inside a flow; it takes only `WHEEL_DIRECTIONS`,
    /// and its refusal lists them; it accepts the advertised `SCROLL_AMOUNT_DEFAULT` a
    /// defaults-filling client sends beside its ticks, and refuses every other amount.
    #[test]
    fn a_wheel_scroll_takes_the_advertised_direction_and_checks_its_amount() {
        let wheel = json!({"view":"view_x","x":10,"y":20,"ticks":2});
        let direction_of = |operation: Operation| match operation {
            Operation::ScrollPage(scroll) => scroll.direction,
            other => panic!("a scroll decodes to {other:?}"),
        };
        let with = |field: &str, value: &str| {
            let mut call = wheel.clone();
            call[field] = json!(value);
            decode("browser_scroll", call)
        };
        assert_eq!(
            direction_of(decode("browser_scroll", wheel.clone()).expect("a wheel scroll decodes")),
            Some(SCROLL_DIRECTION_DEFAULT)
        );
        assert_eq!(
            direction_of(
                decode_flow_step("browser_scroll", wheel.clone(), Some("tab_x"))
                    .expect("a wheel step decodes")
            ),
            Some(SCROLL_DIRECTION_DEFAULT)
        );
        for direction in WHEEL_DIRECTIONS {
            assert_eq!(
                direction_of(with("direction", direction).expect("a wheel direction decodes")),
                ScrollDirection::named(direction).ok()
            );
        }
        let sideways = ScrollDirection::Left.as_str();
        assert_eq!(
            with("direction", sideways).map(|_| ()),
            Err(LanguageError::Invalid(format!(
                "direction must be one of {}: got `{sideways}`",
                WHEEL_DIRECTIONS.join(", ")
            )))
        );
        assert!(with("amount", SCROLL_AMOUNT_DEFAULT.as_str()).is_ok());
        let other_amounts = ScrollAmount::WORDS
            .iter()
            .copied()
            .filter(|amount| *amount != SCROLL_AMOUNT_DEFAULT.as_str());
        for amount in other_amounts.chain(["huge", ""]) {
            assert!(with("amount", amount).is_err(), "{amount}");
        }
    }

    /// ACCEPTANCE Contract 1 and Consolidated inputs 1, and LANGUAGE.md: `browser_evaluate`,
    /// `browser_sequence` and `browser_workspace` are not advertised; `browser_evaluate` is an
    /// unknown tool with no alias; the retired tools and the retired arguments `restrict_hosts`,
    /// `restrict_capabilities` and `browser_flow.dry_run` are neither advertised nor decoded, and
    /// a call that sends one, directly or as a flow step, is refused with catalog-refresh guidance.
    #[test]
    fn the_retired_surface_is_neither_advertised_nor_decoded() {
        let retired_tools = ["browser_sequence", "browser_workspace"];
        let retired_arguments = ["restrict_hosts", "restrict_capabilities"];
        let tools = catalog();
        for tool in &tools {
            assert!(
                !retired_tools.contains(&tool.name.as_str()) && tool.name != "browser_evaluate",
                "{} is advertised",
                tool.name
            );
            let schema = serde_json::to_string(&tool.input_schema).expect("a schema encodes");
            for argument in retired_arguments {
                assert!(
                    !schema.contains(argument),
                    "{} advertises {argument}",
                    tool.name
                );
            }
        }
        let flow = tools
            .iter()
            .find(|tool| tool.name == "browser_flow")
            .expect("browser_flow is in the catalog");
        assert!(flow.input_schema["properties"].get("dry_run").is_none());

        let evaluate = decode("browser_evaluate", json!({"script":"document.title"}))
            .expect_err("browser_evaluate is not a tool");
        assert_eq!(
            evaluate,
            LanguageError::UnknownTool("browser_evaluate".into())
        );
        assert!(evaluate.guidance().contains("`browser_evaluate`"));

        let mut obsolete: Vec<(&str, Value)> = retired_tools
            .into_iter()
            .map(|tool| (tool, json!({})))
            .collect();
        for argument in retired_arguments {
            obsolete.push(("browser_read", json!({ argument: ["example.com"] })));
            obsolete.push((
                "browser_flow",
                json!({"steps":[{"tool":"browser_read","arguments":{ argument: ["example.com"] }}]}),
            ));
        }
        obsolete.push((
            "browser_flow",
            json!({"steps":[{"tool":"browser_read"}],"dry_run":true}),
        ));
        for (tool, call) in obsolete {
            let refusal = decode(tool, call.clone()).expect_err("an obsolete call is refused");
            assert!(
                refusal.guidance().contains(CATALOG_REFRESH),
                "{tool} {call}: {refusal}"
            );
        }
    }
}
