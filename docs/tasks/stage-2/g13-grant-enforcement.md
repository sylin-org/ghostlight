# G13: Per-call grant enforcement at the five points

## Goal

Turn the documented no-op policy seam into real, manifest-driven grant enforcement.
For every tool call, resolve the applicable grant from the tuple (tool name, `computer`
sub-action, read/write class from G05, governing domain via the G07 matcher) and either
allow the call or block it BEFORE it reaches the extension, at the five enforcement
points SPEC section 5 defines. Denials use the G08 denial format and carry the stable
denial id of the denying rule and grant (ADR-0020 commitment 6). Every decision, allow
and deny alike, flows to the audit record with its grant id. With no manifest, behavior
stays byte-identical to today (all-open, ADR-0013), and a test pins that.

This is the core of ADR-0018 step 3 (the full manifest engine,
`docs/adr/0018-governance-observe-then-enforce.md`). G14 (advertisement filtering) and
G15 (shadow mode) build on top of this task; neither is part of it.

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, rule string, message template, and file location in this task comes from
  it verbatim. Read it before writing any code. Load-bearing sections: 4.3 (grants and
  grant resolution), 4.5 (no manifest = all-open, unmatched_domain rule), 5 (domain
  pattern language and matching semantics), 6.1 (audit record fields), 7 (denial id and
  templates), 8 (read/write classification and enforcement mapping).
- G05 (`docs/tasks/stage-2/g05-rw-classification.md`): `src/policy/classify.rs` with
  `RwClass` and `classify(tool, action)`. G13 consumes it; never re-hardcode the table.
- G06 (`docs/tasks/stage-2/g06-audit-recorder.md`, audit wiring at the dispatch
  chokepoint): one audit record per call, `rw` field,
  `computer` action extraction from call arguments. G13 populates the record's
  `decision`, `grant_id`, `denial_id`, and `domain` fields.
- G07 (`docs/tasks/stage-2/g07-domain-matcher.md`): host normalization from a URL string plus pattern matching per
  shared format section 5 (WHATWG parse, lowercase, port ignored, trailing dot
  stripped, punycode/A-label comparison, wildcards never match IP literals). G13 calls
  it; never substring-match a raw URL and never re-implement URL parsing here.
- G08 (`docs/tasks/stage-2/g08-sacred-domains.md`; it also owns the denial format):
  the `Denial` type, the `PolicyDecision::Deny(Denial)` variant, the stable denial id
  (`D-` + 8 hex over SHA-256 of manifest_hash LF grant_id LF rule), and the per-rule
  message templates of shared format section 7. G13 constructs denials through it.
- The stage-2 manifest-loading and grant-resolution prerequisite: whichever G-task
  parses the active manifest (schema 2: `name`, `version`, `grants`, computed content
  hash) and exposes the resolved `Grant` values (`id`, `domains`, `access`, `tools`,
  `exclude_tools`; shared format 4.3) plus the manifest identity to the mcp-server
  role. If that machinery is not yet threaded into `src/mcp/server.rs`, stop and land
  the prerequisite first; do not invent a manifest parser inside this task.

Several prerequisites reshape `src/dispatch.rs`, `src/policy/`, and
`src/mcp/server.rs` before G13 runs. The "Current behavior" section below records the
tree as it stands today; do NOT trust it as the state you will edit. Re-read every file
named there before changing it and integrate against the code the prerequisites
actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles are separate OS processes bridged by tokio-native named-pipe (on
Windows) or Unix-domain-socket (elsewhere) IPC. Stage 1 (docs/tasks/release-1/)
hardened the engine. This is stage 2, the governance layer: a separable overlay
(ADR-0013) attached at a single dispatch chokepoint, landing observe-then-enforce
(ADR-0018), configured through a typed key registry (ADR-0019), with the org policy
experience of ADR-0020 (manifest identity, stable denial ids, shadow mode).

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win, and the
shared format doc is the reconciled single source. Concretely for this task, SPEC
section 5 defines the five enforcement points but parts of its text are superseded:

