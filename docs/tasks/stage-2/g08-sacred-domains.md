# G08: Sacred domains: the never-touch list with structured denials

## Goal

Enforce the `content.security.sacred_domains` configuration key (ADR-0018 step 2) in the
binary at the dispatch chokepoint. A `navigate` whose target host matches a sacred
pattern, or ANY tool call whose current-tab host matches one (the host comes from the tab
context the extension reports, never from tool parameters), is denied before the tool
runs. This task introduces `PolicyDecision::Deny`, the reusable `Denial` type with the
stable denial id of shared-format section 7.1, and the sacred denial message of section
7.2: plain, actionable, naming the never-touch rule, and never leaking the rest of the
list. Every denial is written to the audit log as `decision: "deny"` with its
`denial_id`. With an empty list (the default in every preset) behavior is byte-identical
to today: no extra extension traffic, no denials, all-open stays first-class.

Sacred domains are ALWAYS enforced: regardless of manifest presence, regardless of any
enforcement mode. A user-authored protection is never shadow-only (shared-format 3.4).

## Depends on

- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Every
  field name, file location, rule string, and message template in this task comes from it
  verbatim. Read it before writing any code. Load-bearing sections: 1.1 (user config
  file), 2 (layer model), 3.4 (the `content.security.sacred_domains` key and its
  always-enforced semantics), 4.5 (all-open; the registry still resolves without a
  manifest), 5 (domain pattern language and matching semantics), 5.3 (bypass test
  classes), 6.1 (audit record fields), 7.1 (denial id), 7.2 (sacred denial template).
- G06 (audit flight recorder): writes exactly one JSON Lines record per tool call with
  `decision`, `denial_id`, `domain`, `duration_ms`, `rw`. G08 emits the `deny` decision
  on that record. If no audit writer exists in the tree when you start, stop and report;
  do not improvise one.
- G07 (domain matcher): the section-5 pattern validator, the host matcher, and the
  parser-normalized host extraction (WHATWG-compliant URL parser). G08 reuses all three.
  Locate the module under `src/policy/` (grep for the pattern-matching function). If no
  matcher exists in the tree when you start, stop and report; do not write one here.
- The stage-2 configuration-registry / layered-configuration task: the typed key registry
  (`KeyValue` / constraints per shared-format 3.3), the layered resolver, and user config
  file loading (shared-format 1.1). G08 consumes the resolved value of
  `content.security.sacred_domains`. If `src/policy/mod.rs` still holds the boolean-only
  seed `KeyDef` (a single `minimal_default: bool` field) when you start, stop and report
  that this prerequisite has not landed; do not grow the registry yourself.
- All release-1 tasks in `docs/tasks/release-1/` are assumed landed.

Because these prerequisites reshape `src/dispatch.rs`, `src/policy/`, and
`src/mcp/server.rs` before G08 runs, the "Current behavior" section below records the
tree as it stood at authoring time. Do NOT trust it as the state you will edit. Re-read
every file named below before changing it, and integrate against the code the
prerequisites actually produced.

## Project context

Browser MCP is governed browser automation. A single Rust binary is both the MCP server
(JSON-RPC 2.0 over stdio, hand-rolled, tokio) and the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands. Architecture:

    MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser

The two binary roles run as separate OS processes bridged by tokio-native named-pipe
(Windows) / Unix-domain-socket (elsewhere) IPC.

Stage 1 (docs/tasks/release-1/) hardened the engine. Stage 2 is the governance layer per
ADR-0013 (separable overlay; all-open stays first-class), ADR-0018 (observe-then-enforce
sequencing), ADR-0019 (layered configuration, typed key registry), and ADR-0020 (org
policy experience: stable denial ids, structured denials). G08 is ADR-0018 step 2: the
first real enforcement anywhere in the product, deliberately small (one user-authored
deny-list) so the enforcement seam, the denial format, and the audit deny path are proven
before the full manifest engine (grants) lands.

Authority order: where `docs/SPEC.md` and the ADRs disagree, the ADRs win. The shared
format doc is the reconciled single source for formats and names; sacred domains are its
section 3.4 key, section 7.1 rule `sacred/<pattern>`, and section 7.2 template. SPEC
section 5 does not yet describe sacred domains (tracked as SPEC update item 12); do not
consult the SPEC for this feature's shape.

