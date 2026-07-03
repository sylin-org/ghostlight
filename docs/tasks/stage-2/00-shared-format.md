# Stage 2 Shared Format Reference

Status: authoritative for all stage-2 implementation prompts.
Sources reconciled: ADR-0013, ADR-0017, ADR-0018, ADR-0019, ADR-0020, SPEC sections 4-8,
`src/policy/mod.rs`, `src/policy/redact.rs`, `src/dispatch.rs`, `src/mcp/schemas/tools.json`.
Where SPEC and ADRs disagree, the ADRs win. Every divergence is listed in the final
"SPEC updates needed" section so the SPEC can be amended.

Conventions used throughout:

- "binary" means the `browser-mcp` executable (both roles: mcp-server and native-host).
- All JSON on disk and on the wire is UTF-8. JSON examples show pretty printing for
  readability; wire and audit formats state their own whitespace rules.
- "host" means the hostname component of a URL after parser normalization (section 5).

Crate note: stage 2 requires adding `sha2` (manifest hash, denial ids), `uuid` (v4 event
ids), and an RFC 3339 timestamp source (`chrono` or `time`) to `Cargo.toml`. None of these
are present today. `serde_json` is already built with `preserve_order`, which the canonical
hash (section 4.2) depends on.

---

## 1. File locations per platform

Directory name is `browser-mcp` on every platform, matching the installer's existing use of
`dirs::config_dir().join("browser-mcp")`.

### 1.1. User config file

Holds the user layer of the configuration registry (section 2) and the user's preset
choice. Writable by the user. Created on first `config set` or preset selection; absence is
normal and means "no user layer".

| Platform | Path |
|---|---|
| Windows | `%APPDATA%\browser-mcp\config.json` |
| macOS | `~/Library/Application Support/browser-mcp/config.json` |
| Linux | `~/.config/browser-mcp/config.json` |

Format:

```json
{
  "preset": "safe",
  "config": {
    "content.security.sacred_domains": ["mybank.com", "*.mybank.com"],
    "audit.destination": "file"
  }
}
```

- `preset`: optional. One of `"fully_open"`, `"safe"`, `"restricted"`. Selects the
  preset-default layer (section 2). When absent, the built-in Minimal defaults apply at
  layer 5.
- `config`: optional object. Flat map of dotted key name to JSON value. Each entry is a
  user-layer value. Unknown keys and type-invalid values are rejected at load with a
  warning naming the key; the rest of the file still loads.

### 1.2. Org policy file

The organization manifest (section 4). Lives at an admin-writable-only path and is
delivered by the org's deployment channel (GPO, Intune, Jamf), per ADR-0019. File ACLs
plus the deployment channel are a usage-surface guard, not a cryptographic boundary;
manifest signing stays excluded (SPEC section 10).

| Platform | Path |
|---|---|
| Windows | `%ProgramData%\browser-mcp\policy.json` |
| macOS | `/Library/Application Support/browser-mcp/policy.json` |
| Linux | `/etc/browser-mcp/policy.json` |

The binary loads this file automatically at startup when it exists. No flag can bypass it.

### 1.3. User-supplied manifest

`--manifest file:///path` or `BROWSER_MCP_MANIFEST` (also `env://VAR` holding inline JSON)
selects a user-level manifest, e.g. an imported shared manifest (ADR-0020). Selection rule
for the active grants manifest:

1. If the org policy file exists, it is the active manifest. A user-supplied manifest is
   ignored for grants, with a startup warning on stderr and a note in the first audit
   record of the session.
2. Else, the user-supplied manifest (if any) is active.
3. Else, no manifest: all-open (section 4.5).

Config entries inside a user-supplied manifest apply at the user layer (never locked);
`"level": "mandatory"` in a user-supplied manifest is downgraded to the user layer with a
warning. Only the org policy file can populate the org-mandatory and org-recommended
layers.

NOTE (ADR-0025, docs/adr/0025-manifest-hot-reload.md): as of stage 4 the active manifest
hot-reloads. The org policy path and a file:// user source are watched; the selection
rule above is re-evaluated on every change (including file creation and deletion); an
invalid edit keeps the last-good manifest (fail closed). The startup-fixed description
below this point is retained as history for the stage-2 implementation record.

### 1.4. Default audit file path

Used when `audit.file.path` resolves to the empty string (section 3.4):

| Platform | Path |
|---|---|
| Windows | `%LOCALAPPDATA%\browser-mcp\audit.jsonl` |
| macOS | `~/Library/Application Support/browser-mcp/audit.jsonl` |
| Linux | `~/.local/share/browser-mcp/audit.jsonl` |

---

## 2. Layer model

Per ADR-0019: one typed key registry, one precedence chain. Effective value for a key is
resolved top-down; the first layer that defines the key wins.

| Precedence | Layer | Written by | Where |
|---|---|---|---|
| 1 (highest) | org-mandatory | org policy file entries with `"level": "mandatory"` | section 1.2 |
| 2 | user | user config file `config` map (and user-supplied manifest config entries) | section 1.1 |
| 3 | org-recommended | org policy file entries with `"level": "recommended"` | section 1.2 |
| 4 | preset default | the preset named in the user config file | registry table, section 3.4 |
| 5 (lowest) | built-in Minimal | compiled-in registry defaults | `src/policy/mod.rs` |

Notes:

