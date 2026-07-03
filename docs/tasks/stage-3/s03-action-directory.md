# S03: the action directory in the browser plugin

## Goal

Add the ADR-0022 Decision 2 per-action requirement directory as browser-plugin data: a new
module `src/browser/directory.rs` holding one static table with every action's bound
requirement set plus a curt, agent-targeted description, and a `requires` lookup function.
This task is purely ADDITIVE: `src/browser/classify.rs` stays alive, untouched, and remains
the authority actually consumed by enforcement and audit until s05/s06 switch consumers over.
Nothing consumes the directory in this task.

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) is normative; where this prompt
and the ADR disagree, THE ADR WINS (record the deviation in the ledger). Decision 2 defines
the table's semantics, the population rule, and the absent-vs-empty invariant; this prompt
transcribes its `requires` values and pins the row order, Rust shapes, and descriptions.

## Depends on

- s02 landed: `Capability` exists in `src/governance/ports.rs` with variants `Read`,
  `Action`, `Write`, `Execute` (wire names `"read"`, `"action"`, `"write"`, `"execute"`) and
  derived equality (the tests below compare capability slices with `assert_eq!`).

If `Capability` is missing from `src/governance/ports.rs`, STOP: earlier tasks did not land.

## Current behavior (verify against the tree before editing)

- `src/browser/classify.rs`: `TOOL_CLASSES` (12 entries, tools.json advertised order,
  `computer` deliberately absent), `COMPUTER_ACTION_CLASSES` (13 entries, tools.json `action`
  enum order), and `classify(tool: &str, action: Option<&str>) -> Option<RwClass>` (for
  `"computer"` the action is required; for every other tool it is ignored; unknown inputs
  return `None`). Its `#[cfg(test)]` module parses
  `crate::transport::mcp::tools::TOOLS_JSON` with `serde_json` to build the sacred tool-name
  set and the sacred computer-action set, then asserts the tables mirror the fixture with no
  gaps, no stale entries, no duplicates. Your tests use the same technique.
- `src/transport/mcp/tools.rs`: `pub const TOOLS_JSON: &str =
  include_str!("schemas/tools.json");`. The fixture's advertised tool order is:
  `tabs_context_mcp`, `tabs_create_mcp`, `navigate`, `computer`, `find`, `form_input`,
  `get_page_text`, `javascript_tool`, `read_console_messages`, `read_network_requests`,
  `read_page`, `resize_window`, `update_plan` (13 entries). The `computer` `action` enum
  order is: `left_click`, `right_click`, `type`, `screenshot`, `wait`, `scroll`, `key`,
  `left_click_drag`, `double_click`, `triple_click`, `zoom`, `scroll_to`, `hover` (13).
- `src/browser/mod.rs` registers, in this order: `advertise`, `classify`, `pattern`,
  `redact`, `resource`, `sacred`, `tools`. There is no `directory` module yet.
- `tests/architecture.rs` forbids `governance/` from naming `browser`/`transport`/`mcp`/
  `native`/`url`; the reverse edge (browser code using `crate::governance::ports`) is legal
  and already used by `classify.rs`.

## Required behavior

### 1. New module `src/browser/directory.rs`

Create the module with this exact public surface (formatting is rustfmt's; names, types, and
values are normative):

    use crate::governance::ports::Capability;

    #[derive(Debug, Clone, Copy)]
    pub struct ActionDescriptor {
        pub tool: &'static str,
        pub action: Option<&'static str>,
        pub requires: &'static [Capability],
        pub description: &'static str,
    }

    pub const DIRECTORY: &[ActionDescriptor] = &[ /* the 25 rows below, in table order */ ];

    pub fn requires(tool: &str, action: Option<&str>) -> Option<&'static [Capability]> { ... }

Module doc comment (`//!`): state that this is the action directory of ADR-0022 Decision 2
(per-action bound capability requirement sets plus agent-targeted descriptions, compiled in
as static browser-domain data); that it is additive alongside [`classify`] which remains the
enforcement/audit authority until the s05/s06 switch; that the module is pure (no I/O); and
spell out the absent-vs-empty invariant (see section 3).

### 2. The 25 rows of `DIRECTORY`

