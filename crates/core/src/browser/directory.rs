// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The tool registry of ADR-0024 Decision 1: one [`ToolDescriptor`] per advertised tool,
//! generalizing the ADR-0022 Decision 2 action directory IN PLACE into the single per-tool
//! authority that drives validity, classification (via [`requires`]), advertisement, explain,
//! resource shape, dispatch kind, and result post-processing. This is now the sole
//! enforcement, advertisement, and audit authority (ADR-0022 Decision 8, s06, generalized by
//! ADR-0024 Decision 1); the earlier observe/mutate classification table is deleted. It also
//! backs the `explain` tool (ADR-0022 Decision 7, s07): [`explain_text`] renders this same
//! table as the tool's deterministic response text.
//!
//! Design rule (ADR-0024 Decision 1): descriptors are DATA; the pipeline owns BEHAVIOR. A hook
//! is a plain `fn` value only where the behavior is pure and self-contained
//! ([`ToolDescriptor::postprocess`], [`Handler::Local`]). Behavior that needs the `Browser`
//! handle or the governance core (the navigate landing re-check, the about:blank park,
//! resource resolution round-trips) is expressed as an enum MARKER ([`PostDispatch`]) the
//! pipeline interprets, never as a closure captured here. There is deliberately no per-tool
//! trait with 14 impls: 13 would be identical boilerplate, which multiplies moving parts
//! instead of reducing them. A future special case becomes one field on one descriptor row.
//!
//! The family seam (deliberate): this descriptor shape is intentionally plugin-manifest-like
//! -- name, actions, capability requirements, resource shapes, handler kind, descriptions --
//! so that a future sibling plugin (desktop-mcp and others; see
//! docs/design/ghostlight-service-architecture.md) can declare the same kind of table to the
//! same kind of governor without a rewrite. No family API is built now; the decision here is
//! only to keep this shape close to "what a plugin would declare".
//!
//! Absent-vs-empty invariant (ADR-0022 Decision 2), unchanged: [`requires`] returning `None`
//! is a classification MISS -- the action has no registry entry, and callers must deny it
//! (fail closed). `Some(&[])` means the action's bound requirement set is genuinely empty --
//! it is unconditionally allowed, no resource resolution or grant scan needed. The two states
//! are never to be conflated: `None` and `Some(&[])` are distinct outcomes with opposite
//! consequences.
//!
//! The registry IS the single source (ADR-0034 Decision 4): regression snapshot tests pin
//! the declared surface's structural invariants (names in order, the `computer` action enum,
//! no gaps, no duplicates).
//!
//! The module is pure: no I/O, no allocation beyond what slice iteration needs, no
//! dependencies beyond `core`/`std` plus the `serde_json::Value` type named in one function
//! pointer signature (never constructed here).

use crate::governance::ports::Capability;
use crate::mcp::outcome::{LocalCtx, LocalFuture};
use serde_json::{json, Value};

/// The resource-shape classification driving GRANT-PATH resource resolution only (ADR-0024
/// Decision 1), mirroring today's `resolve_governing_resource` name match exactly. This is NOT
/// used to decide whether the sacred STEP B tab check runs: that check is ARGUMENT-driven (any
/// call carrying a numeric `tabId`), independent of this shape, because tool arguments are not
/// schema-validated and a never-touch check must never be gated by a classification that could
/// itself be wrong for a malformed call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceShape {
    /// No governed resource: `tabs_context_mcp`, `tabs_create_mcp`, `update_plan`, `narrate`,
    /// `explain`.
    DomainLess,
    /// Governed by the tab's current URL, resolved via `tabId`: `read_page`, `computer`,
    /// `find`, `form_input`, and the other tab-scoped tools.
    TabScoped,
    /// Governed by the `url` argument: `navigate` only (extension-mirrored normalization).
    TargetArg,
    /// Existing in-memory recording authority, with no live page lookup. Used for status, stop,
    /// clear, and client-side export after capture authority has already been established.
    RecordingScoped,
}

/// How a call is dispatched once authorized (ADR-0024 Decision 1, grown async by ADR-0035
/// Decision 6).
#[derive(Clone, Copy)]
pub enum Handler {
    /// The default: forward to the extension over native messaging via `Browser::call`. Most
    /// registry rows use this.
    ExtensionForward,
    /// Answered entirely inside the binary: `explain`, and (additively) `script`/`form_fill`.
    /// An async, context-bearing handler (ADR-0035 Decision 6): receives a [`LocalCtx`] and
    /// returns a [`crate::mcp::outcome::CallOutcome`] behind a boxed, pinned future,
    /// since Rust has no native `async fn` pointer type. Dispatch position depends on the
    /// tool's `action: None` variant requirement set (PINS.md SS2): empty answers in the
    /// free-action arm (where `explain`/`script` answer); non-empty falls through sacred +
    /// grant enforcement first and answers at the post-grant position (`form_fill`).
    Local(for<'a> fn(LocalCtx<'a>) -> LocalFuture<'a>),
}

/// A marker for post-dispatch behavior that needs the `Browser` handle or the governance core
/// (ADR-0024 Decision 1: markers, not closures, for Browser-dependent behavior).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostDispatch {
    /// No post-dispatch behavior.
    None,
    /// The navigate landing re-check: re-resolve the tab's post-navigation host and either
    /// amend the scope's grant attribution/domain or deny-and-park (never on shadow).
    NavigateLanding,
}

/// One action variant of a [`ToolDescriptor`]: its bound capability requirement set and its
/// directory-facing description (the text `explain` renders -- distinct from the advertised
/// description the model sees in `tools/list`). A tool with no sub-actions carries exactly one
/// variant with `action: None`; `computer` carries 13, one per `action_key` value.
#[derive(Debug, Clone, Copy)]
pub struct ActionVariant {
    pub action: Option<&'static str>,
    pub requires: &'static [Capability],
    pub directory_description: &'static str,
}

/// One row of the tool registry (ADR-0024 Decision 1, extended by ADR-0034 Decision 4): the
/// single per-tool authority for validity, classification, advertisement, validation, explain,
/// resource shape, dispatch kind, and result post-processing. Descriptors are DATA; the pipeline
/// owns BEHAVIOR (see the module doc comment).
#[derive(Clone, Copy)]
pub struct ToolDescriptor {
    pub tool: &'static str,
    /// The model-facing description advertised in `tools/list`. Distinct from each variant's
    /// `directory_description` (which `explain` renders): the advertised description is rich,
    /// often multi-line, instructional prose; the directory description is a terse capability
    /// label.
    pub advertised_description: &'static str,
    /// The JSON-Schema for this tool's arguments, as an inline JSON literal. The wire target
    /// format -- no DSL, no escape hatch (ADR-0034 Decision 4).
    pub input_schema: fn() -> serde_json::Value,
    /// The agent-facing example (ADR-0031 Decision 2). `None` only on `explain`.
    pub example: Option<ToolExample>,
    /// `Some("action")` on `computer` only: this tool has sub-actions, keyed by this argument
    /// name. `None` for every other tool (any action-like argument is ignored).
    pub action_key: Option<&'static str>,
    /// One entry per action variant; 13 for `computer`, exactly 1 (with `action: None`) for
    /// every other tool.
    pub variants: &'static [ActionVariant],
    pub resource: ResourceShape,
    pub handler: Handler,
    /// Applied to the dispatch result before it is returned, when present: `read_page`'s
    /// secret redaction is the only user today.
    pub postprocess: Option<fn(&mut serde_json::Value, bool)>,
    pub post_dispatch: PostDispatch,
    /// The declared `outputSchema` for this tool's `structuredContent` (ADR-0038 Decision 3),
    /// when this tool has a declared result vocabulary; `None` on every other row. Emitted in
    /// `tools/list` alongside `inputSchema` when present.
    pub output_schema: Option<fn() -> Value>,
}

/// The agent-facing example for a tool (ADR-0031 Decision 2): a sample `call` (as a JSON string
/// literal, parsed lazily) and a one-line `returns` note. Used to generate corrective validation
/// errors ("example call shape: ...") and pinned by the regression snapshot. `None` on `explain`.
#[derive(Clone, Copy)]
pub struct ToolExample {
    pub call: &'static str,
    pub returns: Option<&'static str>,
}

