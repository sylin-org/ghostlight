//! The deterministic model-facing tool catalog and its truthful JSON Schemas.

use std::collections::BTreeMap;

use ghostlight_bridge::service::{ToolAnnotations, ToolDefinition};
use serde_json::{json, Value};

use super::{CAPABILITIES, DEFAULT_TIMEOUT_MS, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, NAMED_KEYS};

/// Return the complete native Ghostlight language in deterministic order.
#[must_use]
pub fn catalog() -> Vec<ToolDefinition> {
    vec![
        tool(
            "browser_tabs",
            "Browser tabs",
            "List controlled tabs, focus one exact tab, or close one exact tab. Use list to recover current tab_ handles.",
            tabs_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_navigate",
            "Navigate",
            "Go to an absolute HTTP(S) URL. Reuses the obvious controlled tab by default; set new_tab true to create one.",
            navigate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_history",
            "Browser history",
            "Move back or forward, or reload the current page. A new document invalidates prior target_ and view_ handles.",
            history_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_window",
            "Browser window",
            "Set tab zoom or resize the containing browser window. Either change invalidates current view_ handles.",
            window_schema(),
            Hints::local_mutation(false),
        ),
        tool(
            "browser_read",
            "Read page",
            "Read bounded useful prose from a page or target. Use browser_inspect or browser_find when you need target_ handles.",
            read_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_inspect",
            "Inspect page",
            "Inspect semantic controls or structure and receive fresh target_ handles for later actions.",
            inspect_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_find",
            "Find on page",
            "Find ranked semantic targets by visible or accessible text and receive fresh target_ handles.",
            find_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_screenshot",
            "Take screenshot",
            "Capture the viewport, full page, or one semantic target. The result includes a view_ handle for coordinate actions.",
            screenshot_schema(),
            Hints::browser_read(),
        ),
        tool(
            "browser_click",
            "Click",
            "Click a current target_ handle or an exact point from a current view_ screenshot.",
            click_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_scroll",
            "Scroll",
            "Scroll the page, defaulting to down by a medium amount, or reveal one current target_ handle.",
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
            "Fill several ordinary form fields. It does not submit unless submit_target is supplied; credential fields stay with the user.",
            fill_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_type_text",
            "Type text",
            "Type through per-character browser input events. Prefer browser_fill_form when typing events do not matter.",
            type_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_press_key",
            "Press key",
            "Send one named key or one literal character, optionally to a current target_ handle.",
            key_schema(),
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
            "Inspect, accept, dismiss, or respond to the current JavaScript dialog.",
            dialog_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_upload",
            "Upload files",
            "Upload one to five explicitly named absolute local paths to an ordinary file input target.",
            upload_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_execute",
            "Execute JavaScript",
            "Execute explicit bounded JavaScript in the page main world. It may read, mutate, or navigate the page.",
            evaluate_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_sequence",
            "Run sequence",
            "Run two to eight fully specified click, fill, type, key, scroll, hover, or wait steps on one tab.",
            sequence_call_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_record",
            "Record browser",
            "Start, inspect, stop, save, or discard a bounded memory-only browser recording. Save auto-stops; omit recording only when exactly one is eligible.",
            record_schema(),
            Hints::browser_action(true),
        ),
        tool(
            "browser_diagnose",
            "Diagnose page",
            "Read bounded opt-in console and network observations. Defaults to problems from both sources; sensitive request data is never collected.",
            diagnose_schema(),
            Hints::browser_read(),
        ),
    ]
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
            "invocation": {"type":"string","pattern":"^invocation_.+$","description":"Opaque invocation handle."},
            "status": {"type":"string","enum":["succeeded","blocked","failed","cancelled","attention_required","unknown"]},
            "effect": {"type":"string","enum":["none","applied","partial","unknown"]},
            "readiness": {"type":"string","enum":["not_applicable","loading","interactive","complete","unknown"]},
            "repeat_safe": {"type":"boolean","description":"Whether repeating the same call is known safe."},
            "summary": {"type":"string","maxLength":500},
            "facts": {"type":"object","description":"Tool-specific canonical facts."},
            "next_steps": {"type":"array","maxItems":2,"items":{"type":"string"}}
        },
        "required": ["invocation","status","effect","readiness","repeat_safe","summary","facts","next_steps"]
    })
}

