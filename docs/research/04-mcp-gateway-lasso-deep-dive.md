# MCP Gateway Deep-Dive: Lasso Security mcp-gateway

**Date:** 2026-07-01 · **Track:** Governance · **Source:** research agent (source-level, verbatim report)

> A content-sanitization/DLP proxy pattern, useful as a contrast to our access-control model,
> plus a few reusable implementation patterns.

**Repo:** `github.com/lasso-security/mcp-gateway` (Python, PyPI `mcp-gateway`, v1.2.1)
**One-line identity:** a **plugin-based proxy/orchestrator** between an MCP client and one or
more downstream ("proxied") MCP servers, applying guardrail + tracing plugins to sanitize
requests/responses in-flight.

## Architecture (verified from source)
- Config is **JSON, not YAML**: it reuses the client's own `mcp.json` /
  `claude_desktop_config.json`. The gateway is an entry in `mcpServers`, nesting downstream
  servers under a **`servers`** key inside its own entry.
- Two protocol boundaries: client ↔ gateway (MCP/stdio) and gateway ↔ proxied servers (spawned
  as child MCP servers).

## (a) Tool visibility / filtering: essentially NONE
Most important negative finding. **Lasso does NOT do per-user or per-role tool masking.**
`register_proxied_capabilities()` exposes every proxied tool individually, namespaced
`f"{server_name}_{tool.name}"`. There is **no allowlist/denylist of tool names, no role concept,
no identity concept** in config. Filtering, if any, happens **per-call at runtime** via guardrail
plugins inspecting args/responses, not at advertisement time. (README's `run_tool`/`get_metadata`
two-tool facade is stale; current source registers each tool individually and defines only
`get_metadata`.)

**Contrast with browser-mcp:** our manifest-driven tool-advertisement filtering + identity-bound
grants + `computer` sub-action classification is a capability Lasso lacks entirely. Lasso is a
content-sanitization proxy, not an access-control/visibility governor.

## (b) ALLOW / DENY (guardrail block behavior)
Blocking is **plugin-driven and binary at the message level**, in Python code, not config. There
is **no declarative allow/deny rule syntax** ("deny tool X on domain Y" is not expressible). The
only knob is which plugins are enabled; each decides block/mask/allow internally.
- A guardrail's `process_request(context) -> Optional[Dict]` returns modified args to allow, or
  `None` to **block**.
- `PluginManager` order: request → tracing then guardrails; response → guardrails then tracing.
- `sanitize_request` returns `None` on plugin error → **fail-CLOSED on request**; response errors
  log and return original → **fail-open on response**.
- Per-plugin: **basic** masks only; **presidio** masks only; **lasso** is the only plugin that
  truly blocks (calls a cloud classify API; on `violations_detected` blocks; response-block
  returns `"[ERROR] Response blocked by security guardrail: …"`).

## (c) AUDIT / logging
Audit is delivered **only via the `xetrack` tracing plugin** (no built-in file/syslog/http audit
subsystem). `xetrack`: SQLite (DuckDB) + optional log files; logs `call_id`, `arguments`,
`response_type`, `content`, `mime_type`. Env config `XETRACK_DB_PATH`, `XETRACK_LOGS_PATH`,
`XETRACK_FLATTEN_ARGUMENTS/RESPONSE`, etc. **No identity/user field, no grant-ID, no domain, no
deny-reason** in the schema, a notable gap vs. our audit requirements.

## (d) Config schema + plugin selection
CLI (argparse): `--mcp-json-path` (required), `-p/--plugin` (repeatable),
`--enable-guardrails`/`--enable-tracing` (deprecated), `--scan`. Plugins are enabled via a
type→[names] map; empty/`all` enables all of that type; discovery via `@register_plugin`
decorator. Per-plugin `load(config)` exists but is currently passed an empty dict. Plugins are
configured via **env vars** today.

## (e) Built-in plugins

| Key | Type | Behavior | Config |
|---|---|---|---|
| `basic` | guardrail | **Masks** 12 secret types → placeholders (GitHub PAT, AWS/GCP keys, JWT, Slack, etc.). Never blocks. | none |
| `presidio` | guardrail | **Masks/anonymizes** PII (CREDIT_CARD, EMAIL, PHONE, SSN, MEDICAL_LICENSE, US_PASSPORT…). Never blocks. | `pii_entities`; extra install |
| `lasso` | guardrail | PII+token masking, custom policy, prompt-injection, harmful content. **Only plugin that BLOCKS** (cloud API). | env `LASSO_API_KEY` |
| `xetrack` | tracing | **Audit logging** to SQLite/DuckDB + logs. Observe-only. | env `XETRACK_*` |

Plus a separate `security_scanner/` (`--scan`) that statically scans MCP servers for tool-
poisoning (supply-chain scanner, not a runtime guardrail).

## Bottom line for browser-mcp
Lasso's model is **orthogonal to ours**: a *content-sanitization/DLP proxy*, configured by
*enabling plugins*, not an *access-control layer*. It has no per-user tool visibility filtering,
no declarative allow/deny, and no identity/domain/grant concept, all of which are our
differentiators. Runtime plugin block is **fail-closed on request, fail-open on response**
(a design precedent).

**Reusable ideas worth borrowing:** the **`@register_plugin` auto-discovery + type/name registry**
pattern (natural fit for our audit *destinations*); the **secret-masking regex table** (12
patterns to port for response/param redaction, PHI-aware logging); the **tracing-vs-guardrail
ordering discipline** (tracing before guardrails on request, reversed on response).

**Caveats:** README's `run_tool` facade is stale vs source; `load()` structured config is not yet
functional; source was read via raw.githubusercontent (gh CLI unauthenticated), so a few multi-
line regexes were summarized, not byte-copied.

### Citations
- Repo/source: `github.com/lasso-security/mcp-gateway` (`config.py`, `gateway.py`, `server.py`,
  `sanitizers.py`, `plugins/base.py`, `plugins/manager.py`,
  `plugins/guardrails/{basic,lasso,presidio}.py`, `plugins/tracing/xetrack.py`, `pyproject.toml`)