- The built-in Minimal defaults equal the "Safe" preset ("Safe is today's Minimal",
  ADR-0019). Layer 5 always defines every key, so resolution never fails.
- Presets are UX, not machinery: choosing a preset sets layer 4 only. Individual user
  edits (layer 2) sit above the preset, so a preset is a starting point the user can then
  edit key by key.
- Org entries declare their level per entry via the `level` field on each config entry in
  the org policy file (section 4.3): `"mandatory"` locks the key, `"recommended"` provides
  an overridable default below the user layer.

### 2.1. Resolved-value triple

Every resolved key carries three facts, surfaced identically by `config list`, the
extension options page, and audit tooling:

```json
{ "value": <JSON value>, "source": "org_mandatory", "locked": true }
```

- `value`: the effective JSON value (typed per section 3.2).
- `source`: enum `"org_mandatory" | "user" | "org_recommended" | "preset" | "builtin"`.
- `locked`: boolean. `true` if and only if `source` is `"org_mandatory"`. Writes to a
  locked key are rejected everywhere (CLI and native-messaging surface, section 9), and
  locked fields render read-only with a "managed by your organization" badge.

---

## 3. Configuration key registry

The registry in `src/policy/mod.rs` is the single source of truth for every configurable
behavior: names, types, constraints, descriptions, per-preset defaults. It drives the CLI,
the extension UI, the generated JSON Schema, and the docs (ADR-0019, ADR-0020).

### 3.1. Key naming convention

- Dotted lowercase: segments separated by `.`, each segment matching `[a-z0-9_]+`.
- Hierarchy is `area.topic[.subtopic].name`, e.g. `content.security.secrets.redact`.
- Key names are a public, stable API surface. Renames require deprecation handling
  (old name accepted with a warning for at least one release).

### 3.2. Value types and JSON representation

| Type | JSON representation | Validation |
|---|---|---|
| bool | `true` / `false` | must be a JSON boolean |
| uint | JSON number | integer, no sign, no fraction, no exponent; `min <= v <= max` (both bounds declared per key) |
| enum | JSON string | exactly one of the key's declared variants, case-sensitive |
| string | JSON string | any UTF-8 string unless the key declares a stricter rule |
| string list | JSON array of strings | duplicates rejected; order preserved; per-key element rule may apply |

Any other JSON shape (null, object, mixed array) is invalid for every key.

### 3.3. `KeyDef` growth

`KeyDef.minimal_default: bool` grows to a typed value plus constraints. Shape (guidance,
not a literal diff):

```rust
pub enum KeyValue {
    Bool(bool),
    Uint(u64),
    Enum(&'static str),
    Str(&'static str),
    StrList(&'static [&'static str]),
}

pub enum KeyConstraint {
    None,
    UintRange { min: u64, max: u64 },
    EnumVariants(&'static [&'static str]),
    DomainPatternList, // each element must be a valid section-5 pattern
}
```

Each `KeyDef` carries one default per preset (`fully_open`, `safe`, `restricted`); the
built-in Minimal equals `safe`. The existing test that pins `Config::minimal()` to the
registry defaults extends to every key.

### 3.4. Initial stage-2 key set

| Key | Type | Constraints | fully_open | safe (= Minimal) | restricted |
|---|---|---|---|---|---|
| `engine.connection.first_call_wait_ms` | uint | min 0, max 60000 | 5000 | 5000 | 5000 |
| `content.security.secrets.redact` | bool | - | false | true | true |
| `content.security.sacred_domains` | string list | each element a valid domain pattern (section 5.1) | `[]` | `[]` | `[]` |
| `audit.enabled` | bool | - | false | true | true |
| `audit.destination` | enum | `file`, `stderr` | `file` | `file` | `file` |
| `audit.file.path` | string | `""` means platform default (section 1.4); otherwise an absolute path | `""` | `""` | `""` |
| `governance.mode` | enum | `observe`, `enforce` | `observe` | `enforce` | `enforce` |

Semantics:

- `engine.connection.first_call_wait_ms`: bound on the first-call wait for the extension
  handshake (ADR-0017; first non-boolean key per ADR-0019).
- `content.security.secrets.redact`: existing key; governs `src/policy/redact.rs`
  behavior in `read_page` output. Unchanged.
- `content.security.sacred_domains`: user-authored never-touch list (ADR-0018 step 2).
  Matching uses the section-5 pattern language. Sacred domains are ALWAYS enforced,
  regardless of `governance.mode` and regardless of manifest presence: a user-authored
  protection is never shadow-only.
- `audit.enabled` / `audit.destination` / `audit.file.path`: the flight recorder
  (ADR-0018 step 1). These are the first keys an organization will want to lock
  (ADR-0019 follow-up), so they ship in the registry, not in a manifest-only `audit`
  block. `syslog`, `http`, and `none` destinations are deferred beyond stage 2.
- `governance.mode`: the manifest-level default enforcement mode. A manifest may set its
  own `mode`, which overrides this key for that manifest; a grant may override the
  manifest (section 4.3). Precedence for the effective mode of a decision:
  per-grant `mode` > manifest `mode` > resolved `governance.mode`.
- The `restricted` preset equals `safe` for every stage-2 key; it diverges once
  grant-adjacent keys exist. It is registered now so the preset name is stable.

---

## 4. Manifest format

JSON document. Delivered as the org policy file (section 1.2) or user-supplied
(section 1.3).

