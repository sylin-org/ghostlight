//! The deterministic model-facing tool catalog and its truthful JSON Schemas.

use std::collections::BTreeMap;

use crate::intake::session::{ToolAnnotations, ToolDefinition};
use serde_json::{json, Map, Value};

use crate::governance::AuthoritySnapshot;

use super::bounds::{
    Range, CLICK_COUNT, COORDINATE_MAX, DEFAULT_BUTTON, DEFAULT_CLICK_COUNT,
    DEFAULT_DIAGNOSTIC_DETAIL, DEFAULT_DIAGNOSTIC_LIMIT, DEFAULT_DIAGNOSTIC_SOURCE,
    DEFAULT_FIND_RESULTS, DEFAULT_FIND_SCOPE, DEFAULT_INSPECT_ITEMS, DEFAULT_INSPECT_SCOPE,
    DEFAULT_KEY_REPEAT, DEFAULT_ON_ERROR, DEFAULT_READ_CHARS, DEFAULT_SCRIPT_RESULT_CHARS,
    DIAGNOSTIC_LIMIT, DIALOG_TEXT_CHARS, DOCUMENT_DEPTH, DURATION_MS, FIELD_CHOICES,
    FIELD_VALUE_CHARS, FILE_NAME_CHARS, FILL_FIELDS, FIND_RESULTS, FLOW_STEP_ID_CHARS,
    HANDLE_CHARS, INLINE_FILE_LIMIT, INSPECT_ITEMS, KEY_REPEAT, KEY_STROKES, MEDIA_TYPE_CHARS,
    READ_CHARS, SCRIPT_RESULT_CHARS, SEARCH_TEXT_CHARS, UPLOAD_PATHS, UPLOAD_PATH_CHARS, URL_MAX,
    WHEEL_TICKS, WINDOW_HEIGHT, WINDOW_WIDTH, ZOOM_PERCENT,
};
use super::choices::{
    DiagnosticDetail, DiagnosticSource, DialogAction, FindScope, HistoryDirection, InspectScope,
    OnError, PointerButton, RecordAction, ScrollAmount, ScrollDirection,
};
use super::handle::HandleKind;
use super::tool::Tool;
use super::wait::{PageCondition, WaitCondition};
use super::{
    DEFAULT_DOCUMENT_DEPTH, MAX_POSTCONDITION_VALUE_CHARS, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS,
    NAMED_KEYS, SCRIPT_MAX_CHARS, SCROLL_AMOUNT_DEFAULT, SCROLL_DIRECTION_DEFAULT, SEMANTIC_ROLES,
};
use super::{HISTORY_RELOAD, WHEEL_DIRECTIONS};

/// Return the complete native Ghostlight language in deterministic order.
#[must_use]
pub fn catalog() -> Vec<ToolDefinition> {
    vec![
        tool(
            Tool::Tabs,
            "List, focus, or close this session's tabs. List gives fresh handles from the live browser; with no tab yet it needs none and names those connected.",
            tabs_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Navigate,
            "Go to an HTTP(S) URL. Reuses the obvious tab; new_tab true opens a new one.",
            navigate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::History,
            "Go back or forward, or reload. A new document invalidates target_ and view_ handles, so collect fresh ones.",
            history_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Window,
            "Set a tab's zoom or resize its browser window. Either invalidates view_ handles, so take a fresh screenshot before coordinate work.",
            window_schema(),
            Hints::local_mutation(false),
        ),
        tool(
            Tool::Read,
            "Read visible page text, including open shadow roots and frames, or one target. To act, get handles from browser_inspect or browser_find.",
            read_schema(),
            Hints::browser_read(),
        ),
        tool(
            Tool::Inspect,
            "Inspect controls or page structure for fresh target_ handles. Scope document returns the outline as a bounded tree, without handles. Form controls also carry their value and what they accept: options, type, limits, hint and group.",
            inspect_schema(),
            Hints::browser_read(),
        ),
        tool(
            Tool::Find,
            "Find targets by visible or accessible text and get fresh target_ handles. Fastest when you know the label. Option text finds its dropdown.",
            find_schema(),
            Hints::browser_read(),
        ),
        tool(
            Tool::Screenshot,
            "Capture the viewport, full page, one target, or a magnified region; returns a view_ handle for coordinate calls.",
            screenshot_schema(),
            Hints::browser_read(),
        ),
        tool(
            Tool::Click,
            "Click a target_ handle, a control found by selector, or a point in a view_ screenshot. Needs action; a selector lookup or expect check also needs read. Prefer a target.",
            with_expect(click_schema()),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Scroll,
            "Scroll the page (default down, medium), reveal a target_, or wheel-scroll at a point in a view_ screenshot.",
            scroll_schema(),
            Hints::browser_action(false),
        ),
        tool(
            Tool::Hover,
            "Hover a target_ handle or a point in a view_ screenshot.",
            hover_schema(),
            Hints::browser_action(false),
        ),
        tool(
            Tool::FillForm,
            &format!(
                "Fill {} to {} fields in one call; needs write even for unsent drafts. Nothing submits unless submit_target names the control. A value a control cannot take is refused before any field changes, with what it accepts. For credentials, set user_authorized_credentials only on the user's explicit instruction.",
                FILL_FIELDS.min, FILL_FIELDS.max
            ),
            with_expect(fill_schema()),
            Hints::browser_action(true),
        ),
        tool(
            Tool::TypeText,
            "Type through real keyboard input. Needs action; a selector lookup or expect check also needs read. Prefer browser_fill_form for plain values. For credentials, set user_authorized_credentials only on the user's explicit instruction.",
            with_expect(type_schema()),
            Hints::browser_action(true),
        ),
        tool(
            Tool::PressKey,
            "Send a named key or literal character, optionally at a target_. Needs action; an expect check also needs read. For Control+A send key a with modifiers Control; strokes sends keys in order.",
            with_expect(key_schema()),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Drag,
            "Drag between two target_ handles or two points in one view_ screenshot.",
            drag_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Wait,
            "Wait for one observable page condition. A false condition is a failed result, not an unknown effect.",
            wait_schema(),
            Hints::browser_read(),
        ),
        tool(
            Tool::Dialog,
            "Read the open JavaScript dialog with status, then accept, dismiss, or answer it; it blocks its page.",
            dialog_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Upload,
            "Attach local paths, inline files, or one captured image to a file input, or drop the image at a point in a view_. Needs write; a selector lookup also needs read.",
            upload_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Execute,
            "Run JavaScript in the page main world. It may read, mutate, or navigate; prefer a semantic tool. A script that is one expression is awaited, so a promise it evaluates to resolves.",
            evaluate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Flow,
            &format!(
                "Run 1 to {} tool calls. tab defaults tab-scoped steps; a step's own tab wins. Later arguments may reference earlier results. Stops at the first failed step by default.",
                super::composition::COMPOSITION_STEP_LIMIT
            ),
            flow_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Record,
            "Record browser work as a short memory-only GIF. One recording per session; save stops an active capture.",
            record_schema(),
            Hints::browser_action(true),
        ),
        tool(
            Tool::Diagnose,
            "Read opt-in console and network observations for a tab. Observation begins with the first call, so reload or reproduce to capture earlier activity. Sensitive request data is never collected.",
            diagnose_schema(),
            Hints::browser_read(),
        ),
        tool(
            Tool::PolicyExplain,
            "Explain the authority in force: one line per capability with its deciding layer, the rules behind them, permanent ceilings, and each layer's document. Available under every authority; use it to learn why a call was refused or what is allowed.",
            explain_policy_schema(),
            Hints::local_read(),
        ),
    ]
}