Trust boundary: the extension holds mechanism only. It reports tab context truthfully and
executes commands; it makes no policy decision. The sacred check, the denial, and the
audit record all live in the binary at the dispatch chokepoint. G08 changes NO extension
file.

## Current behavior

All facts verified against the working tree at authoring time.

`src/dispatch.rs` (31 lines) is the documented chokepoint seam, still a no-op:

- `PolicyDecision` (lines 13-17) has a single variant, `Allow`.
- `pub fn policy_check(_tool: &str) -> PolicyDecision` (lines 23-25) always returns
  `PolicyDecision::Allow`.
- `pub fn audit(_tool: &str) {}` (line 30) does nothing. (G06 will have replaced this.)

`src/mcp/server.rs` (156 lines) is the dispatch caller:

- `run` builds the governance `Config` once per session (line 28: `let config =
  Config::default();`) and threads it through `handle_line` into `handle_tools_call`.
- `handle_tools_call` (lines 116-155) extracts `name` and `arguments`, calls the no-op
  seams (lines 132-133), then `browser.call(name, &args)` (line 135), applies `read_page`
  redaction (lines 140-144), and returns `JsonRpcResponse::success`. Tool execution
  failures become an MCP tool error result with `isError: true` (lines 147-153) via the
  `text_content` helper from `src/mcp/types.rs`.

`src/policy/mod.rs` (104 lines) holds the seed registry: `KeyDef` (lines 25-33, boolean
`minimal_default`), one registered key (`content.security.secrets.redact`), and `Config`
(lines 51-70, `Copy`, one bool field). There is NO `content.security.sacred_domains` key
yet; the configuration-registry prerequisite adds the typed registry it slots into. Note
`Config` is passed by value today because it is `Copy`; if the registry task made it
non-`Copy` (a string list forces that), pass it by reference and adjust call sites.

`src/browser.rs` is the mcp-server's handle to the extension. `Browser::call` (lines
72-115) sends `{ "id", "type": "tool_request", "tool", "args" }` and awaits the
correlated `tool_response` / `tool_error` (60 second timeout, line 25). Its inline tests
(lines 197-224) show the fake-extension pattern G08's tests reuse: `tokio::io::duplex`,
`Browser::attach` on one end, a task on the other end reading framed requests with
`host::read_message` and writing framed replies.

`src/origin.rs` is an 11-line doc-only placeholder (no code). Committed-origin tracking
is NOT available as a domain source; the tab context query below is G08's source.

`extension/service-worker.js` (untouched by this task) provides everything needed:

- `tabs_context_mcp` handler (lines 447-451): calls `ensureGroup(a.createIfEmpty)`; when
  no group exists it returns the plain text `No Browser MCP tab group. Call with
  createIfEmpty: true.`; otherwise it returns `tabContext(...)` (lines 191-194), a text
  content item whose text is pretty-printed JSON:
  `{ "mcpGroupId": <number>, "tabs": [{ "tabId": <number>, "title": "...", "url": "..." }] }`.
- `navigate` handler (lines 460-477) normalizes its `url` argument (lines 462-472):
  `"back"` / `"forward"` are history navigation; a URL matching `/^https?:\/\//i` is used
  as-is; a URL matching `/^(about|chrome|edge|brave):/i` is used as-is; anything else has
  one leading `[a-z]{1,6}:\/+` scheme prefix stripped (case-insensitive, first occurrence
  only) and `https://` prepended. If `new URL(url)` then throws, the extension returns
  `Invalid URL: ...` without navigating.
- Every tabId-bearing handler refuses tabs outside the Browser MCP tab group
  (`Tab <id> is not in the group.`), which is why an unknown tab host never needs to be
  denied by policy: the extension will not act on such a tab anyway.

`src/mcp/schemas/tools.json` (SACRED, read-only): exactly 10 tools require `tabId` --
`navigate` (line 47), `computer` (line 123), `find` (line 142), `form_input` (line 165),
`get_page_text` (line 184), `javascript_tool` (line 207), `read_console_messages` (line
238), `read_network_requests` (line 265), `read_page` (line 297), `resize_window` (line
320). Three tools carry no `tabId`: `tabs_context_mcp`, `tabs_create_mcp`, `update_plan`.

