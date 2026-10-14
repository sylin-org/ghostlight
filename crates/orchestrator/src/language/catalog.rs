//! The deterministic model-facing tool catalog and its truthful JSON Schemas.

use std::collections::BTreeMap;

use crate::intake::session::{ToolAnnotations, ToolDefinition};
use serde_json::{json, Map, Value};

use crate::governance::AuthoritySnapshot;

use super::{
    DEFAULT_DOCUMENT_DEPTH, DEFAULT_FILL_TIMEOUT_MS, DEFAULT_RECORD_TIMEOUT_MS, DEFAULT_TIMEOUT_MS,
    MAX_POSTCONDITION_VALUE_CHARS, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, NAMED_KEYS,
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
            "List, focus, or close this workspace's tabs. List reads the connected browser live and refuses when no browser is connected; use list to recover fresh tab handles for later calls.",
            tabs_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_navigate",
            "Navigate",
            "Go to an absolute HTTP(S) URL in a controlled tab. Reuses the obvious tab by default; set new_tab true to create one.",
            navigate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_history",
            "Browser history",
            "Move back or forward, or reload the current page. A new document invalidates prior target_ and view_ handles, so collect fresh ones afterward.",
            history_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_window",
            "Browser window",
            "Set zoom for one tab or resize the containing browser window. Either change invalidates current view_ handles, so take a fresh screenshot before more coordinate work.",
            window_schema(),
            Hints::local_mutation(false),
        ),
        tool(
            "browser_read",
            "Read page",
            "Read bounded visible text from the full composed page by default, including open shadow roots and embedded frames, or read one target. Article extraction is explicit. When the goal is a target_ handle to act on, use browser_inspect or browser_find instead.",
            read_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_inspect",
            "Inspect page",
            "Inspect semantic controls or page structure. Scopes controls, structure, and all return fresh target_ handles for later actions. Scope document returns the page outline as one bounded tree of kinds and labels, without handles, and diffs it against the previous document snapshot of the same root and depth.",
            inspect_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_find",
            "Find on page",
            "Find ranked semantic targets by visible or accessible text and receive fresh target_ handles. The fastest route to a control when you know its label.",
            find_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_screenshot",
            "Take screenshot",
            "Capture the viewport, the full page, one target, or a magnified region. Every capture returns a fresh view_ handle that later coordinate calls and region captures are built from.",
            screenshot_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_click",
            "Click",
            "Click a current target_ handle, a control found by selector, or an exact point from a current view_ screenshot. Requires action; selector lookup or an expect check also requires read. Prefer a target when one is known; points suit canvases and maps.",
            with_expect(click_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_scroll",
            "Scroll",
            "Scroll the page, defaulting to down by a medium amount, or reveal one current target_. Wheel scrolling at exact coordinates needs a current view_.",
            scroll_schema(),
            Hints::browser_action(false),
        ),
        tool(
            "browser_hover",
            "Hover",
            "Hover a current target_ handle or an exact point from a current view_ screenshot.",
            hover_schema(),
            Hints::browser_action(false),
        ),
        tool(
            "browser_fill_form",
            "Fill form",
            "Fill one to thirty form fields in one call. Requires read and write, including unsent drafts; submit_target also requires action. Nothing submits unless submit_target names the control. For credentials, acknowledge the user's explicit instruction with user_authorized_credentials.",
            with_expect(fill_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_type_text",
            "Type text",
            "Type text through real per-character keyboard input. Requires action; selector lookup or an expect check also requires read. Use browser_fill_form instead when plain values are enough. For credentials, acknowledge the user's explicit instruction with user_authorized_credentials.",
            with_expect(type_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_press_key",
            "Press key",
            "Send one named key or one literal character, optionally aimed at a current target_. Requires action; an expect check also requires read. For a shortcut such as Control+A, send key a with modifiers Control; strokes sends keys in order.",
            with_expect(key_schema()),
            Hints::browser_action(true),
        ),
        tool(
            "browser_drag",
            "Drag",
            "Drag between two current target_ handles or between two exact points from one current view_ screenshot.",
            drag_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_wait",
            "Wait",
            "Wait for one explicit observable page condition. A false condition is a decisive failed result, not an unknown effect.",
            wait_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_dialog",
            "Browser dialog",
            "Inspect, accept, dismiss, or answer the current JavaScript dialog. A dialog blocks its page until it is handled, so deal with it before other work on that tab.",
            dialog_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_upload",
            "Upload files",
            "Attach explicitly supplied local paths, bounded inline files, or one captured image to an ordinary file input, or drop one captured image at a point in a current view. Requires write; selector lookup also requires read.",
            upload_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_execute",
            "Execute JavaScript",
            "Run bounded JavaScript in the page main world. It may read, mutate, or navigate, so prefer a semantic tool when one fits.",
            evaluate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_flow",
            "Run flow",
            "Compose one to twenty ordinary tool calls. Optional tab defaults tab-scoped steps; explicit child tabs win. Later arguments may reference earlier results. Step IDs are optional for simple batches. Stops at the first failed step by default.",
            flow_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_record",
            "Record browser",
            "Record browser work as a short memory-only GIF: start, check status, stop, save, or discard. A session holds one recording at a time. Save auto-stops an active capture.",
            record_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_diagnose",
            "Diagnose page",
            "Read opt-in console and network observations for a tab. Observation begins with the first call, so earlier activity is absent; reload or reproduce to capture it. Sensitive request data is never collected.",
            diagnose_schema(),
            Hints::browser_read(),
        ),
        tool(
            "policy_explain",
            "Explain policy",
            "Read the authority in force as one compiled answer: situation sentence, one line per capability with its deciding layer, the rules behind those lines, permanent ceilings, and each layer's document. Available under every authority, including all-open; use it to learn why another call was refused or what is allowed before acting.",
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

/// The one sentence every ordinary page tool carries for its settlement preference (ADR-0195).
///
/// Thirteen tools repeat it, so it stays one short sentence: what the default does and how to
/// skip it. LANGUAGE.md carries the budget and the busy-page behavior.
const VISUAL_SETTLE: &str = "False skips the brief wait for the page to settle.";

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
        input_schema["properties"]["visual_settle"] = boolean(true, VISUAL_SETTLE);
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
                vec![(
                    "action",
                    constant(
                        "list",
                        "List this workspace's bound tabs from the live browser.",
                    ),
                )],
                vec!["action"],
            ),
            object(
                vec![
                    (
                        "action",
                        constant("focus", "Bring one exact tab and its window into view."),
                    ),
                    (
                        "tab",
                        handle("tab_", "Exact tab handle from browser_tabs list."),
                    ),
                ],
                vec!["action", "tab"],
            ),
            object(
                vec![
                    (
                        "action",
                        constant("close", "Close one exact controlled tab."),
                    ),
                    (
                        "tab",
                        handle("tab_", "Exact tab handle from browser_tabs list."),
                    ),
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
                    ("tab", handle("tab_", "Optional exact tab. Omit to use the only or sole active controlled tab.")),
                    (
                        "beforeunload",
                        enumeration(
                            &["discard"],
                            None,
                            "Explicitly discard a blocking unsaved-change prompt produced by this navigation. Default stops and reports it.",
                        ),
                    ),
                    ("new_tab", new_tab()),
                    (
                        "reuse",
                        enumeration(
                            &["domain", "never"],
                            Some("domain"),
                            "domain reuses a tab an earlier session of this client opened for this host and released (exact URL preferred); it never takes over a tab the person opened. never always creates. new_tab always creates, so domain cannot be combined with it.",
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
                            "Optional exact browser from browser_tabs list. Omit to use the browser the user attended most recently.",
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
    boolean(
        false,
        "True opens the page in a new controlled tab. Omit it for the normal call.",
    )
}

fn history_schema() -> Value {
    union(
        vec![
            history_branch("back", "Go back one page in this tab's history."),
            history_branch("forward", "Go forward one page in this tab's history."),
            object(
                vec![
                    ("action", constant("reload", "Reload the selected page.")),
                    ("tab", tab()),
                    (
                        "bypass_cache",
                        boolean(false, "Request a reload that bypasses browser cache."),
                    ),
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
                    ("action", constant("zoom", "Set visible tab zoom.")),
                    (
                        "percent",
                        integer(25, 500, None, "Whole-number zoom percentage."),
                    ),
                    ("tab", tab()),
                ],
                vec!["action", "percent"],
            ),
            object(
                vec![
                    (
                        "action",
                        constant("resize", "Resize the containing browser window."),
                    ),
                    (
                        "width",
                        integer(320, 7_680, None, "Outer browser-window width in pixels."),
                    ),
                    (
                        "height",
                        integer(240, 4_320, None, "Outer browser-window height in pixels."),
                    ),
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
                    handle(
                        "target_",
                        "Optional current target to read instead of the page.",
                    ),
                ),
                (
                    "mode",
                    enumeration(
                        &["visible", "article"],
                        None,
                        "Document reading mode; visible full-page reading is the default. Explicit article mode prefers the top document's article and falls back to the full page. Ignored with target.",
                    ),
                ),
                (
                    "max_chars",
                    integer(500, 50_000, Some(8_000), "Maximum returned characters."),
                ),
            ],
            vec![],
        ),
        vec![json!({}), json!({"mode":"article","max_chars":50_000}), json!({"target":"target_..."})],
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
                        "What semantic facts to return. Document returns a tree of {kind, label, children} nodes, a snapshot handle, and, when the previous snapshot used the same root and max_depth, a diff whose paths are positions in that tree.",
                    ),
                ),
                (
                    "root",
                    handle(
                        "target_",
                        "Optional subtree root for document scope.",
                    ),
                ),
                (
                    "max_depth",
                    // No schema default: the decoder refuses max_depth outside document scope.
                    integer(
                        1,
                        12,
                        None,
                        &format!(
                            "Levels the document scope tree may reach; {DEFAULT_DOCUMENT_DEPTH} when omitted."
                        ),
                    ),
                ),
                (
                    "max_items",
                    integer(1, 200, Some(80), "Maximum returned semantic items."),
                ),
            ],
            vec![],
        ),
        vec![json!({}), json!({"scope":"all"}), json!({"scope":"document","max_depth":8}), json!({"scope":"document","root":"target_..."})],
    )
}

fn find_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "text",
                    text(1, 500, "Literal visible or accessible text to find."),
                ),
                ("tab", tab()),
                (
                    "scope",
                    enumeration(
                        &["any", "control", "text"],
                        Some("any"),
                        "Kinds of semantic matches to return.",
                    ),
                ),
                (
                    "max_results",
                    integer(1, 50, Some(20), "Maximum ranked matches."),
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
                            "True captures the whole document instead of the viewport. Not with target or a region.",
                        ),
                    ),
                ],
                vec!["full_page"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "target",
                        handle("target_", "Current semantic target to capture."),
                    ),
                ],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "view",
                        handle(
                            "view_",
                            "Screenshot view whose image defines these coordinates; take a screenshot first, then pass its view_ handle.",
                        ),
                    ),
                    (
                        "x",
                        coordinate("Horizontal image coordinate of the region."),
                    ),
                    ("y", coordinate("Vertical image coordinate of the region.")),
                    ("width", extent("Region width in image pixels.")),
                    ("height", extent("Region height in image pixels.")),
                ],
                vec!["view", "x", "y", "width", "height"],
            ),
        ],
        vec![
            json!({}),
            json!({"full_page":true}),
            json!({"target":"target_..."}),
            json!({"view":"view_...","x":120,"y":80,"width":400,"height":300}),
        ],
    )
}

