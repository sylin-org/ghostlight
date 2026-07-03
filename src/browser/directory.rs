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
//! The registry is validated against the tools.json fixture, never the reverse (ADR-0024
//! Decision 1): fixture-mirror tests assert the registry covers exactly the fixture's
//! advertised names, in order, and the `computer` action enum, with no gaps, no stale
//! entries, no duplicates.
//!
//! The module is pure: no I/O, no allocation beyond what slice iteration needs, no
//! dependencies beyond `core`/`std` plus the `serde_json::Value` type named in one function
//! pointer signature (never constructed here).

use crate::governance::ports::Capability;

/// The resource-shape classification driving GRANT-PATH resource resolution only (ADR-0024
/// Decision 1), mirroring today's `resolve_governing_resource` name match exactly. This is NOT
/// used to decide whether the sacred STEP B tab check runs: that check is ARGUMENT-driven (any
/// call carrying a numeric `tabId`), independent of this shape, because tool arguments are not
/// schema-validated and a never-touch check must never be gated by a classification that could
/// itself be wrong for a malformed call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceShape {
    /// No governed resource: `tabs_context_mcp`, `tabs_create_mcp`, `update_plan`, `explain`.
    DomainLess,
    /// Governed by the tab's current URL, resolved via `tabId`: `read_page`, `computer`,
    /// `find`, `form_input`, and the other tab-scoped tools.
    TabScoped,
    /// Governed by the `url` argument: `navigate` only (extension-mirrored normalization).
    TargetArg,
}

/// How a call is dispatched once authorized (ADR-0024 Decision 1).
#[derive(Clone, Copy)]
pub enum Handler {
    /// The default: forward to the extension over native messaging via `Browser::call`. 13 of
    /// the 14 registry rows use this.
    ExtensionForward,
    /// Answered entirely inside the binary, with no extension frame: `explain`. The `fn`
    /// returns the full response text; the pipeline wraps it in the MCP result envelope.
    Local(fn() -> String),
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
/// agent-targeted description (ADR-0022 Decision 2 row, unchanged). A tool with no
/// sub-actions carries exactly one variant with `action: None`; `computer` carries 13, one per
/// `action_key` value.
#[derive(Debug, Clone, Copy)]
pub struct ActionVariant {
    pub action: Option<&'static str>,
    pub requires: &'static [Capability],
    pub description: &'static str,
}

/// One row of the tool registry (ADR-0024 Decision 1): the single per-tool authority for
/// validity, classification, advertisement, explain, resource shape, dispatch kind, and result
/// post-processing. Descriptors are DATA; the pipeline owns BEHAVIOR (see the module doc
/// comment).
#[derive(Clone, Copy)]
pub struct ToolDescriptor {
    pub tool: &'static str,
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
}

/// The tool registry (ADR-0024 Decision 1): 14 descriptors (the 13 trained tools plus
/// `explain`), in tools.json advertised order. `computer`'s 13 variants are in tools.json
/// `action` enum order, absorbing the former 26-row `ActionDescriptor` directory unchanged,
/// byte-for-byte, as `variants`.
pub const REGISTRY: &[ToolDescriptor] = &[
    ToolDescriptor {
        tool: "tabs_context_mcp",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description:
                "List the MCP tab group: the ids, URLs, and titles of the tabs this server controls.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "tabs_create_mcp",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            description: "Open a new empty tab in the MCP tab group; touches no page and no server.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "navigate",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description: "Load a URL in a tab, or go back or forward in its history; a top-level GET.",
        }],
        resource: ResourceShape::TargetArg,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::NavigateLanding,
    },
    ToolDescriptor {
        tool: "computer",
        action_key: Some("action"),
        variants: &[
            ActionVariant {
                action: Some("left_click"),
                requires: &[Capability::Action],
                description:
                    "Left-click at coordinates; commits an activation whose effect the page decides.",
            },
            ActionVariant {
                action: Some("right_click"),
                requires: &[Capability::Action],
                description: "Right-click at coordinates; commits an activation.",
            },
            ActionVariant {
                action: Some("type"),
                requires: &[Capability::Action],
                description: "Type text into the focused element; commits data to page handlers.",
            },
            ActionVariant {
                action: Some("screenshot"),
                requires: &[Capability::Read],
                description: "Capture a screenshot of the visible viewport.",
            },
            ActionVariant {
                action: Some("wait"),
                requires: &[],
                description: "Pause for a duration; touches no page and no server.",
            },
            ActionVariant {
                action: Some("scroll"),
                requires: &[Capability::Read],
                description: "Scroll the viewport; moves the view without committing input to the page.",
            },
            ActionVariant {
                action: Some("key"),
                requires: &[Capability::Action],
                description: "Press a key or key combination; commits input to page handlers.",
            },
            ActionVariant {
                action: Some("left_click_drag"),
                requires: &[Capability::Action],
                description: "Click and drag between two points; commits pointer input to the page.",
            },
            ActionVariant {
                action: Some("double_click"),
                requires: &[Capability::Action],
                description: "Double-click at coordinates; commits an activation.",
            },
            ActionVariant {
                action: Some("triple_click"),
                requires: &[Capability::Action],
                description: "Triple-click at coordinates; commits an activation.",
            },
            ActionVariant {
                action: Some("zoom"),
                requires: &[Capability::Read],
                description: "Capture a zoomed screenshot of a page region.",
            },
            ActionVariant {
                action: Some("scroll_to"),
                requires: &[Capability::Read],
                description: "Scroll an element into view; moves the viewport without committing input.",
            },
            ActionVariant {
                action: Some("hover"),
                requires: &[Capability::Read],
                description: "Move the pointer over a point; commits no activation and no data.",
            },
        ],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "find",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description: "Search the page for elements matching a natural-language description.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "form_input",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Write],
            description: "Fill or set values in form fields; a declared, state-changing write.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "get_page_text",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description: "Extract the page's readable text content, article-first, without HTML.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "javascript_tool",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Execute],
            description:
                "Run arbitrary JavaScript in the page; unbounded, and can bypass the UI entirely.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "read_console_messages",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description: "Read buffered browser console messages from a tab.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "read_network_requests",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description: "Read buffered HTTP network requests observed in a tab.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "read_page",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[Capability::Read],
            description: "Read the page as an accessibility tree of elements with reference ids.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: Some(crate::browser::redact::apply_to_result),
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "resize_window",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            description: "Resize the browser window; browser state only, touches no page content.",
        }],
        resource: ResourceShape::TabScoped,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "update_plan",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            description: "Present a plan of intended actions to the user; informational only.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::ExtensionForward,
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
    ToolDescriptor {
        tool: "explain",
        action_key: None,
        variants: &[ActionVariant {
            action: None,
            requires: &[],
            description: "Show every action available here and the capability each one requires.",
        }],
        resource: ResourceShape::DomainLess,
        handler: Handler::Local(explain_text),
        postprocess: None,
        post_dispatch: PostDispatch::None,
    },
];

