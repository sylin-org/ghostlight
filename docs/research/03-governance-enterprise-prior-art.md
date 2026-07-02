# Governance / Access-Control / Enterprise-Deployment Prior-Art Survey

**Date:** 2026-07-01 · **Track:** Governance · **Source:** research agent (verbatim report)

> The most detailed report of the batch. Surfaced the most important design issue in the whole
> discovery (policy-resolution semantics), plus concrete audit-standard and deployment guidance.

---

## The single most decisive finding

**Our `first-match-wins` grant resolution is the outlier, on the dangerous side.** Every mature
deny-capable policy system IT admins already trust is *order-independent*: AWS IAM and Cedar use
**explicit-deny-wins**; Chrome URLBlocklist uses **most-specific-match + allowlist-over-blocklist**.
The *only* systems using first-match-wins are ordered proxy products (Zscaler, Netskope), whose
own docs warn it's a rule-shadowing footgun requiring GUI guardrails (drag-reorder, shadow
warnings) that a hand-edited JSON file cannot provide. Most tellingly, **our nearest twin,
Vercel's Rust `agent-browser`, deliberately chose deny-overrides** (`deny > confirm > allow >
default`), proven by a unit test literally named `test_policy_deny_takes_precedence`, and it
**separates the domain allowlist from the action policy** rather than fusing them like our grant.
([agent-browser policy.rs](https://github.com/vercel-labs/agent-browser);
[IAM eval logic](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_policies_evaluation-logic.html);
[Cedar auth](https://docs.cedarpolicy.com/auth/authorization.html);
[Chromium URL filter format](https://www.chromium.org/administrators/url-blocklist-filter-format/))

---

## 1. Manifest / Policy Ergonomics

**Verdict: keep the JSON grant-array *shape* (more IT-readable than Cedar/Rego, needs no embedded
engine → preserves zero-dependency), but change the *resolution semantics* and *structure*.**

| Change | Why | Source |
|---|---|---|
| **Drop first-match-wins → explicit-deny-wins + most-specific-domain-match, allow-wins-on-tie** | Only proxies use position-ordering; every trusted static-config system is order-independent. Reordering the file should never change behavior. | IAM, Cedar, Chrome; agent-browser twin |
| **Add a top-level always-wins `deny` block** (by tool / domain / tool@domain, optional `reason`) | A capability-*adding* grant can't express "never allow `javascript_tool` anywhere, even on granted domains." | Cedar forbid; agent-browser |
| **Separate domain-scope from capability-scope** | Fusing `{domains, access, tools}` → N-domains × M-tool-sets duplication, and awkward `tools/list` computation. | agent-browser; MintMCP/Cloudflare |
| **Adopt Chrome URLBlocklist domain grammar** `[scheme://][.]host[:port][/path]` | IT admins already deploy this via GPO/MDM; gives scheme/path scoping free. **Our current `*.example.com` semantics are non-standard 3 ways**: Chrome bare `host` = host+subdomains, leading-dot `.host` = exact; agent-browser's `*.example.com` also matches bare `example.com`; ours matches subdomains only. Pick one; document the specificity metric (required for most-specific-match). | Chrome filter format; agent-browser |
| **Named domain-group aliases + carry-a-reason-on-deny** | Cedar `principal in Group::"…"`; OPA `deny contains reason` feeds our denial message. Consider JSONC/`$comment`. | Cedar; OPA |

**Do NOT switch to Cedar-the-language or OPA/Rego-the-engine.** Both require embedding an
evaluator (breaks single-binary), impose a learning curve, and Cedar's request-oriented model is
awkward for building the `tools/list` advertisement. **Borrow their semantics, keep your shape.**

**MCP-native alignment:** MCP's own `tools/list` filtering is exactly how every RBAC gateway
implements masking. Pomerium's key insight: **restrict only `tools/call`, keep
`tools/list`/`initialize` unblocked** so discovery works
([Pomerium mcp_tool](https://www.pomerium.com/docs/capabilities/mcp/limit-mcp-tools)). MCP
maintainers are explicit that tool annotations (`readOnlyHint`/`destructiveHint`) are **hints,
MUST be treated as untrusted, not a security boundary**, which *validates* our design of
re-deriving observe/mutate in the binary. We can *emit* correct annotations for client UX while
enforcing independently.
([MCP annotations stance](https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/))

---

## 2. Enterprise-Admin Delight Opportunities

- **Pin the extension ID via the `key` field at build time, a hard invariant.** ID = first 128
  bits of `SHA256(DER-pubkey)` remapped `0-f→a-p`. Everything chains off it: native-host
  `allowed_origins` (no wildcards; must list `chrome-extension://<id>/`),
  `ExtensionInstallForcelist`, and the managed-storage path
  `…\3rdparty\extensions\<id>\policy`. Drift breaks all three silently.
  ([manifest key](https://developer.chrome.com/docs/extensions/reference/manifest/key))
- **Ship ready-to-import policy templates**, not just a binary: GPO `.reg`; an Intune Settings
  Catalog force-install profile + a **PowerShell platform script** that writes the
  `3rdparty\extensions\<id>\policy` managed-storage keys (Intune has *no native UI* for
  third-party managed storage, the #1 admin friction); a macOS `.mobileconfig`; a signed
  self-hosted `updates.xml`. "5-minute rollout vs. support ticket."
- **Loudly document the Windows off-store gotcha:** on Windows, an extension not on the Web Store
  **can only be force-installed if the machine is AD-joined, Entra-joined, or CBCM-enrolled.**
  Intune-managed devices qualify; unmanaged BYOD silently refuses.
  ([Chrome Enterprise extension policies](https://support.google.com/chrome/a/answer/7532015))
- **Manifest authoring UX:** a `--validate` mode (schema + shadowed-grant/dead-rule detection); a
  **`--dry-run "https://host" tool`** that prints the resolved grant + decision + reason (mirrors
  `cedar validate` and OPA's "run both evaluators, compare" migration pattern); **example
  templates per persona** (spec Appendix A is a great start).
- **Generated-from-AD-group tooling:** a generator turning "AD group → grant set" into per-group
  manifests (exactly MintMCP's **Virtual MCP Bundles** = curated tool sets per role with SCIM-
  driven membership, and TrueFoundry's per-server/per-env RBAC, but we do it *offline at deploy
  time*, which is our whole thesis).
- **SIEM-ready output out of the box** (see §3).
- **Validate the caller in the binary** (1Password pattern): check the extension ID from the
  native-messaging port sender rather than trusting it. Also document
  `NativeMessagingAllowlist=[org.sylin.browser_mcp]`.

The recurring **de-facto audit field set** across Obot/Cloudflare/MintMCP is `{timestamp,
identity, agent, server, tool, arguments, outcome, duration}`. Our record already matches; keep
`outcome/result` and `duration_ms` prominent. SOC 2 CC7.2 auditors specifically flag when
**denials land only in error logs, not the audit trail**: our `result: denied` on every call is
a genuine differentiator, keep it first-class.

---

## 3. Standards to Align With

**Audit: the strongest alignment opportunity. Adopt OCSF as an output *mode*, not the internal
schema.**
- **OCSF is now a Linux Foundation project (v1.4.0)**, backed by AWS/Splunk/Cisco/IBM/CrowdStrike,
  the vendor-neutral normalization target for Security Lake/Splunk/Datadog. The **API Activity
  class [6003]** maps almost 1:1 to a tool call: `tool→api.operation`, r/w tier→`activity_id`
  (Read=2, Create/Update/Delete), `result→status_id` (1/2), `duration_ms→duration`,
  identity→`actor.user`, domain/url→`dst_endpoint`/`http_request`. Emit denials as [6003] with
  `status_id=2`. ([OCSF api_activity](https://schema.ocsf.io/classes/api_activity))
- **New: OWASP Agent Observability Standard (AOS) *already extends OCSF 6003 for
  agent tool calls***: `activity_name: "Agent Tool Use"`, with `unmapped.aos` carrying
  `tool_call.name`, `tool_call.arguments`, `agent.id`, `session.id`. Aligning our OCSF emitter to
  AOS gives a credible, forward-looking "agent audit" story.
  ([OWASP AOS extend_ocsf](https://aos.owasp.org/spec/trace/extend_ocsf/))
- **Add CEF output** (ArcSight header + k=v: `suser`=principal, `act`=tool, `request`=url,
  `outcome`=result), ingested natively by **Microsoft Sentinel's `CommonSecurityLog` via AMA**
  and by ArcSight. Skip LEEF (QRadar-only).
- **RFC 5424: do it properly, don't only stuff JSON in MSG.** Two modes: (1) JSON-in-MSG for
  Splunk/Sentinel auto-parse (emit UTF-8 **without** BOM; spec wants a BOM but common parsers
  choke), STRUCTURED-DATA = `-`; (2) proper `SD-ELEMENT` under a **PEN-namespaced SD-ID**
  (`audit@<enterprise-number>`) for RFC-strict shops. Splunk's default syslog sourcetype doesn't
  fully parse 5424 SD. **HEC/JSON is the pragmatic Splunk path.**

**Identity: align the metadata now; know the v2 plug-in point.** Our "deployment-channel-IS-
identity" thesis is sound for v1, but standards are moving fast and Anthropic is *in* them:
- **Okta Cross-App Access (XAA) / ID-JAG**: "Identity Assertion JWT Authorization Grant," IETF
  `draft-ietf-oauth-identity-assertion-authz-grant` profiling `draft-ietf-oauth-identity-chaining`,
  on **RFC 8693 token-exchange + RFC 7523**. IdP issues a signed ID-JAG ("this user via this
  app"), exchanged for an access token, replacing user consent with enterprise-governed
  delegation. **Anthropic, Cursor, VS Code, MintMCP, Docker are named early adopters** (Okta OIN
  GA Aug 2026). ([oauth.net/cross-app-access](https://oauth.net/cross-app-access/))
- **Microsoft Entra Agent ID**: first-class agent identities, **Conditional Access for agents**
  (GA July 2026), distinct **"on-behalf-of agent" vs "autonomous agent"** policy templates,
  disable/revoke all agents of a type at once.
  ([Entra Agent ID](https://learn.microsoft.com/en-us/entra/agent-id/what-is-microsoft-entra-agent-id))
- **IETF WIMSE** (`draft-ietf-wimse-arch-07`) targets "systems acting as autonomous agents on
  behalf of an upstream principal invoking downstream workloads."
- **Recommendation:** keep our `identity{principal, resolved_by, groups, resolved_at}` block, but
  (a) add an optional `assertion` slot able to carry an ID-JAG / OAuth subject-token in v2, and
  (b) surface `event_id` as OCSF `metadata.correlation_uid` to correlate with Entra/Okta agent
  logs. Make the identity block a *superset-ready* shape, not a dead-end.

**MCP auth direction** (for a possible future HTTP transport): 2025-06-18 made MCP servers **OAuth
2.1 Resource Servers** (RFC 9728 PRM, RFC 8707 Resource Indicators, mandatory PKCE, `403` =
insufficient). But **stdio transports SHOULD NOT follow the OAuth flow**, so our stdio boundary
correctly stays governance-manifest-based. The **2025-11-25** revision added **URL-Mode
Elicitation (SEP-1036)** and enterprise-managed client registration.

**For our v2 human-in-the-loop:** the standards-native mechanism is **MCP elicitation**
(`ctx.elicit()` with a JSON Schema → client renders an approval form). agent-browser gives a
concrete UX precedent: `Confirm` state with **60-second auto-deny** and **auto-deny when stdin is
not a terminal**. Adopt both for the spec §11 `"approval": "required"` idea.

---

## 4. Risks / Challenges to Our Approach

1. **First-match-wins is a latent misconfiguration generator** in a hand-edited file (§1).
   Highest-priority design fix. Explicit-deny-wins + most-specific-match is drop-in and safer.
2. **Extension-ID / native-host coupling is brittle.** Un-pinned `key`, wildcard-less
   `allowed_origins`, Windows backslash escaping, Linux dir-casing (`native-messaging-hosts` vs
   `NativeMessagingHosts`) all produce silent "native messaging host not found." Mitigate with
   installers that compute paths and a build-time ID invariant.
3. **MV3 service-worker death is architectural.** `connectNative()` keeps the SW alive *only while
   the port is open*; SW still dies after 30s idle / 5-min hard cap, and Chrome doesn't always
   close ports on unload (issues #559/#2688). The extension **must** reconnect in
   `port.onDisconnect`; the binary must idempotently tolerate orphaned/duplicate ports + a
   heartbeat. Core to spec §2.4 resilience.
4. **Windows off-store force-install requires domain/Entra/CBCM join**, a real blocker for
   unmanaged devices; a documentation-and-expectation fix, not code.
5. **Full-URL logging is a genuine PHI leak.** HIPAA §164.312(b) is *required* (6-yr retention),
   but URLs/query strings routinely carry patient IDs. **Recommend defaulting to
   scheme+host+path with query-string redacted/hashed**, gated by the same manifest sensitive-
   field mechanism as `parameters`/`screenshot`.
6. **"Healthcare-grade audit" needs tamper-*evidence*, not just JSON lines.** Add cheap
   **hash-chaining** (rolling SHA-256; each record carries `prev_hash`+`hash`; OCSF has a
   `raw_data_hash` slot); pair with append-only perms / WORM guidance + NTP time-sync. Tamper-
   *evident* not tamper-*proof*. Document that boundary. High ROI, ~few lines, no dependency.
7. **A tampered extension can lie about the current URL**, defeating per-call domain checks (spec
   §9.2 acknowledges this). Force-install + CRX signature verification is the only real
   mitigation; the binary cannot verify extension integrity at runtime. Restate as a known,
   accepted limitation. (See report 08 for the deep CDP-layer analysis.)
8. **The CDP "started debugging this browser" infobar** cannot be suppressed via managed policy
   (only the non-deployable `--silent-debugger-extension-api` flag), an unavoidable UX artifact.

---

## Highest-value takeaways (one line each)
- **Policy:** first-match-wins → explicit-deny-wins + most-specific-match; add a top-level
  always-wins `deny`; split domain-scope from capability-scope; adopt Chrome's domain grammar.
  (Our twin agent-browser already did all of this.)
- **Audit:** keep flat JSON canonical; add **OCSF 6003 (aligned to OWASP AOS)** + **CEF** output
  modes; redact URL query strings by default; add hash-chaining.
- **Identity:** deployment-channel-as-identity is right for v1; make the `identity` block
  ID-JAG/OAuth-superset-ready for a v2 runtime story; use **MCP elicitation** for v2 HITL.
- **Deployment:** pin the extension `key`; ship Intune/GPO/mobileconfig templates + a PowerShell
  script for third-party managed storage; document the Windows off-store domain-join requirement
  and the MV3 reconnect contract loudly.

_Note: one background sub-agent (agent-identity) returned only a rate-limit placeholder; its
substance is fully covered above via direct primary sources (see also report 06)._