/// Project the canonical catalog through one immutable authority snapshot.
///
/// A tool is listed when some host admits what one of its variants needs, and the screenshot
/// tool only while the snapshot allows screenshots (ADR-0203 Decision 7.1).
#[must_use]
pub fn catalog_for(snapshot: &AuthoritySnapshot) -> Vec<ToolDefinition> {
    let screenshots = snapshot.authorize_screenshot().allowed;
    catalog()
        .into_iter()
        .filter(|tool| {
            // With screenshots off every screenshot call would be refused, and no other tool
            // takes one.
            (screenshots || tool.name != Tool::Screenshot.name())
                && super::capability_map::variants(&tool.name)
                    .any(|variant| snapshot.could_admit(variant.requirements))
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Hints {
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
}

impl Hints {
    const fn browser_read() -> Self {
        Self {
            read_only: true,
            destructive: false,
            idempotent: true,
            open_world: true,
        }
    }

    /// A read that never touches a browser or the network at all (ADR-0136).
    const fn local_read() -> Self {
        Self {
            read_only: true,
            destructive: false,
            idempotent: true,
            open_world: false,
        }
    }

    const fn browser_action(destructive: bool) -> Self {
        Self {
            read_only: false,
            destructive,
            idempotent: false,
            open_world: true,
        }
    }

    const fn local_mutation(destructive: bool) -> Self {
        Self {
            read_only: false,
            destructive,
            idempotent: false,
            open_world: false,
        }
    }
}

fn tool(tool: Tool, description: &str, input_schema: Value, hints: Hints) -> ToolDefinition {
    let mut input_schema = input_schema;
    input_schema["properties"]["timeout_ms"] = timeout(tool);
    if tool.settles() {
        // Thirteen tools carry it undescribed: the name, type and default say what it is, and
        // LANGUAGE.md carries the budget and the busy-page behavior (ADR-0195).
        input_schema["properties"]["visual_settle"] = boolean(true, "");
    }
    ToolDefinition {
        name: tool.name().into(),
        description: description.into(),
        input_schema,
        annotations: Some(ToolAnnotations {
            title: Some(tool.title().into()),
            read_only_hint: Some(hints.read_only),
            destructive_hint: Some(hints.destructive),
            idempotent_hint: Some(hints.idempotent),
            open_world_hint: Some(hints.open_world),
        }),
    }
}

fn tabs_schema() -> Value {
    union(
        vec![
            object(
                vec![("action", constant("list", "List this session's tabs."))],
                vec!["action"],
            ),
            object(
                vec![
                    ("action", constant("focus", "Bring the tab to the front.")),
                    ("tab", handle(HandleKind::Tab, "From browser_tabs list.")),
                ],
                vec!["action", "tab"],
            ),
            object(
                vec![
                    ("action", constant("close", "Close the tab.")),
                    ("tab", handle(HandleKind::Tab, "From browser_tabs list.")),
                ],
                vec!["action", "tab"],
            ),
        ],
        vec![
            json!({"action":"list"}),
            json!({"action":"focus","tab":"tab_..."}),
        ],
    )
}

fn navigate_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    ("url", url()),
                    ("tab", tab()),
                    (
                        "beforeunload",
                        enumeration(
                            &["discard"],
                            None,
                            "Discard an unsaved-changes prompt this navigation causes. Default stops and reports it.",
                        ),
                    ),
                    ("new_tab", new_tab()),
                    (
                        "reuse",
                        enumeration(
                            &["domain", "never"],
                            Some("domain"),
                            "domain reuses a tab an earlier session of this client released for this host, never one the person opened. never always creates. new_tab ignores it.",
                        ),
                    ),
                ],
                vec!["url"],
            ),
            object(
                vec![
                    ("url", url()),
                    ("new_tab", new_tab()),
                    (
                        "browser",
                        handle(
                            HandleKind::Browser,
                            "Only with new_tab true. From browser_tabs list. Omit to use the browser the user attended last.",
                        ),
                    ),
                ],
                vec!["url", "new_tab"],
            ),
        ],
        vec![
            json!({"url":"https://example.com"}),
            json!({"url":"https://example.com","new_tab":true}),
        ],
    )
}

fn new_tab() -> Value {
    boolean(false, "True opens a new tab.")
}

fn history_schema() -> Value {
    union(
        vec![
            history_branch(HistoryDirection::Back.as_str(), "Go back one page."),
            history_branch(HistoryDirection::Forward.as_str(), "Go forward one page."),
            object(
                vec![
                    ("action", constant(HISTORY_RELOAD, "Reload the page.")),
                    ("tab", tab()),
                    ("bypass_cache", boolean(false, "")),
                ],
                vec!["action"],
            ),
        ],
        vec![
            json!({"action":"back"}),
            json!({"action":"reload","bypass_cache":true}),
        ],
    )
}

fn history_branch(action: &str, description: &str) -> Value {
    object(
        vec![("action", constant(action, description)), ("tab", tab())],
        vec!["action"],
    )
}

fn window_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    ("action", constant("zoom", "Set tab zoom.")),
                    ("percent", within(ZOOM_PERCENT, None, "")),
                    ("tab", tab()),
                ],
                vec!["action", "percent"],
            ),
            object(
                vec![
                    (
                        "action",
                        constant("resize", "Set the window size in pixels."),
                    ),
                    ("width", within(WINDOW_WIDTH, None, "")),
                    ("height", within(WINDOW_HEIGHT, None, "")),
                    ("tab", tab()),
                ],
                vec!["action", "width", "height"],
            ),
        ],
        vec![
            json!({"action":"zoom","percent":200}),
            json!({"action":"resize","width":800,"height":600}),
        ],
    )
}