/// The tool registry: 22 descriptors (the 13 browser tools plus `narrate`, `wait_for`, `script`,
/// `form_fill`, `file_upload`, `browser_batch`, `upload_image`, `gif_creator`, and `explain`), in
/// the order they appear in `tools/list`. `computer`'s 13 variants are in the
/// schema's `action` enum order, byte-for-byte, as `variants`.
pub const REGISTRY: &[ToolDescriptor] = &[
    ToolDescriptor {
        tool: "tabs_context_mcp",
        advertised_description: "Get context information about the current MCP tab group. Returns all tab IDs inside the group if it exists. CRITICAL: You must get the context at least once before using other browser automation tools so you know what tabs exist. Each new conversation should create its own new tab (using tabs_create) rather than reusing existing tabs, unless the user explicitly asks to use an existing tab.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "createIfEmpty": {
                    "type": "boolean",
                    "description": "Creates a new MCP tab group if none exists, creates a new Window with a new tab group containing an empty tab (which can be used for this conversation). If a MCP tab group already exists, this parameter has no effect."
                }
            },
            "required": [],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"createIfEmpty":true}"#,
            returns: Some("Returns the tab group id and the tabs it contains (tabId, title, url for each). Call this first to get the tabId every other tool needs."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description:
                "List the MCP tab group: the ids, URLs, and titles of the tabs this server controls.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "mcpGroupId": { "type": "number" },
                    "tabs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tabId": { "type": "number" },
                                "title": { "type": "string" },
                                "url": { "type": "string" }
                            },
                            "required": ["tabId", "title", "url"]
                        }
                    }
                },
                "required": ["mcpGroupId", "tabs"]
            })
        }),
    },
    ToolDescriptor {
        tool: "tabs_create_mcp",
        advertised_description: "Creates a new empty tab in the MCP tab group.",
        input_schema: || json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{}"#,
            returns: Some("Returns the new tab's tabId and the group id; use the tabId with navigate to go to a URL."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description: "Open a new empty tab in the MCP tab group; touches no page and no server.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "tabs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tabId": { "type": "number" },
                                "title": { "type": "string" },
                                "url": { "type": "string" }
                            },
                            "required": ["tabId", "title", "url"]
                        }
                    }
                },
                "required": ["tabId", "tabs"]
            })
        }),
    },
    ToolDescriptor {
        tool: "navigate",
        advertised_description: "Navigate to a URL, or go forward/back in browser history. If you don't have a valid tab ID, use tabs_context first to get available tabs.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to navigate to. Can be provided with or without protocol (defaults to https://). Use \"forward\" to go forward in history or \"back\" to go back in history."
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to navigate. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "force": {
                    "type": "boolean",
                    "description": "If the page shows a \"Leave site?\" dialog because of unsaved changes, discard those changes and navigate anyway. Defaults to false: navigation is blocked and an error is returned so you can decide first."
                }
            },
            "required": ["url", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"url":"https://example.com"}"#,
            returns: Some("Returns a short confirmation that the tab navigated to the URL."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description: "Load a URL in a tab, or go back or forward in its history; a top-level GET.",
        }],
        resource: ResourceShape::TargetArg,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::NavigateLanding,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "url": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["tabId", "url", "title"]
            })
        }),
    },
    ToolDescriptor {
        tool: "computer",
        advertised_description: "Use a mouse and keyboard to interact with a web browser, and take screenshots. If you don't have a valid tab ID, use tabs_context first to get available tabs.\n* Whenever you intend to click on an element like an icon, you should consult a screenshot to determine the coordinates of the element before moving the cursor.\n* If you tried clicking on a program or link but it failed to load, even after waiting, try adjusting your click location so that the tip of the cursor visually falls on the element that you want to click.\n* Make sure to click any buttons, links, icons, etc with the cursor tip in the center of the element. Don't click boxes on their edges unless asked.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["left_click", "right_click", "type", "screenshot", "wait", "scroll", "key", "left_click_drag", "double_click", "triple_click", "zoom", "scroll_to", "hover"],
                    "description": "The action to perform:\n* `left_click`: Click the left mouse button at the specified coordinates.\n* `right_click`: Click the right mouse button at the specified coordinates to open context menus.\n* `double_click`: Double-click the left mouse button at the specified coordinates.\n* `triple_click`: Triple-click the left mouse button at the specified coordinates.\n* `type`: Type a string of text.\n* `screenshot`: Take a screenshot of the screen.\n* `wait`: Wait for a specified number of seconds.\n* `scroll`: Scroll up, down, left, or right at the specified coordinates.\n* `key`: Press a specific keyboard key.\n* `left_click_drag`: Drag from start_coordinate to coordinate.\n* `zoom`: Take a screenshot of a specific region for closer inspection.\n* `scroll_to`: Scroll an element into view using its element reference ID from read_page or find tools.\n* `hover`: Move the mouse cursor to the specified coordinates or element without clicking. Useful for revealing tooltips, dropdown menus, or triggering hover states."
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to execute the action on. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "coordinate": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 2,
                    "maxItems": 2,
                    "description": "(x, y): The x (pixels from the left edge) and y (pixels from the top edge) coordinates. Required for `left_click`, `right_click`, `double_click`, `triple_click`, and `scroll`. For `left_click_drag`, this is the end position."
                },
                "duration": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 10,
                    "description": "The number of seconds to wait. Required for `wait`. Maximum 10 seconds."
                },
                "modifiers": {
                    "type": "string",
                    "description": "Modifier keys for click actions. Supports: \"ctrl\", \"shift\", \"alt\", \"cmd\" (or \"meta\"), \"win\" (or \"windows\"). Can be combined with \"+\" (e.g., \"ctrl+shift\", \"cmd+alt\"). Optional."
                },
                "ref": {
                    "type": "string",
                    "description": "Element reference ID from read_page or find tools (e.g., \"ref_1\", \"ref_2\"). Required for `scroll_to` action. Can be used as alternative to `coordinate` for click actions."
                },
                "region": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 4,
                    "maxItems": 4,
                    "description": "(x0, y0, x1, y1): The rectangular region to capture for `zoom`. Coordinates define a rectangle from top-left (x0, y0) to bottom-right (x1, y1) in pixels from the viewport origin. Required for `zoom` action. Useful for inspecting small UI elements like icons, buttons, or text."
                },
                "repeat": {
                    "type": "number",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "Number of times to repeat the key sequence. Only applicable for `key` action. Must be a positive integer between 1 and 100. Default is 1. Useful for navigation tasks like pressing arrow keys multiple times."
                },
                "scroll_direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "The direction to scroll. Required for `scroll`."
                },
                "scroll_amount": {
                    "type": "number",
                    "minimum": 1,
                    "maximum": 10,
                    "description": "The number of scroll wheel ticks. Optional for `scroll`, defaults to 3."
                },
                "start_coordinate": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 2,
                    "maxItems": 2,
                    "description": "(x, y): The starting coordinates for `left_click_drag`."
                },
                "text": {
                    "type": "string",
                    "description": "The text to type (for `type` action) or the key(s) to press (for `key` action). For `key` action: Provide space-separated keys (e.g., \"Backspace Backspace Delete\"). Supports keyboard shortcuts using the platform's modifier key (use \"cmd\" on Mac, \"ctrl\" on Windows/Linux, e.g., \"cmd+a\" or \"ctrl+a\" for select all)."
                }
            },
            "required": ["action", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"action":"screenshot"}"#,
            returns: Some("Returns depend on action: screenshot/zoom return an image (large; prefer read_page/get_page_text when you only need structure or text); clicks/typing/scroll/hover return a short text confirmation."),
        }),
        action_key: Some("action"),
        variants: &[
            ActionVariant {
                action: Some("left_click"),
                requires: &[Capability::Action],
                directory_description:
                    "Left-click at coordinates; commits an activation whose effect the page decides.",
            },
            ActionVariant {
                action: Some("right_click"),
                requires: &[Capability::Action],
                directory_description: "Right-click at coordinates; commits an activation.",
            },
            ActionVariant {
                action: Some("type"),
                requires: &[Capability::Action],
                directory_description: "Type text into the focused element; commits data to page handlers.",
            },
            ActionVariant {
                action: Some("screenshot"),
                requires: &[Capability::Read],
                directory_description: "Capture a screenshot of the visible viewport.",
            },
            ActionVariant {
                action: Some("wait"),
                requires: &[],
                directory_description: "Pause for a duration; touches no page and no server.",
            },
            ActionVariant {
                action: Some("scroll"),
                requires: &[Capability::Read],
                directory_description: "Scroll the viewport; moves the view without committing input to the page.",
            },
            ActionVariant {
                action: Some("key"),
                requires: &[Capability::Action],
                directory_description: "Press a key or key combination; commits input to page handlers.",
            },
            ActionVariant {
                action: Some("left_click_drag"),
                requires: &[Capability::Action],
                directory_description: "Click and drag between two points; commits pointer input to the page.",
            },
            ActionVariant {
                action: Some("double_click"),
                requires: &[Capability::Action],
                directory_description: "Double-click at coordinates; commits an activation.",
            },
            ActionVariant {
                action: Some("triple_click"),
                requires: &[Capability::Action],
                directory_description: "Triple-click at coordinates; commits an activation.",
            },
            ActionVariant {
                action: Some("zoom"),
                requires: &[Capability::Read],
                directory_description: "Capture a zoomed screenshot of a page region.",
            },
            ActionVariant {
                action: Some("scroll_to"),
                requires: &[Capability::Read],
                directory_description: "Scroll an element into view; moves the viewport without committing input.",
            },
            ActionVariant {
                action: Some("hover"),
                requires: &[Capability::Read],
                directory_description: "Move the pointer over a point; commits no activation and no data.",
            },
        ],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "find",
        advertised_description: "Find elements on the page using natural language. Can search for elements by their purpose (e.g., \"search bar\", \"login button\") or by text content (e.g., \"organic mango product\"). Returns up to 20 matching elements with references that can be used with other tools. If more than 20 matches exist, you'll be notified to use a more specific query. If you don't have a valid tab ID, use tabs_context first to get available tabs.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language description of what to find (e.g., \"search bar\", \"add to cart button\", \"product title containing organic\")"
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to search in. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                }
            },
            "required": ["query", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"query":"search bar"}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description: "Search the page for elements matching a natural-language description.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "results": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "ref": { "type": "string" },
                                "role": { "type": "string" },
                                "name": { "type": "string" },
                                "x": { "type": "number" },
                                "y": { "type": "number" }
                            },
                            "required": ["ref", "role", "name", "x", "y"]
                        }
                    },
                    "more": { "type": "boolean" }
                },
                "required": ["results", "more"]
            })
        }),
    },
    ToolDescriptor {
        tool: "form_input",
        advertised_description: "Set values in form elements using element reference ID from the read_page or find tools. If you don't have a valid tab ID, use tabs_context first to get available tabs.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "ref": {
                    "type": "string",
                    "description": "Element reference ID from the read_page or find tools (e.g., \"ref_1\", \"ref_2\")"
                },
                "value": {
                    "type": ["string", "boolean", "number"],
                    "description": "The value to set. For checkboxes use boolean, for selects use option value or text, for other inputs use appropriate string/number"
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to set form value in. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                }
            },
            "required": ["ref", "value", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"ref":"ref_1","value":"hello"}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Write],
            directory_description: "Fill or set values in form fields; a declared, state-changing write.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "get_page_text",
        advertised_description: "Extract raw text content from the page, prioritizing article content. Ideal for reading articles, blog posts, or other text-heavy pages. Returns plain text without HTML formatting. If you don't have a valid tab ID, use tabs_context first to get available tabs. Output is limited to 50000 characters by default; if it exceeds the limit it is truncated with a note giving the full size.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to extract text from. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "max_chars": {
                    "type": "number",
                    "description": "Maximum characters for output (default: 50000). Set to a higher value if your client can handle large outputs."
                }
            },
            "required": ["tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description: "Extract the page's readable text content, article-first, without HTML.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "javascript_tool",
        advertised_description: "Execute JavaScript code in the context of the current page. The code runs in the page's context and can interact with the DOM, window object, and page variables. Returns the result of the last expression or any thrown errors. If you don't have a valid tab ID, use tabs_context first to get available tabs.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Must be set to 'javascript_exec'"
                },
                "text": {
                    "type": "string",
                    "description": "The JavaScript code to execute. Evaluated in the page context with REPL semantics: top-level `await` works, and the result of the last expression is returned automatically -- write the expression you want (e.g. `window.myData.value`, or `await fetch(url).then(r=>r.json())`) rather than `return ...`. You can access and modify the DOM, call page functions, and interact with page variables."
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to execute the code in. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                }
            },
            "required": ["action", "text", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"action":"javascript_exec","text":"document.title"}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Execute],
            directory_description:
                "Run arbitrary JavaScript in the page; unbounded, and can bypass the UI entirely.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "read_console_messages",
        advertised_description: "Read browser console messages (console.log, console.error, console.warn, etc.) from a specific tab. Useful for debugging JavaScript errors, viewing application logs, or understanding what's happening in the browser console. Returns console messages from the current domain only. If you don't have a valid tab ID, use tabs_context first to get available tabs. IMPORTANT: Always provide a pattern to filter messages - without a pattern, you may get too many irrelevant messages.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to read console messages from. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to filter console messages. Only messages matching this pattern will be returned (e.g., 'error|warning' to find errors and warnings, 'MyApp' to filter app-specific logs). You should always provide a pattern to avoid getting too many irrelevant messages."
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of messages to return. Defaults to 100. Increase only if you need more results."
                },
                "onlyErrors": {
                    "type": "boolean",
                    "description": "If true, only return error and exception messages. Default is false (return all message types)."
                },
                "clear": {
                    "type": "boolean",
                    "description": "If true, clear the console messages after reading to avoid duplicates on subsequent calls. Default is false."
                }
            },
            "required": ["tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"pattern":"error|warning"}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description: "Read buffered browser console messages from a tab.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "read_network_requests",
        advertised_description: "Read HTTP network requests (XHR, Fetch, documents, images, etc.) from a specific tab. Useful for debugging API calls, monitoring network activity, or understanding what requests a page is making. Returns all network requests made by the current page, including cross-origin requests. Requests are automatically cleared when the page navigates to a different domain. If you don't have a valid tab ID, use tabs_context first to get available tabs.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to read network requests from. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "urlPattern": {
                    "type": "string",
                    "description": "Optional URL pattern to filter requests. Only requests whose URL contains this string will be returned (e.g., '/api/' to filter API calls, 'example.com' to filter by domain)."
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of requests to return. Defaults to 100. Increase only if you need more results."
                },
                "clear": {
                    "type": "boolean",
                    "description": "If true, clear the network requests after reading to avoid duplicates on subsequent calls. Default is false."
                }
            },
            "required": ["tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"urlPattern":"/api/"}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description: "Read buffered HTTP network requests observed in a tab.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "read_page",
        advertised_description: "Get an accessibility tree representation of elements on the page. By default returns all elements including non-visible ones. Can optionally filter for only interactive elements, limit tree depth, or focus on a specific element. Returns a structured tree that represents how screen readers see the page content. If you don't have a valid tab ID, use tabs_context first to get available tabs. Output is limited to 50000 characters -- if exceeded, the tree is truncated at a line boundary with a note giving the full size; pass a larger max_chars, or use depth/ref_id to focus.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to read from. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "filter": {
                    "type": "string",
                    "enum": ["interactive", "all"],
                    "description": "Filter elements: \"interactive\" for buttons/links/inputs only, \"all\" for all elements including non-visible ones (default: all elements)"
                },
                "depth": {
                    "type": "number",
                    "description": "Maximum depth of the tree to traverse (default: 15). Use a smaller depth if output is too large."
                },
                "ref_id": {
                    "type": "string",
                    "description": "Reference ID of a parent element to read. Will return the specified element and all its children. Use this to focus on a specific part of the page when output is too large."
                },
                "max_chars": {
                    "type": "number",
                    "description": "Maximum characters for output (default: 50000). Set to a higher value if your client can handle large outputs."
                },
                "diff": {
                    "type": "boolean",
                    "description": "Return only changes since your previous read_page on this tab (+ added, - removed, ~ changed)."
                }
            },
            "required": ["tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"filter":"interactive"}"#,
            returns: Some("Returns an accessibility tree; each interactive element carries a reference id of the form `ref_N`. Pass that id to form_input.ref, or use it as computer.ref for click/scroll_to actions."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description: "Read the page as an accessibility tree of elements with reference ids.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: Some(crate::browser::redact::apply_to_result),
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "resize_window",
        advertised_description: "Resize the current browser window to specified dimensions. Useful for testing responsive designs or setting up specific screen sizes. If you don't have a valid tab ID, use tabs_context first to get available tabs.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "width": {
                    "type": "number",
                    "description": "Target window width in pixels"
                },
                "height": {
                    "type": "number",
                    "description": "Target window height in pixels"
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to get the window for. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                }
            },
            "required": ["width", "height", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"width":1280,"height":800}"#,
            returns: None,
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description: "Resize the browser window; browser state only, touches no page content.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "update_plan",
        advertised_description: "Present a plan to the user for approval before taking actions. The user will see the domains you intend to visit and your approach. Once approved, you can proceed with actions on the approved domains without additional permission prompts.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of domains you will visit (e.g., ['github.com', 'stackoverflow.com']). These domains will be approved for the session when the user accepts the plan."
                },
                "approach": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "High-level description of what you will do. Focus on outcomes and key actions, not implementation details. Be concise - aim for 3-7 items."
                }
            },
            "required": ["domains", "approach"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"domains":["example.com"],"approach":["read the page","report the main heading"]}"#,
            returns: Some("Returns the plan echoed back; auto-approved by the engine. The user sees it in their client."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description: "Present a plan of intended actions to the user; informational only.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "narrate",
        advertised_description: "Show a short, temporary narration ribbon in the controlled browser tab so the person watching understands the current workflow phase. Use it for meaningful phase changes, not routine clicks or keystrokes. A new narration replaces the current one.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID in which to show the narration. Must be a tab owned by this session."
                },
                "text": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 240,
                    "description": "One short, user-visible sentence describing the current workflow phase."
                },
                "position": {
                    "type": "string",
                    "enum": ["auto", "top", "bottom"],
                    "default": "auto",
                    "description": "Which viewport edge holds the narration ribbon. Auto avoids recent interaction and scroll activity; defaults to auto."
                },
                "duration_ms": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": 30000,
                    "default": 5000,
                    "description": "How long to show the narration, in milliseconds. Defaults to 5000."
                }
            },
            "required": ["tabId", "text"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"text":"Checking the result before making changes.","position":"auto","duration_ms":5000}"#,
            returns: Some("Shows one timed, pointer-transparent agent narration ribbon; a new call replaces it."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description: "Show temporary agent commentary in an owned tab; touches no page content and requires no RAWX capability.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "shown": { "type": "boolean" },
                    "position": { "type": "string" },
                    "duration_ms": { "type": "number" },
                    "replaced": { "type": "boolean" },
                    "reason": { "type": "string" }
                },
                "required": ["shown", "position", "duration_ms", "replaced"]
            })
        }),
    },
    ToolDescriptor {
        tool: "wait_for",
        advertised_description: "Wait until the page is ready. By default waits for BOTH your condition and page settlement (DOM mutation rate decayed). Provide selector (CSS) or text (visible substring) with state visible|present|gone, or call with neither to wait for settlement alone. min_ms sets a minimum elapsed time; settle:false gates on the condition only. Returns elapsed_ms, settle diagnostics, and the matched element's ref for follow-up clicks. Times out with an error naming what WAS on the page.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to wait on. Must be a tab in the current group. Use tabs_context first if you don't have a valid tab ID."
                },
                "selector": {
                    "type": "string",
                    "description": "A CSS selector to wait for. Provide at most one of selector or text. With state visible (default) the element must be present AND rendered; present only requires it be in the DOM; gone waits for its absence."
                },
                "text": {
                    "type": "string",
                    "description": "A visible-text substring to wait for (matched against accessible names and nearby text). Provide at most one of selector or text."
                },
                "state": {
                    "type": "string",
                    "enum": ["visible", "present", "gone", "settled"],
                    "description": "visible (default): the selector/text is present and rendered. present: in the DOM. gone: absent or hidden. settled: wait for the page to stop churning; valid only with no selector/text."
                },
                "timeout_ms": {
                    "type": "number",
                    "description": "Maximum wait in milliseconds. Default 10000, hard cap 30000."
                },
                "min_ms": {
                    "type": "number",
                    "description": "Minimum elapsed time before returning, even if the condition and settlement are already satisfied. Default 0."
                },
                "settle": {
                    "type": "boolean",
                    "description": "Whether to also wait for page settlement (DOM mutation rate decay). Default true; set false to gate on the condition alone."
                }
            },
            "required": ["tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"text":"Results"}"#,
            returns: Some("Waits for the text AND page settlement; returns elapsed_ms, settle diagnostics, and the matched element's ref."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            directory_description:
                "Wait for a condition and page settlement; observes the DOM, touches nothing.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "found": { "type": "boolean" },
                    "elapsed_ms": { "type": "number" },
                    "ref": { "type": "string" },
                    "settled": { "type": "boolean" },
                    "peak_mutations": { "type": "number" },
                    "final_rate": { "type": "number" }
                },
                "required": ["found", "elapsed_ms"]
            })
        }),
    },
    ToolDescriptor {
        tool: "script",
        advertised_description: "Run a sequence of tool calls in one request. Steps execute in order; each step is validated, authorized, and audited exactly as if called individually. Step arguments may reference a prior step's structured result: $prev.field for the previous step, $N.field for step N (1-indexed), with .0-style numeric segments indexing arrays (example: $prev.results.0.ref after find). Write $$ for a literal leading $. Only tools with structured results (find, tabs_context, tabs_create, navigate, wait_for) can be referenced. Steps may not include script itself. Use wait_for between navigate and reads on dynamic pages.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID the steps run against. Steps inherit this tabId when their own args omit it. Use tabs_context first if you don't have a valid tab ID."
                },
                "steps": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 20,
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool": { "type": "string", "description": "The tool to call for this step (any advertised tool except script itself)." },
                            "args": { "type": "object", "description": "Arguments for this step's tool. May reference a prior step's structured result via $prev.field / $N.field." }
                        },
                        "required": ["tool"],
                        "additionalProperties": false
                    },
                    "description": "Ordered tool calls to execute sequentially."
                },
                "onError": {
                    "type": "string",
                    "enum": ["stop", "continue"],
                    "description": "\"stop\" (default) halts the chain on the first non-ok step; \"continue\" runs remaining steps. A held step always stops the chain regardless."
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "When true, run every step through the real governance decision (registry, schema, sacred, authorize) but do not dispatch. Each step reports would_allow or would_deny with the reason a live run would produce; no mutations, no step audit records."
                },
                "budget_ms": {
                    "type": "number",
                    "description": "Total wall-clock budget for the whole script in milliseconds. Lowers (never raises) the configured ceiling; remaining steps report not_run on exhaustion."
                }
            },
            "required": ["steps"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"steps":[{"tool":"find","args":{"tabId":0,"query":"submit button"}},{"tool":"computer","args":{"action":"left_click","ref":"$prev.results.0.ref"}}]}"#,
            returns: Some("Each step's status (ok, error, denied, held, not_run), its text, and its structured result; a summary line; total duration_ms."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description:
                "Run up to 20 tool calls sequentially in one request; each step is authorized and audited individually.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::Local(crate::mcp::script::script_handler),
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "results": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "step": { "type": "number" },
                                "tool": { "type": "string" },
                                "status": { "type": "string" },
                                "result": { "type": "string" },
                                "structured": {}
                            },
                            "required": ["step", "tool", "status"]
                        }
                    },
                    "summary": { "type": "string" },
                    "duration_ms": { "type": "number" }
                },
                "required": ["results", "summary", "duration_ms"]
            })
        }),
    },
    ToolDescriptor {
        tool: "form_fill",
        advertised_description: "Fill a form by field labels in one call. Provide fields as a map from a label, placeholder, or name attribute to the value (string, number, or boolean for checkboxes). Matching is case-insensitive and specificity-ordered; ambiguous keys are returned unmatched with candidates instead of guessed. submit:true clicks the form's own submit control after filling. Passwords are masked in the result. Falls back cleanly: anything unmatched can be filled with form_input using the refs in the result.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID the form lives in. Use tabs_context first if you don't have a valid tab ID."
                },
                "fields": {
                    "type": "object",
                    "minProperties": 1,
                    "additionalProperties": { "type": ["string", "boolean", "number"] },
                    "description": "Map from a field's label, placeholder, or name attribute to the value to set (string, number, or boolean for checkboxes)."
                },
                "submit": {
                    "type": "boolean",
                    "description": "Click the form's own submit control after filling. Default false (fill only)."
                }
            },
            "required": ["tabId", "fields"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"tabId":0,"fields":{"Email":"user@example.com","Remember me":true},"submit":true}"#,
            returns: Some("Returns which fields were filled, any unmatched keys with candidates, and whether submit succeeded."),
        }),
        action_key: Some("submit"),
        variants: &[
            ActionVariant {
                action: None,
                requires: &[Capability::Read, Capability::Write],
                directory_description:
                    "Fill form fields by label in one call; matches keys to controls and fills them.",
            },
            ActionVariant {
                action: Some("submit"),
                requires: &[Capability::Read, Capability::Write, Capability::Action],
                directory_description:
                    "Fill form fields by label and click the form's own submit control.",
            },
        ],
        resource: ResourceShape::TabScoped,
        handler: Handler::Local(crate::mcp::form_fill::form_fill_handler),
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: Some(|| {
            json!({
                "type": "object",
                "properties": {
                    "filled": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "ref": { "type": "string" },
                                "value": {},
                                "type": { "type": "string" }
                            },
                            "required": ["label", "ref", "type"]
                        }
                    },
                    "unmatched": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "candidates": { "type": "array" }
                            },
                            "required": ["key", "candidates"]
                        }
                    },
                    "skipped": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "ref": { "type": "string" },
                                "reason": { "type": "string" }
                            },
                            "required": ["label", "reason"]
                        }
                    },
                    "submitted": { "type": "boolean" },
                    "submit_ref": { "type": "string" },
                    "observation": { "type": "string" },
                    "duration_ms": { "type": "number" }
                },
                "required": ["filled", "unmatched", "skipped", "submitted", "duration_ms"]
            })
        }),
    },
    ToolDescriptor {
        tool: "file_upload",
        advertised_description: "Upload one or multiple files to a file input element on the page. Do not click on file upload buttons or file inputs -- clicking opens a native file picker dialog that you cannot see or interact with. Instead, use read_page or find to locate the file input element, then use this tool with its ref to upload files directly.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "data": { "type": "string" },
                            "name": { "type": "string" },
                            "mimeType": { "type": "string" }
                        },
                        "required": ["data", "name"]
                    },
                    "description": "Files to upload, as base64-encoded bytes."
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "DEPRECATED. Use `files` instead."
                },
                "ref": {
                    "type": "string",
                    "description": "Element reference ID of the file input from read_page or find tools (e.g., \"ref_1\", \"ref_2\")."
                },
                "tabId": {
                    "type": "number",
                    "description": "Tab ID where the file input is located. Use tabs_context first if you don't have a valid tab ID."
                }
            },
            "required": ["ref", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"ref":"ref_1","tabId":0,"files":[{"data":"aGVsbG8=","name":"hello.txt"}]}"#,
            returns: Some("Uploads the base64-decoded file(s) to the file input at ref; returns a text confirmation with the file names and total size."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Write],
            directory_description:
                "Upload files (base64 bytes) to a file input located by read_page or find, via its ref.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "browser_batch",
        advertised_description: "Execute a sequence of browser tool calls in ONE round trip. Each item is {name, input} where input is exactly what you'd pass to that tool standalone. Actions execute SEQUENTIALLY (not in parallel) and stop on the first error. Use this tool extensively to quickly execute work whenever you can predict two or more steps ahead -- e.g. navigate, click a field, type, press Return, screenshot. Each tool's own permission check runs per item -- if an action navigates to a domain without permission, the next item's check fails and the batch stops. Screenshots and other images are returned interleaved with outputs; coordinates you write in THIS batch refer to the screenshot taken BEFORE this call. browser_batch cannot be nested.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "actions": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Tool name (e.g. computer, navigate, find, tabs_create). browser_batch cannot be nested." },
                            "input": { "type": "object", "description": "That tool's input -- same shape you'd pass when calling it directly." }
                        },
                        "required": ["name", "input"]
                    },
                    "description": "List of tool calls to execute sequentially. Example: [{\"name\":\"computer\",\"input\":{\"action\":\"left_click\",\"coordinate\":[100,200],\"tabId\":123}}, {\"name\":\"computer\",\"input\":{\"action\":\"type\",\"text\":\"hello\",\"tabId\":123}}, {\"name\":\"navigate\",\"input\":{\"url\":\"https://example.com\",\"tabId\":123}}]"
                }
            },
            "required": ["actions"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"actions":[{"name":"navigate","input":{"url":"https://example.com","tabId":0}},{"name":"computer","input":{"action":"screenshot","tabId":0}}]}"#,
            returns: Some("Each action's output, with screenshots interleaved, in order; stops on the first error."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description:
                "Run a sequence of tool calls in one round trip; each item is name+input, authorized per item.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::Local(crate::mcp::browser_batch::browser_batch_handler),
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "upload_image",
        advertised_description: "Upload a previously captured screenshot to a file input or drag & drop target. Supports two approaches: (1) ref -- for targeting specific elements, especially hidden file inputs, (2) coordinate -- for drag & drop to visible locations like Google Docs. Provide either ref or coordinate, not both.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "imageId": { "type": "string", "description": "ID of a previously captured screenshot (from the computer tool's screenshot action), e.g. \"img_...\" as reported in the screenshot result." },
                "ref": { "type": "string", "description": "Element reference ID from read_page or find tools (e.g., \"ref_1\", \"ref_2\"). Use this for file inputs (especially hidden ones). Provide either ref or coordinate, not both." },
                "coordinate": { "type": "array", "description": "Viewport coordinates [x, y] for drag & drop to a visible location like Google Docs. Provide either ref or coordinate, not both." },
                "tabId": { "type": "number", "description": "Tab ID where the target element is located. This is where the image will be uploaded to." },
                "filename": { "type": "string", "description": "Optional filename for the uploaded file (default: \"image.png\")." }
            },
            "required": ["imageId", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"imageId":"img_example","ref":"ref_1","tabId":0}"#,
            returns: Some("Uploads the cached screenshot to the file input at ref (or drag-drops it at coordinate); returns a text confirmation."),
        }),
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Write],
            directory_description:
                "Upload a previously captured screenshot to a file input (ref) or drag-drop target (coordinate).",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::Local(crate::mcp::upload_image::upload_image_handler),
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "gif_creator",
        // gif_creator is additive and not part of the 13 trained schemas. ADR-0073 simplifies the
        // happy path to start -> ordinary browser work -> export; stop remains an optional explicit
        // boundary, and status exposes the reliable lifecycle without touching the live page.
        advertised_description: "Create a short, memory-only GIF of browser work. Call start_recording, use browser tools normally, then call export; export stops capture automatically. Recording also auto-stops after 30 seconds idle or 120 seconds total. Use status to inspect state, stop_recording for an optional explicit boundary, or clear to erase immediately. Export can return the GIF to the client (download:true) or place it on the page with ref or coordinate.",
        input_schema: || json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start_recording", "stop_recording", "status", "clear", "export"],
                    "description": "Action to perform. The usual flow is start_recording, normal browser work, then export."
                },
                "tabId": { "type": "number", "description": "Tab ID to identify which tab group this operation applies to" },
                "coordinate": { "type": "array", "description": "Viewport coordinates [x, y] for drag & drop upload. Required for 'export' action unless 'download' is true." },
                "ref": { "type": "string", "description": "Element reference for upload to a file input. For 'export' action only; mutually exclusive with coordinate." },
                "download": { "type": "boolean", "description": "If true, download the GIF instead of drag/drop upload. For 'export' action only." },
                "filename": { "type": "string", "description": "Optional filename for exported GIF (default: 'recording-[timestamp].gif'). For 'export' action only." },
                "options": { "type": "object", "description": "Optional GIF enhancement options for 'export' action. All default to true." }
            },
            "required": ["action", "tabId"],
            "additionalProperties": false
        }),
        example: Some(ToolExample {
            call: r#"{"action":"start_recording","tabId":0}"#,
            returns: Some("Starts capturing GIF frames for the tab; later stop_recording then export (download:true) to get the animated GIF."),
        }),
        action_key: Some("action"),
        variants: &[
            ActionVariant {
                action: Some("start_recording"),
                requires: &[Capability::Read],
                directory_description:
                    "Start recording browser actions in the tab's group as GIF frames.",
            },
            ActionVariant {
                action: Some("stop_recording"),
                requires: &[],
                directory_description: "Stop recording; keep the captured frames for export.",
            },
            ActionVariant {
                action: Some("status"),
                requires: &[],
                directory_description: "Report recording state and deadlines without reading the live page.",
            },
            ActionVariant {
                action: Some("clear"),
                requires: &[],
                directory_description: "Discard the captured recording frames.",
            },
            ActionVariant {
                action: Some("export"),
                requires: &[Capability::Read],
                directory_description:
                    "Encode the frames to a GIF. Client export requires read; page placement by ref or coordinate requires write.",
            },
        ],
        resource: ResourceShape::TabScoped,
        // ADR-0053 D6: the orchestrator lives in the binary (the form_fill precedent); the
        // extension keeps only the thin screencast capture relay. Wiring only -- the advertised
        // schema above is untouched.
        handler: Handler::Local(crate::mcp::gif_creator::gif_creator_handler),
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
    ToolDescriptor {
        tool: "explain",
        advertised_description: "Returns this server's action directory: every available action, the capability it requires (read, action, write, or execute; some require none), and a short description of what it does, plus definitions of the capability vocabulary. Use it to learn what you are allowed to do in this session. It does not read, summarize, or explain web pages.",
        input_schema: || json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        }),
        example: None,
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            directory_description: "Show every action available here and the capability each one requires.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::Local(|ctx| {
            Box::pin(async move {
                let _ = ctx;
                let mut text = explain_text();
                // ADR-0055 D9: append the managed:// Policy Passport when managed governance is
                // active (a bootstrap is present AND the T2 status sidecar reads). Absent either,
                // nothing is appended -- explain's text stays byte-identical to the all-open form.
                let paths = crate::governance::paths::GovernancePaths::production();
                if paths.managed_bootstrap.exists() {
                    if let Some(cache_path) = paths.managed_cache.as_ref() {
                        let sidecar = crate::governance::managed::status::sidecar_path(cache_path);
                        if let Some(status) =
                            crate::governance::managed::status::read_sidecar(&sidecar)
                        {
                            text.push('\n');
                            text.push_str(&crate::governance::explain::managed_passport(&status));
                        }
                    }
                }
                crate::mcp::outcome::CallOutcome::Success {
                    result: json!({ "content": [ { "type": "text", "text": text } ] }),
                }
            })
        }),
        postprocess: None,
        post_dispatch: PostDispatch::None,
        output_schema: None,
    },
];

