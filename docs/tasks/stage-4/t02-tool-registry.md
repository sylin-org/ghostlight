# T02: the tool registry

## Goal

Implement ADR-0024 Decision 1: generalize the ADR-0022 action directory
(`src/browser/directory.rs`) IN PLACE into the single per-tool authority -- one
`ToolDescriptor` per advertised tool (14), each carrying its action variants (the existing
26 capability rows), resource shape, handler kind, and hooks. Purely a data/lookup change:
nothing outside `directory.rs` consumes the new fields yet (t04 does); the existing
`requires(tool, action)` fn-pointer contract and `explain_text()` output stay byte-stable.

## Authority

ADR-0024 (`docs/adr/0024-tool-registry-and-generic-ingest-pipeline.md`) Decision 1 is
normative, including the data-plus-hooks rule (descriptors are data; the pipeline owns
behavior; enum markers for Browser-dependent behavior) and the family-seam paragraph.
ADR-0022 Decision 2 remains normative for every `requires` value and the absent-vs-empty
invariant. Where this prompt and an ADR disagree, THE ADR WINS -- but per BOOTSTRAP rule
4, the ADR's type sketches are SCHEMATIC and this prompt's Rust signatures are their
normative rendering (`Handler::Local(fn() -> String)` matches the real
`explain_text() -> String`; t04 wraps the String in `text_content`). Rendering a sketch
precisely is not a disagreement; record no deviation for it.

## Depends on

t01 landed (LEDGER RESUME HERE shows t01 committed; the tree is green). No compile
dependency on t01; sequence order is absolute anyway.

STOP preconditions: `src/browser/directory.rs` exists with a 26-row `DIRECTORY` const of
`ActionDescriptor { tool, action, requires, description }` and public `requires()` +
`explain_text()`. If the module has already been reshaped, STOP and record.

## Current behavior (verified 2026-07-03 against `b4b2faf`; re-read the tree)

- `src/browser/directory.rs`: `ActionDescriptor` (4 fields, all `'static`), `DIRECTORY`
  (26 rows: 13 plain tools + 13 `computer` sub-actions, tools.json advertised order with
  computer expanded in place in enum order, `explain` last), `requires(tool, action)`
  (`tool == "computer"` consults action; other tools ignore it; miss = `None`, free =
  `Some(&[])`), `explain_text()` (vocabulary block + one line per row in order; the
  Some-action label hardcodes the literal `computer ({action})`). Inline tests:
  `directory_covers_the_sacred_surface_exactly`, `directory_requires_match_the_adr_table`
  (a full 26-triple `EXPECTED` const), `absent_is_none_and_empty_is_some`,
  `every_description_is_nonempty_ascii_and_short`, plus explain_text structural tests.
  The fixture-mirror helpers parse `crate::transport::mcp::tools::TOOLS_JSON`.
- `src/transport/mcp/server.rs` holds `pinned_explain_text()` (a hand-transcribed full
  pin) and `pinned_explain_text_matches_the_real_directory_formatter` -- the drift guard
  that MUST keep passing unmodified.
- Fixture facts (sacred, frozen): advertised tool order `tabs_context_mcp`,
  `tabs_create_mcp`, `navigate`, `computer`, `find`, `form_input`, `get_page_text`,
  `javascript_tool`, `read_console_messages`, `read_network_requests`, `read_page`,
  `resize_window`, `update_plan`, `explain`. Computer action enum order: `left_click`,
  `right_click`, `type`, `screenshot`, `wait`, `scroll`, `key`, `left_click_drag`,
  `double_click`, `triple_click`, `zoom`, `scroll_to`, `hover`.
- Only `tabs_create_mcp` and `explain` have empty `properties` in their input schemas;
  `resize_window` and `computer` carry a `tabId` property; `navigate` carries `url`.

## Required behavior