| SPEC | Status for G13 |
|---|---|
| 5.1 Tool Advertisement | Out of scope here; that is G14. Per-call enforcement (this task) stays authoritative regardless of what is advertised. |
| 5.2 Pre-Navigation Enforcement | In scope. The `unlisted_domains` tri-state is superseded: manifest with grants active + no match = deny (shared format 4.5). The "denial including the redirect chain" wording is superseded by the fixed G08 templates. |
| 5.3 Per-Call Enforcement | In scope. The tier vocabulary is superseded by shared format section 8 (observe/mutate classes; grant access `read`/`write`/`all`). The `audit.log_denials` / `audit.log_successful_calls` toggles are superseded: every call produces exactly one audit record (shared format section 6). |
| 5.4 `computer` Sub-Action Enforcement | In scope. The "screenshot or wait" enumeration is superseded by the section-8 action table (observe: `screenshot`, `scroll`, `zoom`, `wait`, `hover`, `scroll_to`). The rule that grant-level `tools`/`exclude_tools` checks match the string `"computer"`, never an action name, is retained. |
| 5.5 Denial Response Format | The freeform example is superseded by G08 (shared format section 7: `Denied (D-xxxxxxxx):` marker, per-rule templates, no enumeration of other grants). Retained: a denial is a normal MCP text tool result, not a JSON-RPC error. |

## Current behavior

All facts verified against the working tree at authoring time (before the stage-2
prerequisites land).

`src/dispatch.rs` (31 lines) is the documented seam:

- `PolicyDecision` (lines 13-17) has a single variant, `Allow`.
- `pub fn policy_check(_tool: &str) -> PolicyDecision` (lines 23-25) always returns
  `PolicyDecision::Allow`. The module doc says the overlay replaces these in place and
  that STEP 0 short-circuits to Allow when no manifest is present.
- `pub fn audit(_tool: &str) {}` (line 30) does nothing (G06 replaces it).

`src/mcp/server.rs` (156 lines) is the dispatch caller:

- `run` (line 22) builds `let config = Config::default();` once per session (line 28)
  and threads it through `handle_line` (line 55) to `handle_tools_call` (lines
  116-155).
- `handle_tools_call` extracts `name` and `arguments`, calls the no-op seams
  `dispatch::policy_check(name)` and `dispatch::audit(name)` (lines 132-133), then
  `browser.call(name, &args).await` (line 135). A `read_page` success passes through
  `policy::redact::apply_to_result` (lines 140-144). A tool execution failure becomes
  an MCP tool error result with `isError: true` and text `Error: <e>` (lines 147-153).
- There is no denial branch and no domain awareness anywhere in the binary.

`src/browser.rs` -- the mcp-server's handle to the extension:

- `Browser::call` (line 72) sends `{ "id", "type": "tool_request", "tool", "args" }`
  and awaits the correlated reply (`TOOL_TIMEOUT`, 60 s, line 25). With no extension
  connected it fails fast with `Error::NativeMessaging("browser extension is not
  connected")` (lines 90-96).
- `route_reply` (lines 153-173) routes ANY id-bearing reply: type `tool_error` becomes
  `Err(message)`, every other type becomes `Ok(result)`. A new response type therefore
  routes without changes here.

`src/native/messages.rs` (21 lines) is doc-only: it documents the
`tool_request` / `tool_response` / `tool_error` envelope as prose.

`extension/service-worker.js` -- the policy-free executor:

- The native port listener (lines 31-35) handles exactly one message type:
  `tool_request` dispatches through `dispatch(id, tool, args)` (lines 558-566) into
  the `handlers` table (lines 446-556).
- `navigate` (lines 460-477): refuses tabs outside the group, handles `"back"` /
  `"forward"` via `chrome.tabs.goBack/goForward`, otherwise normalizes the URL --
  if it does not match `/^https?:\/\//i` and not `/^(about|chrome|edge|brave):/i` it
  strips any `/^[a-z]{1,6}:\/+/i` prefix and prepends `https://` (lines 467-470) --
  validates with `new URL(url)` (returns `Invalid URL: "..."` text without navigating
  on failure, line 471), calls `chrome.tabs.update`, awaits `waitForLoad`, and returns
  `Navigated to <tab.url>...` (line 476).
- `tabContext` (lines 191-194) already reads `t.url` for group tabs, so the `tabs`
  permission needed to read a tab URL is already in use.

`src/mcp/schemas/tools.json` (SACRED, never edited): 10 of the 13 tools REQUIRE a
`tabId` argument -- `navigate` (line 47), `computer` (123), `find` (142), `form_input`
(165), `get_page_text` (184), `javascript_tool` (207), `read_console_messages` (238),
`read_network_requests` (265), `read_page` (297), `resize_window` (320). The other 3
take no `tabId`: `tabs_context_mcp`, `tabs_create_mcp`, `update_plan`.

`src/main.rs`: the `--manifest <SOURCE>` flag exists (lines 32-35) and reaches
`run_server` (line 230) but is only logged today (line 232); the manifest-loading
prerequisite threads the parsed manifest into the server loop.

