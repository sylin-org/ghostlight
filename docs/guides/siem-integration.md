# Collecting Ghostlight 1.0 audit

Ghostlight appends one payload-free JSON object for every terminal invocation. It does not send
syslog, open a network listener, or call a hosted collector. SIEM delivery belongs to the endpoint's
existing file-collection agent.

## Select the file

Set `GHOSTLIGHT_AUDIT_FILE` to an absolute local path before starting Ghostlight. Without that
variable, `audit.jsonl` sits beside the runtime discovery file.

The orchestrator creates the parent directory when possible, opens the file append-only, writes one
LF-terminated record, and flushes it. The workbench also reconstructs its bounded History view from
this file.

## Record shape

```json
{
  "timestamp_ms": 1786334400000,
  "invocation": "invocation_opaque",
  "workspace": "workspace_opaque",
  "tool": "browser_read_page",
  "capability": "read",
  "authority": "authority_opaque",
  "allowed": true,
  "reason": "permitted",
  "status": "succeeded",
  "effect": "none",
  "summary": "Read 1240 words of page text.",
  "duration_ms": 412,
  "observed": {
    "host": "example.com",
    "readiness": "complete",
    "count": 1240,
    "width": null,
    "height": null
  }
}
```

Fields are typo-closed:

| Field | Meaning |
| --- | --- |
| `timestamp_ms` | Local observation time as Unix milliseconds. |
| `invocation` | Opaque invocation correlation handle. |
| `workspace` | Opaque admitted MCP workspace handle. |
| `tool` | Exact 1.0 catalog tool name. |
| `capability` | `read`, `action`, `write`, or `execute`. |
| `authority` | Opaque immutable authority snapshot id. |
| `allowed` | Final-boundary authority decision. |
| `reason` | Stable closed reason such as `permitted`, `host_denied`, or `runtime_hold`. |
| `status` | Terminal result status. |
| `effect` | `none`, `applied`, `partial`, or `unknown`. |
| `summary` | Ghostlight-authored sentence naming what happened. Page content never authors it. |
| `duration_ms` | Decode to terminal outcome. For a navigation, the time to a settled landing. |
| `observed` | What the action did, measured where it crossed the browser boundary. |

`observed` is a closed set of measurements gathered at that one boundary. Every field is null when
the action could not see it:

| Field | Meaning |
| --- | --- |
| `host` | Host the action landed on, lowercased. Never the path, query, or fragment. |
| `readiness` | `not_applicable`, `loading`, `interactive`, `complete`, or `unknown`. |
| `count` | However many things the action touched. `summary` names what was counted. |
| `width`, `height` | Pixel size of a capture. |

Records written before 1.0 have no `summary`, `duration_ms`, or `observed`; parse them as absent
rather than as an error.

The landed host is the one piece of page-derived text in a record, and it is deliberate: it answers
"where did the agent go", it is already visible in the user's own tab strip, and the identifying
detail of a URL lives after it. There are deliberately no full URLs, paths, queries, fragments,
client names, page text, target descriptions, selectors, form values, filenames, file bytes,
scripts, screenshots, dialog text, policy rules, or model prompts.

## Collection

Configure the endpoint collector to tail the selected JSONL file and parse one object per line.
Use file identity and offsets so rotation does not duplicate evidence. Apply filesystem access
controls appropriate to operational metadata even though payloads are excluded.

Useful high-signal queries include:

- `allowed = false` grouped by `reason` and `tool`;
- `effect in (partial, unknown)` because those outcomes are never replay-safe;
- `status = attention_required` or `reason = runtime_attention`;
- `observed.host` grouped by `capability`, for where write and execute authority actually went;
- `observed.readiness = loading` beside a long `duration_ms`, which is work that never settled;
- changes in managed `invalid_authority` volume; and
- gaps in expected endpoint delivery, which are collector health rather than browser truth.

Do not join opaque ids to page content or inject full-URL collection into Ghostlight. If a
compliance workflow requires content capture, it is a separate system with a separate consent and
retention decision.
