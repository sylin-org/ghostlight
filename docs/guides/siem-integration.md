# Collect Ghostlight 1.0 audit

Ghostlight appends one content-minimized JSON object for every terminal invocation. It does not
upload audit, send syslog, open a network listener, or call a hosted collector. SIEM delivery uses
the endpoint's existing file-collection agent.

## Select the file

Set `GHOSTLIGHT_AUDIT_FILE` to an absolute local path before starting Ghostlight. Without it,
`audit.jsonl` sits beside runtime discovery. Ghostlight creates the parent directory when possible,
opens the file append-only, writes one LF-terminated record, and flushes it. The workbench rebuilds
its bounded History view from the same file.

## Record shape

```json
{
  "timestamp_ms": 1786334400000,
  "invocation": "invocation_opaque",
  "workspace": "workspace_opaque",
  "tool": "browser_fill_form",
  "capabilities": ["read", "write"],
  "authority": "authority_opaque",
  "policy_seq": 12,
  "allowed": false,
  "reason": "capability_denied",
  "policy_observed": false,
  "policy_mode": "enforce",
  "policy_rule": "capability",
  "denial_id": "D-opaque",
  "policy_tier": "managed",
  "grant_id": "support-sites",
  "status": "blocked",
  "effect": "none",
  "summary": "Blocked: this session may not take that kind of action.",
  "duration_ms": 4,
  "observed": {
    "host": "support.example.com",
    "readiness": null,
    "count": null,
    "width": null,
    "height": null
  },
  "channel": "mcp"
}
```

| Field | Meaning |
| --- | --- |
| `timestamp_ms` | Local observation time as Unix milliseconds. |
| `invocation`, `workspace` | Opaque correlation handles. |
| `tool` | Catalog tool name, `unknown_tool` for an unrecognized request name, or the service's `browser_landing` event. |
| `capabilities` | Complete independent RAWX requirement set. Empty is valid. |
| `authority` | Opaque immutable authority snapshot id. |
| `policy_seq` | Signed managed publish sequence, when active. |
| `allowed`, `reason` | Final-boundary decision and stable reason. |
| `policy_observed` | True when observe mode shadowed a denial without blocking. |
| `policy_mode` | Effective `observe` or `enforce`, when policy decided. |
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
| `permissions` | Bounded actual evaluation checks and a `truncated` flag; includes allowed work as well as refusals. |
| `channel` | `mcp` or `cli`; attribution, not authority. |

The optional singular `capability` field exists only so 1.0 can read historical pre-ADR-0121
records. New records write `capabilities`.

The H1 source correction replaces arbitrary failure-facts retention with typed metadata. For
example, a primitive browser error records `{"reason":"browser_primitive_failed"}` while its
arbitrary error description stays in the permitted client result. Deadline metadata can retain
`before_dispatch`; workspace and recovery failures can retain a closed `cause`. Query the top-level
policy fields for rule attribution and `status`/`effect` for the terminal outcome. A missing
`refusal_facts` is not a success signal: composed and measured outcomes have their own summaries.

H4 adds incremental child records, grouped under one history entry in the workbench. Count actual
operations using child records where `step.preparation_failed` is false, plus ordinary records
without `step` or `composition`. Exclude aggregate wrappers and preparation facts from operation
and denial totals. A missing receipt does not establish that work did not run. Parent completion
can establish an unattempted suffix; a missing parent leaves completion unconfirmed.

Permission checks retain complete requirements, normalized host, verdict, reason, and each
evaluated layer's bounded grant identities and mode. `request_restricted` records restriction
presence; `request_evaluated` says whether this decision reached those checks. A trace can omit
checks after its bound, disclosed by `truncated`; it is never permission for omitted work. This
evidence comes from the original immutable-snapshot evaluation. See [H4 source and verification
state](../tasks/security-hardening/h4-grouped-history.md) before assuming an installed version
writes these fields.

Older versions could retain page content in failed-flow records and browser text in failure
summaries. This correction does not rewrite old files. The reader ignores legacy arbitrary
failure-facts content while preserving the historical record. Historical summary text remains
historical text. See [H1 source and verification state](../tasks/security-hardening/h1-readable-audit.md)
before assuming an installed version includes the correction.

`observed` has exactly five fields:

| Field | Meaning |
| --- | --- |
| `host` | Lowercased governed host attempted or landed on. Never the rest of the URL. |
| `readiness` | `not_applicable`, `loading`, `interactive`, `complete`, or `unknown`. |
| `count` | A bounded measurement named by `summary`. |
| `width`, `height` | Pixel size of a capture. |

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
- `policy_observed = true` when preparing an enforce rollout;
- `effect in (partial, unknown)`, because those outcomes are never replay-safe;
- `status = attention_required` or `reason = runtime_attention`;
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
