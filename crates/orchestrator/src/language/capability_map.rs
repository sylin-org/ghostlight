//! One authoritative RAWX requirement directory for the current Ghostlight language.

use crate::governance::CapabilitySet;

use super::{DialogAction, Operation, RecordAction, Tool};

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
    Tool::Tabs.name(),
    Some("list"),
    CapabilitySet::READ,
    "List this workspace's bound tabs, read live from the browser.",
);

const TABS_FOCUS: CapabilityVariant = variant(
    Tool::Tabs.name(),
    Some("focus"),
    CapabilitySet::EMPTY,
    "Focus one controlled tab.",
);

const TABS_CLOSE: CapabilityVariant = variant(
    Tool::Tabs.name(),
    Some("close"),
    CapabilitySet::ACTION,
    "Close one controlled tab.",
);

const NAVIGATE: CapabilityVariant = variant(
    Tool::Navigate.name(),
    None,
    CapabilitySet::READ,
    "Retrieve a page in a controlled tab.",
);

const HISTORY: CapabilityVariant = variant(
    Tool::History.name(),
    None,
    CapabilitySet::ACTION,
    "Traverse or reload browser history.",
);

const WINDOW_ZOOM: CapabilityVariant = variant(
    Tool::Window.name(),
    Some("zoom"),
    CapabilitySet::READ,
    "Change page zoom.",
);

const WINDOW_RESIZE: CapabilityVariant = variant(
    Tool::Window.name(),
    Some("resize"),
    CapabilitySet::EMPTY,
    "Resize browser chrome.",
);

const READ: CapabilityVariant = variant(
    Tool::Read.name(),
    None,
    CapabilitySet::READ,
    "Read bounded page text.",
);

const INSPECT: CapabilityVariant = variant(
    Tool::Inspect.name(),
    None,
    CapabilitySet::READ,
    "Inspect bounded page structure.",
);

const FIND: CapabilityVariant = variant(
    Tool::Find.name(),
    None,
    CapabilitySet::READ,
    "Find semantic page targets.",
);

const SCREENSHOT: CapabilityVariant = variant(
    Tool::Screenshot.name(),
    None,
    CapabilitySet::READ,
    "Capture a bounded page image.",
);

const CLICK: CapabilityVariant = variant(
    Tool::Click.name(),
    None,
    CapabilitySet::ACTION,
    "Activate a page target.",
);

const CLICK_SELECTOR: CapabilityVariant = variant(
    Tool::Click.name(),
    Some("selector"),
    READ_ACTION,
    "Find a named control and activate it.",
);

const CLICK_EXPECT: CapabilityVariant = variant(
    Tool::Click.name(),
    Some("expect"),
    READ_ACTION,
    "Activate a page target and observe the requested postcondition.",
);

const SCROLL: CapabilityVariant = variant(
    Tool::Scroll.name(),
    None,
    CapabilitySet::READ,
    "Scroll or reveal page content.",
);

const HOVER: CapabilityVariant = variant(
    Tool::Hover.name(),
    None,
    CapabilitySet::READ,
    "Hover over page content.",
);

const FILL: CapabilityVariant = variant(
    Tool::FillForm.name(),
    Some("fill"),
    READ_WRITE,
    "Read targets and fill declared fields.",
);

const FILL_SUBMIT: CapabilityVariant = variant(
    Tool::FillForm.name(),
    Some("submit"),
    READ_WRITE_ACTION,
    "Read targets, fill fields, and submit.",
);

const TYPE: CapabilityVariant = variant(
    Tool::TypeText.name(),
    None,
    CapabilitySet::ACTION,
    "Type through browser input events.",
);

const TYPE_SELECTOR: CapabilityVariant = variant(
    Tool::TypeText.name(),
    Some("selector"),
    READ_ACTION,
    "Find a named control and type through browser input events.",
);