fn read_schema() -> Value {
    examples(
        object(
            vec![
                ("tab", tab()),
                (
                    "target",
                    handle(HandleKind::Target, ""),
                ),
                (
                    "mode",
                    enumeration(
                        &["visible", "article"],
                        None,
                        "article prefers the top document's article, else the full page. Ignored with target.",
                    ),
                ),
                (
                    "max_chars",
                    within(READ_CHARS, Some(DEFAULT_READ_CHARS), ""),
                ),
            ],
            vec![],
        ),
        vec![json!({}), json!({"mode":"article","max_chars":50_000})],
    )
}

fn inspect_schema() -> Value {
    examples(
        object(
            vec![
                ("tab", tab()),
                (
                    "scope",
                    enumeration(
                        InspectScope::WORDS,
                        Some(DEFAULT_INSPECT_SCOPE.as_str()),
                        "document: a {kind, label, children} tree, a snapshot handle, and a diff against the previous snapshot when root and max_depth match.",
                    ),
                ),
                (
                    "root",
                    handle(HandleKind::Target, "Subtree root for document scope."),
                ),
                (
                    "max_depth",
                    // No schema default: the decoder refuses max_depth outside document scope.
                    within(
                        DOCUMENT_DEPTH,
                        None,
                        &format!("Tree levels; {DEFAULT_DOCUMENT_DEPTH} when omitted."),
                    ),
                ),
                (
                    "max_items",
                    within(INSPECT_ITEMS, Some(DEFAULT_INSPECT_ITEMS), ""),
                ),
            ],
            vec![],
        ),
        vec![json!({}), json!({"scope":"document","max_depth":8})],
    )
}

fn find_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "text",
                    text(1, SEARCH_TEXT_CHARS, "Visible or accessible text."),
                ),
                ("tab", tab()),
                (
                    "scope",
                    enumeration(FindScope::WORDS, Some(DEFAULT_FIND_SCOPE.as_str()), ""),
                ),
                (
                    "max_results",
                    within(FIND_RESULTS, Some(DEFAULT_FIND_RESULTS), ""),
                ),
            ],
            vec!["text"],
        ),
        vec![json!({"text":"Submit"})],
    )
}

fn screenshot_schema() -> Value {
    union(
        vec![
            object(vec![("tab", tab())], vec![]),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "full_page",
                        boolean(
                            false,
                            "True captures the whole document. Not with target or a region.",
                        ),
                    ),
                ],
                vec!["full_page"],
            ),
            object(
                vec![("tab", tab()), ("target", handle(HandleKind::Target, ""))],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "view",
                        handle(
                            HandleKind::View,
                            "The region is in this view's image pixels; take a screenshot first.",
                        ),
                    ),
                    ("x", coordinate()),
                    ("y", coordinate()),
                    ("width", extent()),
                    ("height", extent()),
                ],
                vec!["view", "x", "y", "width", "height"],
            ),
        ],
        vec![
            json!({}),
            json!({"full_page":true}),
            json!({"view":"view_...","x":120,"y":80,"width":400,"height":300}),
        ],
    )
}

fn click_schema() -> Value {
    let common = vec![
        ("tab", tab()),
        (
            "button",
            enumeration(PointerButton::WORDS, Some(DEFAULT_BUTTON.as_str()), ""),
        ),
        (
            "click_count",
            within(CLICK_COUNT, Some(usize::from(DEFAULT_CLICK_COUNT)), ""),
        ),
        ("modifiers", modifier_keys()),
        ("selector", semantic_selector()),
    ];
    union(
        vec![
            object(
                with(common.clone(), ("target", handle(HandleKind::Target, ""))),
                vec!["target"],
            ),
            object(
                with_many(
                    common,
                    vec![
                        ("view", handle(HandleKind::View, VIEW_POINT)),
                        ("x", coordinate()),
                        ("y", coordinate()),
                    ],
                ),
                vec!["view", "x", "y"],
            ),
        ],
        vec![
            json!({"target":"target_..."}),
            json!({"view":"view_...","x":120,"y":80}),
        ],
    )
}

fn semantic_selector() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "Lookup needs read.",
        "properties": {
            "name": {"type": "string", "minLength": 1, "maxLength": SEARCH_TEXT_CHARS, "description": "Accessible name, case-insensitive substring unless exact."},
            "role": {"type": "string", "enum": SEMANTIC_ROLES},
            "exact": {"type": "boolean", "default": false}
        },
        "required": ["name"]
    })
}

fn scroll_schema() -> Value {
    union(
        vec![
            object(
                vec![("tab", tab()), ("target", handle(HandleKind::Target, ""))],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "direction",
                        enumeration(
                            ScrollDirection::WORDS,
                            Some(SCROLL_DIRECTION_DEFAULT.as_str()),
                            SCROLL_DIRECTION,
                        ),
                    ),
                    (
                        "amount",
                        enumeration(
                            ScrollAmount::WORDS,
                            Some(SCROLL_AMOUNT_DEFAULT.as_str()),
                            "",
                        ),
                    ),
                ],
                vec![],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "direction",
                        enumeration(
                            WHEEL_DIRECTIONS,
                            Some(SCROLL_DIRECTION_DEFAULT.as_str()),
                            SCROLL_DIRECTION,
                        ),
                    ),
                    ("view", handle(HandleKind::View, VIEW_POINT)),
                    ("x", coordinate()),
                    ("y", coordinate()),
                    ("ticks", within(WHEEL_TICKS, None, "")),
                ],
                vec!["view", "x", "y", "ticks"],
            ),
        ],
        vec![
            json!({}),
            json!({"direction":"down","amount":"page"}),
            json!({"view":"view_...","x":120,"y":80,"direction":"down","ticks":3}),
        ],
    )
}

/// One direction description for page and wheel scrolling, so the merged field reads once.
const SCROLL_DIRECTION: &str = "Wheel scrolling at a view point takes only up or down.";

fn hover_schema() -> Value {
    union(
        vec![
            object(
                vec![("tab", tab()), ("target", handle(HandleKind::Target, ""))],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    ("view", handle(HandleKind::View, VIEW_POINT)),
                    ("x", coordinate()),
                    ("y", coordinate()),
                ],
                vec!["view", "x", "y"],
            ),
        ],
        vec![json!({"target":"target_..."})],
    )
}

fn credential_authorization() -> (&'static str, Value) {
    (
        "user_authorized_credentials",
        boolean(false, "True only when the user explicitly authorized credential entry for this request. Never overrides policy or the person's Pause or End session."),
    )
}

