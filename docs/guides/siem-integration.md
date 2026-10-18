# Collect Ghostlight audit

Ghostlight appends one content-minimized JSON object for every terminal invocation. It does not
upload audit, send syslog, or call a hosted collector. Its only listener is the window's own
address on 127.0.0.1, which serves the history view built from this file to the Ghostlight window
and, while "Others can watch" is on, to browser tabs on the same computer (see the Workbench page
row in [data-flows.md](../trust/data-flows.md)). SIEM delivery uses the endpoint's existing
file-collection agent.

## Select the file

Set `GHOSTLIGHT_AUDIT_FILE` to an absolute local path before starting Ghostlight. Without it,
the file is `~/.ghostlight/state/audit.jsonl`. Ghostlight creates the parent directory when possible,
opens the file append-only, writes one LF-terminated record, and flushes it. The workbench rebuilds
its bounded History view from the same file.

## Record shape (version 2)

```json
{
  "timestamp_ms": 1790000000000,
  "invocation": "invocation_00000000000000000000000000000001",
  "workspace": "workspace_00000000000000000000000000000001",
  "tool": "browser_navigate",
  "capabilities": [
    "read"
  ],
  "authority": "authority_00000000000000000000000000000001",
  "allowed": true,
  "reason": "permitted",
  "status": "succeeded",
  "effect": "applied",
  "summary": "Navigated to example.com.",
  "permissions": {
    "checks": [
      {
        "requirements": [
          "read"
        ],
        "host": "example.com",
        "allowed": true,
        "reason": "permitted",
        "layers": []
      }
    ],
    "truncated": false
  },
  "duration_ms": 412,
  "observed": {
    "host": "example.com",
    "readiness": "complete",
    "count": null,
    "width": null,
    "height": null
  },
  "channel": "mcp",
  "provenance": {
    "connection_id": "connection_00000000000000000000000000000001",
    "observed_at_ms": 1790000000000,
    "channel": "mcp",
    "claimed_agent": "claude-code",
    "source": {
      "agent": "claude-desktop",
      "image": "claude.exe"
    }
  },
  "browser": "Chromium"
}
```

The line above is `tests/golden/audit/success.json`: a navigate that succeeded. Records are written
on one line each; they are shown indented here. `refusal.json` (a click that policy refused) and
`flow.json` (a two-step flow: one child receipt, then the parent) sit beside it, and a test fails
when the service writes a different shape.

| Field | Meaning |
| --- | --- |
| `timestamp_ms` | Local observation time as Unix milliseconds. |
| `invocation`, `workspace` | Opaque correlation handles. |
| `tool` | Catalog tool name, or `unknown_tool` for an unrecognized request name. |
| `capabilities` | Complete independent RAWX requirement set. Empty is valid, and a request that never decoded records it empty. |
| `authority` | Opaque immutable authority snapshot id. |
| `policy_seq` | Signed managed publish sequence, when active. |
| `allowed`, `reason` | Final-boundary decision and stable reason. |
| `policy_rule`, `policy_tier`, `grant_id` | Content-free deciding attribution. |
| `denial_id` | Deterministic `D-` correlation id for an authored denial. |
| `status`, `effect` | Terminal result and physical-effect class. |
| `summary` | Bounded Ghostlight-authored sentence. |
| `refusal_facts` | Optional closed refusal category and typed context; never a copy of client result facts. |
| `duration_ms` | Direct decode-to-terminal or child dispatch-to-terminal elapsed time. |
| `observed` | Closed governed landing and measurement facts. |
| `step` | Optional child correlation: `parent` form, one-based `position`, `total`, and `preparation_failed`. `invocation` remains the parent id. |
| `composition` | Optional safe parent progress. Aggregate wrappers do not represent another attempted child. |
| `composition_tools` | Optional bounded canonical tool plan. Caller labels and arguments are excluded. |
| `permissions` | Bounded actual evaluation checks and a `truncated` flag; includes allowed work as well as refusals. Each child record holds only its own checks, and a flow's own record holds its admission check. |
| `coverage` | Optional document coverage of a read: `targeted`, the inspected, excluded, page excluded and unavailable document counts, and `limited_by_size`. |
| `unconfirmed_history_steps` | Optional count of child receipts whose storage was not confirmed; absent when zero. |
| `channel` | `mcp` or `cli`; attribution, not authority. |
| `provenance` | Opaque `connection_id`, `observed_at_ms` (when the service accepted the connection) and `channel` of the connection that sent the work, its optional `claimed_agent` and its optional `source`. `claimed_agent` is the catalog id of the agent the connection's client claimed to be, kept only when that claim names an agent in Ghostlight's catalog; it is the client's claim, never verified, and the client's own words are never written. `source` is the app Ghostlight saw start that connection, read from the operating system and never from the client. `source.agent` is the catalog id of the agent whose installed app it is, when one is; `source.image` is that app's executable file name, or the nearest program that is not a shell or interpreter, never a path. Attribution, not authority. |
| `browser` | Browser family or product name that performed the work. |

Readers ignore any field they do not know and never write it back, so a collector, `ghostlight
policy simulate` and the workbench read lines written by a later release. A field this version
writes keeps its meaning.

The H1 source correction replaces arbitrary failure-facts retention with typed metadata. For
example, a primitive browser error records `{"reason":"browser_primitive_failed"}` while its
arbitrary error description stays in the permitted client result. Deadline metadata can retain
`before_dispatch`; workspace and recovery failures can retain a closed `cause`. Query the top-level
policy fields for rule attribution and `status`/`effect` for the terminal outcome. A missing
`refusal_facts` is not a success signal: composed and measured outcomes have their own summaries.