fn tabs_schema() -> Value {
    union(
        vec![
            object(
                vec![("action", constant("list", "List controlled tabs."))],
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
                    ("new_tab", constant_bool(false, "Reuse a tab. Omit this field for the normal call.")),
                    ("timeout_ms", timeout()),
                ],
                vec!["url"],
            ),
            object(
                vec![
                    ("url", url()),
                    ("new_tab", constant_bool(true, "Create a new controlled tab.")),
                    (
                        "browser",
                        handle(
                            "browser_",
                            "Optional exact browser from browser_tabs list. Omit to use the browser the user attended most recently.",
                        ),
                    ),
                    ("timeout_ms", timeout()),
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

fn history_schema() -> Value {
    union(
        vec![
            history_branch("back"),
            history_branch("forward"),
            object(
                vec![
                    ("action", constant("reload", "Reload the selected page.")),
                    ("tab", tab()),
                    (
                        "bypass_cache",
                        boolean(false, "Request a reload that bypasses browser cache."),
                    ),
                    ("timeout_ms", timeout()),
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

fn history_branch(action: &str) -> Value {
    object(
        vec![
            ("action", constant(action, "Traverse browser history.")),
            ("tab", tab()),
            ("timeout_ms", timeout()),
        ],
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
                    "max_chars",
                    integer(500, 20_000, Some(8_000), "Maximum returned characters."),
                ),
            ],
            vec![],
        ),
        vec![json!({}), json!({"target":"target_..."})],
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
                        &["controls", "structure", "all"],
                        Some("controls"),
                        "What semantic facts to return.",
                    ),
                ),
                (
                    "max_items",
                    integer(1, 200, Some(80), "Maximum returned semantic items."),
                ),
            ],
            vec![],
        ),
        vec![json!({}), json!({"scope":"all"})],
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
            object(
                vec![
                    ("tab", tab()),
                    (
                        "full_page",
                        constant_bool(false, "Capture the visible viewport. Usually omit."),
                    ),
                    ("timeout_ms", timeout()),
                ],
                vec![],
            ),
            object(
                vec![
                    ("tab", tab()),
                    (
                        "full_page",
                        constant_bool(true, "Capture the full document."),
                    ),
                    ("timeout_ms", timeout()),
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
                    (
                        "full_page",
                        constant_bool(false, "Target capture is never full-page."),
                    ),
                    ("timeout_ms", timeout()),
                ],
                vec!["target"],
            ),
        ],
        vec![
            json!({}),
            json!({"full_page":true}),
            json!({"target":"target_..."}),
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
            integer(1, 2, Some(1), "Single or double click."),
        ),
        ("timeout_ms", timeout()),
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
                                "Current screenshot view that defines the coordinates.",
                            ),
                        ),
                        ("x", coordinate("Horizontal CSS coordinate in the view.")),
                        ("y", coordinate("Vertical CSS coordinate in the view.")),
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
                    ("timeout_ms", timeout()),
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
                            "Scroll direction.",
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
                    ("timeout_ms", timeout()),
                ],
                vec![],
            ),
        ],
        vec![
            json!({}),
            json!({"direction":"down","amount":"page"}),
            json!({"target":"target_..."}),
        ],
    )
}

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
                    ("timeout_ms", timeout()),
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
                            "Current screenshot view that defines the coordinates.",
                        ),
                    ),
                    ("x", coordinate("Horizontal CSS coordinate in the view.")),
                    ("y", coordinate("Vertical CSS coordinate in the view.")),
                    ("timeout_ms", timeout()),
                ],
                vec!["view", "x", "y"],
            ),
        ],
        vec![json!({"target":"target_..."})],
    )
}

fn fill_schema() -> Value {
    examples(
        object(
            vec![
                ("fields", json!({"type":"array","minItems":1,"maxItems":30,"description":"Ordinary form values to set.","items":{"type":"object","additionalProperties":false,"properties":{"target":handle("target_","Current form-control target."),"value":{"type":"string","maxLength":8000,"description":"Literal value, including an empty value to clear."}},"required":["target","value"]}})),
                ("tab", tab()),
                ("submit_target", handle("target_", "Optional current submit control. Supplying it may produce an external effect.")),
                ("timeout_ms", timeout()),
            ],
            vec!["fields"],
        ),
        vec![json!({"fields":[{"target":"target_...","value":"Ada"}]})],
    )
}