### 4.1. Top-level shape

```json
{
  "schema": 2,
  "name": "acme-clinical-pilot",
  "version": "2026.07.1",
  "mode": "observe",
  "identity": {
    "resolved_by": "managed_config",
    "principal": "ACME\\jdoe",
    "groups": ["EA-ServiceNow-RW"],
    "resolved_at": "2026-07-01T14:30:00Z"
  },
  "grants": [
    {
      "id": "servicenow-full",
      "domains": ["servicenow.acme.org", "*.service-now.com"],
      "access": "all",
      "tools": null,
      "description": "Full automation access to ServiceNow"
    },
    {
      "id": "ehr-restricted",
      "domains": ["epic.acme.org"],
      "access": "all",
      "exclude_tools": ["javascript_tool"],
      "description": "EHR automation without arbitrary JS execution",
      "mode": "enforce"
    },
    {
      "id": "research-external",
      "domains": ["*.ieee.org", "scholar.google.com"],
      "access": "read",
      "description": "Read-only research access"
    }
  ],
  "config": [
    { "key": "audit.enabled", "value": true, "level": "mandatory" },
    { "key": "audit.destination", "value": "file", "level": "mandatory" },
    { "key": "content.security.secrets.redact", "value": true, "level": "recommended" }
  ]
}
```

Field rules:

SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the schema
version is 3; schema 2 never shipped and is rejected. Additionally, per ADR-0023
(docs/adr/0023-one-loader-for-the-policy-file.md), duplicate config keys in one manifest
are a validation error. The text below is retained as history.

- `schema`: integer, required. Stage 2 defines schema `2`. The binary rejects unknown
  schema versions with a clear error.
- `name`: string, required. Manifest identity (ADR-0020 commitment 5). Stamped into every
  audit record.
- `version`: string, required. Free-form version label chosen by the author.
- `mode`: optional enum `"observe" | "enforce"`. Manifest-level default enforcement mode.
  When absent, the resolved `governance.mode` config value applies.
- `identity`: optional object, retained from SPEC 4.2 with the same fields
  (`resolved_by`, `principal`, `groups`, `resolved_at`). Informational metadata about how
  the manifest was resolved; included in audit records; never an input to authorization
  (the manifest IS the authorization decision; identity binding is the deployment
  channel, SPEC 8.1).
- `grants`: array, required (may be empty). Section 4.3.
- `config`: optional array of config entries. Section 4.4.

The SPEC 4.1 `defaults` and `audit` blocks are removed: everything in them is either a
registry key (audit settings, and later the screenshot/timeout tunables) or superseded
(`unlisted_domains`, see section 4.5). See "SPEC updates needed".

### 4.2. Content hash

The manifest content hash makes every logged decision attributable to the exact policy
version that made it (ADR-0020). It is COMPUTED by the binary, never stored in the
manifest (storing it would change the content).

Exact definition:

1. Parse the manifest source (file bytes or `env://` variable value) as JSON. A UTF-8 BOM,
   if present, is stripped before parsing.
2. Re-serialize the parsed value with `serde_json` in compact form (no whitespace),
   preserving object key order as authored (the `preserve_order` feature, already
   enabled in `Cargo.toml`).
3. The canonical bytes are the UTF-8 bytes of that compact serialization.
4. `hash` = SHA-256 over the canonical bytes, rendered as 64 lowercase hex characters.

This makes the hash insensitive to whitespace, line endings, and BOM, and sensitive to
content and key order. The same manifest shipped with CRLF or LF hashes identically.

### 4.3. Grants

SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the grant fields `domains`, `access`, `tools`, and `exclude_tools` are replaced in manifest schema 3 by `hosts` (allow/deny polarity, ADR Decision 4) and `allowed` (capability sets, ADR Decision 3). The text below is retained as history for the stage-2 implementation record.

Each grant object:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | Stable human-readable identifier. Used in audit records and denial messages. Unique within the manifest (duplicate ids are a validation error). |
| `domains` | array of strings | yes | Domain patterns, section 5. At least one element. |
| `access` | enum `"read" \| "write" \| "all"` | yes | `read` authorizes observe-class calls; `write` authorizes mutate-class calls; `all` authorizes both. `write` does NOT imply `read` (most manifests should use `read` or `all`; bare `write` exists for completeness and is validated with a lint warning by `policy explain`). |
| `tools` | array of tool names or `null` | no (default `null`) | Positive list: only these tools, further limited by `access`. `null` means all tools the access class allows. Mutually exclusive with `exclude_tools`. |
| `exclude_tools` | array of tool names | no | Negative list: all tools the access class allows except these. Mutually exclusive with `tools`. |
| `description` | string | no | Human-readable. Shown by `policy explain`; not included in denial messages. |
| `mode` | enum `"observe" \| "enforce"` | no | Per-grant override of the manifest-level mode (ADR-0020 commitment 4). |

Tool names in `tools` / `exclude_tools` must be members of the 13-tool list in section 8;
unknown names are a validation error. `computer` sub-actions are not addressable here;
grant-level tool checks apply to the string `"computer"` (SPEC 5.4 rule retained), while
observe/mutate classification applies per sub-action.

Grant resolution (SPEC 4.3 retained): the current tab's URL comes from the extension, not
from tool parameters; grants are evaluated in manifest order; first matching domain
pattern wins. More specific grants belong before broader ones.

