# S08: documentation sync (supersession banners, CLAUDE.md, SPEC updates list, browser checks)

## Goal

Make the written record match the shipped capability model. Stage 3 replaced the
observe/mutate access model with ADR-0022's capabilities, host polarity, schema 3, the audit
`capability` field, and the `explain` tool; the stage-2 shared-format doc, the repo-root
CLAUDE.md, and the live-check backlog still describe the old model. This task is docs-only:
no code, no tests, no SPEC.md edits. It is the last stage-3 task. Commit as
`docs(governance): s08 documentation sync`.

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) is normative; where this prompt
and the ADR disagree, the ADR wins. Per BOOTSTRAP.md rule 4, the shared-format doc stays
authoritative for everything ADR-0022 does NOT supersede (config formats, audit fields other
than `rw`, denial-id mechanics, section 7.2 message voice); the banners below must mark only
the superseded portions, never the whole document.

## Depends on

s01 through s07 landed. Check before starting: `rg -n "\"schema\": 3" examples/` finds
matches, and `rg -n "\"explain\"" src/transport/mcp/schemas/tools.json` finds the 14th tool.
If either check fails, STOP: earlier tasks did not land.

## Current behavior (verify against the tree before editing)

- `docs/tasks/stage-2/00-shared-format.md`: section `### 4.3. Grants` documents the schema-2
  grant fields (`domains`, `access`, `tools`, `exclude_tools`); section `### 6.1. Fields`
  has the audit table with the `rw` row (`"observe"` or `"mutate"`); section
  `## 8. Read/write classification table` is the whole observe/mutate model; section
  `## 10. SPEC updates needed` is a numbered list currently ending at item 13. The string
  `SUPERSEDED` appears nowhere in the file.
- Repo-root `CLAUDE.md` contains, each exactly once: the fragment
  `identity-bound access control, tool-level r/w classification, and structured audit logging`
  (Project Identity); the `**Critical constraint:**` paragraph ending
  `but the schemas themselves are sacred.` (Origin); the Phase 4 bullet
  `- Implement \`computer\` sub-action classification (observe vs mutate).`; and the sentence
  `The model's trained behavior depends on exact schema matching.` (Tool Schema
  Preservation). The file has preexisting non-ASCII (section signs, box-drawing characters in
  the tree diagram); do NOT clean those up; the ASCII rule applies to lines you add.
- `docs/tasks/stage-2/BROWSER-TESTS.md`: the Format section at the top defines four-field
  entries (`## <task-id>-<n>: <one-line purpose>` then `Changed:` / `Steps:` / `Expect:`);
  the last stage-2 entry is `## g15-2:`; earlier stage-3 tasks may have appended entries
  after it. Entries are never deleted.
- `docs/adr/README.md` ALREADY lists ADR-0022 in its table (verified 2026-07-02); Required
  behavior 5 is expected to be a no-op verification.

## Required behavior

### 1. Supersession banners in `docs/tasks/stage-2/00-shared-format.md`

Insert three banner paragraphs. Each is a pure insertion: do NOT rewrite, reword, or delete
any historical text. Exact text and placement:

- Immediately after the heading `### 4.3. Grants`, before `Each grant object:`, insert:
  `SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the grant fields \`domains\`, \`access\`, \`tools\`, and \`exclude_tools\` are replaced in manifest schema 3 by \`hosts\` (allow/deny polarity, ADR Decision 4) and \`allowed\` (capability sets, ADR Decision 3). The text below is retained as history for the stage-2 implementation record.`
- Immediately after the heading `### 6.1. Fields`, before the table, insert:
  `SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the \`rw\` row of the table below is replaced by a \`capability\` field whose value is one of \`read\`, \`action\`, \`write\`, \`execute\`, or \`none\` (ADR Decision 8); every other row is unchanged. The text below is retained as history for the stage-2 implementation record.`
- Immediately after the heading `## 8. Read/write classification table`, before the
  `Authoritative classification` paragraph, insert:
  `SUPERSEDED by ADR-0022 (docs/adr/0022-intent-calibrated-capabilities.md): the observe/mutate classification in this whole section is replaced by the four-capability action directory (\`read\`, \`action\`, \`write\`, \`execute\`) with per-action requirement sets (ADR Decisions 1 and 2). The text below is retained as history for the stage-2 implementation record.`

Each banner is one paragraph on its own, with a blank line before and after.

### 2. Extend the `## 10. SPEC updates needed` list in the same file

Append these four items verbatim after item 13, numbered 14 to 17:

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