Tests today: `tests/mcp_protocol.rs` spawns the binary with a unique
`BROWSER_MCP_ENDPOINT` per test (`drive`, lines 16-46);
`initialize_tools_list_and_tool_call_over_stdio` (line 49) asserts 13 tools, fixture
byte-identity (lines 74-78), and that a no-extension `tools/call` returns
`isError: true` with text containing `not connected` (lines 83-90). It must keep
passing UNCHANGED. `tests/tool_schema_fidelity.rs` guards the sacred fixture text;
UNCHANGED. There is no `tests/tool_enforcement.rs` yet; this task creates it (the name
is the one planned in CLAUDE.md).

`Cargo.toml` runtime deps: `tokio`, `serde`, `serde_json` (`preserve_order`), `clap`,
`tracing`, `tracing-subscriber`, `thiserror`, `anyhow`, `dirs` (+ Windows-only
`winreg`, `windows-sys`). `sha2`, `uuid`, the RFC 3339 time source, and any URL-parsing
crate are owned by the earlier stage-2 tasks (G06/G07/G08 and the manifest task); G13
adds NO dependency of its own. If G07 did not land a real URL parser, stop and report;
do not hand-roll one.

## Required behavior

G13 delivers four things: a pure decision core in `src/policy/enforcement.rs`, a
mechanism-only tab-URL query through the extension, the wiring of both at the dispatch
chokepoint (including the navigate pre/post checks and about:blank parking), and the
audit/denial plumbing for every decision. All policy lives in the binary.

### 1. The five enforcement points

Implement enforcement at exactly these five points. Points 1-4 run BEFORE the tool is
dispatched to the extension; point 5 runs after.

1. Tool call receipt (SPEC 5.3 steps 2-4): classify the call with G05's
   `classify(tool, action)`, resolve the grant for the governing domain, check the
   grant's `tools` / `exclude_tools` list, check the grant's `access` class.
2. Navigate target (SPEC 5.2 steps 1-4): for `navigate` with a URL argument, the
   governing domain is the TARGET URL, checked before dispatch. Navigating AWAY from
   any page is always permitted -- the current tab URL is deliberately NOT checked for
   `navigate` (this is what lets the agent leave `chrome://newtab`, an about:blank
   parking page, or an off-grant page toward an allowed one; the target is what is
   governed).
3. Per-call tab-domain check (SPEC 5.3 step 1): for every OTHER tab-scoped tool, the
   governing domain is the CURRENT URL of the tab named by the call's `tabId`
   argument, obtained from the extension (section 3 below), never from tool
   parameters. This catches drift: user clicks and late redirects between calls.
4. `computer` sub-action classification (SPEC 5.4): the rw class of a `computer` call
   is its sub-action's class from G05 (`classify("computer", Some(action))`);
   grant-level `tools` / `exclude_tools` checks match the literal string `"computer"`,
   never an action name.
5. Result handling (SPEC 5.2 step 5 + 5.5): after a dispatched `navigate` (URL,
   `"back"`, or `"forward"`) succeeds, re-query the tab URL and check the FINAL
   (post-redirect) host; if it matches no grant, park the tab on `about:blank` and
   return a denial. Every decision at every point is written to the audit record, and
   every denial is rendered through G08.

### 2. The pure decision core: `src/policy/enforcement.rs`

Create `src/policy/enforcement.rs`, declared in `src/policy/mod.rs` (one added
`pub mod enforcement;` line next to the existing module declarations). Module doc
comment: this is the per-call grant enforcement of ADR-0018 step 3, consuming G05
(classification), G07 (matching), G08 (denials), and the manifest task's `Grant` type;
SPEC 5.2-5.5 as amended by shared format sections 4.5, 7, and 8.

The core is PURE: no I/O, no async, no clock. Inputs: the active grants slice, the
tool name, the rw class, and the governing-domain observation. Output: allow (with the
resolving grant id, if any) or a structured denial (rule + grant id + domain rendering,
handed to G08). Suggested shape -- adapt names to the types the prerequisites landed
and do NOT duplicate any existing type:

```rust
/// What is known about the page a call governs, after URL normalization (G07).
pub enum CallDomain {
    /// Parser-normalized host of the governing http(s) URL.
    Host(String),
    /// The parking page: exactly "about:blank". Always allowed.
    AboutBlank,
    /// A non-http(s) URL; the payload is the scheme without the trailing colon.
    NonHttp(String),
    /// The tool governs no page (tabs_context_mcp, tabs_create_mcp, update_plan).
    NoPage,
    /// A tab-scoped tool whose tab URL could not be determined. Fails closed.
    Unknown,
}

/// The verdict of the pure core. `allowed_by` is the resolving grant's id
/// (None when no grant participates: all-open is decided before this function,
/// and AboutBlank allows without a grant).
pub enum Verdict {
    Allow { allowed_by: Option<String> },
    Deny(Denial), // G08's type
}

pub fn check_call(grants: &[Grant], tool: &str, rw: RwClass, domain: &CallDomain) -> Verdict
```

