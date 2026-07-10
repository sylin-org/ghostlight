# Ghostlight SIEM integration

Every governed (and ungoverned, when audit is on) tool call produces exactly one JSON
Lines record. This guide covers turning the stream on, the two collection paths (RFC
5424 syslog over UDP, or tailing the JSONL file), the record schema, and paste-ready
starts for Splunk, Microsoft Sentinel, and Elastic.

## Turn it on

    ghostlight config set audit.enabled true
    ghostlight config set audit.destination syslog
    ghostlight config set audit.syslog.address "10.0.0.5:514"

`audit.destination` is one of `file` (default), `stderr`, `syslog`, `none`. Under the
`safe` preset, `audit.enabled` is already true. An organization pins these keys for
everyone via the org policy manifest's `config` block with `"level": "mandatory"` (see
the [compliance team guide](compliance-team.md)).

For file collection instead: leave `audit.destination` at `file`; records append to
`audit.jsonl` in the platform data directory (`%LOCALAPPDATA%\ghostlight\` on Windows,
`~/Library/Application Support/ghostlight/` on macOS, `~/.local/share/ghostlight/` on
Linux), or set an absolute path with `audit.file.path`. One JSON object per line, LF
terminated; open-append-close per record, so rotation-friendly.

## Wire format (syslog)

One RFC 5424 datagram per record, over UDP, payload:

    <134>1 {timestamp} - ghostlight {pid} - - {json}

PRI 134 = facility 16 (local0), severity 6 (info). HOSTNAME, MSGID, and STRUCTURED-DATA
are the RFC nil value `-`. `{timestamp}` is UTC RFC 3339 with millisecond precision.
`{json}` is the record, unchanged from the file format. Example datagram:

    <134>1 2026-07-03T14:32:15.003Z - ghostlight 18104 - - {"event_id":"1c5e...","ts":"2026-07-03T14:32:15.001Z","identity":{"principal":"support-team","resolved_by":"local_file"},"client":{"name":"claude-code","version":"2.1.0"},"tool":"computer","action":"left_click","capability":"action","domain":"app.crm.example.com","decision":"allow","grant_id":"crm-read-write","denial_id":null,"duration_ms":312,"manifest":{"name":"support-team-crm","version":"2026.07.1","hash":"9f31..."},"held":false}

Send failures are logged locally and never break the tool call.

## Record schema (tool calls)

Field order is stable and part of the format. Absent values are `null`, not omitted.

| Field | Type | Meaning |
|---|---|---|
| `event_id` | string | UUID v4, unique per record. |
| `ts` | string | RFC 3339 UTC, millisecond precision. |
| `identity` | object or null | `{principal, resolved_by}` from the active manifest. |
| `client` | object or null | `{name, version}` from the MCP client's initialize. |
| `tool` | string | MCP tool name as received. |
| `action` | string or null | `computer` sub-action (e.g. `left_click`); null otherwise. |
| `capability` | string | `read`, `action`, `write`, `execute`, or `none`. |
| `domain` | string or null | Normalized host of the governed tab at decision time. |
| `decision` | string | `allow`, `deny`, or `shadow_deny` (observe mode's would-deny). |
| `grant_id` | string or null | The grant that resolved an allow. |
| `denial_id` | string or null | Stable `D-` + 8 hex id; matches the message users see. |
| `duration_ms` | number | Dispatch-to-result wall time. |
| `manifest` | object or null | `{name, version, hash}` of the policy in force. |
| `held` | boolean | True when answered with the take-the-wheel pause text. |

Session events share the stream and are distinguishable by an `event` field (and the
absence of `tool`/`decision`): `event` is `session_killed` (panic kill switch),
`manifest_reload` (hot-reload swap), or `user_manifest_ignored` (org policy displaced a
user manifest). Route on `event` presence.

A `license` field may additionally be appended to records in an upcoming release, only
while license state is abnormal (see [PRICING.md](../../PRICING.md); it never affects
behavior).

A `policy_seq` field (a number) is appended to tool-call records under managed policy
(central signed-policy distribution, see the [governance configuration
guide](governance-configuration.md)): the org-signed publish sequence the decision ran
under. Like `license`, it appears only on tool-call records, never on session events, and
never changes behavior. Pivot on it to tie a decision to the exact published policy
version, not just the manifest hash.

## Splunk

UDP input on the syslog port, then extract the JSON payload:

    # inputs.conf
    [udp://514]
    sourcetype = ghostlight:audit

    # props.conf
    [ghostlight:audit]
    KV_MODE = none
    REPORT-json = ghostlight-json

    # transforms.conf
    [ghostlight-json]
    REGEX = -\s-\s(\{.*\})$
    FORMAT = json_payload::$1

Then in SPL:

    sourcetype="ghostlight:audit"
    | spath input=json_payload
    | search decision="deny" OR decision="shadow_deny"
    | stats count by identity.principal, domain, capability, denial_id

(For file collection, a Universal Forwarder monitoring `audit.jsonl` with
`sourcetype=_json` needs no extraction at all.)

## Microsoft Sentinel

Land the syslog stream via the Azure Monitor Agent (or rsyslog forwarder) into the
Syslog table with facility `local0`, then parse in KQL:

    Syslog
    | where Facility == "local0" and ProcessName == "ghostlight"
    | extend record = parse_json(extract(@"-\s-\s(\{.*\})$", 1, SyslogMessage))
    | where record.decision in ("deny", "shadow_deny")
    | project TimeGenerated, principal = record.identity.principal,
              domain = record.domain, capability = record.capability,
              denial_id = record.denial_id, manifest = record.manifest.name

## Elastic

Filebeat/Logstash UDP syslog input, then decode the trailing JSON:

    # logstash pipeline
    input { udp { port => 514 } }
    filter {
      grok { match => { "message" => "- - %{GREEDYDATA:payload}$" } }
      json { source => "payload" target => "ghostlight" }
    }
    output { elasticsearch { index => "ghostlight-audit-%{+YYYY.MM.dd}" } }

(For file collection, Filebeat's `json.keys_under_root: true` on `audit.jsonl` is
simpler.)

## Alerts worth having on day one

- `decision:"deny"` rate spike per principal or domain (agent fighting a policy, or a
  policy that is wrong).
- Any `event:"session_killed"` (a human hit the panic switch; someone should ask why).
- Any `event:"user_manifest_ignored"` (a user-supplied policy was displaced by org
  policy; expected at rollout, interesting later).
- `shadow_deny` volume trending to zero in observe mode (the signal that the policy is
  ready to enforce; see the compliance guide, step 3).
- Records with a `license` field, once license verification ships (compliance noise by
  design; it means the deployment's license state needs attention).