const TYPE_EXPECT: CapabilityVariant = variant(
    Tool::TypeText.name(),
    Some("expect"),
    READ_ACTION,
    "Type through browser input events and observe the requested postcondition.",
);

const PRESS_KEY: CapabilityVariant = variant(
    Tool::PressKey.name(),
    None,
    CapabilitySet::ACTION,
    "Send one keyboard action.",
);

const PRESS_KEY_EXPECT: CapabilityVariant = variant(
    Tool::PressKey.name(),
    Some("expect"),
    READ_ACTION,
    "Send one keyboard action and observe the requested postcondition.",
);

const DRAG: CapabilityVariant = variant(
    Tool::Drag.name(),
    None,
    CapabilitySet::ACTION,
    "Perform one pointer drag.",
);

const UPLOAD: CapabilityVariant = variant(
    Tool::Upload.name(),
    None,
    CapabilitySet::WRITE,
    "Place declared files into a page input.",
);

const UPLOAD_SELECTOR: CapabilityVariant = variant(
    Tool::Upload.name(),
    Some("selector"),
    READ_WRITE,
    "Find a named file input and place declared files into it.",
);

const EXECUTE: CapabilityVariant = variant(
    Tool::Execute.name(),
    None,
    CapabilitySet::EXECUTE,
    "Evaluate explicit page JavaScript.",
);

const WAIT: CapabilityVariant = variant(
    Tool::Wait.name(),
    None,
    CapabilitySet::READ,
    "Observe one bounded page condition.",
);

const FLOW: CapabilityVariant = variant(
    Tool::Flow.name(),
    None,
    CapabilitySet::EMPTY,
    "Compose result-aware governed steps; each child authorizes normally.",
);

const DIALOG_STATUS: CapabilityVariant = variant(
    Tool::Dialog.name(),
    Some("status"),
    CapabilitySet::READ,
    "Inspect JavaScript dialog state.",
);

const DIALOG_RESOLVE: CapabilityVariant = variant(
    Tool::Dialog.name(),
    Some("resolve"),
    CapabilitySet::ACTION,
    "Accept, dismiss, or respond to a dialog.",
);

const RECORD_START: CapabilityVariant = variant(
    Tool::Record.name(),
    Some("start"),
    CapabilitySet::READ,
    "Start capturing a controlled page.",
);

const RECORD_INSPECT: CapabilityVariant = variant(
    Tool::Record.name(),
    Some("inspect"),
    CapabilitySet::EMPTY,
    "Inspect, stop, or discard volatile capture.",
);

const RECORD_SAVE_CLIENT: CapabilityVariant = variant(
    Tool::Record.name(),
    Some("save_client"),
    CapabilitySet::READ,
    "Return or download a captured page recording.",
);

const RECORD_SAVE_TARGET: CapabilityVariant = variant(
    Tool::Record.name(),
    Some("save_target"),
    READ_WRITE,
    "Read captured sources and place a recording into a page target.",
);

const DIAGNOSE: CapabilityVariant = variant(
    Tool::Diagnose.name(),
    None,
    CapabilitySet::READ,
    "Read bounded opt-in browser diagnostics.",
);