fn fill_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "fields",
                    json!({"type":"array","minItems":FILL_FIELDS.min,"maxItems":FILL_FIELDS.max,"description":"Each field gives exactly one of target or selector.","items":{"type":"object","additionalProperties":false,"properties":{"target":handle(HandleKind::Target,""),"selector":semantic_selector(),"value":{"description":"Text replaces the current value; empty clears it. Select: an option's text or value as browser_inspect lists it; multi-select: an array. Checkbox: true or false (sets, never toggles). Radio: true selects it, or a sibling's label selects that one. Date and time: ISO, like 2026-10-14, 14:30, 2026-10-14T14:30. Number and range: within the control's limits.","anyOf":[{"type":"string","maxLength":FIELD_VALUE_CHARS},{"type":"boolean"},{"type":"number"},{"type":"array","items":{"type":"string","maxLength":FIELD_VALUE_CHARS},"maxItems":FIELD_CHOICES}]}},"required":["value"]}}),
                ),
                ("tab", tab()),
                (
                    "submit_target",
                    handle(
                        HandleKind::Target,
                        "Clicked after filling; may cause an external effect.",
                    ),
                ),
                credential_authorization(),
            ],
            vec!["fields"],
        ),
        vec![json!({"fields":[{"target":"target_...","value":"Ada"}]})],
    )
}

fn type_schema() -> Value {
    // Every location shares one text and one clear_first field. Empty text is legal only as an
    // explicit clear; that pairing is a cross-field rule the decoder enforces and the
    // description teaches, because one portable property cannot express it.
    let common = vec![
        (
            "text",
            text(
                0,
                FIELD_VALUE_CHARS,
                "Empty text only clears, and needs clear_first true.",
            ),
        ),
        ("clear_first", boolean(false, "")),
        credential_authorization(),
        ("tab", tab()),
    ];
    union(
        vec![
            object(
                with(common.clone(), ("target", handle(HandleKind::Target, ""))),
                vec!["target", "text"],
            ),
            object(
                with(
                    common.clone(),
                    (
                        "focused",
                        boolean(
                            false,
                            "True types into the focused control instead of target.",
                        ),
                    ),
                ),
                vec!["focused", "text"],
            ),
            object(
                with(common, ("selector", semantic_selector())),
                vec!["selector", "text"],
            ),
        ],
        vec![
            json!({"target":"target_...","text":"hello"}),
            json!({"focused":true,"text":"hello"}),
        ],
    )
}

fn key_schema() -> Value {
    let modifiers = ("modifiers", modifier_keys());
    let shared = vec![
        ("tab", tab()),
        ("target", handle(HandleKind::Target, "")),
        modifiers,
    ];
    // Exclusive by construction: every named key is longer than one character.
    let stroke_item = json!({"oneOf":[{"type":"string","minLength":1,"maxLength":1},{"type":"string","enum":NAMED_KEYS}]});
    union(
        vec![
            object(
                with(shared.clone(), ("key", stroke_item.clone())),
                vec!["key"],
            ),
            object(
                with_many(
                    shared,
                    vec![
                        (
                            "strokes",
                            json!({"type":"array","minItems":KEY_STROKES.min,"maxItems":KEY_STROKES.max,"description":"Replaces key.","items":stroke_item}),
                        ),
                        (
                            "repeat",
                            within(
                                KEY_REPEAT,
                                Some(usize::from(DEFAULT_KEY_REPEAT)),
                                "Repeats the whole sequence.",
                            ),
                        ),
                    ],
                ),
                vec!["strokes"],
            ),
        ],
        vec![
            json!({"key":"Enter"}),
            json!({"key":"a","modifiers":["Control"]}),
            json!({"strokes":["ArrowDown"],"repeat":5}),
        ],
    )
}

fn drag_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    ("source_target", handle(HandleKind::Target, "")),
                    (
                        "destination_target",
                        handle(HandleKind::Target, ""),
                    ),
                    ("tab", tab()),
                ],
                vec!["source_target", "destination_target"],
            ),
            object(
                vec![
                    (
                        "view",
                        handle(HandleKind::View, "Both points are pixels in this view's screenshot image; take one first."),
                    ),
                    ("start_x", coordinate()),
                    ("start_y", coordinate()),
                    ("end_x", coordinate()),
                    ("end_y", coordinate()),
                    ("tab", tab()),
                ],
                vec!["view", "start_x", "start_y", "end_x", "end_y"],
            ),
        ],
        vec![json!({"source_target":"target_...","destination_target":"target_..."})],
    )
}

fn wait_schema() -> Value {
    union(
        WaitCondition::ALL.into_iter().map(wait_branch).collect(),
        vec![
            json!({"condition":"load_ready"}),
            json!({"condition":"text_present","value":"Ready"}),
        ],
    )
}

/// The one condition-specific input a wait branch takes besides its shared fields.
#[derive(Clone, Copy)]
enum WaitInput {
    /// Required text, URL fragment, or duration.
    Text,
    /// Required current target.
    Target,
    /// Optional current target.
    OptionalTarget,
    /// Required typed semantic selector.
    Selector,
}

/// What a condition's branch says it waits for, and the one input it takes besides the shared
/// fields.
fn wait_condition(condition: WaitCondition) -> (&'static str, Option<WaitInput>) {
    use WaitInput::{OptionalTarget, Selector, Target, Text};
    match condition {
        WaitCondition::LoadReady => ("Page finished loading.", None),
        WaitCondition::UrlContains => ("URL contains value.", Some(Text)),
        WaitCondition::TextPresent => ("value is in the visible text.", Some(Text)),
        WaitCondition::TextAbsent => ("value is gone from the visible text.", Some(Text)),
        WaitCondition::TargetPresent => ("target exists.", Some(Target)),
        WaitCondition::TargetAbsent => ("target is gone.", Some(Target)),
        WaitCondition::VisualSettle => (
            "The page, or target, stopped moving and repainting.",
            Some(OptionalTarget),
        ),
        WaitCondition::SelectorPresent => ("selector matches a control.", Some(Selector)),
        WaitCondition::Duration => ("Waits value milliseconds.", Some(Text)),
    }
}

/// The `visual_settle` description: which conditions settle by default, read from the type.
fn wait_settle_description() -> String {
    let off: Vec<&str> = WaitCondition::ALL
        .into_iter()
        .filter(|condition| {
            *condition != WaitCondition::VisualSettle && !condition.settles_by_default()
        })
        .map(WaitCondition::as_str)
        .collect();
    let named = match off.as_slice() {
        [rest @ .., last] if !rest.is_empty() => format!("{}, and {last}", rest.join(", ")),
        _ => off.join(""),
    };
    format!("Also settle the layout once the condition holds. Off by default for {named}; on for the rest.")
}