### 4.4. Config entries

Each entry in the manifest `config` array:

```json
{ "key": "audit.enabled", "value": true, "level": "mandatory" }
```

- `key`: a registered dotted key name. Unknown keys are a validation error (the generated
  JSON Schema catches this at authoring time).
- `value`: must satisfy the key's type and constraints (section 3.2).
- `level`: enum `"mandatory" | "recommended"`. In the org policy file, `mandatory`
  populates layer 1 (locked) and `recommended` populates layer 3. In a user-supplied
  manifest both apply at the user layer, `mandatory` with a downgrade warning
  (section 1.3).

### 4.5. No manifest = all-open

Per ADR-0013, absence of any manifest is a first-class supported mode, preserved by
construction: enforcement STEP 0 short-circuits to Allow when no manifest is present.
Precisely:

- No grant evaluation, no domain restriction, all 13 tools advertised and permitted.
- The configuration registry still resolves (user layer, preset, Minimal), so
  `content.security.sacred_domains` still enforces and the audit flight recorder still
  records when `audit.enabled` is true.
- When a manifest with a non-empty `grants` array IS active, a call whose domain matches
  no grant is denied under `enforce` (rule `unmatched_domain`) or shadow-denied under
  `observe`. The SPEC 4.2 `defaults.unlisted_domains` tri-state is superseded by this
  rule plus the mode switch.

---

## 5. Domain pattern language

### 5.1. Pattern grammar

A pattern is either:

- an exact host: `example.com` matches the host `example.com` and nothing else (no
  subdomains); or
- a wildcard: `*.example.com` (a single leading `*.` label; no other wildcard position is
  legal) matches any host that is a strict subdomain of `example.com` at any depth
  (`foo.example.com`, `a.b.example.com`) and does NOT match `example.com` itself.

To cover a domain and its subdomains, list both: `["example.com", "*.example.com"]`
(SPEC 4.2 rule retained). Patterns containing a scheme, port, path, userinfo, or interior
`*` are validation errors. Patterns are authored in lowercase ASCII; IDN domains must be
authored in punycode (A-label) form, and `policy explain` warns on non-ASCII patterns.

### 5.2. Matching semantics

Matching operates on the HOST produced by a real, WHATWG-compliant URL parser. The matcher
never substring-searches the raw URL string.

- Case: hosts and patterns are compared after ASCII lowercasing (parsers already emit
  lowercase hosts).
- Port: ignored. The host is extracted without the port; a grant for `example.com` covers
  `example.com:8443`.
- Scheme: only `http` and `https` URLs are matchable. Under an active manifest in enforce
  mode, any other scheme (`file:`, `chrome:`, `chrome-extension:`, `data:`,
  `javascript:`, `about:` and the rest) matches no grant and is denied with rule
  `scheme`, EXCEPT `about:blank`, which is always allowed (it is the safe parking page
  used after a post-navigation denial, SPEC 5.2).
- Trailing dot: one trailing dot is stripped from the host before matching
  (`example.com.` matches the pattern `example.com`).
- IP literals: an exact pattern may be an IP literal and matches only the identical
  parser-normalized literal. Wildcard patterns NEVER match IP literals (v4 or v6). IPv6
  brackets are stripped before comparison. Alternate dotted forms (`0x7f.0.0.1`, packed
  decimal, octal) are handled by parser normalization: the matcher only ever sees the
  parser's canonical host.
- IDN / punycode: the parser-normalized host is the A-label (punycode) form; comparison
  happens in A-label space. A homoglyph host (`xn--pple-43d.com` from a Cyrillic
  lookalike) does not match `apple.com`.
- Userinfo: `https://allowed.com@evil.com/` has host `evil.com`. Credentials in the URL
  never participate in matching.
- Redirects and drift: matching applies to the FINAL navigated URL as reported by the
  extension (post-redirect, SPEC 5.2), and every tool call re-checks the current tab URL
  (SPEC 5.3), so user clicks and late redirects that change the domain are caught.

### 5.3. Required negative test classes

The matcher ships with these as named unit tests (ADR-0018 step 3). Each must FAIL to
match a grant for `allowed.com` / `*.allowed.com`:

| Class | Example input | Why |
|---|---|---|
| Userinfo bypass (CVE-2025-47241 class) | `https://allowed.com@evil.com/` | host is `evil.com`; substring matchers are fooled |
| Embedded credentials | `https://user:pass@evil.com/` and `https://allowed.com:token@evil.com/` | same class, with password present |
| IP literal vs wildcard | `http://127.0.0.1/` against `*.allowed.com`; `http://[::1]/` against any wildcard | wildcards never match IPs |
| IP literal alternate forms | `http://0x7f.0.0.1/`, `http://2130706433/` against pattern `127.0.0.1` | must match ONLY if the parser normalizes them to `127.0.0.1`; the test pins the parser behavior either way |
| Trailing dot | `https://evil.com./` against `allowed.com` | normalization must not create a bypass; and `https://allowed.com./` MUST match `allowed.com` (positive twin) |
| Punycode / IDN homoglyph | `https://xn--llowed-vx9c.com/` (or any confusable) against `allowed.com` | A-label comparison only |
| Apex vs wildcard | `https://allowed.com/` against `*.allowed.com` alone | wildcard excludes the apex by definition |
| Suffix stitching | `https://evilallowed.com/` and `https://allowed.com.evil.com/` against `allowed.com` / `*.allowed.com` | label-boundary matching, not string suffix |
| Redirect | navigate to an allowed URL that 302s to `https://evil.com/` | final-URL check must deny and park on `about:blank` |
| Non-http scheme | `file:///etc/passwd`, `javascript:alert(1)` | scheme rule, denied under enforce |