fn type_schema() -> Value {
    let common = vec![
        ("target", handle("target_", "Current editable target.")),
        ("tab", tab()),
        ("timeout_ms", timeout()),
    ];
    union(
        vec![
            object(
                with_many(
                    common.clone(),
                    vec![
                        ("text", text(1, 8_000, "Literal text to type.")),
                        (
                            "clear_first",
                            boolean(false, "Clear the current value before typing."),
                        ),
                    ],
                ),
                vec!["target", "text"],
            ),
            object(
                with_many(
                    common,
                    vec![
                        (
                            "text",
                            constant("", "Empty text is valid only for an explicit clear."),
                        ),
                        (
                            "clear_first",
                            constant_bool(true, "Clear the current value."),
                        ),
                    ],
                ),
                vec!["target", "text", "clear_first"],
            ),
        ],
        vec![json!({"target":"target_...","text":"hello"})],
    )
}

fn key_schema() -> Value {
    examples(
        object(
            vec![
                (
                    "key",
                    json!({"description":"One literal character or supported named key.","oneOf":[{"type":"string","minLength":1,"maxLength":1},{"type":"string","enum":NAMED_KEYS}]}),
                ),
                ("tab", tab()),
                (
                    "target",
                    handle("target_", "Optional current target to receive the key."),
                ),
                (
                    "modifiers",
                    json!({"type":"array","uniqueItems":true,"default":[],"description":"Held modifier keys.","items":{"enum":["Alt","Control","Meta","Shift"]}}),
                ),
            ],
            vec!["key"],
        ),
        vec![
            json!({"key":"Enter"}),
            json!({"key":"a","modifiers":["Control"]}),
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
                    ("timeout_ms", timeout()),
                ],
                vec!["source_target", "destination_target"],
            ),
            object(
                vec![
                    (
                        "view",
                        handle("view_", "Current screenshot view that defines both points."),
                    ),
                    ("start_x", coordinate("Horizontal source coordinate.")),
                    ("start_y", coordinate("Vertical source coordinate.")),
                    ("end_x", coordinate("Horizontal destination coordinate.")),
                    ("end_y", coordinate("Vertical destination coordinate.")),
                    ("tab", tab()),
                    ("timeout_ms", timeout()),
                ],
                vec!["view", "start_x", "start_y", "end_x", "end_y"],
            ),
        ],
        vec![json!({"source_target":"target_...","destination_target":"target_..."})],
    )
}

fn wait_schema() -> Value {
    union(
        vec![
            wait_branch("load_ready", None, None),
            wait_branch("url_contains", Some("Literal URL fragment."), None),
            wait_branch("text_present", Some("Literal text that must appear."), None),
            wait_branch(
                "text_absent",
                Some("Literal text that must disappear."),
                None,
            ),
            wait_branch(
                "target_present",
                None,
                Some("Current target that must be present."),
            ),
            wait_branch(
                "target_absent",
                None,
                Some("Current target that must disappear."),
            ),
        ],
        vec![
            json!({"condition":"load_ready"}),
            json!({"condition":"text_present","value":"Ready"}),
        ],
    )
}

fn wait_branch(
    condition: &str,
    value_description: Option<&str>,
    target_description: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("condition", constant(condition, "Observable condition.")),
        ("tab", tab()),
        ("timeout_ms", timeout()),
    ];
    let mut required = vec!["condition"];
    if let Some(description) = value_description {
        fields.push(("value", text(1, 2_000, description)));
        required.push("value");
    }
    if let Some(description) = target_description {
        fields.push(("target", handle("target_", description)));
        required.push("target");
    }
    object(fields, required)
}

fn dialog_schema() -> Value {
    union(
        vec![
            dialog_branch("status", false),
            dialog_branch("accept", false),
            dialog_branch("dismiss", false),
            dialog_branch("respond", true),
        ],
        vec![
            json!({"action":"status"}),
            json!({"action":"respond","text":"yes"}),
        ],
    )
}