const POLICY_EXPLAIN: CapabilityVariant = variant(
    Tool::PolicyExplain.name(),
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
        Operation::HandleDialog(value) if value.action == DialogAction::Status => &DIALOG_STATUS,
        Operation::HandleDialog(_) => &DIALOG_RESOLVE,
        Operation::Record(value) if value.action == RecordAction::Start => &RECORD_START,
        Operation::Record(value)
            if value.action == RecordAction::Save && value.target.is_some() =>
        {
            &RECORD_SAVE_TARGET
        }
        Operation::Record(value) if value.action == RecordAction::Save => &RECORD_SAVE_CLIENT,
        Operation::Record(_) => &RECORD_INSPECT,
        Operation::Diagnose(_) => &DIAGNOSE,
        Operation::ExplainPolicy(_) => &POLICY_EXPLAIN,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::governance::CapabilitySet;
    use serde_json::{json, Value};

    use super::{entry, requirements, Tool, DIRECTORY};
    use crate::language::{decode, Operation};

    const NONE: CapabilitySet = CapabilitySet::EMPTY;
    const R: CapabilitySet = CapabilitySet::READ;
    const A: CapabilitySet = CapabilitySet::ACTION;
    const W: CapabilitySet = CapabilitySet::WRITE;
    const X: CapabilitySet = CapabilitySet::EXECUTE;
    const RA: CapabilitySet = R.union(A);
    const RW: CapabilitySet = R.union(W);
    const RWA: CapabilitySet = RW.union(A);

    /// ACCEPTANCE Governance 3 and LANGUAGE.md's capability line for every tool: each decoded call
    /// reaches its own directory entry and requires exactly the RAWX set LANGUAGE.md documents
    /// (written here from the document, not copied from `DIRECTORY`); the rows reach every entry
    /// once and every catalog tool; and a flow requires nothing while its child classifies on its
    /// own.
    #[test]
    fn each_call_requires_the_rawx_set_language_md_documents() {
        let upload_path = if cfg!(windows) {
            r"C:\ghostlight\upload.txt"
        } else {
            "/tmp/ghostlight/upload.txt"
        };
        let fill = json!([{"target":"target_1","value":"Ada"}]);
        let rows: Vec<(&str, Option<&str>, CapabilitySet, Vec<Value>)> = vec![
            (
                "browser_tabs",
                Some("list"),
                R,
                vec![json!({"action":"list"})],
            ),
            (
                "browser_tabs",
                Some("focus"),
                NONE,
                vec![json!({"action":"focus","tab":"tab_1"})],
            ),
            (
                "browser_tabs",
                Some("close"),
                A,
                vec![json!({"action":"close","tab":"tab_1"})],
            ),
            (
                "browser_navigate",
                None,
                R,
                vec![
                    json!({"url":"https://example.com"}),
                    json!({"url":"https://example.com","new_tab":true}),
                ],
            ),
            (
                "browser_history",
                None,
                A,
                vec![json!({"action":"back"}), json!({"action":"reload"})],
            ),
            (
                "browser_window",
                Some("zoom"),
                R,
                vec![json!({"action":"zoom","percent":100})],
            ),
            (
                "browser_window",
                Some("resize"),
                NONE,
                vec![json!({"action":"resize","width":1280,"height":720})],
            ),
            ("browser_read", None, R, vec![json!({})]),
            ("browser_inspect", None, R, vec![json!({})]),
            ("browser_find", None, R, vec![json!({"text":"Login"})]),
            ("browser_screenshot", None, R, vec![json!({})]),
            ("browser_click", None, A, vec![json!({"target":"target_1"})]),
            (
                "browser_click",
                Some("selector"),
                RA,
                vec![json!({"selector":{"name":"Save"}})],
            ),
            (
                "browser_click",
                Some("expect"),
                RA,
                vec![json!({"target":"target_1","expect":{"condition":"load_ready"}})],
            ),
            ("browser_scroll", None, R, vec![json!({})]),
            ("browser_hover", None, R, vec![json!({"target":"target_1"})]),
            (
                "browser_fill_form",
                Some("fill"),
                RW,
                vec![json!({"fields":fill})],
            ),
            (
                "browser_fill_form",
                Some("submit"),
                RWA,
                vec![json!({"fields":fill,"submit_target":"target_2"})],
            ),
            (
                "browser_type_text",
                None,
                A,
                vec![json!({"target":"target_1","text":"Ada"})],
            ),
            (
                "browser_type_text",
                Some("selector"),
                RA,
                vec![json!({"selector":{"name":"Draft"},"text":"Ada"})],
            ),
            (
                "browser_type_text",
                Some("expect"),
                RA,
                vec![json!({"focused":true,"text":"Ada","expect":{"condition":"load_ready"}})],
            ),
            ("browser_press_key", None, A, vec![json!({"key":"Enter"})]),
            (
                "browser_press_key",
                Some("expect"),
                RA,
                vec![json!({"key":"Enter","expect":{"condition":"load_ready"}})],
            ),
            (
                "browser_drag",
                None,
                A,
                vec![json!({"source_target":"target_1","destination_target":"target_2"})],
            ),
            (
                "browser_upload",
                None,
                W,
                vec![json!({"target":"target_1","paths":[upload_path]})],
            ),
            (
                "browser_upload",
                Some("selector"),
                RW,
                vec![json!({"selector":{"name":"Attachment"},"paths":[upload_path]})],
            ),
            ("browser_execute", None, X, vec![json!({"script":"1+1"})]),
            (
                "browser_wait",
                None,
                R,
                vec![json!({"condition":"load_ready"})],
            ),
            (
                "browser_flow",
                None,
                NONE,
                vec![json!({"steps":[
                    {"tool":"browser_fill_form","arguments":{"fields":fill}},
                    {"tool":"browser_wait","arguments":{"condition":"load_ready"}}
                ]})],
            ),
            (
                "browser_dialog",
                Some("status"),
                R,
                vec![json!({"action":"status"})],
            ),
            (
                "browser_dialog",
                Some("resolve"),
                A,
                vec![
                    json!({"action":"accept"}),
                    json!({"action":"dismiss"}),
                    json!({"action":"respond","text":"Ada"}),
                ],
            ),
            (
                "browser_record",
                Some("start"),
                R,
                vec![json!({"action":"start"})],
            ),
            (
                "browser_record",
                Some("inspect"),
                NONE,
                vec![
                    json!({"action":"status"}),
                    json!({"action":"stop"}),
                    json!({"action":"discard"}),
                ],
            ),
            (
                "browser_record",
                Some("save_client"),
                R,
                vec![json!({"action":"save"})],
            ),
            (
                "browser_record",
                Some("save_target"),
                RW,
                vec![json!({"action":"save","target":"target_1"})],
            ),
            ("browser_diagnose", None, R, vec![json!({})]),
            ("policy_explain", None, NONE, vec![json!({})]),
        ];

        let mut reached = Vec::new();
        for (tool, variant, expected, calls) in &rows {
            for call in calls {
                let operation = decode(tool, call.clone())
                    .unwrap_or_else(|error| panic!("{tool} {call}: {error}"));
                assert_eq!(requirements(&operation), *expected, "{tool} {call}");
                let reached_entry = entry(&operation);
                assert_eq!(
                    (reached_entry.tool, reached_entry.variant),
                    (*tool, *variant),
                    "{tool} {call} reaches another entry"
                );
            }
            reached.push((*tool, *variant));
        }
        let listed: Vec<_> = DIRECTORY
            .iter()
            .map(|entry| (entry.tool, entry.variant))
            .collect();
        assert_eq!(reached, listed, "the rows reach every entry once, in order");
        let tools: BTreeSet<&str> = listed.iter().map(|(tool, _)| *tool).collect();
        let catalog: BTreeSet<&str> = Tool::ALL.into_iter().map(Tool::name).collect();
        assert_eq!(tools, catalog);

        let (_, _, _, flow_calls) = rows
            .iter()
            .find(|(tool, ..)| *tool == Tool::Flow.name())
            .expect("a row calls browser_flow");
        let flow = decode(Tool::Flow.name(), flow_calls[0].clone()).expect("the flow decodes");
        let Operation::RunFlow(flow) = flow else {
            panic!("browser_flow decodes to a flow")
        };
        let child = decode(&flow.steps[0].tool, flow.steps[0].arguments.clone())
            .expect("the flow's fill step decodes");
        assert_eq!(requirements(&child), RW);
    }
}