---

## 6. Audit record

JSON Lines: one JSON object per record, compact serialization (no internal newlines),
terminated by a single LF, appended to the destination resolved from `audit.enabled`,
`audit.destination`, and `audit.file.path`. Written by the binary only; the extension
never logs (SPEC 7.4 trust boundary). Every tool call produces exactly one record:
permitted, denied, and shadow-denied alike (ADR-0018 step 1). This shape is designed once
and reused by `policy simulate`, the local activity ledger, and session recap
(ADR-0018 follow-up, ADR-0020 commitment 3).

### 6.1. Fields

SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the `rw` row of the table below is replaced by a `capability` field whose value is one of `read`, `action`, `write`, `execute`, or `none` (ADR Decision 8); every other row is unchanged. The text below is retained as history for the stage-2 implementation record.

| Field | Type | Meaning |
|---|---|---|
| `event_id` | string | UUID v4, lowercase, hyphenated. Unique per record. |
| `ts` | string | RFC 3339 UTC timestamp with millisecond precision, e.g. `2026-07-02T14:32:15.003Z`. Record creation time (call completion). |
| `identity` | object or null | `{ "principal": string, "resolved_by": string }` from the active manifest's `identity` block; `null` when absent. |
| `client` | object or null | `{ "name": string, "version": string }` from the MCP `initialize` request's `clientInfo`; `null` if the client did not provide it. Captured once per session. |
| `tool` | string | MCP tool name, one of the 13 in section 8. |
| `action` | string or null | The `computer` sub-action (e.g. `"left_click"`); `null` for every other tool. |
| `rw` | string | `"observe"` or `"mutate"`, per the classification table (section 8). |
| `domain` | string or null | Parser-normalized host of the current tab at decision time; `null` when there is no tab or no URL (e.g. first `tabs_create_mcp` of a session). |
| `decision` | string | `"allow"`, `"deny"`, or `"shadow_deny"`. `shadow_deny` means observe mode evaluated a deny but the call executed (ADR-0020 commitment 4). |
| `grant_id` | string or null | The `id` of the grant that resolved the decision; `null` when no grant matched or no manifest is active. |
| `denial_id` | string or null | The stable denial id (section 7) for `deny` and `shadow_deny`; `null` for `allow`. |
| `duration_ms` | unsigned integer | Wall time from dispatch entry to result, in milliseconds. `0` for calls denied before dispatch. |
| `manifest` | object or null | `{ "name": string, "version": string, "hash": string }` of the active manifest (`hash` per section 4.2, 64 lowercase hex chars); `null` when no manifest is active. |
| `held` | boolean | `true` when the call was answered with the take-the-wheel pause text instead of executing (user hold, G10); on held records `decision` is `"allow"` and `duration_ms` is `0`. `false` on all other records. |

Example (one line on disk):

```json
{"event_id":"a1b2c3d4-e5f6-4890-abcd-ef1234567890","ts":"2026-07-02T14:32:15.003Z","identity":{"principal":"ACME\\jdoe","resolved_by":"managed_config"},"client":{"name":"claude-code","version":"2.1.0"},"tool":"computer","action":"left_click","rw":"mutate","domain":"epic.acme.org","decision":"deny","grant_id":"research-external","denial_id":"D-9f3a1c2e","duration_ms":0,"manifest":{"name":"acme-clinical-pilot","version":"2026.07.1","hash":"4f2d...64hex...9ab0"}}
```

### 6.2. Sensitive-parameter omission rule

Per SPEC 7.2, applied strictly in stage 2:

- Tool call parameters are NEVER written to audit records. There is no `parameters`
  field. (Parameters may contain typed secrets, PHI, or `javascript_tool` source. A
  future opt-in registry key may add them; it is not in the stage-2 key set.)
- Screenshot data is NEVER written to audit records. There is no `screenshot` field.
- Full URLs are not logged; only the normalized host goes in `domain`. (This is stricter
  than SPEC 7.1, which logged `url`; query strings routinely carry identifiers. Recorded
  as a SPEC update.)

---

## 7. Denial format

### 7.1. Denial id

A stable, per-policy-version identifier a developer can hand to their admin and the admin
can trace in the SIEM (ADR-0020 commitment 6).

```
denial_id = "D-" + first 8 lowercase hex chars of
            SHA-256( manifest_hash + "\n" + grant_id + "\n" + rule )
```

All three components are UTF-8 strings joined with exactly one LF between them:

- `manifest_hash`: the 64-hex content hash of the active manifest (section 4.2), or the
  empty string when no manifest is active (sacred-domain denials in all-open mode).
- `grant_id`: the resolving grant's `id`, or the empty string when no grant matched.
- `rule`: the rule class, optionally followed by `/` and a detail token:

| Rule string | Trigger | Detail token |
|---|---|---|
| `unmatched_domain` | manifest active, no grant matched the host | none |
| `access` | grant matched, call's rw class not authorized by `access` | none |
| `tool/<tool_name>` | tool blocked by `tools` / `exclude_tools` | the tool name |
| `sacred/<pattern>` | host matched a `content.security.sacred_domains` pattern | the matching pattern |
| `scheme/<scheme>` | non-http(s) URL under an active manifest | the scheme without `:` |