fn wait_branch(condition: WaitCondition) -> Value {
    let (description, input) = wait_condition(condition);
    let mut fields = vec![
        ("condition", constant(condition.as_str(), description)),
        ("tab", tab()),
        (
            "visual_settle",
            json!({"type":"boolean","description":wait_settle_description()}),
        ),
    ];
    let mut required = vec!["condition"];
    match input {
        None => {}
        Some(WaitInput::Text) => {
            fields.push((
                "value",
                text(
                    1,
                    MAX_POSTCONDITION_VALUE_CHARS,
                    &format!(
                        "Text or URL fragment; page text, not field values. For duration, milliseconds from {} to {} as a string.",
                        DURATION_MS.min, DURATION_MS.max
                    ),
                ),
            ));
            required.push("value");
        }
        Some(kind @ (WaitInput::Target | WaitInput::OptionalTarget)) => {
            fields.push((
                "target",
                handle(
                    HandleKind::Target,
                    "For target_present, target_absent, and visual_settle.",
                ),
            ));
            if matches!(kind, WaitInput::Target) {
                required.push("target");
            }
        }
        Some(WaitInput::Selector) => {
            fields.push(("selector", semantic_selector()));
            required.push("selector");
        }
    }
    object(fields, required)
}

fn dialog_schema() -> Value {
    union(
        vec![
            dialog_branch(
                DialogAction::Status.as_str(),
                "Its kind and message, and a prompt's field text, if open.",
                false,
            ),
            dialog_branch(
                DialogAction::Accept.as_str(),
                "As OK does; a prompt answers with its field text.",
                false,
            ),
            dialog_branch(DialogAction::Dismiss.as_str(), "Dismiss it.", false),
            dialog_branch(DialogAction::Respond.as_str(), "Answer a prompt.", true),
        ],
        vec![
            json!({"action":"status"}),
            json!({"action":"respond","text":"yes"}),
        ],
    )
}

fn dialog_branch(action: &str, action_description: &str, text_required: bool) -> Value {
    let mut fields = vec![
        ("action", constant(action, action_description)),
        ("tab", tab()),
    ];
    let mut required = vec!["action"];
    if text_required {
        fields.push((
            "text",
            text(0, DIALOG_TEXT_CHARS, "Prompt answer; may be empty."),
        ));
        required.push("text");
    }
    object(fields, required)
}

fn upload_schema() -> Value {
    examples(
        object(
            vec![
                ("target", handle(HandleKind::Target, "File input.")),
                ("selector", semantic_selector()),
                (
                    "paths",
                    json!({"type":"array","minItems":UPLOAD_PATHS.min,"maxItems":UPLOAD_PATHS.max,"uniqueItems":true,"description":"Absolute paths.","items":{"type":"string","minLength":1,"maxLength":UPLOAD_PATH_CHARS}}),
                ),
                (
                    "files",
                    json!({"type":"array","minItems":1,"maxItems":INLINE_FILE_LIMIT,"items":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","minLength":1,"maxLength":FILE_NAME_CHARS},"media_type":{"type":"string","maxLength":MEDIA_TYPE_CHARS},"data_base64":{"type":"string"}},"required":["name","data_base64"]}}),
                ),
                (
                    "source_image",
                    handle(HandleKind::Image, "From an earlier screenshot."),
                ),
                (
                    "view",
                    handle(
                        HandleKind::View,
                        "x and y are pixels in this view's image; needs source_image.",
                    ),
                ),
                ("x", coordinate()),
                ("y", coordinate()),
                ("tab", tab()),
            ],
            vec![],
        ),
        vec![
            json!({"target":"target_...","paths":[EXAMPLE_UPLOAD_PATH]}),
            json!({"source_image":"image_...","view":"view_...","x":120,"y":80}),
        ],
    )
}

/// An absolute path in the form the service's own platform accepts, so the advertised upload
/// example is a call the decoder takes wherever Ghostlight runs.
#[cfg(windows)]
pub(crate) const EXAMPLE_UPLOAD_PATH: &str = "C:\\Users\\me\\notes.txt";
/// An absolute path in the form the service's own platform accepts, so the advertised upload
/// example is a call the decoder takes wherever Ghostlight runs.
#[cfg(not(windows))]
pub(crate) const EXAMPLE_UPLOAD_PATH: &str = "/home/me/notes.txt";

fn evaluate_schema() -> Value {
    examples(
        object(
            vec![
                ("script", text(1, SCRIPT_MAX_CHARS, "")),
                ("tab", tab()),
                (
                    "max_result_chars",
                    within(SCRIPT_RESULT_CHARS, Some(DEFAULT_SCRIPT_RESULT_CHARS), ""),
                ),
            ],
            vec!["script"],
        ),
        vec![json!({"script":"document.title"})],
    )
}

fn flow_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "steps",
                    json!({"type":"array","minItems":1,"maxItems":super::composition::COMPOSITION_STEP_LIMIT,"description":"Omitted ids become step_1, step_2, ...","items":{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string","minLength":1,"maxLength":FLOW_STEP_ID_CHARS},"tool":{"type":"string","description":"An advertised non-composite tool."},"arguments":{"type":"object","description":"That tool's arguments. Any value may be a reference object: {\"flow_ref\":{\"step\":\"earlier_id\",\"pointer\":\"/facts/...\"}}."}},"required":["tool"]}}),
                ),
                (
                    "on_error",
                    enumeration(OnError::WORDS, Some(DEFAULT_ON_ERROR.as_str()), ""),
                ),
                ("tab", tab()),
            ],
            vec!["steps"],
        ),
        vec![
            json!({"steps":[{"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com"}},{"id":"read","tool":"browser_read","arguments":{"max_chars":2000}}]}),
        ],
    )
}

fn record_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    (
                        "action",
                        constant(RecordAction::Start.as_str(), "Start recording."),
                    ),
                    ("tab", tab()),
                ],
                vec!["action"],
            ),
            record_branch(RecordAction::Status.as_str(), "Report state and bounds."),
            record_branch(
                RecordAction::Stop.as_str(),
                "Stop and keep the frames in memory.",
            ),
            object(
                vec![
                    (
                        "action",
                        constant(
                            RecordAction::Save.as_str(),
                            "Save the GIF. Needs read for captured sources, plus write when attaching to a target; an active capture stops after admission.",
                        ),
                    ),
                    (
                        "target",
                        handle(
                            HandleKind::Target,
                            "File input to attach the replay to, inside the browser.",
                        ),
                    ),
                    (
                        "download",
                        boolean(
                            false,
                            "The browser saves the replay as a file and chooses where.",
                        ),
                    ),
                ],
                vec!["action"],
            ),
            record_branch(
                RecordAction::Discard.as_str(),
                "Erase captured bytes. This cannot be undone.",
            ),
        ],
        vec![
            json!({"action":"start"}),
            json!({"action":"save"}),
        ],
    )
}