`tests/mcp_protocol.rs` spawns the real binary over stdio (`drive` helper, lines 16-46).
Its first test (lines 49-91) asserts that `tools/call` for `navigate` with empty
arguments and no extension connected returns an `isError` result containing
"not connected". With the default (empty) sacred list this must keep passing unchanged.

`Cargo.toml` dependencies at authoring time: `tokio`, `serde`, `serde_json` (with
`preserve_order`), `clap`, `tracing`, `tracing-subscriber`, `thiserror`, `anyhow`,
`dirs` (plus Windows-only `winreg`, `windows-sys`). No `sha2` yet; G07 will have added a
URL parser dependency for host extraction -- reuse whatever it chose.

## Required behavior

Six parts: the registry key, the `Denial` type with its stable id, the sacred matching
core, enforcement at the dispatch chokepoint, the audit deny record, and tests. Re-read
the prerequisite code first; reuse its types and helpers, add only what is missing.

### 1. The registry key

Ensure `content.security.sacred_domains` is registered exactly per shared-format 3.4:

- Type: string list. Constraint: every element must be a valid section-5.1 domain
  pattern (an exact host like `example.com`, or a single leading `*.` wildcard like
  `*.example.com`); validate each element with the G07 pattern validator at config load,
  rejecting invalid entries with a warning that names the key (the registry task's
  standard invalid-value handling).
- Default in ALL three presets (`fully_open`, `safe`, `restricted`) and in the built-in
  Minimal: the empty list.
- Description (exact string):
  `Domains the agent must never touch: any tool call on a tab showing one of these domains, and any navigation targeting one, is denied. Always enforced.`

If the configuration-registry task already registered this key, verify the above and move
on. If the typed registry exists but the key is absent, add it following the registry's
established conventions. Expose the resolved value on `Config` as
`pub fn sacred_domains(&self) -> &[String]` (or reuse the accessor the registry task
already provides). Duplicates in the list are rejected per shared-format 3.2; order is
preserved (it determines which pattern a denial id derives from, see part 3). The key
needs no special lock handling: org-lockability comes free from the layer model.

### 2. The `Denial` type and the stable denial id

Create `src/policy/denial.rs` and declare `pub mod denial;` in `src/policy/mod.rs`.
Module doc comment: this is the structured denial of shared-format section 7, introduced
with the sacred-domains rule and reused by the manifest engine (grants), shadow mode, and
audit; the id scheme makes every denial traceable to the exact policy version.

```rust
/// A structured policy denial (shared-format section 7). Carried by
/// `PolicyDecision::Deny`; its `denial_id` goes into the audit record and its
/// `message` is returned to the agent as a normal text tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    /// Rule string per shared-format 7.1, e.g. "sacred/*.mybank.com".
    pub rule: String,
    /// The resolving grant's id. Always `None` until the manifest engine lands.
    pub grant_id: Option<String>,
    /// Stable denial id: "D-" + 8 lowercase hex chars (shared-format 7.1).
    pub denial_id: String,
    /// Parser-normalized host named in the message.
    pub domain: String,
    /// Full agent-facing message (shared-format 7.2 template).
    pub message: String,
}
```

The id function, public because the manifest engine reuses it verbatim:

```rust
/// denial_id = "D-" + first 8 lowercase hex chars of
/// SHA-256(manifest_hash + "\n" + grant_id + "\n" + rule), all UTF-8, exactly one
/// LF between components (shared-format 7.1). `manifest_hash` is the empty string
/// when no manifest is active; `grant_id` is the empty string when no grant matched.
pub fn denial_id(manifest_hash: &str, grant_id: &str, rule: &str) -> String
```

Use the `sha2` crate (`Sha256`). Check `Cargo.toml` first: if an earlier stage-2 task
already added `sha2`, reuse it; if not, add `sha2 = "0.10"`. This is the one new
dependency this task is allowed (explicitly authorized by the shared format doc's crate
note); add nothing else.

The sacred constructor:

```rust
/// Build the denial for a host matching a sacred-domains pattern. No manifest and no
/// grant participate today, so the id preimage uses empty manifest_hash and grant_id.
pub fn sacred(host: &str, pattern: &str) -> Denial
```