fn click_schema() -> Value {
    let common = vec![
        ("tab", tab()),
        (
            "button",
            enumeration(
                &["primary", "middle", "secondary"],
                Some("primary"),
                "Pointer button.",
            ),
        ),
        (
            "click_count",
            integer(1, 3, Some(1), "Single, double, or triple click."),
        ),
        (
            "modifiers",
            json!({"type":"array","uniqueItems":true,"default":[],"description":"Held modifier keys.","items":{"enum":["Alt","Control","Meta","Shift"]}}),
        ),
        ("selector", semantic_selector()),
    ];
    union(
        vec![
            object(
                with(
                    common.clone(),
                    (
                        "target",
                        handle("target_", "Current semantic target to click."),
                    ),
                ),
                vec!["target"],
            ),
            object(
                with_many(
                    common,
                    vec![
                        (
                            "view",
                            handle(
                                "view_",
                                "Screenshot view that defines these coordinates; take one first if you have none.",
                            ),
                        ),
                        ("x", coordinate(IMAGE_X)),
                        ("y", coordinate(IMAGE_Y)),
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
        "description": "Typed semantic selector resolved against the live document. Lookup requires read.",
        "properties": {
            "name": {"type": "string", "minLength": 1, "maxLength": 500, "description": "Required accessible-name text."},
            "role": {"type": "string", "enum": ["button","link","checkbox","radio","textbox","searchbox","combobox","listbox","select","slider","spinbutton","tab","menuitem","option","heading","image"], "description": "Optional closed role filter."},
            "exact": {"type": "boolean", "default": false, "description": "Require the whole accessible name to equal the text."}
        },
        "required": ["name"]
    })
}

fn scroll_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    ("tab", tab()),
                    (
                        "target",
                        handle("target_", "Current semantic target to reveal."),
                    ),
                ],
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
                        enumeration(
                            &["small", "medium", "large", "page"],
                            Some("medium"),
                            "Scroll distance.",
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
                        enumeration(&["up", "down"], Some("down"), SCROLL_DIRECTION),
                    ),
                    (
                        "view",
                        handle(
                            "view_",
                            "Screenshot view that defines these coordinates; take one first if you have none.",
                        ),
                    ),
                    ("x", coordinate(IMAGE_X)),
                    ("y", coordinate(IMAGE_Y)),
                    ("ticks", integer(1, 10, None, "Wheel ticks to send.")),
                ],
                vec!["direction", "view", "x", "y", "ticks"],
            ),
        ],
        vec![
            json!({}),
            json!({"direction":"down","amount":"page"}),
            json!({"target":"target_..."}),
            json!({"view":"view_...","x":120,"y":80,"direction":"down","ticks":3}),
        ],
    )
}

