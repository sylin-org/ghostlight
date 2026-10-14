//! The deterministic model-facing tool catalog and its truthful JSON Schemas.

use std::collections::BTreeMap;

use crate::intake::session::{ToolAnnotations, ToolDefinition};
use serde_json::{json, Map, Value};

use crate::governance::AuthoritySnapshot;

use super::{
    DEFAULT_DOCUMENT_DEPTH, DEFAULT_FILL_TIMEOUT_MS, DEFAULT_RECORD_TIMEOUT_MS, DEFAULT_TIMEOUT_MS,
    MAX_POSTCONDITION_VALUE_CHARS, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, NAMED_KEYS, SEMANTIC_ROLES,
};

/// Canonical tool names in the complete catalog.
pub const EXPECTED_TOOL_NAMES: [&str; 23] = [
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
    "browser_wait",
    "browser_dialog",
    "browser_upload",
    "browser_execute",
    "browser_flow",
    "browser_record",
    "browser_diagnose",
    "policy_explain",
];

/// Canonical tool count for the complete catalog.
pub const CATALOG_TOOL_COUNT: usize = EXPECTED_TOOL_NAMES.len();

/// Return the complete native Ghostlight language in deterministic order.
#[must_use]
pub fn catalog() -> Vec<ToolDefinition> {
    vec![
        tool(
            "browser_tabs",
            "Browser tabs",
            "List, focus, or close this session's tabs. List reads the live browser, refuses when none is connected, and returns fresh tab handles.",
            tabs_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_navigate",
            "Navigate",
            "Go to an HTTP(S) URL. Reuses the obvious tab; new_tab true opens a new one.",
            navigate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_history",
            "Browser history",
            "Go back or forward, or reload. A new document invalidates target_ and view_ handles, so collect fresh ones.",
            history_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_window",
            "Browser window",
            "Set a tab's zoom or resize its browser window. Either invalidates view_ handles, so take a fresh screenshot before coordinate work.",
            window_schema(),
            Hints::local_mutation(false),
        ),
        tool(
            "browser_read",
            "Read page",
            "Read visible page text, including open shadow roots and frames, or one target. To act, get handles from browser_inspect or browser_find.",
            read_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_inspect",
            "Inspect page",
            "Inspect controls or page structure for fresh target_ handles. Scope document returns the outline as a bounded tree, without handles.",
            inspect_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_find",
            "Find on page",
            "Find ranked targets by visible or accessible text and get fresh target_ handles. Fastest when you know the label.",
            find_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_screenshot",
            "Take screenshot",
            "Capture the viewport, full page, one target, or a magnified region; returns a view_ handle for coordinate calls.",
            screenshot_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_click",
            "Click",
            "Click a target_ handle, a control found by selector, or a point in a view_ screenshot. Needs action; a selector lookup or expect check also needs read. Prefer a target.",
            with_expect(click_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_scroll",
            "Scroll",
            "Scroll the page (default down, medium), reveal a target_, or wheel-scroll at a point in a view_ screenshot.",
            scroll_schema(),
            Hints::browser_action(false),
        ),
        tool(
            "browser_hover",
            "Hover",
            "Hover a target_ handle or a point in a view_ screenshot.",
            hover_schema(),
            Hints::browser_action(false),
        ),
        tool(
            "browser_fill_form",
            "Fill form",
            "Fill 1 to 30 fields in one call; needs write even for unsent drafts. Nothing submits unless submit_target names the control. A value a select does not offer is refused before any field changes. For credentials, set user_authorized_credentials only on the user's explicit instruction.",
            with_expect(fill_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_type_text",
            "Type text",
            "Type through real keyboard input. Needs action; a selector lookup or expect check also needs read. Prefer browser_fill_form for plain values. For credentials, set user_authorized_credentials only on the user's explicit instruction.",
            with_expect(type_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_press_key",
            "Press key",
            "Send a named key or literal character, optionally at a target_. Needs action; an expect check also needs read. For Control+A send key a with modifiers Control; strokes sends keys in order.",
            with_expect(key_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_drag",
            "Drag",
            "Drag between two target_ handles or two points in one view_ screenshot.",
            drag_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_wait",
            "Wait",
            "Wait for one observable page condition. A false condition is a failed result, not an unknown effect.",
            wait_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_dialog",
            "Browser dialog",
            "Inspect, accept, dismiss, or answer the open JavaScript dialog. A dialog blocks its page until handled.",
            dialog_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_upload",
            "Upload files",
            "Attach local paths, inline files, or one captured image to a file input, or drop the image at a point in a view_. Needs write; a selector lookup also needs read.",
            upload_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_execute",
            "Execute JavaScript",
            "Run JavaScript in the page main world. It may read, mutate, or navigate; prefer a semantic tool.",
            evaluate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_flow",
            "Run flow",
            "Run 1 to 20 tool calls. tab defaults tab-scoped steps; a step's own tab wins. Later arguments may reference earlier results. Stops at the first failed step by default.",
            flow_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_record",
            "Record browser",
            "Record browser work as a short memory-only GIF. One recording per session; save stops an active capture.",
            record_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_diagnose",
            "Diagnose page",
            "Read opt-in console and network observations for a tab. Observation begins with the first call, so reload or reproduce to capture earlier activity. Sensitive request data is never collected.",
            diagnose_schema(),
            Hints::browser_read(),
        ),
        tool(
            "policy_explain",
            "Explain policy",
            "Explain the authority in force: one line per capability with its deciding layer, the rules behind them, permanent ceilings, and each layer's document. Available under every authority; use it to learn why a call was refused or what is allowed.",
            explain_policy_schema(),
            Hints::local_read(),
        ),
    ]
}

/// Project the canonical catalog through one immutable authority snapshot.
#[must_use]
pub fn catalog_for(snapshot: &AuthoritySnapshot) -> Vec<ToolDefinition> {
    catalog()
        .into_iter()
        .filter(|tool| {
            super::capability_map::variants(&tool.name)
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

fn tool(
    name: &str,
    title: &str,
    description: &str,
    input_schema: Value,
    hints: Hints,
) -> ToolDefinition {
    let mut input_schema = input_schema;
    input_schema["properties"]["timeout_ms"] = timeout(name);
    if super::settlement::PAGE_TOOLS.contains(&name) {
        // Thirteen tools carry it undescribed: the name, type and default say what it is, and
        // LANGUAGE.md carries the budget and the busy-page behavior (ADR-0195).
        input_schema["properties"]["visual_settle"] = boolean(true, "");
    }
    ToolDefinition {
        name: name.into(),
        description: description.into(),
        input_schema,
        output_schema: Some(outcome_schema()),
        annotations: Some(ToolAnnotations {
            title: Some(title.into()),
            read_only_hint: Some(hints.read_only),
            destructive_hint: Some(hints.destructive),
            idempotent_hint: Some(hints.idempotent),
            open_world_hint: Some(hints.open_world),
        }),
    }
}

fn outcome_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "history_storage": {"type":"string","enum":[super::audit_health::Storage::Saved,super::audit_health::Storage::Unconfirmed],"description":"Whether saving this receipt was confirmed, independent of browser effects."},
            "invocation": {"type":"string","pattern":"^invocation_.+$","description":"Opaque invocation handle."},
            "status": {"type":"string","enum":["succeeded","blocked","failed","cancelled","attention_required","unknown"]},
            "effect": {"type":"string","enum":["none","applied","partial","unknown"]},
            "readiness": {"type":"string","enum":["not_applicable","loading","interactive","complete","unknown"]},
            "repeat_safe": {"type":"boolean","description":"Whether repeating the same call is known safe."},
            "summary": {"type":"string","maxLength":500},
            "facts": {"type":"object","description":"Tool-specific canonical facts."},
            "next_steps": {"type":"array","maxItems":2,"items":{"type":"string"}}
        },
        "required": ["history_storage","invocation","status","effect","readiness","repeat_safe","summary","facts","next_steps"]
    })
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
                    ("action", constant("focus", "")),
                    ("tab", handle("tab_", "From browser_tabs list.")),
                ],
                vec!["action", "tab"],
            ),
            object(
                vec![
                    ("action", constant("close", "")),
                    ("tab", handle("tab_", "From browser_tabs list.")),
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
                            "domain reuses a tab an earlier session of this client released for this host, never one the person opened. never always creates. domain cannot combine with new_tab.",
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
                            "browser_",
                            "From browser_tabs list. Omit to use the browser the user attended last.",
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
            history_branch("back", "Go back one page."),
            history_branch("forward", "Go forward one page."),
            object(
                vec![
                    ("action", constant("reload", "Reload the page.")),
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
                    ("percent", integer(25, 500, None, "")),
                    ("tab", tab()),
                ],
                vec!["action", "percent"],
            ),
            object(
                vec![
                    ("action", constant("resize", "")),
                    ("width", integer(320, 7_680, None, "")),
                    ("height", integer(240, 4_320, None, "")),
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
                    handle("target_", ""),
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
                    integer(500, 50_000, Some(8_000), ""),
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
                        &["controls", "structure", "all", "document"],
                        Some("controls"),
                        "document: a {kind, label, children} tree, a snapshot handle, and a diff against the previous snapshot when root and max_depth match.",
                    ),
                ),
                (
                    "root",
                    handle("target_", "Subtree root for document scope."),
                ),
                (
                    "max_depth",
                    // No schema default: the decoder refuses max_depth outside document scope.
                    integer(
                        1,
                        12,
                        None,
                        &format!("Tree levels; {DEFAULT_DOCUMENT_DEPTH} when omitted."),
                    ),
                ),
                (
                    "max_items",
                    integer(1, 200, Some(80), ""),
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
                ("text", text(1, 500, "Visible or accessible text.")),
                ("tab", tab()),
                (
                    "scope",
                    enumeration(&["any", "control", "text"], Some("any"), ""),
                ),
                ("max_results", integer(1, 50, Some(20), "")),
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
                vec![("tab", tab()), ("target", handle("target_", ""))],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "view",
                        handle(
                            "view_",
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
            enumeration(&["primary", "middle", "secondary"], Some("primary"), ""),
        ),
        ("click_count", integer(1, 3, Some(1), "")),
        ("modifiers", modifier_keys()),
        ("selector", semantic_selector()),
    ];
    union(
        vec![
            object(
                with(common.clone(), ("target", handle("target_", ""))),
                vec!["target"],
            ),
            object(
                with_many(
                    common,
                    vec![
                        ("view", handle("view_", VIEW_POINT)),
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
            "name": {"type": "string", "minLength": 1, "maxLength": 500},
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
                vec![("tab", tab()), ("target", handle("target_", ""))],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "direction",
                        enumeration(
                            &["up", "down", "left", "right"],
                            Some("down"),
                            SCROLL_DIRECTION,
                        ),
                    ),
                    (
                        "amount",
                        enumeration(&["small", "medium", "large", "page"], Some("medium"), ""),
                    ),
                ],
                vec![],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "direction",
                        enumeration(&["up", "down"], Some("down"), SCROLL_DIRECTION),
                    ),
                    ("view", handle("view_", VIEW_POINT)),
                    ("x", coordinate()),
                    ("y", coordinate()),
                    ("ticks", integer(1, 10, None, "")),
                ],
                vec!["direction", "view", "x", "y", "ticks"],
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
                vec![("tab", tab()), ("target", handle("target_", ""))],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    ("view", handle("view_", VIEW_POINT)),
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
        boolean(false, "True only when the user explicitly authorized credential entry for this request. Never overrides policy or Pause/Stop."),
    )
}

fn fill_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "fields",
                    json!({"type":"array","minItems":1,"maxItems":30,"description":"Each field gives exactly one of target or selector.","items":{"type":"object","additionalProperties":false,"properties":{"target":handle("target_",""),"selector":semantic_selector(),"value":{"description":"Empty string clears. Checkboxes and radios take booleans; numeric inputs take numbers.","anyOf":[{"type":"string","maxLength":8000},{"type":"boolean"},{"type":"number"}]}},"required":["value"]}}),
                ),
                ("tab", tab()),
                (
                    "submit_target",
                    handle(
                        "target_",
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
                8_000,
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
                with(common.clone(), ("target", handle("target_", ""))),
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
    let shared = vec![("tab", tab()), ("target", handle("target_", "")), modifiers];
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
                            json!({"type":"array","minItems":1,"maxItems":20,"description":"Replaces key.","items":stroke_item}),
                        ),
                        (
                            "repeat",
                            integer(1, 100, Some(1), "Repeats the whole sequence."),
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
                    ("source_target", handle("target_", "")),
                    (
                        "destination_target",
                        handle("target_", ""),
                    ),
                    ("tab", tab()),
                ],
                vec!["source_target", "destination_target"],
            ),
            object(
                vec![
                    (
                        "view",
                        handle("view_", "Both points are pixels in this view's screenshot image; take one first."),
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
    use WaitInput::{OptionalTarget, Selector, Target, Text};
    union(
        vec![
            wait_branch("load_ready", "Page finished loading.", None),
            wait_branch("url_contains", "URL contains value.", Some(Text)),
            wait_branch("text_present", "value is in the visible text.", Some(Text)),
            wait_branch(
                "text_absent",
                "value is gone from the visible text.",
                Some(Text),
            ),
            wait_branch("target_present", "target exists.", Some(Target)),
            wait_branch("target_absent", "target is gone.", Some(Target)),
            wait_branch(
                "visual_settle",
                "The page, or target, stopped moving and repainting.",
                Some(OptionalTarget),
            ),
            wait_branch(
                "selector_present",
                "selector matches a control.",
                Some(Selector),
            ),
            wait_branch("duration", "Waits value milliseconds.", Some(Text)),
        ],
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

fn wait_branch(condition: &str, description: &str, input: Option<WaitInput>) -> Value {
    let mut fields = vec![
        ("condition", constant(condition, description)),
        ("tab", tab()),
        (
            "visual_settle",
            json!({"type":"boolean","description":"Also settle the layout once the condition holds. Off by default for url_contains, text_present, text_absent, and target_absent; on for the rest."}),
        ),
    ];
    let mut required = vec!["condition"];
    match input {
        None => {}
        Some(WaitInput::Text) => {
            fields.push(("value", text(1, 2_000, "Text or URL fragment; page text, not field values. For duration, milliseconds from 0 to 10000 as a string.")));
            required.push("value");
        }
        Some(kind @ (WaitInput::Target | WaitInput::OptionalTarget)) => {
            fields.push((
                "target",
                handle(
                    "target_",
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
            dialog_branch("status", "Report whether one is open.", false),
            dialog_branch("accept", "Accept it.", false),
            dialog_branch("dismiss", "Dismiss it.", false),
            dialog_branch("respond", "Answer a prompt.", true),
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
        fields.push(("text", text(0, 2_000, "Prompt answer; may be empty.")));
        required.push("text");
    }
    object(fields, required)
}

fn upload_schema() -> Value {
    examples(
        object(
            vec![
                ("target", handle("target_", "File input.")),
                ("selector", semantic_selector()),
                (
                    "paths",
                    json!({"type":"array","minItems":1,"maxItems":5,"uniqueItems":true,"description":"Absolute paths.","items":{"type":"string","minLength":1,"maxLength":4096}}),
                ),
                (
                    "files",
                    json!({"type":"array","minItems":1,"maxItems":5,"items":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","minLength":1,"maxLength":255},"media_type":{"type":"string","maxLength":100},"data_base64":{"type":"string"}},"required":["name","data_base64"]}}),
                ),
                (
                    "source_image",
                    handle("image_", "From an earlier screenshot."),
                ),
                (
                    "view",
                    handle(
                        "view_",
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
const EXAMPLE_UPLOAD_PATH: &str = "C:\\Users\\me\\notes.txt";
/// An absolute path in the form the service's own platform accepts, so the advertised upload
/// example is a call the decoder takes wherever Ghostlight runs.
#[cfg(not(windows))]
const EXAMPLE_UPLOAD_PATH: &str = "/home/me/notes.txt";

fn evaluate_schema() -> Value {
    examples(
        object(
            vec![
                ("script", text(1, 20_000, "")),
                ("tab", tab()),
                ("max_result_chars", integer(100, 20_000, Some(8_000), "")),
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
                    json!({"type":"array","minItems":1,"maxItems":super::history::COMPOSITION_STEP_LIMIT,"description":"Omitted ids become step_1, step_2, ...","items":{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string","minLength":1,"maxLength":64},"tool":{"type":"string","description":"An advertised non-composite tool."},"arguments":{"type":"object","description":"That tool's arguments. Any value may be a reference object: {\"flow_ref\":{\"step\":\"earlier_id\",\"pointer\":\"/facts/...\"}}."}},"required":["tool"]}}),
                ),
                (
                    "on_error",
                    enumeration(&["stop", "continue"], Some("stop"), ""),
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
                        constant("start", "Start recording."),
                    ),
                    ("tab", tab()),
                ],
                vec!["action"],
            ),
            record_branch("status", "Report state and bounds."),
            record_branch("stop", "Stop and keep the frames in memory."),
            object(
                vec![
                    (
                        "action",
                        constant(
                            "save",
                            "Save the GIF. Needs read for captured sources, plus write when attaching to a target; an active capture stops after admission.",
                        ),
                    ),
                    (
                        "target",
                        handle(
                            "target_",
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
            record_branch("discard", "Erase captured bytes. This cannot be undone."),
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
                    enumeration(&["both", "console", "network"], Some("both"), ""),
                ),
                (
                    "detail",
                    enumeration(
                        &["problems", "all"],
                        Some("problems"),
                        "problems: warnings, errors, failed requests, and HTTP failures.",
                    ),
                ),
                (
                    "match",
                    text(
                        1,
                        500,
                        "Case-insensitive literal filter, not a regular expression.",
                    ),
                ),
                ("after", handle("diag_", "Cursor from a prior result.")),
                ("limit", integer(1, 200, Some(50), "")),
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
    raw_object(fields, required)
}

fn raw_object(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
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
            "condition":{"type":"string","enum":["load_ready","url_contains","text_present","text_absent"]},
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

fn handle(prefix: &str, description: &str) -> Value {
    described(
        json!({"type":"string","maxLength":160,"pattern":format!("^{prefix}.+$")}),
        description,
    )
}

/// The held modifier keys of a pointer or key call.
fn modifier_keys() -> Value {
    json!({"type":"array","uniqueItems":true,"default":[],"items":{"enum":["Alt","Control","Meta","Shift"]}})
}

fn tab() -> Value {
    handle("tab_", "Omit when one tab is unambiguous.")
}

fn url() -> Value {
    json!({"type":"string","format":"uri","pattern":"^https?://","maxLength":4096})
}

/// The `timeout_ms` field every tool accepts, defaulting to what that tool has always been given.
fn timeout(tool: &str) -> Value {
    let default = match tool {
        "browser_fill_form" => DEFAULT_FILL_TIMEOUT_MS,
        "browser_record" => DEFAULT_RECORD_TIMEOUT_MS,
        _ => DEFAULT_TIMEOUT_MS,
    };
    json!({"type":"integer","minimum":MIN_TIMEOUT_MS,"maximum":MAX_TIMEOUT_MS,"default":default})
}

/// What the `view` field of every point-taking call says about its x and y.
///
/// Views map image pixels through their own scale, which is not 1 for full-page and magnified
/// captures, so page CSS pixels would land in the wrong place. The coordinates carry no
/// description of their own: the view they belong to says it once.
const VIEW_POINT: &str = "x and y are pixels in this view's screenshot image; take one first.";

/// A pointer coordinate in the pixels of the call's view image.
fn coordinate() -> Value {
    json!({"type":"number","minimum":0,"maximum":1_000_000})
}

/// A region size in the pixels of the call's view image.
fn extent() -> Value {
    json!({"type":"number","exclusiveMinimum":0,"maximum":1_000_000})
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{json, Value};

    use super::{
        boolean, catalog, catalog_for, constant, enumeration, handle, integer, object, text, union,
        CATALOG_TOOL_COUNT, EXPECTED_TOOL_NAMES,
    };
    use crate::governance::GovernanceFacade;

    #[test]
    fn catalog_is_exact_deterministic_and_fully_described() {
        let tools = catalog();
        assert_eq!(tools.len(), CATALOG_TOOL_COUNT);
        let names: Vec<_> = tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(names, EXPECTED_TOOL_NAMES);
        for tool in tools {
            assert!(
                tool.output_schema.is_some(),
                "{} lacks output schema",
                tool.name
            );
            assert!(
                tool.annotations.is_some(),
                "{} lacks annotations",
                tool.name
            );
            assert!(
                tool.input_schema.get("examples").is_some(),
                "{} lacks examples",
                tool.name
            );
        }
    }

    #[test]
    fn declared_output_fields_accept_canonical_receipts_and_storage_states() {
        use crate::language::{audit_health::Storage, outcome::Outcome};
        use crate::work::result::{Effect, InvocationResult, Readiness, Status};

        let outcome = Outcome::PageOpened {
            host: Some("example.com".into()),
        };
        let mut result = InvocationResult::new(
            "invocation_schema_test",
            Status::Succeeded,
            Effect::Applied,
            Readiness::Complete,
            false,
            &outcome.summary(),
            json!({"created":true,"url":"https://example.com/"}),
            outcome.next_steps(),
        );
        for storage in [Storage::Saved, Storage::Unconfirmed] {
            result.history_storage = storage;
            let serialized = serde_json::to_value(&result).expect("canonical receipt serializes");
            let fields = serialized.as_object().expect("receipt is an object");
            for tool in catalog() {
                let schema = tool.output_schema.expect("output schema is declared");
                assert_eq!(schema["additionalProperties"], false);
                let properties = schema["properties"].as_object().expect("declared fields");
                let required = schema["required"].as_array().expect("required fields");
                for key in required {
                    assert!(
                        fields.contains_key(key.as_str().expect("required field name")),
                        "{} requires absent receipt field {key}",
                        tool.name
                    );
                }
                for (key, value) in fields {
                    let property = properties.get(key).unwrap_or_else(|| {
                        panic!("{} rejects canonical receipt field {key}", tool.name)
                    });
                    let correct_type = match property["type"].as_str() {
                        Some("string") => value.is_string(),
                        Some("boolean") => value.is_boolean(),
                        Some("object") => value.is_object(),
                        Some("array") => value.is_array(),
                        other => panic!("unhandled receipt field type {other:?}"),
                    };
                    assert!(
                        correct_type,
                        "{} declares the wrong type for {key}",
                        tool.name
                    );
                    if let Some(values) = property["enum"].as_array() {
                        assert!(
                            values.contains(value),
                            "{} rejects canonical value {value} for {key}",
                            tool.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn postconditions_are_discoverable_on_the_tools_that_execute_them() {
        let supported = [
            "browser_click",
            "browser_fill_form",
            "browser_type_text",
            "browser_press_key",
        ];
        for tool in catalog() {
            let expectation = &tool.input_schema["properties"]["expect"];
            if !supported.contains(&tool.name.as_str()) {
                assert!(
                    expectation.is_null(),
                    "{} does not execute expect",
                    tool.name
                );
                continue;
            }
            assert_eq!(expectation["type"], "object", "{}", tool.name);
            assert_eq!(expectation["additionalProperties"], false);
            let mut input = tool.input_schema["examples"][0].clone();
            input["expect"] = json!({"condition":"text_present","value":"Ready"});
            assert!(crate::language::decode(&tool.name, input.clone()).is_ok());
            input["expect"]["value"] = json!("x".repeat(
                expectation["properties"]["value"]["maxLength"]
                    .as_u64()
                    .expect("advertised text bound") as usize
                    + 1
            ));
            assert!(crate::language::decode(&tool.name, input.clone()).is_err());
            input["expect"] = json!({"condition":"load_ready"});
            assert!(crate::language::decode(&tool.name, input.clone()).is_ok());
            input["expect"]["value"] = json!("unexpected");
            assert!(crate::language::decode(&tool.name, input).is_err());
        }
    }

    #[test]
    fn policy_projection_is_quiet_when_all_open_and_removes_impossible_tools() {
        let all_open = GovernanceFacade::new(None, None).snapshot();
        assert_eq!(catalog_for(&all_open), catalog());

        let path = std::env::temp_dir().join(format!(
            "ghostlight-catalog-policy-{}.json",
            uuid::Uuid::new_v4()
        ));
        fs::write(
            &path,
            r#"{"schema":3,"name":"read only","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        let read_only = GovernanceFacade::new(Some(path.clone()), None).snapshot();
        let names: Vec<_> = catalog_for(&read_only)
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        assert!(names.contains(&"browser_read".into()));
        assert!(names.contains(&"browser_tabs".into()));
        assert!(names.contains(&"browser_record".into()));
        assert!(!names.contains(&"browser_click".into()));
        assert!(!names.contains(&"browser_fill_form".into()));
        assert!(!names.contains(&"browser_execute".into()));

        fs::write(
            &path,
            r#"{"schema":3,"name":"observe","version":"1","mode":"observe","grants":[]}"#,
        )
        .unwrap();
        let observe = GovernanceFacade::new(Some(path.clone()), None).snapshot();
        assert_eq!(catalog_for(&observe), catalog());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn catalog_annotations_are_conservative_for_every_tool() {
        let expected = [
            ("browser_tabs", false, true, false, true),
            ("browser_navigate", false, true, false, true),
            ("browser_history", false, true, false, true),
            ("browser_window", false, false, false, false),
            ("browser_read", true, false, true, true),
            ("browser_inspect", true, false, true, true),
            ("browser_find", true, false, true, true),
            ("browser_screenshot", true, false, true, true),
            ("browser_click", false, true, false, true),
            ("browser_scroll", false, false, false, true),
            ("browser_hover", false, false, false, true),
            ("browser_fill_form", false, true, false, true),
            ("browser_type_text", false, true, false, true),
            ("browser_press_key", false, true, false, true),
            ("browser_drag", false, true, false, true),
            ("browser_wait", true, false, true, true),
            ("browser_dialog", false, true, false, true),
            ("browser_upload", false, true, false, true),
            ("browser_execute", false, true, false, true),
            ("browser_flow", false, true, false, true),
            ("browser_record", false, true, false, true),
            ("browser_diagnose", true, false, true, true),
            ("policy_explain", true, false, true, false),
        ];

        for (tool, &(name, read_only, destructive, idempotent, open_world)) in
            catalog().iter().zip(expected.iter())
        {
            let annotations = tool.annotations.as_ref().expect("annotations are required");
            assert_eq!(tool.name, name);
            assert_eq!(annotations.read_only_hint, Some(read_only), "{name}");
            assert_eq!(annotations.destructive_hint, Some(destructive), "{name}");
            assert_eq!(annotations.idempotent_hint, Some(idempotent), "{name}");
            assert_eq!(annotations.open_world_hint, Some(open_world), "{name}");
        }
    }

    #[test]
    fn catalog_descriptions_teach_the_remedy() {
        let tools = catalog();
        let description = |name: &str| -> String {
            tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("missing {name}"))
                .description
                .clone()
        };

        assert!(
            description("browser_history").contains("collect fresh ones"),
            "navigation must teach that stale handles need recollection"
        );
        assert!(
            description("browser_window").contains("take a fresh screenshot"),
            "window changes must teach that coordinate work needs a new view"
        );
        assert!(
            description("browser_dialog").contains("blocks its page"),
            "dialog handling must teach that dialogs block their page"
        );
        assert!(
            description("browser_diagnose").contains("Observation begins with the first call"),
            "diagnose must teach when observation starts"
        );

        let screenshot = tool_schema("browser_screenshot");
        let view = property(&screenshot, "view").expect("screenshot view property");
        assert!(
            view.get("description")
                .and_then(Value::as_str)
                .is_some_and(|text| text.contains("take a screenshot first")),
            "region capture must teach where its view comes from"
        );

        let click = tool_schema("browser_click");
        let click_view = property(&click, "view").expect("click view property");
        assert!(
            click_view
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|text| text.contains("take one first")),
            "coordinate clicks must teach where their view comes from"
        );
    }

    #[test]
    fn catalog_strings_are_ascii() {
        for tool in catalog() {
            assert!(tool.name.is_ascii(), "{} name", tool.name);
            assert!(tool.description.is_ascii(), "{} description", tool.name);
            let annotations = tool.annotations.as_ref().expect("annotations are required");
            let title = annotations.title.as_deref().expect("title is required");
            assert!(title.is_ascii(), "{} title", tool.name);
            let schema = serde_json::to_string(&tool.input_schema).expect("schema serializes");
            assert!(schema.is_ascii(), "{} schema", tool.name);
        }
    }

    #[test]
    fn catalog_uses_portable_top_level_object_schemas() {
        for tool in catalog() {
            assert_eq!(
                tool.input_schema.get("type"),
                Some(&Value::String("object".into()))
            );
            assert_eq!(
                tool.input_schema.get("additionalProperties"),
                Some(&Value::Bool(false)),
                "{}",
                tool.name
            );
            for keyword in ["oneOf", "anyOf", "allOf"] {
                assert!(
                    tool.input_schema.get(keyword).is_none(),
                    "{} has unsupported top-level {keyword}",
                    tool.name
                );
            }
        }

        // Overlapping branches become one field schema, never a field oneOf a value can match
        // twice (MSURF-1).
        let screenshot = tool_schema("browser_screenshot");
        let full_page = property(&screenshot, "full_page").expect("full_page property");
        assert_eq!(full_page["type"], "boolean");
        assert!(full_page.get("const").is_none() && full_page.get("oneOf").is_none());

        let typing = tool_schema("browser_type_text");
        let text = property(&typing, "text").expect("text property");
        assert_eq!(
            (&text["type"], &text["minLength"], &text["maxLength"]),
            (&json!("string"), &json!(0), &json!(8_000))
        );

        let wait = tool_schema("browser_wait");
        let condition = property(&wait, "condition").expect("condition property");
        assert_eq!(condition["enum"].as_array().map(Vec::len), Some(9));
        let meaning = condition["description"]
            .as_str()
            .expect("condition meaning");
        for value in condition["enum"].as_array().expect("condition values") {
            let value = value.as_str().expect("string condition");
            assert!(
                meaning.contains(&format!("{value}: ")),
                "{value} lacks a meaning"
            );
        }
    }

    #[test]
    fn union_merges_overlapping_branch_fields_into_one_schema() {
        let merged = union(
            vec![
                object(
                    vec![
                        ("action", constant("list", "List them.")),
                        ("text", text(1, 10, "Words.")),
                        ("clear", boolean(false, "Clear first.")),
                        (
                            "direction",
                            enumeration(&["up", "down", "left"], Some("down"), "Way."),
                        ),
                        ("target", handle("target_", "First meaning.")),
                        ("count", integer(1, 5, None, "How many.")),
                    ],
                    vec!["action", "text"],
                ),
                object(
                    vec![
                        ("action", constant("show", "Show one.")),
                        ("text", constant("", "Empty.")),
                        ("clear", json!({"type":"boolean","const":true})),
                        (
                            "direction",
                            enumeration(&["up", "down"], Some("down"), "Way."),
                        ),
                        ("target", handle("target_", "Second meaning.")),
                        ("count", integer(3, 9, None, "How many.")),
                    ],
                    vec!["action"],
                ),
                object(
                    vec![("action", constant("hide", "List them."))],
                    vec!["action"],
                ),
            ],
            vec![],
        );
        let field = |name: &str| merged["properties"][name].clone();
        assert_eq!(merged["required"], json!(["action"]));
        assert_eq!(
            field("action"),
            json!({"type":"string","enum":["list","show","hide"],"description":"list, hide: List them. show: Show one."})
        );
        assert_eq!(
            field("text"),
            json!({"type":"string","minLength":0,"maxLength":10,"description":"Words. Empty."})
        );
        assert_eq!(
            field("clear"),
            json!({"type":"boolean","default":false,"description":"Clear first."})
        );
        assert_eq!(
            field("direction"),
            json!({"type":"string","enum":["up","down","left"],"default":"down","description":"Way."})
        );
        assert_eq!(field("target")["pattern"], "^target_.+$");
        assert_eq!(
            field("target")["description"],
            "First meaning. Second meaning."
        );
        assert_eq!(
            (
                field("count")["minimum"].clone(),
                field("count")["maximum"].clone()
            ),
            (json!(1), json!(9))
        );

        let mixed = union(
            vec![
                object(vec![("value", text(0, 5, "Text."))], vec![]),
                object(vec![("value", boolean(false, "Flag."))], vec![]),
            ],
            vec![],
        );
        assert_eq!(
            mixed["properties"]["value"],
            json!({"description":"Text. Flag.","anyOf":[
                {"type":"string","minLength":0,"maxLength":5},
                {"type":"boolean","default":false}
            ]})
        );
    }

    #[test]
    fn a_handle_field_refuses_an_empty_value_without_a_minimum_length() {
        let read = tool_schema("browser_read");
        let target = property(&read, "target").expect("target property");
        assert!(target.get("minLength").is_none());
        let valid = json!({"tab":"tab_1","target":"target_1"});
        assert!(crate::language::decode("browser_read", valid).is_ok());
        for empty in ["", "target_"] {
            let call = json!({"tab":"tab_1","target":empty});
            assert!(crate::language::decode("browser_read", call).is_err());
        }
    }

    #[test]
    #[should_panic(expected = "differ in `items`")]
    fn union_refuses_variants_it_cannot_merge_honestly() {
        union(
            vec![
                object(
                    vec![("list", json!({"type":"array","items":{"type":"string"}}))],
                    vec![],
                ),
                object(
                    vec![("list", json!({"type":"array","items":{"type":"integer"}}))],
                    vec![],
                ),
            ],
            vec![],
        );
    }

    #[test]
    fn flow_schema_unifies_batches_without_client_authority_or_simulation() {
        for tool in catalog() {
            let encoded = serde_json::to_string(&tool.input_schema).unwrap();
            assert!(!encoded.contains("restrict_hosts"));
            assert!(!encoded.contains("restrict_capabilities"));
        }
        let schema = tool_schema("browser_flow");
        assert!(property(&schema, "dry_run").is_none());
        assert_eq!(
            schema.pointer("/properties/steps/items/required"),
            Some(&serde_json::json!(["tool"]))
        );
        assert_eq!(
            schema.pointer("/properties/steps/items/additionalProperties"),
            Some(&Value::Bool(false))
        );
    }

    fn tool_schema(name: &str) -> Value {
        catalog()
            .into_iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .input_schema
    }

    fn property<'a>(branch: &'a Value, name: &str) -> Option<&'a Value> {
        branch.get("properties").and_then(|value| value.get(name))
    }
}