- `rule` = `"sacred/"` + the matching pattern (shared-format 7.1 table).
- `denial_id` = `denial_id("", "", &rule)`.
- `message` (shared-format 7.2, exact template; `<denial_id>` already contains `D-`):
  `Denied (<denial_id>): <host> is on the user's never-touch list. Do not retry or work around this; choose a different approach or ask the user directly.`
- The message names ONLY the host and the id. It never contains the matching pattern,
  any other list entry, any file path, or any config key name. The pattern exists only
  inside the hashed rule preimage and is not shown to the agent, and it does not appear
  in the audit record either (shared-format 6.1 has no rule field). The sacred template
  takes no tool name, so the `computer (<action>)` rendering rule of 7.2 does not apply
  here.

Concrete pinned values (verified by test, part 6): with empty manifest hash and grant id,
`rule = "sacred/mybank.com"` yields `denial_id = "D-171052e3"` and
`rule = "sacred/*.mybank.com"` yields `denial_id = "D-af6633ec"`.

### 3. The sacred matching core

Create `src/policy/sacred.rs` and declare `pub mod sacred;` in `src/policy/mod.rs`.
Module doc comment: the never-touch list check (ADR-0018 step 2, shared-format 3.4);
pure functions, no I/O; delegates pattern matching to the G07 domain matcher; always
enforced, in every mode, with or without a manifest.

Two pure functions:

```rust
/// First sacred pattern, in authored list order, matching `host`; `None` when none
/// match. List order matters: it fixes which pattern the denial id derives from.
pub fn first_match<'a>(host: &str, patterns: &'a [String]) -> Option<&'a str>
```

Delegates each per-pattern check to the G07 matcher (exact host equals; `*.` wildcard
matches strict subdomains only, never the apex, never IP literals). Do not reimplement
any matching semantics here.

```rust
/// The host the extension will navigate to for a given `navigate` url argument,
/// mirroring extension/service-worker.js lines 460-477 exactly. `None` when there is
/// no http(s) target: "back"/"forward", about:/chrome:/edge:/brave: URLs, or anything
/// the URL parser rejects after normalization.
pub fn navigate_target_host(url_arg: &str) -> Option<String>
```

The mirror rules, in order (they MUST agree with the extension, or a schemeless or
scheme-mangled URL bypasses the list):

1. `url_arg` exactly `"back"` or `"forward"` (case-sensitive): `None`.
2. Starts with `http://` or `https://` (ASCII case-insensitive): parse as-is.
3. Starts with one of `about:`, `chrome:`, `edge:`, `brave:` (ASCII case-insensitive):
   `None` (no domain host; the sacred list governs domains only).
4. Otherwise: strip ONE leading scheme prefix if present -- 1 to 6 ASCII alphabetic
   characters, then `:`, then one or more `/` (the extension's
   `url.replace(/^[a-z]{1,6}:\/+/i, "")`) -- then prepend `https://`, then parse.
5. "Parse" means the same WHATWG-compliant parser and host-extraction G07 pinned
   (lowercased host, one trailing dot stripped, punycode A-label form, port and
   userinfo discarded). Parse failure yields `None`: the extension will refuse the same
   URL with `Invalid URL`, so there is nothing to deny.

Hand-roll the two prefix checks with plain `str`/`char` operations. Do NOT add the
`regex` crate.

### 4. Enforcement at the dispatch chokepoint

Extend `PolicyDecision` in `src/dispatch.rs` (extend, never fork a second decision
type):

```rust
pub enum PolicyDecision {
    Allow,
    /// The call is blocked; the tool must not execute. Introduced with the
    /// sacred-domains rule (ADR-0018 step 2).
    Deny(crate::policy::denial::Denial),
}
```

(`Copy` is lost; the sole current consumer binds the result with `let _decision`, so
adjust call sites as needed.)

Replace the no-op `policy_check` with the real chokepoint check. Signature guidance --
adapt to what G06 left in place, keeping dispatch the single seam:

```rust
/// Per-call policy check. Today this enforces exactly one rule, the user-authored
/// sacred-domains list (always enforced, shared-format 3.4). The manifest engine
/// adds grant evaluation here later; all-open remains a short-circuit by construction.
pub async fn policy_check(
    browser: &Browser,
    config: &Config,
    tool: &str,
    args: &serde_json::Value,
) -> PolicyDecision
```

Evaluation order, exactly:

- STEP A (all-open fast path): if `config.sacred_domains()` is empty, return `Allow`
  immediately. No extension traffic, no parsing, no allocation on this path. This is the
  byte-identical invariant.
- STEP B (current-tab check, ANY tool): if `args` carries a numeric `tabId` (read with
  `Value::as_i64`; missing or non-numeric means skip this step), resolve the tab's
  current host:
  1. Send an internal tab-context query through the existing handle:
     `browser.call("tabs_context_mcp", &json!({ "createIfEmpty": false })).await`.
  2. On success, take `result.content[0].text`, parse it as JSON, and find the entry in
     `tabs` whose `tabId` equals the call's `tabId`. Take its `url` string and extract
     the parser-normalized host with the G07 helper.
  3. If every step succeeds and `first_match(host, list)` returns a pattern, return
     `Deny(Denial::sacred(&host, pattern))`.
  4. If ANY step fails -- the call errors (extension not connected), the text is not
     JSON (the `No Browser MCP tab group` reply), the tab id is absent from the list,
     the url is empty or unparseable -- the host is unknown and this step does NOT
     deny. Rationale (write it in a comment): a deny requires a positive match; tabs
     outside the group are refused by the extension itself, and a failing extension
     fails the real call identically. Do not fabricate protection from a failed lookup.
- STEP C (navigate target check): if `tool == "navigate"`, read the `url` string
  argument; if `navigate_target_host(url)` yields a host and `first_match` yields a
  pattern, return `Deny(Denial::sacred(&target_host, pattern))`. This runs even when
  STEP B could not resolve the tab (the target check is local and needs no extension).
- Otherwise return `Allow`.

Notes:

- STEP B runs for `navigate` too: a tab currently showing a sacred domain may not be
  touched AT ALL, including navigating it away. Never-touch means the user, not the
  agent, moves that tab. STEP B runs before STEP C so a sacred current tab denies with
  the tab's host in the message.
- `tabs_context_mcp`, `tabs_create_mcp`, and `update_plan` carry no `tabId` and no
  navigation target: with a non-empty list they skip both steps and are allowed.
  `update_plan` has a `domains` parameter; it is informational and is NOT checked (a
  plan is not an action). Do not redact or filter `tabs_context_mcp` output.
- The internal `tabs_context_mcp` lookup is machinery, not an MCP tool call: it produces
  NO audit record of its own. Exactly one audit record per `tools/call` (shared-format
  section 6).
- No mode logic anywhere: sacred denials do not consult `governance.mode`, any manifest
  mode, or shadow machinery (G15 adds the mode switch for OTHER rules and carves sacred
  out; G08 keeps it unconditional by construction).

Wire the decision in `handle_tools_call` in `src/mcp/server.rs` (or wherever the
prerequisites moved the seam):

- `Allow`: unchanged path -- execute via `browser.call`, redact `read_page`, return the
  result. The audit record stays as G06 wrote it, except: if G06's record leaves
  `domain` null and STEP B resolved a host for this call, pass that host through so the
  record is truthful. Do not add tab lookups for calls where the sacred machinery did
  not run.
- `Deny(d)`: do NOT call `browser.call` for the tool. Write one audit record via G06's
  writer with `decision: "deny"`, `denial_id: d.denial_id`, `grant_id: null`,
  `domain`: the STEP-B host when resolved, else `null` (shared-format 6.1 defines
  `domain` as the current tab's host at decision time -- for a navigate-target denial
  with an unresolvable tab this is `null` even though the message names the target),
  `duration_ms: 0` (denied before dispatch), and the usual `tool` / `action` / `rw`
  fields G06 already populates. Then return a NORMAL tool result: 
  `JsonRpcResponse::success(id, text_content(d.message))`. No `isError` flag, no
  JSON-RPC error: a denial is a policy outcome the agent should read and adapt to, not
  a transport or tool failure (shared-format 7.2).

### 5. Audit deny record

Covered above; restated as requirements on the record (all shared-format 6.1):

- Exactly one record per denied call: `decision` is the string `"deny"`, `denial_id`
  matches `^D-[0-9a-f]{8}$`, `grant_id` is JSON `null`, `duration_ms` is `0`, `domain`
  is the resolved current-tab host or `null`, `rw` and `action` come from the existing
  G05/G06 classification wiring.