### 3. Repo-root `CLAUDE.md`: four surgical edits

Apply exactly these replacements and nothing else; keep every other byte of the file
identical. List all four changed paragraphs in the ledger entry.

- Edit A (Project Identity): replace the fragment
  `identity-bound access control, tool-level r/w classification, and structured audit logging`
  with
  `identity-bound access control, per-action capability classification (read, action, write, execute), and structured audit logging`
- Edit B (Origin): replace the whole paragraph starting `**Critical constraint:**` with:
  `**Critical constraint:** Preserve the exact MCP tool names, parameter signatures, and description strings from the reference implementation's tool schemas. Claude was trained against these schemas. The 13 trained tool schemas must stay byte-identical to what the official Claude in Chrome extension advertises; exactly one additive, argument-less governance tool named \`explain\` is sanctioned on top (ADR-0022 Decision 7). No other addition, removal, or edit is sanctioned. Our governance layer shapes which tools are visible and when they execute, but the trained schemas themselves are sacred.`
- Edit C (Phase 4): replace the bullet
  `- Implement \`computer\` sub-action classification (observe vs mutate).`
  with
  `- Implement \`computer\` sub-action classification (per-action capability requirements: read, action, write, execute).`
- Edit D (Tool Schema Preservation): replace the sentence
  `The model's trained behavior depends on exact schema matching.`
  with
  `The model's trained behavior depends on exact schema matching. The one sanctioned exception is the additive \`explain\` directory tool (ADR-0022 Decision 7); it is not part of the trained surface and its schema is pinned by \`tests/tool_schema_fidelity.rs\`.`

### 4. Append stage-3 live checks to `docs/tasks/stage-2/BROWSER-TESTS.md`

Append four entries after the current LAST entry in the file (never delete or reorder
existing entries; if s07 already queued an explain entry, append s-live-3 anyway: it is the
consolidated stage-3 pass). Use the file's own `Changed:` / `Steps:` / `Expect:` format with
these headings and this pinned content:

- `## s-live-1: read grant end to end (capability enforcement live)`
  Changed: stage 3 (s01-s06) replaced observe/mutate with per-action capability requirements
  over schema-3 grants; first live run of a read-only grant against real Chrome.
  Steps: save this manifest and start the MCP client with `--manifest file://<path>`:
  `{ "schema": 3, "name": "s-live-read-check", "version": "1", "grants": [ { "id": "read-only", "hosts": { "allow": ["example.com", "*.example.com"] }, "allowed": ["read"] } ] }`
  Then: (1) `tabs_create_mcp`; (2) `navigate` to `https://example.com/`; (3) `read_page` and
  a `computer` `screenshot` on that tab; (4) a `computer` `left_click` anywhere on the page;
  (5) a `form_input` call on that tab (any element ref; the denial happens before dispatch,
  so no matching element needs to exist).
  Expect: steps 1-3 succeed normally (`navigate` requires `read` under ADR-0022; a read grant
  can navigate, read, and screenshot). Step 4 returns text starting `Denied (D-` containing
  `'computer (left_click)' needs the 'action' capability on example.com, and grant 'read-only' allows read`
  and the click visibly does not happen. Step 5's denial contains
  `'form_input' needs the 'write' capability on example.com, and grant 'read-only' allows read`.
- `## s-live-2: denied_domain live (allow * with a deny carve-out)`
  Changed: s04/s05 added host polarity; `hosts.deny` carves holes out of `allow`, producing
  the new `denied_domain` rule attributed to the denying grant. First live run.
  Steps: start the MCP client with
  `{ "schema": 3, "name": "s-live-deny-check", "version": "1", "grants": [ { "id": "everything-but", "hosts": { "allow": ["*"], "deny": ["example.com"] }, "allowed": ["read", "action", "write"] } ] }`
  active. (1) `navigate` to `https://example.com/`; (2) `navigate` to `https://example.org/`,
  then `read_page` and a `left_click` there.
  Expect: step 1 is denied with text starting `Denied (D-` containing
  `example.com is excluded by grant 'everything-but': your policy denies this site explicitly`
  and the browser does not navigate. Step 2 works end to end (allow `*` covers everywhere
  else), including the click.