Decision procedure, in this exact order (order is load-bearing: the denial id depends
on the rule string, so the first failing rule must be deterministic):

- STEP 0 lives at the caller (section 4): when no manifest is active, `check_call` is
  never invoked and the call is allowed. The core itself always has a manifest.
- `CallDomain::AboutBlank` -> `Allow { allowed_by: None }`. The parking page is always
  allowed (shared format 5.2); no grant, tool, or access check applies on it.
- `CallDomain::NonHttp(scheme)` -> `Deny` with rule `scheme/<scheme>`, empty grant id.
- `CallDomain::Unknown` -> `Deny` with rule `unmatched_domain`, empty grant id (fail
  closed: under a manifest, a call whose governing page cannot be established is never
  allowed).
- `CallDomain::Host(host)`:
  1. Resolve the grant: iterate `grants` in manifest order; the first grant with any
     domain pattern matching `host` (G07 matcher) wins (shared format 4.3). No match
     -> `Deny`, rule `unmatched_domain`, empty grant id.
  2. Tool-list check on the resolving grant G (the checked name is the literal tool
     string; `"computer"` for computer calls): if `G.tools` is a non-null list, the
     name must be in it; else if `G.exclude_tools` is present, the name must NOT be in
     it. Fail -> `Deny`, rule `tool/<tool name>`, grant id `G.id`.
  3. Access check: rw `Observe` requires `G.access` in {`read`, `all`}; rw `Mutate`
     requires `G.access` in {`write`, `all`} (`write` does NOT imply `read`). Fail ->
     `Deny`, rule `access`, grant id `G.id`.
  4. Otherwise `Allow { allowed_by: Some(G.id) }`.
  The tool-list check runs BEFORE the access check (a grant that both excludes the
  tool and lacks the class denies with rule `tool/...`); pin this with a test.
- `CallDomain::NoPage` (the 3 tools without `tabId`): no host exists, so domain
  matching cannot pick a grant. Use the union rule, mirroring G14's advertisement
  membership test so per-call is never more permissive than advertisement:
  1. Let `candidates` = grants passing the tool-list check for this tool, in manifest
     order.
  2. `candidates` empty -> `Deny`, rule `tool/<tool name>`, empty grant id.
  3. If any candidate's `access` covers the call's rw class ->
     `Allow { allowed_by: Some(first such candidate's id) }`.
  4. Else -> `Deny`, rule `access`, grant id = first candidate's id (deterministic:
     manifest order).

Unclassifiable calls: when G05's `classify` returns `None` (tool not on the sacred
surface, or a `computer` call with a missing/unknown action), the caller denies with
rule `tool/<tool name>` and empty grant id before building a `CallDomain` (a call
whose class cannot be determined is never authorized under a manifest). Put this in
the core or the caller, but pin it with a test either way.

Domain rendering for G08's `<domain>` template substitution: the host string for
`Host`; the literal string `(unknown)` for `NoPage` and `Unknown`. The audit `domain`
field is the host for `Host` and JSON `null` otherwise (shared format 6.1); never put
`(unknown)` in the audit record.

### 3. Mechanism-only tab-URL query (extension + wire + `Browser`)

The binary needs the current URL of a specific tab. Reporting a fact is mechanism, so
the extension may answer it, but it makes no decision about it.

Wire protocol -- extend the doc comment in `src/native/messages.rs` with the new pair:

```json
{ "id": "<string>", "type": "tab_url_request", "tabId": <number> }
{ "id": "<string>", "type": "tab_url_response", "result": { "url": "<string or null>" } }
```

Extension (`extension/service-worker.js`): in the native port `onMessage` listener
(lines 31-35), add a branch next to the `tool_request` branch:

```js
} else if (msg && msg.type === "tab_url_request" && msg.id) {
  chrome.tabs.get(msg.tabId).then(
    (tab) => { try { nativePort && nativePort.postMessage({ id: msg.id, type: "tab_url_response", result: { url: tab.url || null } }); } catch { /* port gone */ } },
    () => { try { nativePort && nativePort.postMessage({ id: msg.id, type: "tab_url_response", result: { url: null } }); } catch { /* port gone */ } }
  );
}
```

That is the ENTIRE extension change: no matching, no allow/deny, no URL
interpretation. An unknown or closed tab reports `url: null` and the binary fails
closed. Keep the comment style ASCII and note in a one-line comment that this is
mechanism only (the binary decides).