/// The agent onboarding guide (ADR-0031 Decision 1, ADR-0034 Decision 6): the prose fields served
/// at handshake in `initialize.instructions`. Each capability contributes its own guide; the
/// registry composes them. Today only the browser capability exists. `cost_notes` (C11, ADR-0038
/// Decision 5, PINS.md SS16) is a fifth, capability-level field appended after `denials`: unlike
/// the other four (summary/workflow/flow/denials, ADR-0031 Decision 1's original set), it teaches
/// cost discipline across several tools at once rather than the workflow contract, but composes
/// into the exact same `initialize.instructions` string.
///
/// Prose revised 2026-07-10 (ergonomics pass; ADR-0031 Decision 1 field contract unchanged): the
/// `flow` field now reveals the higher-level tools (`narrate`, `wait_for`, `script`,
/// `browser_batch`, `form_fill`) and steers tool choice, and all cost guidance is consolidated
/// into `cost_notes` (the duplicated COST DISCIPLINE clause left `workflow`). The five-field set
/// and the non-empty, `tabId`, and `Cost notes:` guarantees the tests pin are all preserved.
pub const AGENT_GUIDE: AgentGuide = AgentGuide {
    summary: "Ghostlight drives the user's own authenticated browser. You observe and act on the web pages they're already logged into, in an isolated Ghostlight tab group separate from their own tabs. Default (no policy) is unrestricted; a policy can scope what's allowed.",
    workflow: "BEFORE ANYTHING ELSE: GET A tabId. Every tool that touches a page requires a `tabId` (a number) -- it is required, not optional. Get one with tabs_context_mcp (pass `createIfEmpty: true` to create the group if none exists; usually your first call) or tabs_create_mcp (open a new tab). Then navigate (tabId + url) to go somewhere.",
    flow: "tabs_context_mcp -> navigate -> read (read_page for structure, get_page_text for prose, find for one element; screenshot only to see layout) -> act (form_fill for forms, computer for clicks and keys, form_input for a single field) -> re-read to confirm. On dynamic pages, use wait_for between navigating and reading so you see the settled page, not a spinner. When a person is watching a longer workflow, use narrate at meaningful phase changes, not for routine clicks or keystrokes. When you can predict two or more steps ahead, run them in one call: script chains steps and passes results forward (e.g. `$prev.results.0.ref` after a find), and browser_batch runs a fixed sequence in one round-trip.",
    denials: "If a call is denied you'll see `Denied (D-xxxxxxxx): ...`. Call explain (no arguments) to see what's permitted -- you can do this any time to plan, not just after a denial -- and hand the denial id to the policy administrator.",
    cost_notes: "Cost notes: prefer read_page (structured tree) or get_page_text (plain text) over screenshots when you only need structure or text; a screenshot or zoom costs roughly 1,600 tokens, so capture one only when you need to see layout. read_page full is large on complex pages -- filter interactive is dramatically smaller, and diff true returns only what changed since your last read. get_page_text can return tens of thousands of tokens on document-heavy pages; prefer find for targeted lookups. Each script or browser_batch step is still one browser round-trip -- they save your tokens and turns, not the browser's work.",
};