/// One direction description for page and wheel scrolling, so the merged field reads once.
const SCROLL_DIRECTION: &str =
    "Scroll direction. Wheel scrolling at a view point takes only up or down.";

fn hover_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    ("tab", tab()),
                    (
                        "target",
                        handle("target_", "Current semantic target to hover."),
                    ),
                ],
                vec!["target"],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "view",
                        handle(
                            "view_",
                            "Screenshot view that defines these coordinates; take one first if you have none.",
                        ),
                    ),
                    ("x", coordinate(IMAGE_X)),
                    ("y", coordinate(IMAGE_Y)),
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
        boolean(false, "Set true only when the user explicitly authorized credential entry for this request. Existing user authorization is sufficient. This does not override configured policy or human Pause/Stop."),
    )
}

fn fill_schema() -> Value {
    examples(
        object(
            vec![
                ("fields", json!({"type":"array","minItems":1,"maxItems":30,"description":"Ordinary form values to set. Each field provides exactly one of target or selector.","items":{"type":"object","additionalProperties":false,"properties":{"target":handle("target_","Current form-control target."),"selector":semantic_selector(),"value":{"description":"Literal value, including an empty value to clear. Checkboxes and radios accept booleans; numeric inputs accept finite numbers.","anyOf":[{"type":"string","maxLength":8000},{"type":"boolean"},{"type":"number"}]}},"required":["value"]}})),
                ("tab", tab()),
                ("submit_target", handle("target_", "Optional current submit control. Supplying it may produce an external effect.")),
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
                "Literal text to type. Empty text only clears, and needs clear_first true.",
            ),
        ),
        (
            "clear_first",
            boolean(false, "Clear the current value before typing."),
        ),
        credential_authorization(),
        ("tab", tab()),
    ];
    union(
        vec![
            object(
                with(
                    common.clone(),
                    ("target", handle("target_", "Current editable target.")),
                ),
                vec!["target", "text"],
            ),
            object(
                with(
                    common.clone(),
                    (
                        "focused",
                        boolean(
                            false,
                            "True types into the currently focused editable control instead of `target`.",
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
    let modifiers = (
        "modifiers",
        json!({"type":"array","uniqueItems":true,"default":[],"description":"Held modifier keys.","items":{"enum":["Alt","Control","Meta","Shift"]}}),
    );
    let shared = vec![
        ("tab", tab()),
        (
            "target",
            handle("target_", "Optional current target to receive the key."),
        ),
        modifiers,
    ];
    // Exclusive by construction: every named key is longer than one character.
    let stroke_item = json!({"description":"One literal character or supported named key.","oneOf":[{"type":"string","minLength":1,"maxLength":1},{"type":"string","enum":NAMED_KEYS}]});
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
                            json!({"type":"array","minItems":1,"maxItems":20,"description":"Ordered keystroke sequence replacing `key`.","items":stroke_item}),
                        ),
                        (
                            "repeat",
                            integer(1, 100, Some(1), "Repetitions of the whole stroke sequence."),
                        ),
                    ],
                ),
                vec!["strokes"],
            ),
        ],
        vec![
            json!({"key":"Enter"}),
            json!({"key":"a","modifiers":["Control"]}),
            json!({"strokes":["a","c"],"modifiers":["Control"]}),
            json!({"strokes":["ArrowDown"],"repeat":5}),
        ],
    )
}