Binary (`src/browser.rs`): add a public async method on `Browser`, e.g.
`pub async fn tab_url(&self, tab_id: i64) -> Result<Option<String>>`, that sends the
`tab_url_request` frame and awaits the correlated reply exactly like `call` does
(same `pending` map, same `TOOL_TIMEOUT`, same fail-fast `NativeMessaging` error when
no extension is connected). Factor the shared send-and-await logic out of `call` into
a private helper rather than duplicating it. `route_reply` needs NO change (any
non-`tool_error` reply already routes as `Ok(result)`); read `result.url` as
`Option<String>`. Doc-comment the method: the URL feeds policy only; it is never
trusted from tool parameters (shared format 4.3).

### 4. Wiring at the dispatch chokepoint

Integrate in `handle_tools_call` in `src/mcp/server.rs` (or wherever the G06/manifest
prerequisites moved the chokepoint), keeping `src/dispatch.rs` as the seam:

- Reuse `dispatch::PolicyDecision`'s `Deny(Denial)` variant (introduced by G08; if it
  is somehow absent, add it with G08's `Denial` type). Do not fork a second decision
  enum; G15 later adds `ShadowDeny` to this same type. Replace
  the no-op `policy_check` with the real entry point (its signature grows to take the
  active grants, the tool, the rw class, and the `CallDomain`; it may simply delegate
  to `policy::enforcement::check_call`). Keep the module doc truthful: update the
  "no-op" wording to describe the real overlay, preserving the STEP 0 note.
- STEP 0 first: when no manifest is active, skip grant machinery entirely and dispatch
  exactly as today. Do not query the tab URL in this case (all-open must add zero new
  frames and zero new latency). If the sacred-domains task (ADR-0018 step 2) has
  already landed its check at the chokepoint, leave it in place and ahead of grant
  evaluation (it applies even with no manifest); if it has not landed, do not
  implement it here.
- With a manifest active, per call:
  1. Reuse G06's extraction of the `computer` `action` from the arguments; classify
     via G05. Unclassifiable -> deny (section 2).
  2. Build the `CallDomain`:
     - `navigate`: read the `url` argument. `"back"` / `"forward"` -> no pre-check
       (skip straight to dispatch; point 5 covers the landing). Otherwise mirror the
       extension's normalization EXACTLY (service-worker.js lines 467-470): if the
       string does not start with `http://` or `https://` (ASCII case-insensitive)
       and does not match `^(about|chrome|edge|brave):` (case-insensitive), strip a
       leading prefix matching `^[a-z]{1,6}:/+` (case-insensitive) and prepend
       `https://`. Parse the result with G07's parser: unparseable -> dispatch
       without pre- or post-check (the extension refuses invalid URLs without
       navigating; nothing to govern); exactly `about:blank` -> `AboutBlank`;
       non-http(s) -> `NonHttp(scheme)`; else `Host(host)`.
     - The 9 other tab-scoped tools (`computer`, `find`, `form_input`,
       `get_page_text`, `javascript_tool`, `read_console_messages`,
       `read_network_requests`, `read_page`, `resize_window`): read the `tabId`
       argument. Missing or non-integer `tabId`, extension not connected, query
       error, or `url: null` -> `Unknown` (fail closed). Otherwise classify the
       reported URL the same way (`about:blank` -> `AboutBlank`; non-http(s) ->
       `NonHttp`; else `Host`).
     - `tabs_context_mcp`, `tabs_create_mcp`, `update_plan` -> `NoPage`. No tab-URL
       query is sent for these.
  3. Run the decision. `Deny(d)`: do NOT call `browser.call`. Write the audit record
     (section 6) and return the denial result (section 7). `Allow`: dispatch via
     `browser.call` exactly as today (including the existing `read_page` redaction
     pass and the existing `isError` handling for execution failures), then write the
     audit record.
  4. Point 5, `navigate` only (dispatched with a valid parsed target, or
     `"back"`/`"forward"`), on a successful (`Ok`) result: query the tab URL again.
     `about:blank` or a host matching any grant -> pass the navigate result through
     unchanged. Otherwise: best-effort park the tab by sending
     `browser.call("navigate", {"url": "about:blank", "tabId": <tabId>})` (ignore its
     outcome), then replace the result with a denial -- rule `unmatched_domain` for an
     off-grant http(s) host, `scheme/<scheme>` for a non-http(s) landing, `Unknown`
     handling (rule `unmatched_domain`) if the post-query fails -- with empty grant id
     and the FINAL host as the domain. The audit record for this call is a deny with
     the real elapsed `duration_ms` (the navigation ran). A failed (`Err`) dispatch
     gets no post-check.