/// The agent onboarding guide's prose fields (ADR-0031 Decision 1; `cost_notes` added by C11).
#[derive(Clone, Copy)]
pub struct AgentGuide {
    pub summary: &'static str,
    pub workflow: &'static str,
    pub flow: &'static str,
    pub denials: &'static str,
    pub cost_notes: &'static str,
}

/// Render the agent onboarding guide into the single string MCP's `initialize.instructions`
/// field expects (ADR-0031 Decision 1; `cost_notes` appended last by C11, ADR-0038 Decision 5).
/// The fields are concatenated with clear separators. Served once at handshake, before any tool
/// call, so any model gets the workflow contract without having to derive it from per-tool
/// descriptions.
pub fn agent_guide_text() -> String {
    format!(
        "{}\n\n{}\n\nTypical flow: {}\n\n{}\n\n{}",
        AGENT_GUIDE.summary,
        AGENT_GUIDE.workflow,
        AGENT_GUIDE.flow,
        AGENT_GUIDE.denials,
        AGENT_GUIDE.cost_notes
    )
}

/// Render the `tools/list` advertisement JSON from the registry (ADR-0034 Decision 5): the
/// complete `tools` array with each tool's `name`, `description`, `inputSchema`, and `example`
/// (when present), in registry order. This is the single source of the advertised surface --
/// no separate fixture file.
pub fn advertised_tools_json() -> Value {
    let tools: Vec<Value> = REGISTRY
        .iter()
        .map(|d| {
            let mut entry = json!({
                "name": d.tool,
                "description": d.advertised_description,
                "inputSchema": (d.input_schema)(),
            });
            if let Some(ex) = d.example {
                let call: Value = serde_json::from_str(ex.call).unwrap_or(json!({}));
                entry["example"] = if let Some(returns) = ex.returns {
                    json!({ "call": call, "returns": returns })
                } else {
                    json!({ "call": call })
                };
            }
            if let Some(schema) = d.output_schema {
                entry["outputSchema"] = schema();
            }
            entry
        })
        .collect();
    json!({ "tools": tools })
}