fn drag_schema() -> Value {
    union(
        vec![
            object(
                vec![
                    ("source_target", handle("target_", "Current drag source.")),
                    (
                        "destination_target",
                        handle("target_", "Current drop destination."),
                    ),
                    ("tab", tab()),
                ],
                vec!["source_target", "destination_target"],
            ),
            object(
                vec![
                    (
                        "view",
                        handle("view_", "Screenshot view that defines both points; take one first if you have none."),
                    ),
                    ("start_x", coordinate("Horizontal source pixel in the view's screenshot image.")),
                    ("start_y", coordinate("Vertical source pixel in the view's screenshot image.")),
                    ("end_x", coordinate("Horizontal destination pixel in the view's screenshot image.")),
                    ("end_y", coordinate("Vertical destination pixel in the view's screenshot image.")),
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
            wait_branch("load_ready", "The page finished loading.", None),
            wait_branch("url_contains", "The page URL contains value.", Some(Text)),
            wait_branch(
                "text_present",
                "value appears in the visible page text.",
                Some(Text),
            ),
            wait_branch(
                "text_absent",
                "value is gone from the visible page text.",
                Some(Text),
            ),
            wait_branch("target_present", "target is present.", Some(Target)),
            wait_branch("target_absent", "target is gone.", Some(Target)),
            wait_branch(
                "visual_settle",
                "The page, or target when given, stopped moving and repainting.",
                Some(OptionalTarget),
            ),
            wait_branch(
                "selector_present",
                "A control matching selector exists.",
                Some(Selector),
            ),
            wait_branch(
                "duration",
                "Waits value milliseconds, then succeeds.",
                Some(Text),
            ),
        ],
        vec![
            json!({"condition":"load_ready"}),
            json!({"condition":"text_present","value":"Ready"}),
            json!({"condition":"visual_settle"}),
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
            json!({"type":"boolean","description":"Also wait briefly for the layout to settle once the condition holds. Off by default for url_contains, text_present, text_absent, and target_absent; on for the rest."}),
        ),
    ];
    let mut required = vec!["condition"];
    match input {
        None => {}
        Some(WaitInput::Text) => {
            fields.push(("value", text(1, 2_000, "Text or URL fragment for url_contains, text_present, and text_absent, page text, not field values. For duration, whole milliseconds from 0 to 10000, written as a string.")));
            required.push("value");
        }
        Some(kind @ (WaitInput::Target | WaitInput::OptionalTarget)) => {
            fields.push(("target", handle("target_", "Current target for target_present and target_absent, or the optional subject of visual_settle.")));
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
            dialog_branch("status", "Report whether a dialog is open.", false),
            dialog_branch("accept", "Accept the open dialog.", false),
            dialog_branch("dismiss", "Dismiss the open dialog.", false),
            dialog_branch("respond", "Answer a prompt with text.", true),
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
            text(0, 2_000, "Prompt response, including an empty response."),
        ));
        required.push("text");
    }
    object(fields, required)
}

fn upload_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "target",
                    handle("target_", "Current ordinary file-input target."),
                ),
                ("selector", semantic_selector()),
                (
                    "paths",
                    json!({"type":"array","minItems":1,"maxItems":5,"uniqueItems":true,"description":"Absolute local file paths selected explicitly for upload.","items":{"type":"string","minLength":1,"maxLength":4096}}),
                ),
                (
                    "files",
                    json!({"type":"array","minItems":1,"maxItems":5,"description":"Bounded inline files supplied directly.","items":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","minLength":1,"maxLength":255},"media_type":{"type":"string","maxLength":100},"data_base64":{"type":"string","description":"Standard base64 file bytes."}},"required":["name","data_base64"]}}),
                ),
                (
                    "source_image",
                    handle("image_","Captured image_ handle from an earlier screenshot. Attach it to a file input, or drop it at a view point."),
                ),
                (
                    "view",
                    handle("view_","Current screenshot view that defines the drop coordinates; requires source_image."),
                ),
                ("x", coordinate(IMAGE_X)),
                ("y", coordinate(IMAGE_Y)),
                ("tab", tab()),
            ],
            vec![],
        ),
        vec![
            json!({"target":"target_...","paths":[EXAMPLE_UPLOAD_PATH]}),
            json!({"target":"target_...","files":[{"name":"notes.txt","media_type":"text/plain","data_base64":"SGk="}]}),
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
                (
                    "script",
                    text(
                        1,
                        20_000,
                        "JavaScript source evaluated in the page main world.",
                    ),
                ),
                ("tab", tab()),
                (
                    "max_result_chars",
                    integer(
                        100,
                        20_000,
                        Some(8_000),
                        "Maximum serialized result characters.",
                    ),
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
                    json!({"type":"array","minItems":1,"maxItems":super::history::COMPOSITION_STEP_LIMIT,"description":"Ordinary tool calls executed in order. Omitted ids become step_1, step_2, and so on.","items":{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string","minLength":1,"maxLength":64,"description":"Optional unique step id for result references."},"tool":{"type":"string","description":"Current advertised non-composite Ghostlight tool."},"arguments":{"type":"object","description":"Arguments for that tool. Any value may be an explicit reference object: {\"flow_ref\":{\"step\":\"earlier_id\",\"pointer\":\"/facts/...\"}}."}},"required":["tool"]}}),
                ),
                (
                    "on_error",
                    enumeration(
                        &["stop", "continue"],
                        Some("stop"),
                        "Whether later steps run after a failed step.",
                    ),
                ),
                ("tab", tab()),
            ],
            vec!["steps"],
        ),
        vec![
            json!({"steps":[{"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com"}},{"id":"read","tool":"browser_read","arguments":{"max_chars":2000}}]}),
            json!({"steps":[{"tool":"browser_inspect"},{"tool":"browser_read","arguments":{"max_chars":2000}}]}),
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
                        constant("start", "Start a bounded memory-only recording."),
                    ),
                    ("tab", tab()),
                ],
                vec!["action"],
            ),
            record_branch("status", "Return content-free recording state and bounds."),
            record_branch("stop", "Stop capture and keep the frozen frames in memory."),
            object(
                vec![
                    (
                        "action",
                        constant(
                            "save",
                            "Save an immutable animated GIF. Requires read for captured sources, plus write when attaching to a target; active capture stops after capability admission.",
                        ),
                    ),
                    (
                        "target",
                        handle(
                            "target_",
                            "Attach the replay to this current file input, inside the browser. Requires read + write.",
                        ),
                    ),
                    (
                        "download",
                        boolean(
                            false,
                            "Let the browser save the replay as a file. It chooses where.",
                        ),
                    ),
                ],
                vec!["action"],
            ),
            record_branch("discard", "Erase captured bytes. This cannot be undone."),
        ],
        vec![
            json!({"action":"start"}),
            json!({"action":"stop"}),
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
                ("source", enumeration(&["both","console","network"],Some("both"),"Observation source.")),
                ("detail", enumeration(&["problems","all"],Some("problems"),"Problems returns warnings, errors, failed requests, and HTTP failures.")),
                ("match", text(1,500,"Optional case-insensitive literal filter; regular expressions are not accepted.")),
                ("after", handle("diag_","Opaque continuation cursor from a prior diagnostic result.")),
                ("limit", integer(1,200,Some(50),"Maximum returned observations.")),
            ],
            vec![],
        ),
        vec![json!({}),json!({"source":"console","detail":"all","limit":100})],
    )
}