fn record_branch(action: &str, description: &str) -> Value {
    object(
        vec![("action", constant(action, description))],
        vec!["action"],
    )
}

/// Input schema for the policy explain operation: no arguments.
fn explain_policy_schema() -> Value {
    examples(object(Vec::new(), Vec::new()), vec![json!({})])
}

fn diagnose_schema() -> Value {
    examples(
        object(
            vec![
                ("tab", tab()),
                (
                    "source",
                    enumeration(
                        DiagnosticSource::WORDS,
                        Some(DEFAULT_DIAGNOSTIC_SOURCE.as_str()),
                        "",
                    ),
                ),
                (
                    "detail",
                    enumeration(
                        DiagnosticDetail::WORDS,
                        Some(DEFAULT_DIAGNOSTIC_DETAIL.as_str()),
                        "problems: warnings, errors, failed requests, and HTTP failures.",
                    ),
                ),
                (
                    "match",
                    text(
                        1,
                        SEARCH_TEXT_CHARS,
                        "Case-insensitive literal filter, not a regular expression.",
                    ),
                ),
                (
                    "after",
                    handle(HandleKind::Diagnostics, "Cursor from a prior result."),
                ),
                (
                    "limit",
                    within(DIAGNOSTIC_LIMIT, Some(DEFAULT_DIAGNOSTIC_LIMIT), ""),
                ),
            ],
            vec![],
        ),
        vec![
            json!({}),
            json!({"source":"console","detail":"all","limit":100}),
        ],
    )
}

fn object(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    let properties: BTreeMap<_, _> = fields.into_iter().collect();
    let mut schema = json!({"type":"object","additionalProperties":false,"properties":properties});
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}

/// Advertise a conditional call's branches as one portable object schema.
///
/// Some model APIs reject composition at the root (ADR-0125), so the typed decoder owns which
/// fields belong together. The envelope lists every field any branch accepts and requires only
/// the fields every branch requires. Each field becomes one schema that admits every value some
/// branch admits (`merge_property`). A field `oneOf` of overlapping branch variants would make
/// ordinary values match twice, which JSON Schema treats as invalid.
fn union(branches: Vec<Value>, examples: Vec<Value>) -> Value {
    let mut properties: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut required: Option<Vec<String>> = None;

    for branch in &branches {
        let branch_properties = branch
            .get("properties")
            .and_then(Value::as_object)
            .expect("union branches are object schemas");
        for (name, schema) in branch_properties {
            let variants = properties.entry(name.clone()).or_default();
            if !variants.contains(schema) {
                variants.push(schema.clone());
            }
        }

        let branch_required: Vec<String> = branch
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        required = Some(match required {
            Some(current) => current
                .into_iter()
                .filter(|name| branch_required.contains(name))
                .collect(),
            None => branch_required,
        });
    }

    let properties: BTreeMap<String, Value> = properties
        .into_iter()
        .map(|(name, variants)| {
            let schema = merge_property(&name, variants);
            (name, schema)
        })
        .collect();

    let mut schema = json!({
        "type":"object",
        "additionalProperties":false,
        "properties":properties,
        "examples":examples
    });
    let required = required.unwrap_or_default();
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}

/// Keywords `merge_same_type` combines across variants instead of requiring them to agree.
const MERGED_KEYWORDS: &[&str] = &[
    "type",
    "const",
    "enum",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "pattern",
    "format",
    "default",
    "description",
];

/// One field schema that admits every value any of the field's branch variants admits.
///
/// Variants of one JSON type merge into one schema. Only variants of genuinely different types
/// stay apart, under `anyOf`, where no value can match twice.
fn merge_property(name: &str, variants: Vec<Value>) -> Value {
    if variants.len() == 1 {
        return variants.into_iter().next().expect("one variant");
    }
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for variant in variants {
        let kind = variant
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("union field `{name}` has a variant without a type"))
            .to_owned();
        match groups.iter_mut().find(|(group, _)| *group == kind) {
            Some((_, members)) => members.push(variant),
            None => groups.push((kind, vec![variant])),
        }
    }
    let mut merged: Vec<Value> = groups
        .into_iter()
        .map(|(kind, members)| merge_same_type(name, &kind, members))
        .collect();
    if merged.len() == 1 {
        return merged.pop().expect("one merged type");
    }
    let descriptions: Vec<String> = merged
        .iter_mut()
        .filter_map(|schema| schema.as_object_mut()?.remove("description"))
        .filter_map(|description| description.as_str().map(str::to_owned))
        .collect();
    json!({"description":joined(descriptions.iter().map(String::as_str)),"anyOf":merged})
}