### 5. What each tool's governing domain is (summary table)

| Tool | Governing domain |
|---|---|
| `navigate` (URL target) | target URL, pre-dispatch; final URL, post-dispatch |
| `navigate` (`"back"`/`"forward"`) | none pre-dispatch; final URL post-dispatch |
| `computer`, `find`, `form_input`, `get_page_text`, `javascript_tool`, `read_console_messages`, `read_network_requests`, `read_page`, `resize_window` | current URL of the `tabId` tab, queried from the extension |
| `tabs_context_mcp`, `tabs_create_mcp`, `update_plan` | none (`NoPage`, union rule) |

### 6. Audit flow

Every call under this task produces exactly one audit record through the G06/audit
machinery, populated per shared format 6.1:

- `decision`: `"allow"` or `"deny"` (never `"shadow_deny"`; that value is G15's).
- `grant_id`: the resolving grant's id for allows and grant-attributed denials
  (`access`, `tool/...` with a resolving grant); JSON `null` when no grant matched
  (`unmatched_domain`, `scheme/...`, fail-closed, `AboutBlank` allows, union-rule
  denials with no candidate).
- `denial_id`: G08's `D-<8 hex>` for denies; `null` for allows.
- `domain`: the parser-normalized host, or `null` (never `(unknown)`).
- `rw`, `tool`, `action`: from G05/G06 as already wired.
- `duration_ms`: `0` for calls denied before dispatch (points 1-4); real wall time for
  allows and for post-navigation denials (point 5).
- `manifest`: the active manifest's `{ name, version, hash }` (from the
  manifest-loading task); present on every record while a manifest is active.