The id is deterministic across restarts for the same manifest version, so repeated
denials of the same kind share one id, and a manifest edit changes every id (correct:
decisions are attributable to the exact policy version).

### 7.2. Denial message shown to the agent

Denials return a normal MCP tool result whose content is a single `{type:"text"}` item
(not a JSON-RPC error), so the agent can read and adapt. Template rules: plain language,
starts with `Denied (D-xxxxxxxx):`, states what was blocked and by which grant, gives the
agent one actionable next step, and NEVER enumerates other grants, other domains, manifest
paths, group names, or config values.

Templates per rule class (`<...>` are substitutions):

- `access` (mutate on read grant):
  `Denied (D-<id>): '<tool>' needs write access on <domain>, and grant '<grant_id>' allows read only. Observation tools (read_page, get_page_text, find, screenshot) remain available. Give this denial id to your administrator to request write access.`
- `access` (observe on write-only grant):
  `Denied (D-<id>): '<tool>' needs read access on <domain>, and grant '<grant_id>' allows write only. Give this denial id to your administrator.`
- `unmatched_domain`:
  `Denied (D-<id>): no grant covers <domain>. Tool use is limited to domains your policy grants. Give this denial id to your administrator if access to <domain> is needed.`
- `tool`:
  `Denied (D-<id>): grant '<grant_id>' does not permit '<tool>' on <domain>. Other tools in your access class remain available. Give this denial id to your administrator to request '<tool>'.`
- `sacred`:
  `Denied (D-<id>): <domain> is on the user's never-touch list. Do not retry or work around this; choose a different approach or ask the user directly.`
- `scheme`:
  `Denied (D-<id>): the URL scheme '<scheme>:' is not permitted under the active policy. Only http and https pages can be automated.`

For `computer` calls, `<tool>` renders as `computer (<action>)`, e.g.
`computer (left_click)`.

Shadow mode: under effective mode `observe`, a would-deny call EXECUTES normally and the
agent sees the ordinary tool result with no denial text; the audit record carries
`decision: "shadow_deny"` with the same `denial_id` enforce mode would have produced.
Status surfaces (doctor, config list, extension popup) must badge shadow mode plainly;
observing must never present as protection (ADR-0020).

---

## 8. Read/write classification table

SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the observe/mutate classification in this whole section is replaced by the four-capability action directory (`read`, `action`, `write`, `execute`) with per-action requirement sets (ADR Decisions 1 and 2). The text below is retained as history for the stage-2 implementation record.

Authoritative classification of the full tool surface. Tool names verified against
`src/mcp/schemas/tools.json` (13 tools; note the `_mcp` suffixes on the tab tools, which
are part of the sacred schema surface). The `computer` action enum (13 actions) is
verified against the same file.

| Tool | Class | Notes |
|---|---|---|
| `navigate` | mutate | Changes browser state; also the pre/post-navigation enforcement point (section 5.2 of SPEC). |
| `computer` | split | Per sub-action, below. |
| `read_page` | observe | |
| `get_page_text` | observe | |
| `find` | observe | |
| `read_console_messages` | observe | |
| `read_network_requests` | observe | |
| `tabs_context_mcp` | observe | |
| `tabs_create_mcp` | mutate | Creates a tab. |
| `form_input` | mutate | |
| `javascript_tool` | mutate | Always mutate, no exceptions (SPEC 3.1 rule retained). |
| `resize_window` | mutate | Changes browser state. |
| `update_plan` | observe | Informational pass-through. |

`computer` sub-actions (all 13 enum values):

| Class | Actions |
|---|---|
| observe | `screenshot`, `scroll`, `zoom`, `wait`, `hover`, `scroll_to` |
| mutate | `left_click`, `right_click`, `double_click`, `triple_click`, `type`, `key`, `left_click_drag` |

Rationale for the observe set: these read or reveal page state without committing input
that changes application state. `scroll`, `hover`, and `scroll_to` dispatch input events
but only move the viewport or pointer; a read-only grant that cannot scroll cannot read a
page below the fold, which would make `access: "read"` useless in practice. This
deliberately supersedes SPEC 3.3 (which classified them mutate on "dispatches input"
grounds); see "SPEC updates needed".

Enforcement mapping: `access: "read"` authorizes observe-class calls, `"write"`
authorizes mutate-class calls, `"all"` authorizes both (section 4.3). Grant-level
`tools` / `exclude_tools` checks match the tool name `"computer"`, never an action name
(SPEC 5.4 rule retained). Tool advertisement filtering (SPEC 5.1) uses this table with
the read/write vocabulary; advertisement remains a visibility optimization, and per-call
enforcement stays authoritative.

---

## 9. Native-messaging settings protocol

The extension options page and popup are the friendly surface for status and settings
(ADR-0019 commitment 5). The extension remains policy-free presentation (ADR-0005): it
renders what the binary reports and submits edits; the binary resolves layers, validates,
and rejects writes to locked keys. No embedded HTTP server.

Transport: the existing native messaging channel (4-byte LE length-prefixed UTF-8 JSON,
`src/native/host.rs`), same envelope style as the tool protocol
(`src/native/messages.rs`): every request carries a caller-chosen string `id`, every
response echoes it. The binary answers settings messages from its own resolved state; an
active MCP session is not required.