A refusal because a target handle went stale has a fixed shape. Its `refusal_facts` is
`{"reason":"workspace_unusable","cause":{"stale_target":{"reason":"<word>"}}}`, and `<word>` is
one of four: `removed` (the page no longer has the control), `navigated` (the page moved on),
`tab_closed` (the tab is gone) and `unknown` (Ghostlight has no record of the handle). The word
is all the record keeps. The handles the refusal offered, the replacement it named and the
control names it listed reach the client only and are never audited. The shape is part of the
audit v2 contract, which freezes at 2.0: a word is not renamed or removed, and a collector
ignores a word it does not know. A line from an earlier build carries the bare string
`"stale_target"` as its `cause`; a reader keeps such a line and omits its `refusal_facts`.

A refusal because the page would not act on a control has the shape
`{"reason":"page_refused","cause":"<word>"}`, and `<word>` is one of four: `not_visible`,
`disabled`, `read_only` and `not_editable`. Nothing happened in the page. The page's own message
is never kept. The same audit v2 rules apply to these words.

H4 adds incremental child records, grouped under one history entry in the workbench. Count actual
operations using child records where `step.preparation_failed` is false, plus ordinary records
without `step` or `composition`. Exclude aggregate wrappers and preparation facts from operation
and denial totals. A missing receipt does not establish that work did not run. Parent completion
can establish an unattempted suffix; a missing parent leaves completion unconfirmed.

Permission checks retain complete requirements, normalized host, verdict, reason, and each
evaluated layer's bounded grant identities. A trace can omit checks after its bound, disclosed by
`truncated`; it is never permission for omitted work. This
evidence comes from the original immutable-snapshot evaluation. See [H4 source and verification
state](../tasks/security-hardening/h4-grouped-history.md) before assuming an installed version
writes these fields.

Two values read differently on lines from builds before 2.0. There, a request that never decoded
records `capabilities` as `["read"]`, and a flow's own record has no `permissions` checks, because
the flow's admission check was dropped when its first step started.

Older versions could retain page content in failed-flow records and browser text in failure
summaries. This correction does not rewrite old files. The reader ignores arbitrary
failure-facts content while preserving the record. Historical summary text remains historical
text. See [H1 source and verification state](../tasks/security-hardening/h1-readable-audit.md)
before assuming an installed version includes the correction.

`observed` has five fields, and a sixth that appears only when it is true:

| Field | Meaning |
| --- | --- |
| `host` | Lowercased governed host attempted or landed on. Never the rest of the URL. |
| `readiness` | `not_applicable`, `loading`, `interactive`, `complete`, or `unknown`. |
| `count` | A bounded measurement named by `summary`. |
| `width`, `height` | Pixel size of a capture. |
| `rebound` | Present, as `true`, when the action used a handle that followed its control through a re-render the page proved. A closed fact: it names no handle, row or link. |

The governed host and an optional normalized target name inside `summary` are the deliberate
page-derived exceptions. The target name is bounded to 80 characters and can be removed by
`privacy.preserve_target_names: false`. Audit never contains paths, queries, fragments, arbitrary
page text, selectors, target handles, form values, filenames, file bytes, scripts, screenshots,
recordings, dialog text, policy payloads, credentials, or model prompts.

## Collect and query

Configure the endpoint collector to tail the file and parse one JSON object per nonblank line. Track file
identity and offsets so rotation or replacement does not duplicate evidence. Apply filesystem
access controls appropriate to operational metadata.

Useful signals include:

- `allowed = false` grouped by `reason`, `tool`, `policy_tier`, and `grant_id`;
- `ghostlight policy simulate` over collected audit when preparing a policy change;
- `effect in (partial, unknown)`, because those outcomes are never replay-safe;
- `denial_id` for the user-to-administrator feedback loop;
- `policy_seq` changes and rollback or invalid-authority events;
- `observed.host` grouped by decision and RAWX set; and
- gaps in endpoint delivery, which indicate collector health rather than browser truth.

Do not join opaque ids to page content or add full-URL collection to Ghostlight. Content capture,
if required, is a separate system with its own consent and retention decision.

## Saving failures and recovery markers (H7)

A terminal result's `history_storage` is `saved` or `unconfirmed`, independently of `status`,
`effect`, and `repeat_safe`. Saved means the writer acknowledged append and synchronization;
unconfirmed means it did not, even if some bytes reached the file. Browser effects are not rolled
back. The current workbench can hold receipts whose durable storage was not confirmed.

The audit record is unchanged. The result an MCP client receives omits `history_storage` when it
is `saved`; read an absent field as `saved`.

When saving recovers, a separate JSONL entry has this shape:

```json
{"audit_gap":{"id":"gap_example","started_at_ms":1,"recovered_at_ms":2,"unconfirmed_receipts":3}}
```

This is a gap marker, not an operation. Its counter covers direct, child, preparation, and
asynchronous receipts. Deduplicate markers by `audit_gap.id`: a sync failure can leave a marker
on disk before its retry. A parent may also carry `unconfirmed_history_steps`; saving it does not
confirm the missing child receipts. Do not count gaps as operations, denials, or successful writes.

Recovery retains no failed receipt queue and performs no delayed backfill. Malformed and oversized
history lines are skipped with explicit omission counts in the human surface and policy simulation.
A crash while storage is unavailable can lose volatile receipts and gap counts before a marker is
saved. This file is neither an atomic record of website effects nor tamper-proof storage.
See [H7 verification](../tasks/security-hardening/h7-audit-health.md) for deployment limits.