If the audit record type is missing any of these fields, add them per shared format
6.1; if they exist, populate them. Do not change the JSON Lines framing, destination
resolution, or the no-parameters/no-screenshot omission rules (those are the audit
task's).

### 7. Denial responses

A denial returns a normal MCP tool result, not a JSON-RPC error: a success
`JsonRpcResponse` whose result is `{ "content": [{ "type": "text", "text": <message> }] }`
with NO `isError` flag (a denial is a policy outcome, not an execution failure; the
agent must be able to read it and adapt). The message text comes from G08's per-rule
templates verbatim (shared format 7.2): it starts with `Denied (D-xxxxxxxx):`, names
only the denying grant, gives one actionable next step, and never enumerates other
grants, domains, manifest paths, group names, or config values. For `computer` calls
the `<tool>` substitution renders as `computer (<action>)`. If G08 exposes a
message-building function, call it; do not re-author template strings here.

### 8. Tests

Unit tests, inline in `src/policy/enforcement.rs` (pure, no I/O). Build small `Grant`
fixtures in code. At minimum, and by name:

1. `first_matching_grant_wins`: two grants whose patterns both match the host; the
   earlier one resolves (assert via differing `access` outcomes).
2. `unmatched_domain_denies`: host matching no grant -> deny, rule `unmatched_domain`,
   empty grant id.
3. `access_rules`: mutate on a `read` grant -> deny `access` with that grant's id;
   observe on a `write` grant -> deny `access` (write does not imply read); observe on
   `read` -> allow; mutate on `write` -> allow; both on `all` -> allow.
4. `tool_list_rules`: positive `tools` list without the called tool -> deny
   `tool/<name>`; `exclude_tools` containing it -> deny `tool/<name>`; a `computer`
   call is checked as the string `"computer"` regardless of action.
5. `tool_check_precedes_access_check`: a grant that both excludes the tool AND lacks
   the class denies with rule `tool/...`, not `access`.
6. `computer_subactions_split`: on a `read` grant, `computer`+`screenshot` allows and
   `computer`+`left_click` denies `access`; both allow on `all`.
7. `scheme_and_about_blank`: `NonHttp("chrome")`, `NonHttp("file")`,
   `NonHttp("javascript")` deny with rule `scheme/<scheme>`; `AboutBlank` allows with
   no grant id.
8. `unknown_fails_closed`: `Unknown` -> deny `unmatched_domain`, empty grant id.
9. `no_page_union_rule`: read-only manifest: `tabs_context_mcp` (observe) allows with
   the first read grant's id, `tabs_create_mcp` (mutate) denies `access`;
   an `all` grant allows `tabs_create_mcp`; a manifest whose every grant excludes the
   tool denies `tool/<name>` with empty grant id.
10. `unclassifiable_denies`: an unknown tool name, and `computer` with a missing or
    unknown action, deny with rule `tool/<name>` under a manifest.

Integration tests, new file `tests/tool_enforcement.rs`, using the subprocess pattern
of `tests/mcp_protocol.rs` (unique `BROWSER_MCP_ENDPOINT` per spawn; no extension
connected, so a call that PASSES policy reaches dispatch and returns the
`isError: true` / `not connected` result -- that contrast is the test signal). The
test writes a temp restrictive manifest (schema 2, `name`, `version`) with grants:

```json
[
  { "id": "example-full", "domains": ["example.com", "*.example.com"], "access": "all" },
  { "id": "research-read", "domains": ["research.example.org"], "access": "read" }
]
```

plus manifest `config` entries enabling audit to a temp absolute path
(`audit.enabled` true, `audit.destination` `"file"`, `audit.file.path` set), passes it
via the `--manifest` source form the manifest-loading task supports
(`file:///...` per shared format 1.3), and asserts:

1. Permitted call passes policy: `navigate` to `https://example.com/` returns the
   `not connected` execution error (NOT a `Denied (` text) -- policy allowed it
   through to dispatch.
2. Denied domain: `navigate` to `https://evil.com/` returns a single text item
   starting `Denied (D-` and containing `no grant covers evil.com`; no `isError`.
3. Denied access with grant attribution: `navigate` (mutate) to
   `https://research.example.org/` returns `Denied (D-` text naming grant
   `research-read` and read-only access.
4. Denied scheme: `navigate` to `file:///etc/passwd` returns `Denied (D-` with the
   scheme wording.
5. Fail closed: `read_page` with a `tabId` (no extension, tab URL unknowable) returns
   `Denied (D-`, not the `not connected` error.
6. Union rule end to end: `tabs_create_mcp` is allowed (reaches `not connected`)
   under this manifest; rerun with a read-only-grants manifest and it returns
   `Denied (D-`.
7. Audit shows both: after the run, read the temp audit JSONL; assert one record with
   `decision: "allow"` and `grant_id: "example-full"` for case 1, and one with
   `decision: "deny"`, a `denial_id` matching the `D-...` in the response text of
   case 2, `grant_id` null, and `duration_ms: 0`. If wiring `audit.file.path` through
   manifest config proves impossible because a prerequisite key is missing, fall back
   to `audit.destination` `"stderr"` and capture the child's stderr; do not skip the
   assertion.
8. All-open invariant: spawn with NO manifest and drive
   `initialize` / `tools/list` / a `tools/call`; assert responses byte-equal the
   current behavior (13 tools, fixture identity, `not connected` error result) and
   that NO `Denied (` text appears anywhere. `tests/mcp_protocol.rs` itself must also
   pass unchanged.

Denial-id determinism (ADR-0020): drive the same denied call twice in one session and
across two spawns with the same manifest file; assert the `D-...` id in the response
text is identical every time.

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. G13 does not touch
   tool advertisement at all (`tools_list_result` stays as the prerequisites left it).
2. The extension holds mechanism only. The ONLY extension change is the
   `tab_url_request` handler of section 3, which reports a fact and decides nothing.
   No matching, no classification, no denial text, no manifest awareness in JS.
   Enforcement lives in the binary at the dispatch chokepoint.
3. All-open stays first-class: with no manifest and default config, behavior is
   byte-identical to today. STEP 0 short-circuits before any grant machinery and
   before any tab-URL query (zero added frames, zero added latency). Preserve and
   test it (integration test 8).
4. ASCII only in ALL code and docs, including comments, message strings, and this
   task's test fixtures: no em-dashes, arrows, or curly quotes. Use ` -- ` where the
   codebase uses it.
5. The engine is truthful: a denial says plainly what was blocked, by which grant, and
   what to do next (G08 templates verbatim); an allow whose execution failed reports
   the execution failure, never a denial; nothing is silently dropped. Do not soften,
   embellish, or extend the template wording.
6. No new runtime dependencies. `sha2`, `uuid`, the time source, and the URL parser
   belong to the prerequisite tasks; if any is missing from `Cargo.toml` when you
   start, the prerequisite has not landed -- stop and report rather than adding it
   yourself. Extension stays vanilla JS.
7. Rust 2021 edition; `thiserror` for library error types; doc comments on every
   public item and module; `cargo fmt` clean; `cargo clippy --all-targets --
   -D warnings` clean. Unit tests inline, integration tests in `tests/`.
8. Do NOT copy code from the reference implementation, the official extension, or any
   other project; implement from the behavior described here.

Task-specific:

9. Use the shared format doc's names exactly: rule strings `unmatched_domain`,
   `access`, `tool/<tool_name>`, `scheme/<scheme>` (rule `sacred/<pattern>` belongs to
   the sacred-domains task, not G13); grant access values `read` / `write` / `all`;
   classes `observe` / `mutate`; audit decisions `allow` / `deny`. Never invent
   synonyms.
10. Deny before dispatch: a denied call (points 1-4) must never produce a
    `tool_request` frame. The only post-dispatch denial is the navigate final-URL
    check (point 5), and its only extension traffic is the tab-URL query plus the
    about:blank parking navigate.
11. Fail closed under a manifest: unknowable tab URL, unclassifiable call, or a
    missing `tabId` on a tab-scoped tool is a deny, never a pass-through. (With no
    manifest, none of this machinery runs.)
12. The current tab URL comes ONLY from the extension query (or, for point 5, the
    re-query). Never derive the governing domain from tool parameters, tool result
    text (do not parse `Navigated to ...`), or cached prior calls.
13. Reuse prerequisite types and functions: G05 `classify`/`RwClass`, G07 matcher and
    host normalization, G08 `Denial`/id/templates, the manifest task's
    `Grant`/manifest identity, G06's audit record and action extraction. Duplicate
    none of them.

## Verification

1. From the repo root: `cargo fmt --check`, `cargo clippy --all-targets --
   -D warnings`, and `cargo test` all clean. `tests/tool_schema_fidelity.rs` and
   `tests/mcp_protocol.rs` pass WITHOUT edits. If
   `target/debug/browser-mcp.exe` is locked by a running session, rename it aside
   (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild.
2. Grep the changed files for non-ASCII bytes (e.g.
   `rg -n "[^\x00-\x7F]" src/policy/enforcement.rs extension/service-worker.js`);
   none.
3. Manual, live (binary changes need an MCP client restart; the extension change needs
   a reload at chrome://extensions): with a restrictive manifest active
   (`example-full` / `research-read` fixture above, plus a grant covering a real site
   you can open):
   - Navigate to a granted domain: works exactly as before.
   - A mutate action (`computer` `left_click`) on a read-only domain: the agent
     receives the `Denied (D-...)` text naming the grant; the click does not happen.
   - An observe action (`computer` `screenshot`) on the same read-only domain: works.
   - Click a link BY HAND in the governed tab to an off-grant domain, then have the
     agent call `read_page`: denied (drift caught by the per-call tab check).
   - Navigate to an allowed URL that redirects off-grant (e.g. a shortener): the tab
     ends parked on `about:blank` and the agent receives the denial.
   - Check the audit file: one record per call; allows carry the grant id; denies
     carry `denial_id` and `grant_id`; the same denial repeated shows the same
     `D-...` id.
4. Remove the manifest, restart the client: everything behaves exactly as before this
   task (all-open), and no tab-URL query frames appear (verify with `--debug`
   observability if enabled).

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Tool advertisement filtering (G14). `tools/list` output is untouched by this task;
  per-call enforcement must be correct even for tools G14 would hide.
- Shadow mode (G15). G13 consults neither `governance.mode` nor any manifest/grant
  `mode` field: every fully evaluated deny BLOCKS. Do not add a `ShadowDeny` variant,
  a `shadow_deny` decision value, mode parsing, or status badges; G15 wraps this
  task's verdict later.
- Identity verification beyond the deployment-channel assumption (SPEC section 8). The
  manifest `identity` block is informational metadata stamped into audit records by
  the manifest/audit tasks; G13 performs no OS-identity lookup, no group resolution,
  and never uses identity as an authorization input.
- Sacred domains (`content.security.sacred_domains`, rule `sacred/<pattern>`,
  ADR-0018 step 2). A separate task owns it; G13 neither implements nor removes it.
- Manifest parsing, validation, source selection (org policy file vs `--manifest`),
  content hashing, config layering, presets, or registry keys (prerequisite tasks).
- Changes to the G07 matcher or its pattern grammar, and changes to G08's denial id
  scheme or template wording. Consume both as landed.
- `policy explain`, `policy simulate`, JSON Schema generation, doctor/status surfaces,
  and the native-messaging settings protocol (other stage-2 tasks).
- Per-frame committed-origin enforcement. `src/origin.rs` is a doc-only stub; stage-2
  per-call checks govern the top-level tab URL via `chrome.tabs.get`. Do not build
  CDP `Page.frameNavigated` tracking here.
- Any approval/hold flow on `update_plan` (it stays a pass-through observe tool), any
  interactive prompt, and any new CLI surface.
- Any change to the IPC transport, the native-host zombie-fix `std::process::exit(0)`,
  the installer, the screenshot pipeline, or the redaction overlay.
- New dependencies in `Cargo.toml`, including dev-dependencies.