/// Look up a tool's registry row by name. Linear scan over 14 rows; the validity check the
/// pipeline uses (replacing the transport layer's former per-call fixture re-parse).
pub fn descriptor(tool: &str) -> Option<&'static ToolDescriptor> {
    REGISTRY.iter().find(|row| row.tool == tool)
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
        Some(_) => {
            let action = action?;
            row.variants
                .iter()
                .find(|variant| variant.action == Some(action))
                .map(|variant| variant.requires)
        }
        None => row.variants.first().map(|variant| variant.requires),
    }
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
                variant.description
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mcp::tools::TOOLS_JSON;
    use std::collections::HashSet;

    fn sacred_tool_names_in_order() -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(TOOLS_JSON).unwrap();
        v["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn sacred_computer_actions_in_order() -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(TOOLS_JSON).unwrap();
        let computer = v["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "computer")
            .expect("computer tool present");
        computer["inputSchema"]["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn registry_covers_the_sacred_surface_exactly() {
        let sacred_names = sacred_tool_names_in_order();
        let registry_names: Vec<String> = REGISTRY.iter().map(|row| row.tool.to_string()).collect();
        assert_eq!(
            registry_names, sacred_names,
            "registry tool order must match the fixture's advertised order exactly"
        );

        let with_action_key: Vec<&ToolDescriptor> = REGISTRY
            .iter()
            .filter(|row| row.action_key.is_some())
            .collect();
        assert_eq!(
            with_action_key.len(),
            1,
            "exactly one descriptor may carry an action_key"
        );
        assert_eq!(with_action_key[0].tool, "computer");
        assert_eq!(with_action_key[0].action_key, Some("action"));

        let sacred_actions = sacred_computer_actions_in_order();
        let computer_actions: Vec<String> = with_action_key[0]
            .variants
            .iter()
            .map(|variant| {
                variant
                    .action
                    .expect("every computer variant has an action")
                    .to_string()
            })
            .collect();
        assert_eq!(computer_actions, sacred_actions);
        assert_eq!(computer_actions.len(), 13);

        for row in REGISTRY.iter().filter(|row| row.tool != "computer") {
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

        let total_variants: usize = REGISTRY.iter().map(|row| row.variants.len()).sum();
        assert_eq!(total_variants, 26);

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

    #[test]
    fn every_description_is_nonempty_ascii_and_short() {
        for row in REGISTRY {
            for variant in row.variants {
                assert!(
                    !variant.description.is_empty(),
                    "empty description: {} {:?}",
                    row.tool,
                    variant.action
                );
                assert!(
                    variant.description.is_ascii(),
                    "non-ascii description: {} {:?}",
                    row.tool,
                    variant.action
                );
                assert!(
                    variant.description.len() <= 90,
                    "description too long ({} chars): {} {:?}",
                    variant.description.len(),
                    row.tool,
                    variant.action
                );
                assert_eq!(
                    variant.description,
                    variant.description.trim(),
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
