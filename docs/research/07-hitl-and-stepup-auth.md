# Human-in-the-Loop Approval & Step-Up Auth

**Date:** 2026-07-01 · **Track:** Security / Delight · **Source:** research agent (verbatim report)

> The key finding: approval can be **protocol-native** (MCP elicitation / MRTR) driven by our
> binary: no custom extension UI required. Plus the MCP tool-annotation "risk vocabulary."

## 1. "Pause and ask a human": implementations

### MCP Elicitation (the standards-native mechanism)
Lets an MCP *server* request user input mid-tool-call, pausing until the client answers. Flow:
`tools/call` → server needs info → returns an elicitation request → client shows UI → user
answers → client retries the call.
- Two modes: **form** (`mode: "form"`, restricted flat JSON Schema; **MUST NOT** request
  secrets) and **url** (`mode: "url"`, 2025-11-25; out-of-band OAuth/payment; data other than the
  URL not exposed to the client).
- Three-action response: `accept` / `decline` / `cancel`.
- Security: client MUST show which server asks, show the full URL, require explicit consent, MUST
  NOT auto-prefetch, MUST bind requests to client+user identity (phishing defense: verify the
  user who *completes* a flow is the one who *started* it).

**2026-07 spec change (important):** the 2026-07-28 RC **replaces** server-initiated
sampling/elicitation with **Multi Round-Trip Requests (MRTR, SEP-2322)**: the server returns an
`InputRequiredResult` with `inputRequests` (each `{type, message, schema}`) + opaque
`requestState`; the client gathers answers and re-issues the *original* call with
`inputResponses`. Stateless so any server instance can resume. Example: `"Delete 3 files?"` with
a boolean schema.

**General MCP HITL guidance:** principle is "explicit user approval for every tool/resource
access"; the spec mandates the *principle* but little mechanism, pushing teams toward external
tools (HumanLayer) for production workflows.

### LangGraph / OpenAI / Claude Code (framework precedents)
- **LangGraph** `interrupt()` + `HumanInTheLoopMiddleware`: per-tool `interrupt_on` map;
  decisions `approve`/`edit`/`reject`/`respond`; resume via `Command(resume=…)`; state persisted
  via checkpointer.
- **OpenAI Agents SDK** `needs_approval` (bool or async callable); run pauses,
  `RunResult.interruptions` holds `ToolApprovalItem`; resumable state serialize/`from_json`.
- **Claude Code / Agent SDK permissions:** Ask / Allow / Deny; `ask` falls through to a
  `canUseTool` callback; **plan mode routes write tools to `canUseTool` regardless of allow
  rules** (writes can't be auto-approved while planning).

## 2. Step-up authentication: RFC 9470
RS signals the token's auth event is insufficient: `401` + `WWW-Authenticate: Bearer
error="insufficient_user_authentication"` with `acr_values` (strength) and/or `max_age`
(recency). Client re-authorizes requesting the `acr` claim as essential + `prompt=login`. The
standardized way to force *fresh, stronger* human auth before a sensitive/write op.

## 3. Risk-based classification: MCP tool annotations (the "risk vocabulary")
Four boolean hints: **`readOnlyHint`** (default false), **`destructiveHint`** (default true),
**`idempotentHint`** (default false), **`openWorldHint`** (default true). Conservative defaults →
unmarked tools treated as destructive + open-world. Governance use: `readOnlyHint:true` from a
*trusted* server may skip confirmation; `destructiveHint:true` triggers approval.

**Critical caveat:** these are **hints, not enforceable guarantees**, untrusted unless from a
trusted server. They inform UX/policy but are **not a security boundary**; enforcement must come
from a policy engine cross-referencing multiple signals. (Directly relevant: our read/write
classification driving visibility/approval: annotations can't be the enforcement point; the
binary must be.)

Identity platforms: Okta's blueprint uses **CIBA-based step-up** for high-risk actions, action-
scoped tokens, least-privilege time-boxed access, an agent "kill switch," and validates
authorization **at the moment of action, not just at token issuance**.

## 4. Approval UX patterns
- **Inline elicitation** (blocking, in-context): MCP form/URL, LangGraph `interrupt()`, OpenAI
  interruptions.
- **Out-of-band / decoupled**: OIDC **CIBA** and **HumanLayer** (push/Slack/email; poll via
  `auth_req_id` or webhook). For autonomous/long-running flows with no shared screen.
- **Time-boxed / action-scoped grants**: CIBA issues short-lived single-purpose tokens; no
  persisted consent (one-time per action by design).
- **One-time vs standing approval**: LangGraph/OpenAI support per-call; HumanLayer's learned
  auto-approvals move toward standing grants.

## 5. Emerging standards for approval & consent
- **RFC 9470**: Step-Up Authentication Challenge (fresh/stronger auth gating).
- **RFC 9396, Rich Authorization Requests (RAR):** `authorization_details` JSON array for
  fine-grained, per-action scoped consent (vs coarse `scope`). Applicable to per-action, per-
  domain agent consent.
- **OIDC CIBA**: decoupled/out-of-band approval.
- **MCP MRTR / SEP-2322**: protocol-native mid-call approval (2026-07 RC).

## Relevance to browser-mcp
- Our read/write classification maps onto MCP annotation semantics (`readOnlyHint`/
  `destructiveHint`), but treat annotations as **UX hints only; enforcement lives in the Rust
  binary** (matches the "hints, not guarantees" warning and our architecture).
- For "fresh human auth before a write," RFC 9470 is the standards-based gate; CIBA is the
  out-of-band flow; RFC 9396 RAR expresses per-action scoped consent; **MCP elicitation (URL
  mode) / MRTR** is the in-protocol way to trigger human interaction without leaking credentials
  through the client or LLM context.

## Sources
- https://modelcontextprotocol.io/specification/draft/client/elicitation ·
  https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/
- https://docs.langchain.com/oss/python/langchain/human-in-the-loop ·
  https://openai.github.io/openai-agents-python/human_in_the_loop/
- https://datatracker.ietf.org/doc/rfc9470/ · https://datatracker.ietf.org/doc/html/rfc9396 ·
  https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/
- https://blog.christianposta.com/ai-agents-and-oidc-ciba/ · https://www.humanlayer.dev/docs/core/agent-webhooks