fn dialog_branch(action: &str, text_required: bool) -> Value {
    let mut fields = vec![
        ("action", constant(action, "Dialog action.")),
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
                (
                    "paths",
                    json!({"type":"array","minItems":1,"maxItems":5,"uniqueItems":true,"description":"Absolute local file paths selected explicitly for upload.","items":{"type":"string","minLength":1,"maxLength":4096}}),
                ),
                ("tab", tab()),
                ("timeout_ms", timeout()),
            ],
            vec!["target", "paths"],
        ),
        vec![json!({"target":"target_...","paths":["C:\\absolute\\file.txt"]})],
    )
}

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
                ("timeout_ms", timeout()),
            ],
            vec!["script"],
        ),
        vec![json!({"script":"document.title"})],
    )
}

fn sequence_call_schema() -> Value {
    examples(
        object(
            vec![
                ("steps", sequence_steps()),
                ("tab", tab()),
                ("timeout_ms", timeout()),
            ],
            vec!["steps"],
        ),
        vec![
            json!({"steps":[{"action":"click","target":"target_..."},{"action":"wait","condition":"load_ready"}]}),
        ],
    )
}

fn sequence_steps() -> Value {
    let mut branches = vec![
        step_object(
            vec![
                ("action", constant("click", "Click step.")),
                ("target", handle("target_", "Current target.")),
                (
                    "button",
                    enumeration(
                        &["primary", "middle", "secondary"],
                        Some("primary"),
                        "Pointer button.",
                    ),
                ),
                ("click_count", integer(1, 2, Some(1), "Click count.")),
            ],
            vec!["action", "target"],
        ),
        step_object(
            vec![
                ("action", constant("fill", "Fill step.")),
                ("target", handle("target_", "Current form target.")),
                ("value", text(0, 8_000, "Literal field value.")),
            ],
            vec!["action", "target", "value"],
        ),
    ];
    branches.extend(sequence_type_steps());
    branches.push(step_object(
        vec![
            ("action", constant("press_key", "Keyboard step.")),
            (
                "key",
                json!({"oneOf":[{"type":"string","minLength":1,"maxLength":1},{"type":"string","enum":NAMED_KEYS}]}),
            ),
            ("target", handle("target_", "Optional current target.")),
            (
                "modifiers",
                json!({"type":"array","uniqueItems":true,"default":[],"items":{"enum":["Alt","Control","Meta","Shift"]}}),
            ),
        ],
        vec!["action", "key"],
    ));
    branches.extend(sequence_scroll_steps());
    branches.push(step_object(
        vec![
            ("action", constant("hover", "Hover step.")),
            ("target", handle("target_", "Current target.")),
        ],
        vec!["action", "target"],
    ));
    branches.extend(sequence_wait_steps());

    json!({
        "type":"array",
        "minItems":2,
        "maxItems":8,
        "description":"Fully specified same-tab steps executed in order.",
        "items":{"oneOf":branches}
    })
}

fn sequence_type_steps() -> Vec<Value> {
    let common = vec![
        ("action", constant("type_text", "Typing step.")),
        ("target", handle("target_", "Current editable target.")),
    ];
    vec![
        step_object(
            with_many(
                common.clone(),
                vec![
                    ("text", text(1, 8_000, "Literal text.")),
                    ("clear_first", boolean(false, "Clear first.")),
                ],
            ),
            vec!["action", "target", "text"],
        ),
        step_object(
            with_many(
                common,
                vec![
                    (
                        "text",
                        constant("", "Empty text is valid only for an explicit clear."),
                    ),
                    (
                        "clear_first",
                        constant_bool(true, "Clear the current value."),
                    ),
                ],
            ),
            vec!["action", "target", "text", "clear_first"],
        ),
    ]
}