/// Merge same-typed variants into one schema that admits each of them.
///
/// Closed values (`const` or `enum`) join one enum, or stay one const. Once any variant is open,
/// length or numeric bounds widen to cover every variant, closed values included. A pattern,
/// format, or default survives only when no variant disagrees. Every other keyword must already
/// agree; a difference there is a catalog authoring error and fails the catalog build.
fn merge_same_type(name: &str, kind: &str, variants: Vec<Value>) -> Value {
    if variants.len() == 1 {
        return variants.into_iter().next().expect("one variant");
    }
    let variants: Vec<Map<String, Value>> = variants
        .into_iter()
        .map(|variant| match variant {
            Value::Object(schema) => schema,
            _ => panic!("union field `{name}` has a variant that is not a schema object"),
        })
        .collect();
    let mut merged = Map::new();
    merged.insert("type".into(), json!(kind));
    match closed_values(kind, &variants) {
        Some(mut values) if values.len() == 1 => {
            merged.insert("const".into(), values.remove(0));
        }
        Some(values) => {
            merged.insert("enum".into(), Value::Array(values));
        }
        None => {
            for &(keyword, lower) in bound_keywords(kind) {
                if let Some(bound) = widened_bound(&variants, keyword, lower) {
                    merged.insert(keyword.into(), bound);
                }
            }
            for keyword in ["pattern", "format"] {
                if let Some(shared) = shared_keyword(&variants, keyword) {
                    merged.insert(keyword.into(), shared);
                }
            }
        }
    }
    if let Some(default) = agreed_default(&variants) {
        merged.insert("default".into(), default);
    }
    let description = merged_description(kind, &variants);
    if !description.is_empty() {
        merged.insert("description".into(), json!(description));
    }
    for variant in &variants {
        for (key, value) in variant {
            if MERGED_KEYWORDS.contains(&key.as_str()) {
                continue;
            }
            assert!(
                variants.iter().all(|other| other.get(key) == Some(value)),
                "union field `{name}` has variants that differ in `{key}`; give them one schema"
            );
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

/// The values a variant admits when it is closed: its `const`, or its `enum` list.
fn closed_list(variant: &Map<String, Value>) -> Option<Vec<Value>> {
    if let Some(value) = variant.get("const") {
        return Some(vec![value.clone()]);
    }
    variant.get("enum").and_then(Value::as_array).cloned()
}

/// Every value the variants admit, in first-seen order, when all of them are closed.
///
/// Booleans that admit both values are simply open booleans.
fn closed_values(kind: &str, variants: &[Map<String, Value>]) -> Option<Vec<Value>> {
    let mut values: Vec<Value> = Vec::new();
    for variant in variants {
        for value in closed_list(variant)? {
            if !values.contains(&value) {
                values.push(value);
            }
        }
    }
    if kind == "boolean" && values.len() == 2 {
        return None;
    }
    Some(values)
}

/// The lower and upper bound keywords that apply to one JSON type.
fn bound_keywords(kind: &str) -> &'static [(&'static str, bool)] {
    match kind {
        "string" => &[("minLength", true), ("maxLength", false)],
        "integer" | "number" => &[("minimum", true), ("maximum", false)],
        _ => &[],
    }
}

/// The widest bound covering every variant, or none when any variant is unbounded there.
///
/// A closed variant contributes its own values: their lengths for a string, the values
/// themselves for a number.
fn widened_bound(variants: &[Map<String, Value>], keyword: &str, lower: bool) -> Option<Value> {
    let mut widest: Option<Value> = None;
    for variant in variants {
        let candidates: Vec<Value> = match closed_list(variant) {
            Some(values) => values
                .iter()
                .filter_map(|value| match value {
                    Value::String(text) => Some(json!(text.chars().count())),
                    Value::Number(_) => Some(value.clone()),
                    _ => None,
                })
                .collect(),
            None => vec![variant.get(keyword)?.clone()],
        };
        for candidate in candidates {
            let wider = match (&widest, candidate.as_f64()) {
                (None, _) => true,
                (Some(current), Some(next)) => {
                    let current = current.as_f64().expect("bounds are numbers");
                    if lower {
                        next < current
                    } else {
                        next > current
                    }
                }
                (Some(_), None) => panic!("bound `{keyword}` is not a number"),
            };
            if wider {
                widest = Some(candidate);
            }
        }
    }
    widest
}

/// A keyword's value when every variant declares the same one.
fn shared_keyword(variants: &[Map<String, Value>], keyword: &str) -> Option<Value> {
    let first = variants.first()?.get(keyword)?;
    variants
        .iter()
        .all(|variant| variant.get(keyword) == Some(first))
        .then(|| first.clone())
}

/// The default every declaring variant agrees on, if any.
fn agreed_default(variants: &[Map<String, Value>]) -> Option<Value> {
    let mut declared = variants.iter().filter_map(|variant| variant.get("default"));
    let first = declared.next()?;
    declared
        .all(|default| default == first)
        .then(|| first.clone())
}

/// One description for a merged field.
///
/// A string field whose variants are all single constants is a discriminator such as `action`
/// or `condition`, so each value keeps its own meaning: `list: ... focus: ...`. Values that share
/// a description share one entry. Any other field joins its distinct descriptions in order.
///
/// Every value of a discriminator must say what it does: a value without a description would
/// read `focus: ` with nothing after it, and building the catalog in a test fails on one.
fn merged_description(kind: &str, variants: &[Map<String, Value>]) -> String {
    fn description(variant: &Map<String, Value>) -> Option<&str> {
        variant.get("description").and_then(Value::as_str)
    }
    let discriminator = kind == "string" && variants.iter().all(|v| v.contains_key("const"));
    if !discriminator {
        return joined(variants.iter().filter_map(description));
    }
    let mut entries: Vec<(Vec<&str>, &str)> = Vec::new();
    for variant in variants {
        let value = variant["const"].as_str().expect("string constants");
        let meaning = description(variant).unwrap_or_default();
        debug_assert!(!meaning.is_empty(), "the {value} value says what it does");
        match entries
            .iter_mut()
            .find(|(_, existing)| *existing == meaning)
        {
            Some((values, _)) if values.contains(&value) => {}
            Some((values, _)) => values.push(value),
            None => entries.push((vec![value], meaning)),
        }
    }
    entries
        .iter()
        .map(|(values, meaning)| format!("{}: {meaning}", values.join(", ")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Distinct descriptions in first-seen order, as one text.
fn joined<'a>(descriptions: impl Iterator<Item = &'a str>) -> String {
    let mut distinct: Vec<&str> = Vec::new();
    for description in descriptions {
        if !distinct.contains(&description) {
            distinct.push(description);
        }
    }
    distinct.join(" ")
}

fn examples(mut schema: Value, values: Vec<Value>) -> Value {
    schema
        .as_object_mut()
        .expect("schema is an object")
        .insert("examples".into(), Value::Array(values));
    schema
}

fn with_expect(mut schema: Value) -> Value {
    schema["properties"]["expect"] = json!({
        "type":"object",
        "additionalProperties":false,
        "description":"Checked after the effect; needs read, admitted before the effect. A failed check keeps the effect.",
        "properties":{
            "condition":{"type":"string","enum":PageCondition::EXPECTABLE.map(PageCondition::as_str)},
            "value":{"type":"string","minLength":1,"maxLength":MAX_POSTCONDITION_VALUE_CHARS,"description":"Omit for load_ready, else required; page text, not field values."}
        },
        "required":["condition"]
    });
    schema
}

fn with<'a>(mut fields: Vec<(&'a str, Value)>, field: (&'a str, Value)) -> Vec<(&'a str, Value)> {
    fields.push(field);
    fields
}

fn with_many<'a>(
    mut fields: Vec<(&'a str, Value)>,
    more: Vec<(&'a str, Value)>,
) -> Vec<(&'a str, Value)> {
    fields.extend(more);
    fields
}

/// Attach a description to a field schema; an empty one leaves the field to its name, type,
/// enum, bounds and default (ADR-0198 decision 26).
fn described(mut schema: Value, description: &str) -> Value {
    if !description.is_empty() {
        schema["description"] = json!(description);
    }
    schema
}

fn constant(value: &str, description: &str) -> Value {
    described(json!({"type":"string","const":value}), description)
}

fn boolean(default: bool, description: &str) -> Value {
    described(json!({"type":"boolean","default":default}), description)
}

fn text(minimum: usize, maximum: usize, description: &str) -> Value {
    described(
        json!({"type":"string","minLength":minimum,"maxLength":maximum}),
        description,
    )
}

/// An integer field that accepts `range`.
fn within(range: Range, default: Option<usize>, description: &str) -> Value {
    integer(range.min, range.max, default, description)
}

fn integer(minimum: usize, maximum: usize, default: Option<usize>, description: &str) -> Value {
    let mut schema = json!({"type":"integer","minimum":minimum,"maximum":maximum});
    if let Some(default) = default {
        schema["default"] = json!(default);
    }
    described(schema, description)
}

fn enumeration(values: &[&str], default: Option<&str>, description: &str) -> Value {
    let mut schema = json!({"type":"string","enum":values});
    if let Some(default) = default {
        schema["default"] = json!(default);
    }
    described(schema, description)
}

fn handle(kind: HandleKind, description: &str) -> Value {
    let prefix = kind.prefix();
    described(
        json!({"type":"string","maxLength":HANDLE_CHARS,"pattern":format!("^{prefix}.+$")}),
        description,
    )
}

/// The held modifier keys of a pointer or key call.
fn modifier_keys() -> Value {
    json!({"type":"array","uniqueItems":true,"default":[],"items":{"enum":["Alt","Control","Meta","Shift"]}})
}

fn tab() -> Value {
    handle(HandleKind::Tab, "Omit when unambiguous.")
}

fn url() -> Value {
    json!({"type":"string","format":"uri","pattern":"^https?://","maxLength":URL_MAX})
}

/// The `timeout_ms` field every tool accepts, defaulting to what that tool has always been given.
fn timeout(tool: Tool) -> Value {
    json!({"type":"integer","minimum":MIN_TIMEOUT_MS,"maximum":MAX_TIMEOUT_MS,"default":tool.default_timeout_ms()})
}

/// What the `view` field of every point-taking call says about its x and y.
///
/// Views map image pixels through their own scale, which is not 1 for full-page and magnified
/// captures, so page CSS pixels would land in the wrong place. The coordinates carry no
/// description of their own: the view they belong to says it once.
const VIEW_POINT: &str = "x and y are pixels in this view's screenshot image; take one first.";

/// A pointer coordinate in the pixels of the call's view image.
fn coordinate() -> Value {
    json!({"type":"number","minimum":0,"maximum":COORDINATE_MAX})
}

/// A region size in the pixels of the call's view image.
fn extent() -> Value {
    json!({"type":"number","exclusiveMinimum":0,"maximum":COORDINATE_MAX})
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;

    use super::{catalog, catalog_for};
    use crate::governance::GovernanceFacade;
    use crate::language::Tool;

    /// The one object a schema leaves open: a flow step's arguments, which the child tool's own
    /// decoder closes.
    const FLOW_STEP_ARGUMENTS: (&str, &str) = (
        "browser_flow",
        "/properties/steps/items/properties/arguments",
    );

    /// Every object schema in `schema`, with its JSON pointer, through properties, items and the
    /// branches of `oneOf` and `anyOf`.
    fn objects<'a>(schema: &'a Value, at: String, found: &mut Vec<(String, &'a Value)>) {
        let is_object = schema.get("type") == Some(&Value::String("object".into()))
            || schema.get("properties").is_some();
        if is_object {
            found.push((at.clone(), schema));
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, field) in properties {
                objects(field, format!("{at}/properties/{name}"), found);
            }
        }
        if let Some(items) = schema.get("items") {
            objects(items, format!("{at}/items"), found);
        }
        for keyword in ["oneOf", "anyOf"] {
            for (index, branch) in schema
                .get(keyword)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                objects(branch, format!("{at}/{keyword}/{index}"), found);
            }
        }
    }

    /// LANGUAGE.md's contract rules and ACCEPTANCE Contract 2 ("typo-closed at every object
    /// level"): every input schema is a top-level object with no root `oneOf`, `anyOf` or `allOf`,
    /// which Kiro and Bedrock refuse, and every object at every level sets
    /// `additionalProperties: false`, except a flow step's arguments.
    #[test]
    fn every_schema_is_a_portable_typo_closed_object() {
        let mut open = Vec::new();
        for tool in catalog() {
            let schema = &tool.input_schema;
            assert_eq!(schema["type"], "object", "{}", tool.name);
            for keyword in ["oneOf", "anyOf", "allOf"] {
                assert!(
                    schema.get(keyword).is_none(),
                    "{} has a root {keyword}",
                    tool.name
                );
            }
            let mut found = Vec::new();
            objects(schema, String::new(), &mut found);
            assert!(found.iter().any(|(at, _)| at.is_empty()), "{}", tool.name);
            for (at, object) in found {
                if object.get("additionalProperties") != Some(&Value::Bool(false)) {
                    open.push((tool.name.clone(), at));
                }
            }
        }
        let expected = vec![(
            FLOW_STEP_ARGUMENTS.0.to_owned(),
            FLOW_STEP_ARGUMENTS.1.to_owned(),
        )];
        assert_eq!(open, expected);
    }

    /// ACCEPTANCE Governance 21: all-open lists the exact canonical catalog, a read-only grant
    /// drops the tools no grant can admit and keeps the ones it can, and screenshots off drops
    /// exactly browser_screenshot.
    #[test]
    fn the_listed_catalog_follows_the_authority_snapshot() {
        let all_open = GovernanceFacade::new(None).snapshot();
        assert_eq!(catalog_for(&all_open), catalog());

        let listed = |name: &str, policy: &str| -> Vec<String> {
            let path = crate::scratch::Scratch::file(name);
            fs::write(&path, policy).expect("the policy is written");
            let snapshot = GovernanceFacade::new(Some(path.to_path_buf())).snapshot();
            catalog_for(&snapshot)
                .into_iter()
                .map(|tool| tool.name)
                .collect()
        };

        let read_only = listed(
            "catalog-policy.json",
            r#"{"schema":4,"name":"read only","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        );
        for kept in [Tool::Read, Tool::Tabs, Tool::Record] {
            assert!(
                read_only.contains(&kept.name().to_owned()),
                "{}",
                kept.name()
            );
        }
        for dropped in [Tool::Click, Tool::FillForm, Tool::Execute] {
            assert!(
                !read_only.contains(&dropped.name().to_owned()),
                "{}",
                dropped.name()
            );
        }

        let screenshots_off = listed(
            "catalog-screenshots-off.json",
            r#"{"schema":4,"name":"no screenshots","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}],"config":[{"key":"browser.screenshots.enabled","value":false,"level":"mandatory"}]}"#,
        );
        let mut expected: Vec<String> = catalog().into_iter().map(|tool| tool.name).collect();
        expected.retain(|name| name != Tool::Screenshot.name());
        assert_eq!(screenshots_off, expected);
    }
}