- Repeated denials of the same kind carry the same `denial_id` across calls and across
  binary restarts (the id derives only from the rule string while no manifest exists).

### 6. Tests

Unit tests inline in the module they exercise; dispatch/server tests inline in those
files (the fake-extension pattern from `src/browser.rs` tests lines 197-224 works in any
module: `tokio::io::duplex`, `Browser::attach`, a task answering framed requests).
Required tests, by name and assertion:

In `src/policy/denial.rs`:

1. `denial_id_is_stable_and_pinned`:
   - `denial_id("", "", "sacred/mybank.com") == "D-171052e3"`
   - `denial_id("", "", "sacred/*.mybank.com") == "D-af6633ec"`
   - changing any one component changes the id; the format matches `^D-[0-9a-f]{8}$`.
2. `sacred_message_is_exact_and_leaks_nothing`: `sacred("www.mybank.com",
   "*.mybank.com")` yields the exact message
   `Denied (D-af6633ec): www.mybank.com is on the user's never-touch list. Do not retry or work around this; choose a different approach or ask the user directly.`
   and the message does not contain `*.mybank.com`, `sacred/`, or the substring
   `config`.

In `src/policy/sacred.rs`:

3. `first_match_honors_list_order`: with `["*.mybank.com", "mybank.com"]`, host
   `a.mybank.com` matches `*.mybank.com` and host `mybank.com` matches `mybank.com`;
   with the reversed list the wildcard still wins only for subdomains.
