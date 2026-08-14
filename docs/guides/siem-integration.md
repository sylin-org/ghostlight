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
| `tool` | Exact 1.0 catalog tool name. |
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
| `duration_ms` | Decode-to-terminal elapsed time. |
| `observed` | Closed governed landing and measurement facts. |
| `channel` | `mcp` or `cli`; attribution, not authority. |

The optional singular `capability` field exists only so 1.0 can read historical pre-ADR-0121
records. New records write `capabilities`.

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

Configure the endpoint collector to tail the file and parse one JSON object per line. Track file
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