- `## s-live-3: the explain tool live (advertised, correct output, no spurious calls)`
  Changed: s07 added `explain`, the one sanctioned tool-surface addition (ADR-0022 Decision
  7); only a live client shows whether a trained model ignores it during normal browsing.
  Steps: with any posture (no manifest is fine): (1) list the advertised tools in the client;
  (2) ask the agent to call `explain`; (3) run a short normal browsing session (navigate
  somewhere, screenshot, then ask the agent to "explain this page").
  Expect: step 1 shows `explain` alongside the 13 trained tools. Step 2 returns a single text
  block starting `Capabilities: read =` with one requires line per tool and per computer
  action. Step 3 answers from page content WITHOUT invoking the `explain` tool; record any
  spurious invocation in the live log as a rename signal per ADR-0022 Decision 7.
- `## s-live-4: audit capability field live`
  Changed: s06 replaced the audit record's `rw` field with `capability` (ADR-0022 Decision
  8); this confirms the real JSONL from a live session.
  Steps: with `audit.enabled` true and `audit.file.path` pointed at a scratch file, re-run
  the s-live-1 session (same manifest, same calls), then open the audit JSONL file.
  Expect: no line contains an `rw` key. The `tabs_create_mcp` line has `"capability":"none"`
  and `"grant_id":null`; the `navigate`, `read_page`, and screenshot lines have
  `"capability":"read"` and `"grant_id":"read-only"`; the denied left_click line has
  `"capability":"action"`, `"decision":"deny"`, `"duration_ms":0`, `"grant_id":"read-only"`;
  the denied form_input line has `"capability":"write"`.

### 5. Verify `docs/adr/README.md` lists ADR-0022

Run `rg -n "0022-intent-calibrated-capabilities" docs/adr/README.md`. It must match exactly
one table row. It does in the current tree, so expect a no-op. ONLY if the row is missing,
append this row to the table (after the 0021 row):
`| [0022](0022-intent-calibrated-capabilities.md) | Intent-calibrated capabilities: epistemic classification, per-action requirements, host polarity | Accepted |`

## Constraints

1. Docs only: no changes under `src/`, `tests/`, `examples/`, or `extension/`; no
   `docs/SPEC.md` edits (the SPEC amendment happens via the updates-needed list, per house
   rule); no change to any tools.json mention outside the pinned CLAUDE.md edits.
2. Files touched are exactly: `docs/tasks/stage-2/00-shared-format.md`, `CLAUDE.md`,
   `docs/tasks/stage-2/BROWSER-TESTS.md`, `docs/tasks/stage-3/LEDGER.md`, and (only if the
   row is missing) `docs/adr/README.md`.
3. Banners and list items are insertions only; never rewrite or delete shared-format history.
4. ASCII only in every line you ADD (preexisting non-ASCII in CLAUDE.md stays as is).
5. One commit: `docs(governance): s08 documentation sync`.

## Tests (minimum)

No code tests change. Pinned doc assertions (all via rg from the repo root):

- `rg -c "SUPERSEDED by ADR-0022" docs/tasks/stage-2/00-shared-format.md` prints `3`.
- `rg -n "observe vs mutate" CLAUDE.md` prints nothing.
- `rg -n "tool-level r/w classification" CLAUDE.md` prints nothing.
- `rg -c "^## s-live-" docs/tasks/stage-2/BROWSER-TESTS.md` prints `4`.
- `rg -c "0022-intent-calibrated-capabilities" docs/adr/README.md` prints `1`.
- `rg -n "^17\." docs/tasks/stage-2/00-shared-format.md` prints one line (the new item 17).

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` all green
with results identical to the s07 run (nothing compiled changed). All Tests assertions above
pass. `git status` shows only the files in Constraint 2. ASCII scan of added lines only:
`git diff -U0 -- docs/tasks/stage-2/00-shared-format.md CLAUDE.md docs/tasks/stage-2/BROWSER-TESTS.md docs/adr/README.md docs/tasks/stage-3/LEDGER.md | grep "^+" | rg -n "[^\x00-\x7F]"`
produces no output (run before committing). Update the ledger (files touched, the four
CLAUDE.md paragraphs changed, browser checks queued: 4), commit, then follow BOOTSTRAP.md
"Completion" to write the RUN SUMMARY.

## Out of scope

- Rewriting or deleting shared-format history (banners are pure insertions).
- Editing `docs/SPEC.md` (deferred to a later SPEC amendment pass driven by section 10).
- README marketing copy, `docs/research/`, or any ADR text (ADRs are immutable).
- Renaming `explain` or altering its schema or output (ADR-0022 Decision 7; s07 owns it).
- Fixing CLAUDE.md's stale repository-structure tree or other preexisting drift not named in
  Required behavior 3.
- Any code, test, example, template, or extension change.