4. `navigate_target_mirrors_the_extension`: a table over
   `("back", None)`, `("forward", None)`,
   `("https://mybank.com/x", Some("mybank.com"))`,
   `("HTTPS://MYBANK.COM", Some("mybank.com"))`,
   `("mybank.com/login", Some("mybank.com"))` (schemeless),
   `("ftp://mybank.com/", Some("mybank.com"))` (scheme-strip mirror),
   `("about:blank", None)`, `("chrome://settings", None)`,
   `("javascript:alert(1)", None)` (unparseable after https:// prepend),
   `("https://mybank.com@evil.com/", Some("evil.com"))` (userinfo: real host wins),
   `("https://evil.com@mybank.com/", Some("mybank.com"))`,
   `("https://mybank.com./", Some("mybank.com"))` (trailing dot).
5. `sacred_bypass_classes` (the shared-format 5.3 classes turned to the deny-list
   direction; list `["mybank.com", "*.mybank.com", "127.0.0.1"]`, matching on the
   parser-normalized host of each URL):
   MUST match (denied): `https://user:pass@mybank.com/` (credential noise around the
   real host), `https://mybank.com./` (trailing dot), `https://sub.a.mybank.com/`
   (wildcard depth), `http://127.0.0.1/` (exact IP pattern), `http://mybank.com:8443/`
   (port ignored).
   MUST NOT match (allowed): `https://mybank.com@evil.com/` (userinfo bypass, real host
   `evil.com`), `https://evilmybank.com/` and `https://mybank.com.evil.com/` (suffix
   stitching), `http://[::1]/` (wildcards never match IP literals), a homoglyph host --
   write the URL in Rust source as `"https://myb\u{0430}nk.com/"` (Cyrillic a via
   escape, keeping the source ASCII) and assert the parser-normalized host starts with
   `xn--` and matches nothing.
   Plus apex-vs-wildcard: with the list `["*.mybank.com"]` alone, `https://mybank.com/`
   does not match and `https://www.mybank.com/` does.

At the chokepoint (inline tests in `src/mcp/server.rs` or `src/dispatch.rs`, whichever
holds the wiring; use the duplex fake extension):

6. `sacred_tab_denies_every_tool_and_never_runs_it`: fake extension answers
   `tabs_context_mcp` with
   `{ "mcpGroupId": 1, "tabs": [{ "tabId": 5, "title": "", "url": "https://www.mybank.com/account" }] }`
   (as the text of a content item) and fails the test if any OTHER `tool_request`
   arrives. Config with sacred `["*.mybank.com"]`. For each of `read_page`
   (`{"tabId":5}`), `computer` (`{"action":"screenshot","tabId":5}`),
   `javascript_tool` (`{"action":"javascript_exec","text":"1","tabId":5}`), and
   `navigate` (`{"url":"https://example.com","tabId":5}` -- navigating AWAY is denied
   too): the decision is `Deny`, `rule == "sacred/*.mybank.com"`,
   `domain == "www.mybank.com"`, and the extension received only `tabs_context_mcp`
   frames.
7. `navigate_target_denied_even_when_tab_is_clean`: fake extension reports tab 5 on
   `https://example.com/`; sacred `["mybank.com"]`; `navigate` with
   `{"url":"mybank.com","tabId":5}` (schemeless) yields `Deny` with
   `denial_id == "D-171052e3"` and a message naming `mybank.com`. The same call with
   `{"url":"https://example.org","tabId":5}` yields `Allow`.
8. `empty_list_is_byte_identical`: with the default config (empty list), a `read_page`
   call reaches the fake extension directly -- the FIRST frame it receives is the
   `read_page` `tool_request`, no `tabs_context_mcp` pre-flight ever -- and the returned
   result is exactly what the fake extension replied. Also assert `policy_check`
   returns `Allow` without touching the browser (e.g. it returns `Allow` even with a
   `Browser` that has no connection).
9. `denied_call_writes_one_deny_record`: using the G06 test harness pattern (temp-file
   audit destination or whatever G06's tests use), a denied call produces exactly one
   record with `decision == "deny"`, `denial_id` matching `^D-[0-9a-f]{8}$`,
   `grant_id == null`, `duration_ms == 0`, and `domain == "www.mybank.com"`; and the
   internal `tabs_context_mcp` lookup produced NO record.

Existing tests must pass unchanged, in particular `tests/tool_schema_fidelity.rs` and
`tests/mcp_protocol.rs` (its navigate-with-no-extension assertion proves the empty-list
default does not intercept).

## Constraints

Hard rules; every one applies:

1. NEVER modify `src/mcp/schemas/tools.json`, tool names, parameters, or description
   strings. `tests/tool_schema_fidelity.rs` must pass unchanged. G08 does not touch tool
   advertisement: all 13 tools stay advertised; sacred domains deny per call, they do
   not hide tools.
2. The extension holds mechanism only: no policy, access, or redaction decisions in
   extension JS. G08 changes NO file under `extension/`. The sacred check, denial
   formatting, and audit all live in the binary at the dispatch chokepoint.
3. All-open stays first-class: with no manifest and default config (empty sacred list),
   behavior is byte-identical to today -- no pre-flight traffic, no denials, identical
   results. Test 8 pins this; STEP A guarantees it by construction.
4. ASCII only in ALL code and docs: no em-dashes, arrows, or curly quotes anywhere,
   including comments, message strings, and this task's diffs. Use ` -- ` where the
   codebase uses it. The one non-ASCII test host is written as a `\u{0430}` escape.
5. The engine is truthful: a denial is reported plainly with its stable id; a failed
   tab-context lookup never denies (no fabricated protection) and never silently
   swallows the real call's own error. Sacred enforcement is unconditional -- never
   downgraded to observation.
6. No new runtime dependencies except `sha2` (and only if an earlier stage-2 task has
   not already added it). No `regex`, no `once_cell`, no second URL parser -- reuse
   G07's. Extension stays vanilla JS (and untouched here).
7. Rust 2021 edition; `thiserror` for library error types (the `Denial` struct is data,
   not an error type; do not impl `std::error::Error` for it); doc comments on every
   public item and module; `cargo fmt` clean; `cargo clippy --all-targets -- -D
   warnings` clean. Unit tests inline; integration tests in `tests/` only if the
   chokepoint tests cannot live inline.
8. Do NOT copy code from the official Anthropic extension, the reference repo, or any
   other project; implement from the behavior described here.

Task-specific:

9. Sacred is ALWAYS enforced: no `governance.mode` consultation, no manifest
   consultation, no shadow path, no way to configure it off other than emptying the
   list. Do not add mode machinery (G15's job).
10. Never leak the list: the denial message names only the one matched host and the
    denial id. The matching pattern appears only in the hashed rule preimage. The audit
    record carries no rule or pattern field.
11. Exactly one audit record per MCP tool call, denied or allowed; the internal
    `tabs_context_mcp` lookup writes none.
12. A deny requires a positive match on a resolved host. Unknown host (lookup failure,
    tab not in group, unparseable URL) never denies.
13. First match in authored list order wins; the denial id derives from that pattern.
14. The navigate-target normalization must mirror `extension/service-worker.js` lines
    460-477 exactly as specified in part 3; any divergence is a bypass.
15. Reuse the G07 matcher and host extraction; reuse the G06 audit writer; extend the
    existing `PolicyDecision` in `src/dispatch.rs`. Do not fork parallel types. If a
    prerequisite is missing from the tree (see Depends on), stop and report instead of
    improvising it.

## Verification

Run from the repository root:

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
   are all clean, including every test named in part 6, `tests/tool_schema_fidelity.rs`
   unchanged, and `tests/mcp_protocol.rs` unchanged.
2. `git diff --stat`: no change under `extension/`, no change to
   `src/mcp/schemas/tools.json`. `Cargo.toml` gains at most `sha2`.
3. Grep the new files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy/sacred.rs src/policy/denial.rs`); there must be
   none (the Cyrillic test host is an escape sequence).
4. Rebuild the binary. If `target/debug/browser-mcp.exe` is locked by a running
   session, rename it aside (`mv target/debug/browser-mcp.exe
   target/debug/browser-mcp.exe.old-1`) and rebuild. Binary changes need an MCP client
   restart; no extension reload is needed (nothing changed there).
5. Live check with Claude Code and Chrome connected. Edit the user config file
   (Windows: `%APPDATA%\browser-mcp\config.json`; see shared-format 1.1) to
   `{ "config": { "content.security.sacred_domains": ["example.com", "*.example.com"] } }`,
   restart the MCP client, then:
   - Ask the agent to navigate a tab to `example.com`: the tool result starts with
     `Denied (D-` and names `example.com`; the browser did not navigate.
   - Manually navigate a group tab to `https://example.com/`, then ask the agent to
     read or screenshot that tab: denied, message names the never-touch list; asking it
     to navigate that tab elsewhere is also denied.
   - Ask the agent to navigate to `example.org`: works normally.
   - If `audit.enabled` resolves true, the audit JSONL shows one `"decision":"deny"`
     record per denial with a stable `denial_id` (identical across repeats) and
     `"grant_id":null`.
6. Remove the key (or set it to `[]`), restart the MCP client, and confirm everything
   behaves exactly as before this task: no denials, no `tabs_context_mcp` pre-flight
   chatter in debug logs.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- Grants and the manifest engine (G13): no manifest parsing, no grant resolution, no
  `unmatched_domain` / `access` / `tool` / `scheme` rules, no denial templates other
  than the sacred one, no manifest content hash (pass `""` to `denial_id`), no
  post-navigation final-URL check, and no parking on `about:blank` after a denial. A
  redirect that lands a tab on a sacred domain is caught by STEP B on the NEXT call;
  that is this task's whole redirect story.
- Shadow mode (G15): no `governance.mode` consultation, no `shadow_deny` decision, no
  observe/enforce switch, no status badges. Sacred is unconditional here and stays
  unconditional after G15.
- Kill switch (G10/G11): no session halt, no global disable surface.
- Extension-side changes of ANY kind: no new message types, no URL reporting fields, no
  tab filtering, no popup or options text. The existing `tabs_context_mcp` reply is the
  only domain source.
- Tool advertisement filtering (G14): all 13 tools stay listed.
- Redacting or filtering sacred-tab URLs out of `tabs_context_mcp` / `tabs_create_mcp`
  results, and checking `update_plan`'s informational `domains` parameter. Observation
  of the tab list is not touch.
- The `scheme` denial rule (shared-format 7.1): non-http(s) navigate targets simply
  yield no host and are not the sacred list's business; denying them belongs to the
  manifest engine.
- CLI surfaces (`config set`/`config list`), `policy explain`, `policy simulate`, JSON
  Schema generation, doctor sections, and the native-messaging settings protocol (other
  stage-2 tasks own these).
- Editing `docs/SPEC.md` (the sacred-domains SPEC amendment is shared-format "SPEC
  updates needed" item 12, a separate docs task) or the shared format doc itself.
- Caching tab hosts across calls, watching tab events, or any domain source other than
  the per-call `tabs_context_mcp` lookup. Stale state is a bypass; ask fresh each time.
- Any new dependency beyond `sha2`, including dev-dependencies.