/// Look up a tool's registry row by name. Linear scan over 22 rows; the validity check the
/// pipeline uses.
pub fn descriptor(tool: &str) -> Option<&'static ToolDescriptor> {
    REGISTRY.iter().find(|row| row.tool == tool)
}

/// The advertised tool names, in [`REGISTRY`] (advertised) order (ADR-0051 Phase 1). This is the
/// single DERIVED source of truth for "the current advertised surface" that BEHAVIOR tests assert
/// against -- e.g. a spawn test proving the wire delivered the full set, or a protocol test counting
/// `tools/list` -- so an additive tool (ADR-0034 Decision 7) does not require editing a hardcoded
/// count or name array in every such test. The FIDELITY guards
/// (`tests/tool_schema_fidelity.rs`, `tests/all_open_golden.rs`, and the `explain`-text literal in
/// `mcp/pipeline.rs`) stay hand-maintained on purpose: they are the intentional drift catchers and
/// must NOT be rewired to derive from here, or a wrong `REGISTRY` change would validate itself.
pub fn advertised_tool_names() -> Vec<&'static str> {
    REGISTRY.iter().map(|row| row.tool).collect()
}

/// The number of advertised tools (see [`advertised_tool_names`]). Derived from [`REGISTRY`].
pub fn advertised_tool_count() -> usize {
    REGISTRY.len()
}