### 9.1. Requests (extension to binary)

```json
{ "id": "<string>", "type": "get_status" }
{ "id": "<string>", "type": "get_config" }
{ "id": "<string>", "type": "set_config_key", "key": "<dotted key>", "value": <JSON value> }
```

`set_config_key` writes the user layer (section 1.1) only. There is no mechanism, on this
surface or any other, to write org layers.

### 9.2. Responses (binary to extension)

`get_status` reply:

```json
{
  "id": "<echoed>",
  "type": "status",
  "result": {
    "version": "<binary version string>",
    "session_active": true,
    "governance": {
      "manifest": { "name": "...", "version": "...", "hash": "<64 hex>" },
      "mode": "observe",
      "shadow": true
    },
    "audit": { "enabled": true, "destination": "file", "path": "<resolved path>" }
  }
}
```

- `governance.manifest` is `null` when no manifest is active; `governance.mode` is the
  effective manifest-level mode; `shadow` is `true` when the effective mode is `observe`
  while a manifest with grants is active (the badge input; observing must never present
  as protection).

`get_config` reply (one entry per registered key; this is the `chrome://policy` analog
and renders the options page without any extension-side knowledge of keys):

```json
{
  "id": "<echoed>",
  "type": "config",
  "result": {
    "keys": [
      {
        "key": "content.security.secrets.redact",
        "type": "bool",
        "description": "Redact values of secret fields (password/OTP/payment) in read_page output.",
        "value": true,
        "source": "org_mandatory",
        "locked": true
      },
      {
        "key": "engine.connection.first_call_wait_ms",
        "type": "uint",
        "min": 0,
        "max": 60000,
        "description": "Upper bound on the first-call wait for the extension handshake.",
        "value": 5000,
        "source": "builtin",
        "locked": false
      },
      {
        "key": "audit.destination",
        "type": "enum",
        "variants": ["file", "stderr"],
        "description": "Where audit records are written.",
        "value": "file",
        "source": "preset",
        "locked": false
      }
    ]
  }
}
```

Constraint fields appear per type: `min`/`max` for uint, `variants` for enum; string-list
keys use `"type": "string_list"`, strings `"type": "string"`.

`set_config_key` replies:

```json
{ "id": "<echoed>", "type": "config_ok",
  "result": { "key": "<key>", "value": <stored value>, "source": "user", "locked": false } }

{ "id": "<echoed>", "type": "config_error",
  "error": { "code": "locked", "message": "This setting is managed by your organization." } }
```

`error.code` enum:

| Code | Meaning |
|---|---|
| `locked` | key resolves from org-mandatory; write rejected; message is exactly the "managed by your organization" wording so the UI badge and rejection agree |
| `unknown_key` | key is not in the registry |
| `invalid_value` | value fails the key's type or constraints; `message` names the constraint (e.g. "expected an integer between 0 and 60000") |

The mcp-server role applies a successfully written key to the running session where the
key's semantics allow it (e.g. `content.security.secrets.redact` takes effect on the next
call); keys read at startup (e.g. `audit.destination`) note "takes effect on restart" in
their registry description. The extension never caches config; it re-requests
`get_config` after every write.

---

## 10. SPEC updates needed

Each item is a divergence where an ADR (or this reconciliation) supersedes the current
SPEC text. Amend the SPEC accordingly.

1. **Tool classification (SPEC 3.1, 3.3, 5.4).** `navigate` moves from the Observe tier
   to mutate. `computer` sub-actions `scroll`, `hover`, `scroll_to` move from mutate to
   observe (observe set: `screenshot`, `scroll`, `zoom`, `wait`, `hover`, `scroll_to`);
   SPEC 3.3's "scroll is Mutate because it dispatches input" rationale is replaced by the
   viewport-only rationale in section 8. The Manage tier dissolves: `resize_window` is
   mutate, `update_plan` is observe; "always available regardless of access tier" no
   longer applies. SPEC 5.4's enumeration ("screenshot or wait" observe, all else mutate)
   is superseded by the section-8 table.
2. **Grant access vocabulary (SPEC 4.1, 4.2, 5.1).** `access` changes from
   `"observe" | "mutate"` to `"read" | "write" | "all"`, with read = observe-class,
   write = mutate-class, all = both, and write not implying read. Observe/mutate remain
   the classification vocabulary for tools and audit (`rw` field).
3. **Manifest identity (SPEC 4.1, 4.2).** Manifests gain required top-level `name` and
   `version` and a computed SHA-256 content hash over canonical bytes (section 4.2);
   `schema` bumps to 2. The SPEC's `identity` block (principal, resolved_by, groups,
   resolved_at) is retained unchanged.
4. **Mode switch (new in SPEC 4/5).** `mode: observe | enforce` at manifest level with
   per-grant override, defaulting from the `governance.mode` registry key; shadow
   enforcement (`shadow_deny`) per ADR-0020.
5. **`defaults` block removed (SPEC 4.1, 4.2).** `unlisted_domains` is superseded by:
   manifest with grants active + no match = deny (or shadow_deny in observe mode); no
   manifest = all-open (ADR-0013). Screenshot/timeout/tab tunables move to future
   registry keys instead of a manifest `defaults` block.