fn object(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    raw_object(fields, required)
}

fn raw_object(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    let properties: BTreeMap<_, _> = fields.into_iter().collect();
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
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
            .expect("union branches declare required fields")
            .iter()
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

    json!({
        "type":"object",
        "additionalProperties":false,
        "properties":properties,
        "required":required.unwrap_or_default(),
        "examples":examples
    })
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
        "description":"Optional condition checked after the effect. Requires read as well as the action's capabilities, admitted before the effect. A failed check preserves the applied effect.",
        "properties":{
            "condition":{"type":"string","enum":["load_ready","url_contains","text_present","text_absent"]},
            "value":{"type":"string","minLength":1,"maxLength":MAX_POSTCONDITION_VALUE_CHARS,"description":"Required text for url_contains, text_present, and text_absent, page text, not field values; omit for load_ready."}
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

fn constant(value: &str, description: &str) -> Value {
    json!({"type":"string","const":value,"description":description})
}

fn boolean(default: bool, description: &str) -> Value {
    json!({"type":"boolean","default":default,"description":description})
}

fn text(minimum: usize, maximum: usize, description: &str) -> Value {
    json!({"type":"string","minLength":minimum,"maxLength":maximum,"description":description})
}

fn integer(minimum: usize, maximum: usize, default: Option<usize>, description: &str) -> Value {
    let mut schema =
        json!({"type":"integer","minimum":minimum,"maximum":maximum,"description":description});
    if let Some(default) = default {
        schema
            .as_object_mut()
            .expect("integer schema is an object")
            .insert("default".into(), json!(default));
    }
    schema
}

fn enumeration(values: &[&str], default: Option<&str>, description: &str) -> Value {
    let mut schema = json!({"type":"string","enum":values,"description":description});
    if let Some(default) = default {
        schema
            .as_object_mut()
            .expect("enum schema is an object")
            .insert("default".into(), json!(default));
    }
    schema
}

fn handle(prefix: &str, description: &str) -> Value {
    json!({"type":"string","minLength":prefix.len()+1,"maxLength":160,"pattern":format!("^{prefix}.+$"),"description":description})
}

fn tab() -> Value {
    handle(
        "tab_",
        "Optional exact controlled tab. Omit only when one tab is unambiguous.",
    )
}

fn url() -> Value {
    json!({"type":"string","format":"uri","pattern":"^https?://","maxLength":4096,"description":"Absolute HTTP(S) URL."})
}

/// The `timeout_ms` field every tool accepts, defaulting to what that tool has always been given.
fn timeout(tool: &str) -> Value {
    let default = match tool {
        "browser_fill_form" => DEFAULT_FILL_TIMEOUT_MS,
        "browser_record" => DEFAULT_RECORD_TIMEOUT_MS,
        _ => DEFAULT_TIMEOUT_MS,
    };
    json!({"type":"integer","minimum":MIN_TIMEOUT_MS,"maximum":MAX_TIMEOUT_MS,"default":default,"description":"Maximum call time in milliseconds."})
}

/// A pointer x coordinate. Views map image pixels through their own scale, which is not 1 for
/// full-page and magnified captures, so page CSS pixels would land in the wrong place.
const IMAGE_X: &str = "Horizontal pixel in the view's screenshot image, from its left edge.";

/// A pointer y coordinate, in the same image pixels as `IMAGE_X`.
const IMAGE_Y: &str = "Vertical pixel in the view's screenshot image, from its top edge.";

fn coordinate(description: &str) -> Value {
    json!({"type":"number","minimum":0,"maximum":1_000_000,"description":description})
}

fn extent(description: &str) -> Value {
    json!({"type":"number","exclusiveMinimum":0,"maximum":1_000_000,"description":description})
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