/// Look up the bound capability requirement set for one action. `action` is consulted only
/// when `tool`'s descriptor carries an `action_key` (`computer` today); for every other tool it
/// is ignored.
///
/// Returns `None` when the (tool, action) pair has no registry entry -- a classification MISS,
/// which callers must treat as a denial (fail closed), never as "no requirements". Returns
/// `Some(&[])` when the action's bound requirement set is genuinely empty -- the action is
/// unconditionally allowed. See the module doc comment for the absent-vs-empty invariant
/// (ADR-0022 Decision 2).
pub fn requires(tool: &str, action: Option<&str>) -> Option<&'static [Capability]> {
    let row = descriptor(tool)?;
    match row.action_key {
        // `form_fill` (C10) also carries an `action_key` (`"submit"`), but unlike `computer` its
        // `action: None` variant is a real, reachable row (the no-submit default) rather than an
        // impossible state -- so the lookup matches on `action` DIRECTLY (`None` finds the
        // `action: None` variant when one exists) instead of failing closed the moment `action`
        // is `None`. This is a strict widening: `computer` has no `action: None` variant at all,
        // so `requires("computer", None)` still misses exactly as before.
        Some(_) => row
            .variants
            .iter()
            .find(|variant| variant.action == action)
            .map(|variant| variant.requires),
        None => row.variants.first().map(|variant| variant.requires),
    }
}