### 1. The new types (in `directory.rs`)

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ResourceShape {
        DomainLess,
        TabScoped,
        TargetArg,
    }

    #[derive(Clone, Copy)]
    pub enum Handler {
        ExtensionForward,
        Local(fn() -> String),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PostDispatch {
        None,
        NavigateLanding,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct ActionVariant {
        pub action: Option<&'static str>,
        pub requires: &'static [Capability],
        pub description: &'static str,
    }

    #[derive(Clone, Copy)]
    pub struct ToolDescriptor {
        pub tool: &'static str,
        pub action_key: Option<&'static str>,
        pub variants: &'static [ActionVariant],
        pub resource: ResourceShape,
        pub handler: Handler,
        pub postprocess: Option<fn(&mut serde_json::Value, bool)>,
        pub post_dispatch: PostDispatch,
    }

Doc comments: transcribe the ADR-0024 Decision 1 semantics (data-plus-hooks rule;
markers for Browser-dependent behavior; the family-seam sentence that the shape is
deliberately plugin-manifest-like). `Handler::Local`'s doc states it is for tools
answered inside the binary with no extension frame (`explain`); its `fn` returns the
full response text.

### 2. The 14 descriptors (`REGISTRY`)

`pub const REGISTRY: &[ToolDescriptor]` in the fixture's advertised tool order. The
`variants` arrays carry EXACTLY the 26 existing rows' `(action, requires, description)`
triples, unchanged byte-for-byte (transcribe from the current `DIRECTORY`; computer's 13
variants in enum order). New per-tool fields:

| tool                  | action_key     | resource   | handler          | postprocess | post_dispatch   |
|-----------------------|----------------|------------|------------------|-------------|-----------------|
| tabs_context_mcp      | None           | DomainLess | ExtensionForward | None        | None            |
| tabs_create_mcp       | None           | DomainLess | ExtensionForward | None        | None            |
| navigate              | None           | TargetArg  | ExtensionForward | None        | NavigateLanding |
| computer              | Some("action") | TabScoped  | ExtensionForward | None        | None            |
| find                  | None           | TabScoped  | ExtensionForward | None        | None            |
| form_input            | None           | TabScoped  | ExtensionForward | None        | None            |
| get_page_text         | None           | TabScoped  | ExtensionForward | None        | None            |
| javascript_tool       | None           | TabScoped  | ExtensionForward | None        | None            |
| read_console_messages | None           | TabScoped  | ExtensionForward | None        | None            |
| read_network_requests | None           | TabScoped  | ExtensionForward | None        | None            |
| read_page             | None           | TabScoped  | ExtensionForward | Some(...)   | None            |
| resize_window         | None           | TabScoped  | ExtensionForward | None        | None            |
| update_plan           | None           | DomainLess | ExtensionForward | None        | None            |
| explain               | None           | DomainLess | Local(...)       | None        | None            |

- `read_page.postprocess` is `Some(crate::browser::redact::apply_to_result)` -- verify
  that function's signature is `fn(&mut serde_json::Value, bool)` and adjust the field
  type to match the REAL signature if it differs (record a deviation if so; never wrap).
- `explain.handler` is `Local(explain_text)` (the existing formatter, unchanged output).
- `ResourceShape` mirrors today's `resolve_governing_resource` name-match exactly:
  `navigate` = TargetArg; `tabs_context_mcp`/`tabs_create_mcp`/`update_plan` = DomainLess
  (plus `explain`, which never reached that match); everything else = TabScoped. This
  drives ONLY grant-path resource resolution (t04); the sacred STEP B check stays
  argument-driven (any call carrying a numeric `tabId`), NOT shape-driven -- pin this
  distinction in the `ResourceShape` doc comment now so t04 cannot misread it.

### 3. Lookup surface

- `pub fn descriptor(tool: &str) -> Option<&'static ToolDescriptor>` -- linear scan, the
  validity check t04 will use.
- `requires(tool, action)` KEEPS its exact signature and semantics (the governance
  fn-pointer contract): reimplement over `descriptor()` + `variants` (for a tool with
  `action_key`, the action argument selects the variant, absent/unknown action = `None`;
  for every other tool the action argument is ignored and the single variant answers).
  The absent-vs-empty invariant is unchanged.
- `explain_text()` reimplemented over the registry: same iteration order (descriptor
  order, variants in order), label generalized to `{tool} ({action})` from row data (no
  hardcoded "computer" literal). Output must be byte-identical: the server-side pin
  (`pinned_explain_text_matches_the_real_directory_formatter`) is the oracle and MUST NOT
  be edited.
- DELETE `ActionDescriptor` and the flat `DIRECTORY` const (the variants absorb them).
  Rework the inline tests onto the registry per the Tests section; the old test bodies'
  pinned VALUES are transcribed, never re-derived.

### 4. Module docs

Rewrite the module doc: the registry is the ADR-0024 Decision 1 single per-tool
authority (successor to the ADR-0022 action directory, absorbing it as `variants`);
state the data-plus-hooks rule, the absent-vs-empty invariant, the
fixture-validated-never-generated rule, and the family-seam sentence. Update
`src/browser/mod.rs`'s doc sentence naming the directory to name the registry (keep the
`[`directory`]` module name and link; the MODULE is not renamed -- only its contents
generalize).

## Constraints

1. Only `src/browser/directory.rs` and `src/browser/mod.rs` change (plus LEDGER).
   `git diff --stat` must show exactly those (plus the ledger).
2. Byte-stability oracles: `explain_text()` output (server-side pin untouched and green);
   every `requires()` result for all 26 (tool, action) pairs plus the miss cases
   (unchanged semantics); `tests/tool_schema_fidelity.rs` and tools.json untouched.
3. `tests/architecture.rs` green (registry stays browser-side; governance untouched).
4. Nothing outside the module consumes the new fields yet: `rg -n
   "ResourceShape|Handler::|PostDispatch|descriptor\(" src/ --glob '!src/browser/directory.rs'`
   returns nothing (except this task's own mod.rs doc text, which must not name the types).
5. ASCII only; no new dependencies; commit message
   `refactor(architecture): t02 tool registry`.

## Tests (minimum)

Rework/extend the inline `#[cfg(test)]` module, keeping the fixture-mirror technique:

1. `registry_covers_the_sacred_surface_exactly` (rework of
   `directory_covers_the_sacred_surface_exactly`): descriptor tool names equal the
   fixture's 14 advertised names in order; exactly one descriptor has `action_key`
   (`computer`, key `"action"`); its variant action set equals the fixture's 13-action
   enum in order; every other descriptor has exactly one variant with `action: None`;
   total variants across the registry == 26; no duplicate (tool, action) pairs.
2. `registry_requires_match_the_adr_table` (rework): the same 26-triple `EXPECTED` const
   as today (transcribed), asserted positionally against the flattened
   (descriptor, variant) sequence.
3. `absent_is_none_and_empty_is_some`: byte-identical assertions to today's test (the
   function under test kept its contract).
4. `every_description_is_nonempty_ascii_and_short`: same assertions, iterating variants.
5. `per_tool_fields_match_the_adr_table` (NEW): a pinned
   `EXPECTED_TOOLS: &[(&str, Option<&str>, ResourceShape, /* handler kind as bool or
   tag */, bool /* postprocess.is_some() */, PostDispatch)]` const carrying section 2's
   table, asserted positionally. (Handler does not derive PartialEq; assert via
   `matches!`.)
6. `explain_text_is_unchanged_by_the_registry_reshape` (NEW, belt to the server-side
   suspender): pin the first line (`Capabilities: read = ...` exact) and the last line
   (the explain row) and the total line count of `explain_text()` -- transcribe the
   values from the CURRENT output before reshaping.
7. Existing server-side `pinned_explain_text_matches_the_real_directory_formatter`:
   untouched, green.

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all green;
constraint-4 rg clean; ASCII scan on both touched files; tools.json/fidelity byte-
untouched. No browser checks queued (pure data; nothing observable live yet). Update
LEDGER (entry + RESUME HERE -> t03) and commit.

## Out of scope

- ANY consumer change: the pipeline, `is_known_tool`, sacred, resource resolution,
  redaction call sites, explain handler dispatch (all t04).
- The governance API (t03), tab-URL (t05), hot-reload (t06), deletions elsewhere (t07).
- Renaming the `directory` module or files; adding tools; touching descriptions or
  requirement sets (ADR-0022 owns them; byte-stable).
