# Governed Browser Automation MCP Server

## Design Specification v0.1

**Author:** Leonardo Botinelly / Kintsugi Architecture  
**Date:** 2026-07-01  
**Status:** Draft  
**License:** TBD (intended open-source)

---

## 1. Purpose

This document specifies a single-binary MCP server that gives AI coding agents (Claude Code, Cursor, etc.) governed access to the user's **own authenticated Chromium browser session** — enabling them to observe and interact with web applications the user is already logged in to.

The system is an **unconstrained engine with an optional governance overlay**. The engine exposes the full browser-automation capability surface with no built-in limits; governance is a *separable* layer that can gate that surface — set by enterprise policy, chosen by the user, or absent entirely. The same binary serves an unrestricted personal tool and a default-deny enterprise deployment with no code changes.

### 1.1. Core Concerns (independent lifecycles)

- **Engine** — MCP↔CDP translation and the complete browser-automation capability surface (navigation, input, reads, extraction, scripting). Hosts the MCP server on stdio and the native-messaging host for the extension. **It has no built-in limits and no opinions about policy** — capability is its only job.
- **Governance overlay** — an optional, separable layer (co-hosted in the binary, never in the extension) that gates, masks, classifies (read/write), and audits engine calls per a capability manifest. **In all-open mode the overlay is a pass-through** and the engine's full surface is exposed.
- **Identity** — resolution of "who is connecting" into "which manifest (if any) applies." Pluggable; resolved at connection time. In enterprise the *deployment channel is* the identity resolution; in personal use there is no identity step.

### 1.2. Operating Postures

Three postures, one engine, no code changes:

- **All-open (personal default)** — no manifest; the overlay is inert. A first-class, fully-supported unrestricted browser-automation MCP. *Not* "enterprise minus governance" — it must be excellent on its own terms.
- **User-chosen** — the user opts into whatever limits *they* want ("keep the agent to these sites"). Governance here is a user-facing control feature, not an IT mandate.
- **Policy-enforced (enterprise)** — manifest pushed via Intune/GPO to managed machines. Default-deny, full audit, identity-bound. The deployment channel is the identity resolution; no governance infrastructure to build.

### 1.3. Design Principles

1. **Unconstrained engine, governance as overlay.** The engine enables full capability; access control is a separable overlay with its own lifecycle. The engine never bakes in policy.
2. **"All-open" is first-class.** Zero restrictions is a valid, supported, default configuration — excellent on its own terms, not a degraded mode.
3. **The user's context is sacred.** We attach to the user's real, authenticated, live browser session. We never relocate their work to a cloud/fresh/separate-profile browser to gain a technical property; where such a technique is unavoidable it is at most an *opt-in* deployment profile, never the default.
4. **Delight is layered and composable** (mirroring the architecture):
   - **L0 — base capability delight** *(engine; every persona, all-open included)* — automating monotonous browser work in the user's own context: fast, token-lean, install-just-works.
   - **L1 — control delight** *(overlay; user-chosen)* — confidence the agent stays where the user wants.
   - **L2 — governance delight** *(overlay; enterprise)* — the org can say *yes* to a powerful tool because it's default-deny, audited, identity-bound.

   Governance-as-delight is real but **composite and additive (L0 + L2)** — never a substitute for L0. L0 is load-bearing for every persona, so the engine is built to be excellent for everyone, and the overlay UX must itself be delightful, not merely correct.
5. **Separation of concerns.** Engine, overlay, identity, and audit have independent lifecycles; a change to one must not force a change to another.
6. **Prior art is a concern-surface, not a feature catalog.** Competitor and standards research informs our design by surfacing hazards and questions (see §1.4); we do not import paradigms — anything that moves away from the user's context (Principle 3) is rejected regardless of how common it is.

### 1.4. Prior Art and Positioning

This design is informed by, but architecturally distinct from, the following:

| Project | Relationship |
|---|---|
| [open-claude-in-chrome](https://github.com/noemica-io/open-claude-in-chrome) (Noemica) | Reference implementation for extension-based MCP-to-CDP pipeline and tool schemas. Fork-and-rewrite origin. |
| [agent-browser](https://github.com/vercel-labs/agent-browser) (Vercel) | Precedent for domain allowlists and action policies in browser automation. Different execution model (Playwright CLI, not extension-based). |
| [chrome-devtools-mcp](https://github.com/ChromeDevTools/chrome-devtools-mcp) (Google) | Precedent for headless browser MCP. Different execution model (Puppeteer, separate browser instance). |
| [auto-browser](https://github.com/LvcidPsyche/auto-browser) | Precedent for HIPAA/SOC2 compliance templates and operator identity in browser automation. Different execution model (Playwright + noVNC). |
| MCP Gateway ecosystem (TrueFoundry, MintMCP, Cloudflare, etc.) | Precedent for MCP-level RBAC, tool allowlisting, and audit. Not browser-specific. |
| Okta XAA / Entra Agent Identity | Precedent for identity-governed agent access. Protocol-layer; not an implementation. |

**The gap this project fills:** No existing project combines extension-based browser automation (user's authenticated session), identity-bound capability projection (enterprise directory → per-connection manifest), tool-level r/w classification, default-deny posture, and healthcare-grade audit — in a single deployable artifact.

---

## 2. Architecture

### 2.1. Process Model

```
MCP Client ←—stdio—→ Binary ←—native messaging—→ Extension ←—CDP—→ Browser
   (1)                  (2)                          (3)            (4)
```

Three processes, two protocol boundaries. Compare to the reference implementation's five processes and four protocol boundaries (MCP Client → stdio → Node MCP server → TCP → Node native host → native messaging → Extension → CDP → Browser).

| Component | Runtime | Responsibility |
|---|---|---|
| **MCP Client** | External (Claude Code, etc.) | Sends MCP tool calls over stdio, receives results. |
| **Binary** | Rust, single portable executable | MCP protocol handling (stdin/stdout). Native messaging to extension. Policy loading and enforcement. Audit record generation. Screenshot compression. |
| **Extension** | Manifest V3 JavaScript (Chromium) | CDP command execution. Event forwarding (console, network). Tab group management. No policy logic. |
| **Browser** | User's Chromium browser | The automation target. Runs under the user's existing session, SSO cookies, and profile. |

### 2.2. Communication Protocols

**Binary ↔ MCP Client:** MCP over stdio. JSON-RPC 2.0 per the MCP specification. The binary is launched as a subprocess by the MCP client.

**Binary ↔ Extension:** Chromium Native Messaging. 4-byte little-endian length prefix + UTF-8 JSON payload. The browser launches the binary as a native messaging host when the extension connects. The binary serves both roles simultaneously: native messaging host (for the extension) and MCP server (for the client on stdio). This is the key architectural simplification — a single process at the center of the star.

**Extension ↔ Browser:** Chrome DevTools Protocol via `chrome.debugger` API. The extension attaches to target tabs and dispatches CDP commands.

### 2.3. Startup Sequence

1. MCP client (Claude Code) launches the binary as a subprocess with stdio pipes.
2. Binary initializes MCP server on stdin/stdout.
3. Binary loads the capability manifest (see §4) from the configured source.
4. Binary registers as a native messaging host and waits for the extension to connect.
5. Extension service worker starts (or is woken by keepalive alarm), discovers the native messaging host, connects.
6. Binary receives the extension connection, acknowledges.
7. Binary computes the advertised tool set from the manifest (see §5.1) and responds to `tools/list`.
8. MCP client sees the available tools and begins issuing calls.

### 2.4. Extension Design

The extension is deliberately thin. It is a CDP executor and event buffer, not a policy engine.

**Responsibilities:**
- Maintain a debugger attachment to target tabs (with `Emulation.setDeviceMetricsOverride` for coordinate normalization).
- Execute CDP commands received from the binary and return results.
- Buffer console messages and network requests for retrieval.
- Manage the "MCP" tab group (create, track, recover after service worker restart).
- Send keepalive alarms to survive Manifest V3 service worker lifecycle.

**Non-responsibilities:**
- No domain checking. The binary decides; the extension executes.
- No tool classification. The binary filters; the extension doesn't know about tiers.
- No audit logging. The binary logs; the extension is untrusted execution.
- No identity resolution. The extension has no concept of "who."

**Service worker resilience:** The extension must recover gracefully from service worker termination (a Manifest V3 inevitability). State recovery on wake:
- Query `chrome.tabGroups` for existing "MCP" group and rebuild the tab set.
- Reconnect to native messaging host.
- Re-attach debugger to tracked tabs.
- All tab/group state queries go to the live `chrome.tabs`/`chrome.tabGroups` API, never to in-memory sets.

---

## 3. Tool Taxonomy

### 3.1. Included Tools

The following 13 tools are derived from the Claude in Chrome tool schemas. Tool names, parameter signatures, and descriptions are preserved exactly to inherit the model's trained behavior.

#### Observe Tier

Tools that read browser state without modifying application state.

| Tool | Description | Notes |
|---|---|---|
| `navigate` | Navigate to URL, back, forward | Primary policy enforcement point. Domain checked pre- and post-navigation. |
| `computer` (action: `screenshot`) | Capture visible page | JPEG only. Quality 55, fallback to 30 if > 500KB. |
| `computer` (action: `wait`) | Wait for specified duration | Passive. No state change. |
| `read_page` | Accessibility tree with element refs | Returns interactive elements and their ref IDs. |
| `get_page_text` | Extract article/main text content | Text-only extraction. |
| `find` | Find elements by text/attributes | Read-only DOM query. |
| `read_console_messages` | Console output (filtered) | Buffered event replay. |
| `read_network_requests` | Network activity | Buffered event replay. |
| `tabs_context` | Get tab group context | Session metadata. |

#### Mutate Tier

Tools that change browser or application state. Only available when the grant specifies `"access": "mutate"` for the current domain.

| Tool | Description | Notes |
|---|---|---|
| `computer` (actions: `left_click`, `right_click`, `double_click`, `type`, `key`, `scroll`, `hover`, `drag`) | Mouse, keyboard, scroll interactions | The `computer` tool is a single MCP tool with an `action` parameter. Classification is per-action, enforced in the binary. |
| `form_input` | Set form values by element ref | Shadow DOM traversal for web components. |
| `javascript_tool` | Execute JS in page context | Always Mutate. No exceptions. Requires explicit grant. |
| `tabs_create` | Create new tab | Opens a URL; subject to domain check. |

#### Manage Tier

Session housekeeping. Always available regardless of access tier.

| Tool | Description | Notes |
|---|---|---|
| `resize_window` | Resize browser window | No security implication. |
| `update_plan` | Present plan to user (auto-approved) | Informational pass-through. |

### 3.2. Excluded Tools

The following tools from the reference implementation are excluded:

| Tool | Reason |
|---|---|
| `gif_creator` | Stub in reference implementation. No functional code. |
| `shortcuts_list` | Stub. |
| `shortcuts_execute` | Stub. |
| `switch_browser` | Stub. |
| `upload_image` | Niche. Can be added in a future version if demand warrants. |

### 3.3. The `computer` Tool Split

The `computer` tool presents a classification challenge: it is a single MCP tool with an `action` parameter that spans both Observe and Mutate behaviors. The binary enforces classification per-action:

- **Observe actions:** `screenshot`, `wait`
- **Mutate actions:** `left_click`, `right_click`, `double_click`, `type`, `key`, `scroll`, `hover`, `drag`

`scroll` is classified as Mutate despite not modifying application state, because it is an input action dispatched via CDP `Input.dispatchMouseEvent`. Splitting `computer` sub-actions across tiers at the MCP schema level would break the trained tool interface. Instead, the binary inspects the `action` parameter and applies tier enforcement internally. If the current domain's grant is Observe-only, a `computer` call with `action: "left_click"` is denied; a `computer` call with `action: "screenshot"` is permitted.

---

## 4. Capability Manifest

### 4.1. Schema

```json
{
  "schema": 1,
  "identity": {
    "resolved_by": "managed_config",
    "principal": "GEISINGER\\jdoe",
    "groups": ["EA-ServiceNow-RW", "Research-External-RO"],
    "resolved_at": "2026-07-01T14:30:00Z"
  },
  "grants": [
    {
      "id": "servicenow-full",
      "domains": ["servicenow.geisinger.org", "*.service-now.com"],
      "access": "mutate",
      "tools": null,
      "description": "Full automation access to ServiceNow"
    },
    {
      "id": "epic-restricted",
      "domains": ["epic.geisinger.org", "mychart.geisinger.org"],
      "access": "mutate",
      "exclude_tools": ["javascript_tool"],
      "description": "EHR automation without arbitrary JS execution"
    },
    {
      "id": "research-external",
      "domains": ["*.gartner.com", "*.forrester.com", "*.ieee.org", "scholar.google.com"],
      "access": "observe",
      "description": "Read-only access for asset capability research"
    }
  ],
  "defaults": {
    "unlisted_domains": "deny",
    "screenshot_format": "jpeg",
    "screenshot_quality": 55,
    "screenshot_fallback_quality": 30,
    "screenshot_max_bytes": 512000,
    "page_load_timeout_ms": 10000,
    "max_concurrent_tabs": 5
  },
  "audit": {
    "enabled": true,
    "destination": "syslog",
    "syslog_facility": "local0",
    "include_tool_parameters": false,
    "include_screenshots_in_log": false,
    "include_page_text_in_log": false,
    "log_denials": true,
    "log_successful_calls": true
  }
}
```

### 4.2. Field Definitions

**`schema`** — Integer. Manifest schema version. The binary validates this before proceeding.

**`identity`** — Object. Metadata about how the manifest was resolved. Informational; included in audit records. Not used for authorization decisions (the manifest *is* the authorization decision).

- `resolved_by` — Enum: `"managed_config"`, `"local_file"`, `"environment"`, `"http"`. How the binary obtained this manifest.
- `principal` — String. The resolved identity (e.g., UPN, SAM account name). Included in audit records.
- `groups` — Array of strings. The group memberships that produced this grant set. Informational.
- `resolved_at` — ISO 8601 timestamp. When the manifest was resolved.

**`grants`** — Array of grant objects. Each grant authorizes access to a set of domains at a specified tier.

- `id` — String. Human-readable identifier for this grant. Used in audit records and denial messages.
- `domains` — Array of domain patterns. Wildcards: `*.example.com` matches `foo.example.com` and `bar.baz.example.com` but not `example.com`. Use `["example.com", "*.example.com"]` for both. Patterns are matched against the hostname of the URL, case-insensitive.
- `access` — Enum: `"observe"`, `"mutate"`. The maximum tier available for matched domains.
- `tools` — Array of tool names (positive list), or `null` for "all tools in this tier." Mutually exclusive with `exclude_tools`.
- `exclude_tools` — Array of tool names (negative list). All tools in the tier *except* these. Mutually exclusive with `tools`.
- `description` — String. Human-readable description. Included in denial messages.

**`defaults`** — Object. Global settings.

- `unlisted_domains` — Enum: `"deny"`, `"observe"`, `"mutate"`. Action when a domain matches no grant. Enterprise deployments: `"deny"`. Personal use: `"observe"` or `"mutate"`.
- Screenshot and timeout settings as documented.
- `max_concurrent_tabs` — Integer. Maximum tabs in the MCP tab group.

**`audit`** — Object. Audit configuration.

- `destination` — Enum: `"file"`, `"syslog"`, `"http"`, `"stderr"`, `"none"`.
- `include_tool_parameters` — Boolean. Whether tool call parameters are included in audit records. Default `false` in healthcare (parameters may contain PHI from form fills).
- `include_screenshots_in_log` — Boolean. Whether screenshot data is included in audit records. Default `false` (screenshots may capture PHI).
- `log_denials` — Boolean. Whether denied tool calls are logged. Should always be `true` in enterprise.

### 4.3. Grant Resolution

When a tool call arrives, the binary resolves the applicable grant:

1. Determine the current tab's URL (from the extension, not from the tool call parameters).
2. Extract the hostname.
3. Iterate grants in order. First matching domain pattern wins.
4. If no grant matches, apply `defaults.unlisted_domains`.
5. Check: does the grant's tier include the requested tool (per §3)?
6. Check: is the tool excluded by `exclude_tools` or absent from `tools`?
7. If all checks pass, dispatch. Otherwise, deny with a structured error.

Grant order matters: more specific grants should appear before broader ones. `epic.geisinger.org` (with `exclude_tools: ["javascript_tool"]`) should precede `*.geisinger.org` (if such a grant exists).

### 4.4. Manifest Sources

The binary accepts a `--manifest` flag or `BROWSER_MCP_MANIFEST` environment variable specifying the manifest source. Supported sources:

| Source | Format | Use Case |
|---|---|---|
| `file:///path/to/manifest.json` | Local JSON file | Development, personal use, simple enterprise (Intune file push). |
| `managed://` | `chrome.storage.managed` | Enterprise Chrome policy. IT pushes via GPO/Intune. Binary reads from the extension's managed storage on connection. |
| `env://VARIABLE_NAME` | JSON in an environment variable | CI/CD, container deployments, Claude Code config. |
| (no manifest) | Built-in default | Personal use. Equivalent to `unlisted_domains: "observe"`, audit to stderr. |

The `http://` and `https://` sources are explicitly excluded from v1 to avoid a runtime network dependency (see §8).

---

## 5. Policy Enforcement

### 5.1. Tool Advertisement

At MCP connection time, after loading the manifest, the binary computes the set of tools to advertise via `tools/list`:

1. Start with Manage-tier tools (always included).
2. If any grant has `access: "observe"` or `access: "mutate"`, include all Observe-tier tools.
3. If any grant has `access: "mutate"`, include all Mutate-tier tools *unless* every mutate grant excludes them.
4. Tools excluded by *all* grants that would otherwise include them are omitted.

The result: Claude Code (or any MCP client) only sees tools it can plausibly use. If no grant includes Mutate, the client never learns that `form_input` exists.

**Note:** Tool advertisement is a visibility optimization, not a security boundary. The per-call enforcement (§5.3) is authoritative. A client that somehow sends a call for an unadvertised tool still hits per-call checks.

### 5.2. Pre-Navigation Enforcement

On `navigate` with a URL:

1. Parse the URL. Extract hostname.
2. Check hostname against all grant domain patterns.
3. If no match and `unlisted_domains` is `"deny"`: return a structured denial.
4. If match found: proceed with navigation.
5. After page load (or after timeout): check the *final* URL (post-redirect). If the final hostname does not match any grant (and `unlisted_domains` is `"deny"`): navigate to `about:blank`, return a denial including the redirect chain.

For `navigate` with `"back"` or `"forward"`: the binary cannot predict the destination URL. It allows the navigation, then applies the post-navigation check on the resulting URL.

### 5.3. Per-Call Enforcement

On every tool call (not just `navigate`):

1. Query the extension for the current tab's URL.
2. Resolve the applicable grant (§4.3).
3. Check: is the requested tool permitted by the resolved grant and tier?
4. If denied: return a structured error. Log the denial (if `audit.log_denials`).
5. If permitted: dispatch the command to the extension.
6. On response: log the successful call (if `audit.log_successful_calls`).

**Why check on every call:** The browser state can change between calls. A user might click a link that navigates to a different domain. A redirect might land on an unauthorized domain. The per-call check catches drift that pre-navigation alone would miss.

### 5.4. `computer` Sub-Action Enforcement

For `computer` tool calls, the binary inspects the `action` parameter:

1. If `action` is `screenshot` or `wait`: classify as Observe. Requires the current domain's grant to be at least `"observe"`.
2. If `action` is any other value: classify as Mutate. Requires the current domain's grant to be `"mutate"`.
3. Apply grant-level tool checks (`tools`/`exclude_tools`) against the string `"computer"`, not against the action name.

### 5.5. Denial Response Format

Denied tool calls return a structured MCP error:

```json
{
  "type": "text",
  "text": "DENIED: Tool 'form_input' requires 'mutate' access on domain 'epic.geisinger.org'. Current grant 'epic-restricted' excludes 'javascript_tool' but permits 'form_input'. However, the current domain grant is 'observe' only. Contact your administrator to request elevated access."
}
```

The denial message is informative enough for the AI agent to adjust its approach (try a different tool, report to the user) without leaking security-sensitive details (no manifest paths, no group names, no internal hostnames beyond what the agent already knows).

---

## 6. Screenshot Pipeline

Screenshots are the highest-bandwidth artifact in the MCP conversation. Unmanaged, they exhaust context windows and API request limits.

### 6.1. Compression Strategy

All screenshots use JPEG format. PNG is never used (a single Retina PNG can be 5-10MB as base64).

1. Capture via CDP `Page.captureScreenshot` with `format: "jpeg"`, `quality: [defaults.screenshot_quality]`, `optimizeForSpeed: true`.
2. If the base64 result exceeds `defaults.screenshot_max_bytes`: recapture with `quality: [defaults.screenshot_fallback_quality]`.
3. Return the compressed result.

### 6.2. Coordinate Normalization

On debugger attachment, the binary instructs the extension to call `Emulation.setDeviceMetricsOverride` with `deviceScaleFactor: 1`. This normalizes the coordinate space so that screenshot pixel dimensions match CDP input dispatch coordinates, regardless of display DPI.

### 6.3. Screenshot-per-Action Policy

The reference implementation returns a screenshot after every `computer` action (click, type, key, etc.). The official Claude in Chrome extension does *not* — it only returns screenshots on explicit `screenshot` and `scroll` actions.

This binary follows the official behavior: screenshots are returned only on `screenshot` and `scroll` actions. For other `computer` actions, the tool returns a text confirmation of the action taken, and the agent can request a screenshot separately. This reduces context consumption by approximately 10x in multi-step workflows.

---

## 7. Audit Model

### 7.1. Audit Record Schema

Every tool call (permitted or denied) produces a structured audit record:

```json
{
  "timestamp": "2026-07-01T14:32:15.003Z",
  "event_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "identity": {
    "principal": "GEISINGER\\jdoe",
    "resolved_by": "managed_config"
  },
  "tool": "form_input",
  "domain": "servicenow.geisinger.org",
  "url": "https://servicenow.geisinger.org/incident.do?sys_id=abc123",
  "grant_id": "servicenow-full",
  "access_tier_required": "mutate",
  "access_tier_granted": "mutate",
  "result": "permitted",
  "duration_ms": 342,
  "parameters": null,
  "screenshot": null
}
```

### 7.2. Sensitive Field Handling

**`parameters`** — Only populated if `audit.include_tool_parameters` is `true`. In healthcare deployments, tool parameters may contain PHI (e.g., text typed into a patient record via `form_input`, or JS code that reads patient data via `javascript_tool`). Default: `null` (not logged).

**`screenshot`** — Only populated if `audit.include_screenshots_in_log` is `true`. Screenshots may capture PHI visible on the page. Default: `null` (not logged).

**`url`** — Always logged. The URL itself may contain PHI (e.g., patient IDs in query strings). Organizations requiring URL redaction should implement it at the SIEM/log aggregation layer, not in the binary. The binary logs what it sees; downstream systems apply retention and redaction policy.

### 7.3. Audit Destinations

| Destination | Behavior |
|---|---|
| `file` | Append JSON lines to a local file. Path configured via `audit.file_path`. |
| `syslog` | Send via syslog protocol (RFC 5424). Facility configurable. Feeds Splunk, Sentinel, etc. |
| `http` | POST JSON to an HTTP endpoint. For dedicated audit services. |
| `stderr` | Print to stderr. For development and debugging. |
| `none` | No audit logging. Not recommended for enterprise. |

### 7.4. Trust Boundary

Audit records are generated by the binary, not the extension. The extension is untrusted execution — it executes CDP commands and returns results, but it does not log. If the extension is tampered with (a modified content script, an injected response), the binary still logs what it dispatched and what came back. The audit trail is as trustworthy as the binary.

---

## 8. Deployment

### 8.1. Enterprise Deployment (Intune/GPO)

Three artifacts, all pushed through existing IT channels:

**1. Binary.** A single executable (no runtime dependencies) deployed to a well-known path (e.g., `C:\Program Files\BrowserMCP\browser-mcp.exe` on Windows, `/opt/browser-mcp/browser-mcp` on Linux/macOS). Pushed via Intune app deployment or SCCM package.

**2. Extension.** A Chromium extension (unpacked folder or .crx). Force-installed via Chrome enterprise policy (`ExtensionInstallForcelist`) or Edge policy equivalent. The extension's native messaging host manifest points to the binary path.

**3. Capability manifest.** Pushed as a managed extension configuration via Chrome's `ExtensionSettings` policy (`managed_storage` schema), or as a signed JSON file at a well-known path. The manifest is role-specific — IT generates manifests per AD group and pushes them to the appropriate machines.

**Identity resolution in this model is the deployment channel itself.** The binary doesn't need to speak LDAP or OIDC. The fact that machine X, joined to the domain under user Y, received manifest Z *is* the identity resolution. The manifest's `identity` block is metadata for audit, not an input to authorization.

### 8.2. Personal/Developer Deployment

1. Download the binary.
2. Load the extension unpacked in the browser (`chrome://extensions` → Developer mode → Load unpacked).
3. Register native messaging host (a one-line install script, as in the reference implementation).
4. Add to MCP client: `claude mcp add browser-mcp -- /path/to/browser-mcp`
5. No manifest needed. The binary defaults to `unlisted_domains: "observe"`, audit to stderr.

### 8.3. Claude Code Integration

The binary is an MCP server launched as a subprocess. Claude Code configuration:

```json
{
  "mcpServers": {
    "browser-mcp": {
      "command": "/path/to/browser-mcp",
      "args": [],
      "env": {
        "BROWSER_MCP_MANIFEST": "file:///path/to/manifest.json"
      }
    }
  }
}
```

Or with managed config (enterprise):

```json
{
  "mcpServers": {
    "browser-mcp": {
      "command": "C:\\Program Files\\BrowserMCP\\browser-mcp.exe",
      "args": ["--manifest", "managed://"]
    }
  }
}
```

---

## 9. Security Considerations

### 9.1. Trust Surface

The binary + extension combination has full CDP access to the user's browser session. This includes: page content, cookies, session tokens, network requests, form data, and arbitrary JavaScript execution. This is identical to the trust surface of the official Claude in Chrome extension and every other extension-based browser automation tool.

The governance model does not reduce the trust surface of the *code*. It constrains the *usage surface* by limiting which domains are reachable and which tools are available. An organization deploying this tool trusts the binary and extension code in the same way it trusts any other software it deploys to managed endpoints.

### 9.2. Extension Tampering

If the extension is modified (by a malicious actor with access to the user's machine or browser profile), it could bypass domain restrictions by lying about the current URL or intercepting CDP responses. The binary mitigates this partially by including the current URL in audit records (which the extension provides — a tampered extension could falsify this too).

Full mitigation requires the extension to be force-installed via enterprise policy (preventing user modification) and validated via Chrome's built-in extension integrity checks (CRX signature verification for packaged extensions). The binary cannot independently verify the extension's integrity at runtime.

### 9.3. Manifest Tampering

A locally stored manifest file could be modified by a user with write access to the file system. For enterprise deployments where the manifest must be authoritative:

- Use `chrome.storage.managed` (Chrome enterprise policy), which the user cannot modify.
- Or store the manifest in a directory with restricted ACLs (readable by the binary's execution context, not writable by the logged-in user).
- Or sign the manifest (v2 consideration — adds complexity and a key management requirement).

### 9.4. MCP Client Trust

The binary trusts the MCP client (Claude Code) to relay tool calls faithfully. A modified MCP client could send forged tool calls. This is inherent to the MCP architecture — the binary is a server, not a gatekeeper for the client. The audit trail records what was dispatched regardless of client behavior.

### 9.5. No Content Inspection

The binary does not inspect page content for PHI or other sensitive data. It governs structurally (which domains, which tools), not semantically (what data is on the page). Content-level DLP is a separate concern handled by network-layer tools (e.g., Zscaler, Netskope) or browser-level DLP (e.g., Chrome Enterprise data controls).

---

## 10. Scope Boundaries (v1 Exclusions)

The following are explicitly out of scope for v1:

| Exclusion | Rationale |
|---|---|
| **Built-in IdP integration** (OIDC, SAML, LDAP) | Identity resolution happens at deployment time (manifest push), not runtime. Adding IdP integration would introduce a network dependency and require credential management in the binary. |
| **Remote policy service** | A manifest fetched via HTTP on every tool call adds a network dependency and a failure mode. Manifest changes propagate via the deployment channel (Intune refresh cycle). |
| **Multi-user multiplexing** | One binary, one identity, one manifest, one browser profile. Shared machines use separate profiles. |
| **Content inspection / DLP** | Semantic analysis of page content is a different tool with different expertise requirements. |
| **Manifest signing / attestation** | Adds key management complexity. Enterprise deployments use `chrome.storage.managed` (tamper-resistant by design). Signing is a v2 enhancement for file-based manifests. |
| **Cross-browser support** | v1 targets Chromium-based browsers only (Chrome, Edge, Brave, Arc). Firefox uses a different extension API and native messaging model. |
| **`upload_image` tool** | Niche use case. Can be added later without schema changes. |

---

## 11. Future Considerations (v2+)

**Dynamic grant refresh.** The `chrome.storage.managed` source could be polled periodically (e.g., every 5 minutes) to pick up mid-session manifest changes pushed by IT. The binary would re-resolve grants and re-advertise tools if the manifest changed.

**Per-session purpose tagging.** The MCP client could pass a `purpose` parameter at connection time (e.g., "incident-response", "asset-research", "training"). The manifest could include purpose-scoped grants — different tool sets for different declared intents.

**Manifest signing.** For file-based manifests, a detached signature (e.g., Ed25519) verified against a pinned public key in the binary. Prevents local tampering without `chrome.storage.managed`.

**Audit enrichment.** Post-v1, the audit record could include a hash of the screenshot (without the screenshot itself) for forensic correlation, or a DOM snapshot hash for change tracking.

**Content boundary markers.** Following `agent-browser`'s precedent, wrapping tool output in delimiters so the model can distinguish tool output from untrusted page content (prompt injection mitigation).

**Conditional human-in-the-loop.** For high-risk grants (e.g., EHR write access), the manifest could specify `"approval": "required"`, causing the binary to pause and prompt the user (via the extension's popup or a system notification) before dispatching Mutate-tier calls.

---

## Appendix A: Manifest Examples

### A.1. Enterprise Healthcare (Default-Deny)

```json
{
  "schema": 1,
  "identity": {
    "resolved_by": "managed_config",
    "principal": "GEISINGER\\jdoe",
    "groups": ["Dept-EA", "App-ServiceNow-Admin", "App-Epic-ClinicalRead"],
    "resolved_at": "2026-07-01T08:00:00Z"
  },
  "grants": [
    {
      "id": "servicenow",
      "domains": ["servicenow.geisinger.org"],
      "access": "mutate",
      "description": "ServiceNow incident and change management"
    },
    {
      "id": "epic-read",
      "domains": ["epic.geisinger.org"],
      "access": "observe",
      "description": "EHR read-only for clinical data review"
    },
    {
      "id": "research",
      "domains": ["*.gartner.com", "*.forrester.com", "*.ieee.org", "scholar.google.com", "learn.microsoft.com"],
      "access": "observe",
      "description": "External research resources"
    },
    {
      "id": "internal-docs",
      "domains": ["confluence.geisinger.org", "sharepoint.geisinger.org"],
      "access": "observe",
      "description": "Internal documentation"
    }
  ],
  "defaults": {
    "unlisted_domains": "deny",
    "max_concurrent_tabs": 3
  },
  "audit": {
    "enabled": true,
    "destination": "syslog",
    "syslog_facility": "local0",
    "include_tool_parameters": false,
    "include_screenshots_in_log": false,
    "log_denials": true,
    "log_successful_calls": true
  }
}
```

### A.2. Personal / Developer (Unrestricted)

```json
{
  "schema": 1,
  "identity": {
    "resolved_by": "local_file",
    "principal": "local-user",
    "groups": [],
    "resolved_at": "2026-07-01T14:00:00Z"
  },
  "grants": [],
  "defaults": {
    "unlisted_domains": "mutate",
    "max_concurrent_tabs": 10
  },
  "audit": {
    "enabled": true,
    "destination": "stderr",
    "include_tool_parameters": true,
    "log_denials": true,
    "log_successful_calls": false
  }
}
```

### A.3. QA / Testing (Scoped Mutate)

```json
{
  "schema": 1,
  "identity": {
    "resolved_by": "environment",
    "principal": "ci-runner",
    "groups": ["QA-Automation"],
    "resolved_at": "2026-07-01T10:00:00Z"
  },
  "grants": [
    {
      "id": "staging",
      "domains": ["*.staging.geisinger.org"],
      "access": "mutate",
      "description": "Full automation on staging environment"
    },
    {
      "id": "production-readonly",
      "domains": ["*.geisinger.org"],
      "access": "observe",
      "description": "Read-only verification on production"
    }
  ],
  "defaults": {
    "unlisted_domains": "deny"
  },
  "audit": {
    "enabled": true,
    "destination": "file",
    "file_path": "/var/log/browser-mcp/qa-audit.jsonl",
    "include_tool_parameters": true,
    "log_denials": true,
    "log_successful_calls": true
  }
}
```

---

## Appendix B: Comparison Matrix

| Capability | Claude in Chrome (Official) | open-claude-in-chrome (Noemica) | agent-browser (Vercel) | This Project |
|---|---|---|---|---|
| Execution model | Extension (user's browser) | Extension (user's browser) | Playwright (separate instance) | Extension (user's browser) |
| Domain control | Blocklist (58 domains) | No restrictions | Static allowlist | Identity-bound allowlist, default-deny |
| Tool-level access control | None | None | Action policy (static JSON) | Per-domain r/w tier + tool mask |
| Identity binding | None | None | None | Enterprise directory → manifest |
| Audit | None | None | None | Structured, per-call, configurable destination |
| Deployment model | Chrome Web Store | Manual (developer mode) | npm install | Enterprise policy (force-install) or manual |
| Runtime dependencies | Chrome, Claude Code | Node.js, Chrome | Node.js / Rust binary | None (single binary + extension) |
| Process count | 5 | 5 | 2 | 3 |
| Healthcare-ready | No (no audit, no RBAC) | No | Partial (compliance templates) | Yes (audit, RBAC, default-deny, PHI-aware logging) |

---

*End of specification.*