/// Per-call capability refinement for additive tools whose delivery mode changes the effect.
/// The descriptor remains the source of the baseline used by advertisement and explain.
pub fn requires_for_call(
    descriptor: &ToolDescriptor,
    action: Option<&str>,
    args: &Value,
) -> Option<&'static [Capability]> {
    if descriptor.tool == "gif_creator"
        && action == Some(crate::recording::action::EXPORT)
        && gif_page_target(args)
    {
        return Some(&[Capability::Write]);
    }
    requires(descriptor.tool, action)
}

/// Resolve recording-only operations without probing the current tab. Starting capture and page
/// placement still use the descriptor's ordinary tab-scoped authority.
pub fn resource_for_call(
    descriptor: &ToolDescriptor,
    action: Option<&str>,
    args: &Value,
) -> ResourceShape {
    if descriptor.tool != "gif_creator" {
        return descriptor.resource;
    }
    if action == Some(crate::recording::action::START)
        || (action == Some(crate::recording::action::EXPORT) && gif_page_target(args))
    {
        ResourceShape::TabScoped
    } else {
        ResourceShape::RecordingScoped
    }
}

fn gif_page_target(args: &Value) -> bool {
    args.get("coordinate").is_some_and(|value| !value.is_null())
        || args.get("ref").is_some_and(|value| !value.is_null())
}