Row order is the tools.json advertised order with `computer` expanded in place into its 13
action rows in the tools.json enum order. In the table: an `action` of `-` means
`action: None`, otherwise `action: Some("<value>")`; a `requires` of `-` means
`requires: &[]`, and `read`/`action`/`write`/`execute` mean `requires: &[Capability::Read]`
/ `&[Capability::Action]` / `&[Capability::Write]` / `&[Capability::Execute]`. The
description column is the exact string literal contents; transcribe verbatim, do not invent.

| #  | tool                  | action          | requires | description |
|----|-----------------------|-----------------|----------|-------------|
| 1  | tabs_context_mcp      | -               | read     | List the MCP tab group: the ids, URLs, and titles of the tabs this server controls. |
| 2  | tabs_create_mcp       | -               | -        | Open a new empty tab in the MCP tab group; touches no page and no server. |
| 3  | navigate              | -               | read     | Load a URL in a tab, or go back or forward in its history; a top-level GET. |
| 4  | computer              | left_click      | action   | Left-click at coordinates; commits an activation whose effect the page decides. |
| 5  | computer              | right_click     | action   | Right-click at coordinates; commits an activation. |
| 6  | computer              | type            | action   | Type text into the focused element; commits data to page handlers. |
| 7  | computer              | screenshot      | read     | Capture a screenshot of the visible viewport. |
| 8  | computer              | wait            | -        | Pause for a duration; touches no page and no server. |
| 9  | computer              | scroll          | read     | Scroll the viewport; moves the view without committing input to the page. |
| 10 | computer              | key             | action   | Press a key or key combination; commits input to page handlers. |
| 11 | computer              | left_click_drag | action   | Click and drag between two points; commits pointer input to the page. |
| 12 | computer              | double_click    | action   | Double-click at coordinates; commits an activation. |
| 13 | computer              | triple_click    | action   | Triple-click at coordinates; commits an activation. |
| 14 | computer              | zoom            | read     | Capture a zoomed screenshot of a page region. |
| 15 | computer              | scroll_to       | read     | Scroll an element into view; moves the viewport without committing input. |
| 16 | computer              | hover           | read     | Move the pointer over a point; commits no activation and no data. |
| 17 | find                  | -               | read     | Search the page for elements matching a natural-language description. |
| 18 | form_input            | -               | write    | Fill or set values in form fields; a declared, state-changing write. |
| 19 | get_page_text         | -               | read     | Extract the page's readable text content, article-first, without HTML. |
| 20 | javascript_tool       | -               | execute  | Run arbitrary JavaScript in the page; unbounded, and can bypass the UI entirely. |
| 21 | read_console_messages | -               | read     | Read buffered browser console messages from a tab. |
| 22 | read_network_requests | -               | read     | Read buffered HTTP network requests observed in a tab. |
| 23 | read_page             | -               | read     | Read the page as an accessibility tree of elements with reference ids. |
| 24 | resize_window         | -               | -        | Resize the browser window; browser state only, touches no page content. |
| 25 | update_plan           | -               | -        | Present a plan of intended actions to the user; informational only. |

The `requires` values transcribe ADR-0022 Decision 2's table exactly (12 tools + 13 computer
actions = 25 rows). The `explain` tool's row is NOT added here; s07 adds it as row 26.

### 3. The lookup function

`pub fn requires(tool: &str, action: Option<&str>) -> Option<&'static [Capability]>`, same
lookup shape as `classify::classify`:

- `tool == "computer"`: the action is required. Absent action returns `None`. Otherwise
  return the `requires` of the `DIRECTORY` row with `tool == "computer"` and
  `action == Some(<the action>)`; an unknown action returns `None`.
- Every other tool: the `action` argument is IGNORED. Return the `requires` of the
  `DIRECTORY` row with a matching `tool` and `action: None`; an unknown tool returns `None`.

The function's doc comment MUST spell out the absent-vs-empty distinction, citing ADR-0022
Decision 2: `None` is a classification MISS (callers deny; fail closed), while `Some(&[])`
means "requires nothing" (unconditionally allowed). The two are distinct states and must
never be conflated.

### 4. Registration