6. **`audit` block removed from the manifest (SPEC 4.1, 4.2, 7.3).** Audit configuration
   becomes registry keys (`audit.enabled`, `audit.destination`, `audit.file.path`),
   lockable via org config entries (ADR-0019 follow-up). Stage 2 destinations are
   `file` and `stderr` only; `syslog`, `http`, and `none` are deferred. SPEC 7.3's
   `audit.file_path` name becomes `audit.file.path`.
7. **Layered configuration model (new SPEC section).** Five layers with precedence
   org-mandatory > user > org-recommended > preset default > built-in Minimal; resolved
   triple (value, source, locked); presets fully_open/safe/restricted; typed key
   registry (ADR-0019). File locations per section 1.
8. **Manifest sources (SPEC 4.4).** Add the auto-loaded org policy file
   (ProgramData / /Library/Application Support / /etc) as the primary org source, taking
   precedence over `--manifest`/env; `managed://` is deferred out of stage 2; the
   no-manifest row's "equivalent to unlisted_domains: observe" wording is replaced by
   all-open per ADR-0013.
9. **Audit record schema (SPEC 7.1, 7.2).** New field set per section 6 (`event_id`,
   `ts`, `identity`, `client`, `tool`, `action`, `rw`, `domain`, `decision`, `grant_id`,
   `denial_id`, `duration_ms`, `manifest`). `result` becomes `decision` with the added
   `shadow_deny` value; `access_tier_required`/`access_tier_granted` are dropped in
   favor of `rw` plus `grant_id`; `client` and `manifest` identity are added
   (ADR-0020 commitment 5). `url`, `parameters`, and `screenshot` fields are removed:
   parameters and screenshots are never logged in stage 2 (no opt-in key yet), and only
   the normalized host is logged (stricter than SPEC 7.2's "url always logged").
10. **Denial format (SPEC 5.5).** The freeform example is replaced by the stable denial
    id scheme and per-rule templates of section 7; denial messages name only the
    resolving grant and never enumerate other grants or policy contents. Denials return
    a normal text tool result (as SPEC 5.5 already shows) with the leading
    `Denied (D-xxxxxxxx):` marker.
11. **Domain matching detail (SPEC 4.2).** Wildcard semantics are retained; add the
    normalization rules (parsed-host-only matching, case, port, scheme, trailing dot,
    IP-literal, punycode/A-label) and the required negative test classes of section 5.3,
    including the CVE-2025-47241 userinfo class and redirect handling as published test
    cases (ADR-0018 step 3).
12. **Sacred domains (new in SPEC 5).** `content.security.sacred_domains` enforced in
    the binary regardless of manifest presence and regardless of mode (ADR-0018 step 2).
13. **Settings surface (new SPEC section or 2.4 extension).** The native-messaging
    settings protocol of section 9 (get_status / get_config / set_config_key, locked-key
    rejection), extension as policy-free presentation (ADR-0019 commitment 5).
14. **Manifest schema 3 grant shape (ADR-0022 Decisions 3, 4, 6; supersedes item 2 above).**
    Grants drop `domains`, `access`, `tools`, and `exclude_tools` in favor of
    `hosts: { "allow": [...], "deny": [...] }` (default deny; `*` is the explicit everything
    token; most-specific match wins, exact tie goes to deny; per-grant scope only) and
    `allowed: [capability, ...]` with subset-containment enforcement. `schema` bumps to 3;
    schema 2 never shipped and is rejected.
15. **Capability classification (ADR-0022 Decisions 1, 2; SPEC 3.1, 3.3, 5.4; supersedes
    item 1 above).** The observe/mutate/manage tiering is replaced by four capabilities
    (`read`, `action`, `write`, `execute`) and a per-action requirement table compiled into
    the binary; no directory entry means deny, `requires: []` means unconditionally allowed.
16. **Audit `capability` field (ADR-0022 Decision 8; SPEC 7.1, 7.2; amends item 9 above).**
    The audit record's `rw` field is replaced by `capability`, a string: `read`, `action`,
    `write`, `execute`, or `none`.
17. **Advertised surface is 13 plus 1 (ADR-0022 Decision 7; SPEC 3, 5.1).** The 13 trained
    tool schemas remain byte-identical; exactly one additive, argument-less governance tool
    named `explain` is sanctioned on top, advertised under every manifest and always allowed.
18. **One loader for the policy file (ADR-0023; SPEC 4.4).** The policy file has exactly
    one parser and one schema authority (the manifest parser, schema 3); org config
    layers derive from the parsed manifest's `config` entries; duplicate config keys are
    a validation error; every load path performs one parse per invocation or change.
19. **Tool registry and generic ingest pipeline (ADR-0024; SPEC 3, 5).** One per-tool
    descriptor table (capability variants, resource shape, handler kind, hooks) drives
    validity, classification, enforcement input, advertisement, explain, and result
    post-processing; governance owns audit-record selection through a per-call scope;
    the sacred check and grant path share one tab-URL resolution per call. The 13
    trained tool schemas plus `explain` remain byte-identical (ADR-0022 Decision 7).
20. **Manifest hot-reload (ADR-0025; SPEC 4.4, 2).** The org policy path and a file://
    user manifest source are watched; grants/mode/hash swap atomically per call
    snapshot; an advertised-set change emits `notifications/tools/list_changed`; policy
    transitions record `manifest_reload` / `user_manifest_ignored` session events;
    invalid edits keep the last-good manifest.