/// The `explain` tool's deterministic response text (ADR-0022 Decision 7): the capability
/// vocabulary paragraph, one blank line, then every [`REGISTRY`] row's variants in order
/// (`computer`'s 13 actions in enum order), one line per variant -- `{tool}: requires
/// {capability or nothing}. {description}` for a variant with no action, `{tool} ({action}):
/// requires {capability or nothing}. {description}` for a variant with one. Pure formatting
/// over static data: no I/O, and deterministic across calls and sessions.
pub fn explain_text() -> String {
    let total_variants: usize = REGISTRY.iter().map(|row| row.variants.len()).sum();
    let mut lines = Vec::with_capacity(total_variants + 2);
    lines.push(
        "Capabilities: read = retrieve and observe only; action = dispatch UI input whose \
         effect the page decides (this can trigger writes); write = declared state-changing \
         operations; execute = arbitrary code."
            .to_string(),
    );
    lines.push(String::new());
    for row in REGISTRY {
        for variant in row.variants {
            let requirement = variant
                .requires
                .first()
                .map(Capability::as_str)
                .unwrap_or("nothing");
            let label = match variant.action {
                Some(action) => format!("{} ({action})", row.tool),
                None => row.tool.to_string(),
            };
            lines.push(format!(
                "{label}: requires {requirement}. {}",
                variant.directory_description
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn declared_tool_names_in_order() -> Vec<String> {
        REGISTRY.iter().map(|d| d.tool.to_string()).collect()
    }

    fn declared_computer_actions_in_order() -> Vec<String> {
        let computer = REGISTRY
            .iter()
            .find(|d| d.tool == "computer")
            .expect("computer tool present");
        let schema = (computer.input_schema)();
        schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn registry_covers_the_declared_surface_exactly() {
        let declared_names = declared_tool_names_in_order();
        let registry_names: Vec<String> = REGISTRY.iter().map(|row| row.tool.to_string()).collect();
        assert_eq!(
            registry_names, declared_names,
            "registry tool order must match the fixture's advertised order exactly"
        );

        let with_action_key: Vec<&ToolDescriptor> = REGISTRY
            .iter()
            .filter(|row| row.action_key.is_some())
            .collect();
        assert_eq!(
            with_action_key.len(),
            3,
            "computer, form_fill (C10), and gif_creator (ADR-0050 D5) carry an action_key"
        );
        let computer = with_action_key
            .iter()
            .find(|d| d.tool == "computer")
            .expect("computer present");
        assert_eq!(computer.action_key, Some("action"));
        let form_fill = with_action_key
            .iter()
            .find(|d| d.tool == "form_fill")
            .expect("form_fill present");
        assert_eq!(form_fill.action_key, Some("submit"));

        let declared_actions = declared_computer_actions_in_order();
        let computer_actions: Vec<String> = computer
            .variants
            .iter()
            .map(|variant| {
                variant
                    .action
                    .expect("every computer variant has an action")
                    .to_string()
            })
            .collect();
        assert_eq!(computer_actions, declared_actions);
        assert_eq!(computer_actions.len(), 13);

        for row in REGISTRY.iter().filter(|row| {
            row.tool != "computer" && row.tool != "form_fill" && row.tool != "gif_creator"
        }) {
            assert_eq!(
                row.variants.len(),
                1,
                "{} must have exactly one variant",
                row.tool
            );
            assert_eq!(
                row.variants[0].action, None,
                "{} variant must have action None",
                row.tool
            );
        }
        assert_eq!(
            form_fill.variants.len(),
            2,
            "form_fill carries two variants"
        );

        let total_variants: usize = REGISTRY.iter().map(|row| row.variants.len()).sum();
        assert_eq!(total_variants, 39);

        let mut seen = HashSet::new();
        for row in REGISTRY {
            for variant in row.variants {
                assert!(
                    seen.insert((row.tool, variant.action)),
                    "duplicate row: {} {:?}",
                    row.tool,
                    variant.action
                );
            }
        }
    }

    #[test]
    fn registry_requires_match_the_adr_table() {
        const EXPECTED: &[(&str, Option<&str>, &[Capability])] = &[
            ("tabs_context_mcp", None, &[Capability::Read]),
            ("tabs_create_mcp", None, &[]),
            ("navigate", None, &[Capability::Read]),
            ("computer", Some("left_click"), &[Capability::Action]),
            ("computer", Some("right_click"), &[Capability::Action]),
            ("computer", Some("type"), &[Capability::Action]),
            ("computer", Some("screenshot"), &[Capability::Read]),
            ("computer", Some("wait"), &[]),
            ("computer", Some("scroll"), &[Capability::Read]),
            ("computer", Some("key"), &[Capability::Action]),
            ("computer", Some("left_click_drag"), &[Capability::Action]),
            ("computer", Some("double_click"), &[Capability::Action]),
            ("computer", Some("triple_click"), &[Capability::Action]),
            ("computer", Some("zoom"), &[Capability::Read]),
            ("computer", Some("scroll_to"), &[Capability::Read]),
            ("computer", Some("hover"), &[Capability::Read]),
            ("find", None, &[Capability::Read]),
            ("form_input", None, &[Capability::Write]),
            ("get_page_text", None, &[Capability::Read]),
            ("javascript_tool", None, &[Capability::Execute]),
            ("read_console_messages", None, &[Capability::Read]),
            ("read_network_requests", None, &[Capability::Read]),
            ("read_page", None, &[Capability::Read]),
            ("resize_window", None, &[]),
            ("update_plan", None, &[]),
            ("narrate", None, &[]),
            ("wait_for", None, &[Capability::Read]),
            ("script", None, &[]),
            ("form_fill", None, &[Capability::Read, Capability::Write]),
            (
                "form_fill",
                Some("submit"),
                &[Capability::Read, Capability::Write, Capability::Action],
            ),
            ("file_upload", None, &[Capability::Write]),
            ("browser_batch", None, &[]),
            ("upload_image", None, &[Capability::Write]),
            ("gif_creator", Some("start_recording"), &[Capability::Read]),
            ("gif_creator", Some("stop_recording"), &[]),
            ("gif_creator", Some("status"), &[]),
            ("gif_creator", Some("clear"), &[]),
            ("gif_creator", Some("export"), &[Capability::Read]),
            ("explain", None, &[]),
        ];

        let flattened: Vec<(&str, Option<&str>, &[Capability])> = REGISTRY
            .iter()
            .flat_map(|row| {
                row.variants
                    .iter()
                    .map(move |variant| (row.tool, variant.action, variant.requires))
            })
            .collect();

        assert_eq!(flattened.len(), EXPECTED.len());
        for (actual, expected) in flattened.iter().zip(EXPECTED.iter()) {
            assert_eq!(actual, expected, "row order/content mismatch");
        }
    }

    #[test]
    fn absent_is_none_and_empty_is_some() {
        assert_eq!(requires("no_such_tool", None), None);
        assert_eq!(requires("computer", None), None);
        assert_eq!(requires("computer", Some("no_such_action")), None);
        assert_eq!(requires("tabs_create_mcp", None), Some(&[][..]));
        assert_eq!(requires("update_plan", None), Some(&[][..]));
        assert_eq!(requires("narrate", None), Some(&[][..]));
        assert_eq!(requires("explain", None), Some(&[][..]));
        assert_eq!(requires("computer", Some("wait")), Some(&[][..]));
        assert_eq!(requires("navigate", None), Some(&[Capability::Read][..]));
        assert_eq!(
            requires("javascript_tool", None),
            Some(&[Capability::Execute][..])
        );
        assert_eq!(requires("form_input", None), Some(&[Capability::Write][..]));
        assert_eq!(
            requires("computer", Some("left_click")),
            Some(&[Capability::Action][..])
        );
        assert_eq!(
            requires("read_page", Some("left_click")),
            Some(&[Capability::Read][..]),
            "action is ignored for non-computer tools"
        );
    }

    /// C10 (PINS.md SS13, ADR-0036 Decision 4): `form_fill`'s two variants differ by whether
    /// `submit` was requested, per the boolean-action-key mapping (PINS.md SS13 point 1).
    #[test]
    fn form_fill_requires_vary_by_submit_variant() {
        assert_eq!(
            requires("form_fill", None),
            Some(&[Capability::Read, Capability::Write][..])
        );
        assert_eq!(
            requires("form_fill", Some("submit")),
            Some(&[Capability::Read, Capability::Write, Capability::Action][..])
        );
    }

    #[test]
    fn gif_export_classification_follows_its_delivery_boundary() {
        let gif = descriptor("gif_creator").unwrap();
        assert_eq!(
            requires_for_call(gif, Some("export"), &json!({"download": true})),
            Some(&[Capability::Read][..])
        );
        assert_eq!(
            requires_for_call(gif, Some("export"), &json!({"ref": "ref_1"})),
            Some(&[Capability::Write][..])
        );
        assert_eq!(
            resource_for_call(gif, Some("status"), &json!({"tabId": 1})),
            ResourceShape::RecordingScoped
        );
        assert_eq!(
            resource_for_call(gif, Some("export"), &json!({"coordinate": [1, 2]})),
            ResourceShape::TabScoped
        );
    }

    #[test]
    fn every_description_is_nonempty_ascii_and_short() {
        for row in REGISTRY {
            for variant in row.variants {
                assert!(
                    !variant.directory_description.is_empty(),
                    "empty description: {} {:?}",
                    row.tool,
                    variant.action
                );
                assert!(
                    variant.directory_description.is_ascii(),
                    "non-ascii description: {} {:?}",
                    row.tool,
                    variant.action
                );
                assert!(
                    variant.directory_description.len() <= 110,
                    "description too long ({} chars): {} {:?}",
                    variant.directory_description.len(),
                    row.tool,
                    variant.action
                );
                assert_eq!(
                    variant.directory_description,
                    variant.directory_description.trim(),
                    "description has leading/trailing whitespace: {} {:?}",
                    row.tool,
                    variant.action
                );
            }
        }
    }

    #[test]
    fn per_tool_fields_match_the_adr_table() {
        #[allow(clippy::type_complexity)]
        const EXPECTED_TOOLS: &[(&str, Option<&str>, ResourceShape, bool, bool, PostDispatch)] = &[
            (
                "tabs_context_mcp",
                None,
                ResourceShape::DomainLess,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "tabs_create_mcp",
                None,
                ResourceShape::DomainLess,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "navigate",
                None,
                ResourceShape::TargetArg,
                false,
                false,
                PostDispatch::NavigateLanding,
            ),
            (
                "computer",
                Some("action"),
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "find",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "form_input",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "get_page_text",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "javascript_tool",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "read_console_messages",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "read_network_requests",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "read_page",
                None,
                ResourceShape::TabScoped,
                false,
                true,
                PostDispatch::None,
            ),
            (
                "resize_window",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "update_plan",
                None,
                ResourceShape::DomainLess,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "narrate",
                None,
                ResourceShape::DomainLess,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "wait_for",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "script",
                None,
                ResourceShape::DomainLess,
                true,
                false,
                PostDispatch::None,
            ),
            (
                "form_fill",
                Some("submit"),
                ResourceShape::TabScoped,
                true,
                false,
                PostDispatch::None,
            ),
            (
                "file_upload",
                None,
                ResourceShape::TabScoped,
                false,
                false,
                PostDispatch::None,
            ),
            (
                "browser_batch",
                None,
                ResourceShape::DomainLess,
                true,
                false,
                PostDispatch::None,
            ),
            (
                "upload_image",
                None,
                ResourceShape::TabScoped,
                true,
                false,
                PostDispatch::None,
            ),
            (
                "gif_creator",
                Some("action"),
                ResourceShape::TabScoped,
                // Local since ADR-0053 D6: the orchestrator moved into the binary; the extension
                // keeps only the screencast capture relay.
                true,
                false,
                PostDispatch::None,
            ),
            (
                "explain",
                None,
                ResourceShape::DomainLess,
                true,
                false,
                PostDispatch::None,
            ),
        ];

        assert_eq!(REGISTRY.len(), EXPECTED_TOOLS.len());
        for (row, expected) in REGISTRY.iter().zip(EXPECTED_TOOLS.iter()) {
            let (tool, action_key, resource, is_local, has_postprocess, post_dispatch) = *expected;
            assert_eq!(row.tool, tool);
            assert_eq!(row.action_key, action_key, "{tool}: action_key mismatch");
            assert_eq!(row.resource, resource, "{tool}: resource mismatch");
            assert!(
                matches!(row.handler, Handler::Local(_)) == is_local,
                "{tool}: handler kind mismatch"
            );
            assert_eq!(
                row.postprocess.is_some(),
                has_postprocess,
                "{tool}: postprocess mismatch"
            );
            assert_eq!(
                row.post_dispatch, post_dispatch,
                "{tool}: post_dispatch mismatch"
            );
        }
    }

    /// Belt to the server-side pin `pinned_explain_text_matches_the_real_directory_formatter`
    /// (the byte-exact oracle, untouched by this task): pins the first line, the last line, and
    /// the total line count, all transcribed from the CURRENT output before the registry
    /// reshape.
    #[test]
    fn explain_text_is_unchanged_by_the_registry_reshape() {
        let text = explain_text();
        let lines: Vec<&str> = text.split('\n').collect();
        let total_variants: usize = REGISTRY.iter().map(|row| row.variants.len()).sum();
        assert_eq!(
            lines.len(),
            total_variants + 2,
            "vocab + blank + one per variant"
        );
        assert_eq!(
            lines[0],
            "Capabilities: read = retrieve and observe only; action = dispatch UI input whose \
             effect the page decides (this can trigger writes); write = declared state-changing \
             operations; execute = arbitrary code."
        );
        assert!(text.is_ascii());
        assert_eq!(lines[1], "", "blank separator line");
        assert_eq!(
            lines[2],
            "tabs_context_mcp: requires read. List the MCP tab group: the ids, URLs, and \
             titles of the tabs this server controls."
        );
        assert_eq!(
            lines[5],
            "computer (left_click): requires action. Left-click at coordinates; commits an \
             activation whose effect the page decides."
        );
        let last = *lines.last().unwrap();
        assert_eq!(
            last,
            "explain: requires nothing. Show every action available here and the capability \
             each one requires."
        );
    }
}