In `src/browser/mod.rs`: add `pub mod directory;` between `pub mod classify;` and
`pub mod pattern;` (alphabetical order). In the module doc comment, insert this exact
sentence (wrapping across `//!` lines is free; the text, including the intra-doc-link
backticks, is normative) immediately before the sentence that begins `It may depend on`:

    Stage 3 adds the action directory ([`directory`], ADR-0022 Decision 2: per-action
    capability requirement sets and agent-facing descriptions; additive alongside
    [`classify`] until the s05/s06 switch).

## Constraints

1. Purely additive: the only files changed are `src/browser/directory.rs` (new) and
   `src/browser/mod.rs` (registration + one doc sentence). `classify.rs` is byte-identical
   before and after.
2. Do not touch `src/transport/mcp/schemas/tools.json`, `tests/tool_schema_fidelity.rs`, or
   anything under `src/governance/`.
3. `tests/architecture.rs` must pass: the directory lives in `browser/` and uses
   `crate::governance::ports::Capability`, which is a legal browser-to-governance edge.
4. The module is pure: no I/O, no new dependencies, ASCII only.

## Tests (minimum)

Inline `#[cfg(test)]` in `directory.rs`, mirroring `classify.rs`'s fidelity technique
(reproduce its `sacred_tool_names()` and `sacred_computer_actions()` helpers, parsing
`crate::transport::mcp::tools::TOOLS_JSON` with `serde_json`). Exactly these test names:

- `directory_covers_the_sacred_surface_exactly`: the tool names of rows with `action: None`
  equal the fixture's tool-name set minus `computer` (no gaps, no stale entries); no
  `action: None` row has tool `computer`; every row with a `Some` action has tool
  `computer`; the action set of the `computer` rows equals the fixture's `action` enum set
  and has 13 entries; `DIRECTORY.len() == 25`; no duplicate `(tool, action)` pairs.
- `directory_requires_match_the_adr_table`: declare a local literal
  `const EXPECTED: &[(&str, Option<&str>, &[Capability])]` with all 25 entries in section
  2's table order, assert `DIRECTORY.len() == EXPECTED.len()`, and assert each row's
  `(tool, action, requires)` equals its positional `EXPECTED` entry (this also pins the row
  order).
- `absent_is_none_and_empty_is_some`: assert exactly these:
  `requires("no_such_tool", None) == None`; `requires("computer", None) == None`;
  `requires("computer", Some("no_such_action")) == None`;
  `requires("tabs_create_mcp", None) == Some(&[][..])`;
  `requires("update_plan", None) == Some(&[][..])`;
  `requires("computer", Some("wait")) == Some(&[][..])`;
  `requires("navigate", None) == Some(&[Capability::Read][..])`;
  `requires("javascript_tool", None) == Some(&[Capability::Execute][..])`;
  `requires("form_input", None) == Some(&[Capability::Write][..])`;
  `requires("computer", Some("left_click")) == Some(&[Capability::Action][..])`;
  and `requires("read_page", Some("left_click")) == Some(&[Capability::Read][..])` with the
  message `action is ignored for non-computer tools`.
- `every_description_is_nonempty_ascii_and_short`: for every `DIRECTORY` row,
  `!description.is_empty()`, `description.is_ascii()`, `description.len() <= 90`, and
  `description == description.trim()`.

## Verification

`cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean; `cargo test`
all green including the four new tests, with `tests/architecture.rs`,
`tests/all_open_golden.rs`, `tests/mcp_protocol.rs`, and `tests/tool_schema_fidelity.rs`
passing unmodified. `git diff --stat` (plus untracked files) shows only
`src/browser/directory.rs` and `src/browser/mod.rs`. ASCII scan on both touched files:
`rg -n "[^\x00-\x7F]" src/browser/directory.rs src/browser/mod.rs` produces no output.
No browser checks are queued for this task (pure static data; nothing observable in a live
browser yet).

## Out of scope

- Consuming the directory anywhere: enforcement, dispatch, advertisement, audit, simulate,
  and explain all still consume `classify.rs` until s05/s06.
- The `explain` tool row and any `tools.json` or fidelity-test change (s07).
- Deleting or modifying `classify.rs` or `RwClass` (s05/s06).
- Any change under `src/governance/` (the `Capability` type landed in s02).
- Documentation outside code comments (s08).