fn sequence_scroll_steps() -> Vec<Value> {
    vec![
        step_object(
            vec![
                ("action", constant("scroll", "Scroll step.")),
                ("target", handle("target_", "Current target to reveal.")),
            ],
            vec!["action", "target"],
        ),
        step_object(
            vec![
                ("action", constant("scroll", "Scroll step.")),
                (
                    "direction",
                    enumeration(
                        &["up", "down", "left", "right"],
                        Some("down"),
                        "Scroll direction.",
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
            vec!["action"],
        ),
    ]
}

fn sequence_wait_steps() -> Vec<Value> {
    vec![
        sequence_wait_branch("load_ready", None, None),
        sequence_wait_branch("url_contains", Some("Literal URL fragment."), None),
        sequence_wait_branch("text_present", Some("Literal text that must appear."), None),
        sequence_wait_branch(
            "text_absent",
            Some("Literal text that must disappear."),
            None,
        ),
        sequence_wait_branch(
            "target_present",
            None,
            Some("Current target that must be present."),
        ),
        sequence_wait_branch(
            "target_absent",
            None,
            Some("Current target that must disappear."),
        ),
    ]
}

fn sequence_wait_branch(
    condition: &str,
    value_description: Option<&str>,
    target_description: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("action", constant("wait", "Wait step.")),
        ("condition", constant(condition, "Observable condition.")),
    ];
    let mut required = vec!["action", "condition"];
    if let Some(description) = value_description {
        fields.push(("value", text(1, 2_000, description)));
        required.push("value");
    }
    if let Some(description) = target_description {
        fields.push(("target", handle("target_", description)));
        required.push("target");
    }
    step_object(fields, required)
}

fn step_object(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    raw_object(fields, required)
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
            record_id_branch("status", "Return content-free recording state and bounds."),
            record_id_branch("stop", "Stop capture and keep the frozen frames in memory."),
            object(
                vec![
                    (
                        "action",
                        constant(
                            "save",
                            "Save an immutable animated GIF; active capture stops first.",
                        ),
                    ),
                    ("recording", recording()),
                    (
                        "target",
                        handle(
                            "target_",
                            "Attach the replay to this current file input, inside the browser.",
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
            record_id_branch("discard", "Erase captured bytes. This cannot be undone."),
        ],
        vec![
            json!({"action":"start"}),
            json!({"action":"stop"}),
            json!({"action":"save"}),
        ],
    )
}

fn record_id_branch(action: &str, description: &str) -> Value {
    object(
        vec![
            ("action", constant(action, description)),
            ("recording", recording()),
        ],
        vec!["action"],
    )
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
    let mut fields = fields;
    fields.push((
        "restrict_hosts",
        json!({"type":"array","minItems":1,"uniqueItems":true,"description":"Optional host patterns that can only narrow this call's authority. Usually omit.","items":{"type":"string","minLength":1,"maxLength":253,"pattern":"^(\\*\\.)?[^/:*]+$"}}),
    ));
    fields.push((
        "restrict_capabilities",
        json!({"type":"array","minItems":1,"uniqueItems":true,"description":"Optional capabilities that can only narrow this call's authority. Usually omit.","items":{"type":"string","enum":CAPABILITIES}}),
    ));
    raw_object(fields, required)
}

fn raw_object(fields: Vec<(&str, Value)>, required: Vec<&str>) -> Value {
    let properties: BTreeMap<_, _> = fields.into_iter().collect();
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}

fn union(branches: Vec<Value>, examples: Vec<Value>) -> Value {
    json!({"type":"object","oneOf":branches,"examples":examples})
}

fn examples(mut schema: Value, values: Vec<Value>) -> Value {
    schema
        .as_object_mut()
        .expect("schema is an object")
        .insert("examples".into(), Value::Array(values));
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

fn constant_bool(value: bool, description: &str) -> Value {
    json!({"type":"boolean","const":value,"description":description})
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

fn recording() -> Value {
    handle(
        "recording_",
        "Optional recording handle. Omit only when exactly one recording is eligible.",
    )
}

fn url() -> Value {
    json!({"type":"string","format":"uri","pattern":"^https?://","maxLength":4096,"description":"Absolute HTTP(S) URL."})
}

fn timeout() -> Value {
    json!({"type":"integer","minimum":MIN_TIMEOUT_MS,"maximum":MAX_TIMEOUT_MS,"default":DEFAULT_TIMEOUT_MS,"description":"Maximum call time in milliseconds."})
}

fn coordinate(description: &str) -> Value {
    json!({"type":"number","minimum":0,"maximum":1_000_000,"description":description})
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::catalog;

    const EXPECTED_TOOL_NAMES: [&str; 22] = [
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
        "browser_sequence",
        "browser_record",
        "browser_diagnose",
    ];

    #[test]
    fn catalog_is_exact_deterministic_and_fully_described() {
        let tools = catalog();
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
            ("browser_sequence", false, true, false, true),
            ("browser_record", false, true, false, true),
            ("browser_diagnose", true, false, true, true),
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
    fn screenshot_target_branch_advertises_explicit_false_full_page() {
        let schema = tool_schema("browser_screenshot");
        let branches = schema
            .get("oneOf")
            .and_then(Value::as_array)
            .expect("screenshot branches");
        let target_branch = branches
            .iter()
            .find(|branch| is_required(branch, "target"))
            .expect("target screenshot branch");

        assert_eq!(
            property(target_branch, "full_page").and_then(|value| value.get("const")),
            Some(&Value::Bool(false))
        );
        assert!(!is_required(target_branch, "full_page"));
        assert_eq!(
            target_branch.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
    }

    #[test]
    fn sequence_schema_discriminates_type_scroll_and_wait_shapes() {
        let schema = tool_schema("browser_sequence");
        let branches = schema
            .pointer("/properties/steps/items/oneOf")
            .and_then(Value::as_array)
            .expect("sequence step branches");
        assert_eq!(branches.len(), 14);
        assert!(branches
            .iter()
            .all(|branch| { branch.get("additionalProperties") == Some(&Value::Bool(false)) }));

        let type_steps = action_branches(branches, "type_text");
        assert_eq!(type_steps.len(), 2);
        let clear_step = type_steps
            .iter()
            .find(|branch| {
                property(branch, "text").and_then(|value| value.get("const"))
                    == Some(&Value::String(String::new()))
            })
            .expect("explicit clear step");
        assert_eq!(
            property(clear_step, "clear_first").and_then(|value| value.get("const")),
            Some(&Value::Bool(true))
        );
        assert!(is_required(clear_step, "clear_first"));
        let nonempty_step = type_steps
            .iter()
            .find(|branch| {
                property(branch, "text").and_then(|value| value.get("minLength"))
                    == Some(&Value::from(1))
            })
            .expect("non-empty typing step");
        assert_eq!(
            property(nonempty_step, "clear_first").and_then(|value| value.get("default")),
            Some(&Value::Bool(false))
        );

        let scroll_steps = action_branches(branches, "scroll");
        assert_eq!(scroll_steps.len(), 2);
        let target_scroll = scroll_steps
            .iter()
            .find(|branch| property(branch, "target").is_some())
            .expect("target scroll step");
        assert!(is_required(target_scroll, "target"));
        assert!(property(target_scroll, "direction").is_none());
        assert!(property(target_scroll, "amount").is_none());
        let directional_scroll = scroll_steps
            .iter()
            .find(|branch| property(branch, "target").is_none())
            .expect("directional scroll step");
        assert_eq!(
            property(directional_scroll, "direction").and_then(|value| value.get("default")),
            Some(&Value::String("down".into()))
        );
        assert_eq!(
            property(directional_scroll, "amount").and_then(|value| value.get("default")),
            Some(&Value::String("medium".into()))
        );

        let wait_steps = action_branches(branches, "wait");
        assert_eq!(wait_steps.len(), 6);
        for (condition, required_field) in [
            ("load_ready", None),
            ("url_contains", Some("value")),
            ("text_present", Some("value")),
            ("text_absent", Some("value")),
            ("target_present", Some("target")),
            ("target_absent", Some("target")),
        ] {
            let branch = wait_steps
                .iter()
                .find(|branch| {
                    property(branch, "condition").and_then(|value| value.get("const"))
                        == Some(&Value::String(condition.into()))
                })
                .unwrap_or_else(|| panic!("missing {condition} wait branch"));
            assert_eq!(
                property(branch, "value").is_some(),
                required_field == Some("value")
            );
            assert_eq!(
                property(branch, "target").is_some(),
                required_field == Some("target")
            );
            if let Some(field) = required_field {
                assert!(
                    is_required(branch, field),
                    "{condition} must require {field}"
                );
            }
        }
    }

    fn tool_schema(name: &str) -> Value {
        catalog()
            .into_iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .input_schema
    }

    fn action_branches<'a>(branches: &'a [Value], action: &str) -> Vec<&'a Value> {
        branches
            .iter()
            .filter(|branch| {
                property(branch, "action").and_then(|value| value.get("const"))
                    == Some(&Value::String(action.into()))
            })
            .collect()
    }

    fn property<'a>(branch: &'a Value, name: &str) -> Option<&'a Value> {
        branch.get("properties").and_then(|value| value.get(name))
    }

    fn is_required(branch: &Value, name: &str) -> bool {
        branch
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| required.iter().any(|value| value.as_str() == Some(name)))
    }
}
