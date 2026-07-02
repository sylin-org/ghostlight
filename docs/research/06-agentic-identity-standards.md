# Agentic Identity & "OAuth for Agents" Standards (2025-2026)

**Date:** 2026-07-01 · **Track:** Governance · **Source:** research agent (verbatim report)

> Landscape scan of the fast-moving agent-identity standards. Mostly **out of v1 scope** (we
> exclude runtime IdP integration); kept as a concern-surface and a v2 plug-in-point map. The
> one shipped item that matters for positioning: MCP Enterprise-Managed Authorization.

The landscape is converging on three foundations, recombined for AI-agent/workload delegation:
**RFC 8693 (Token Exchange), JWT (RFC 7519), SPIFFE**.

## 1. Cross-App Access / ID-JAG
`draft-ietf-oauth-identity-assertion-authz-grant` (rev 04, 21 May 2026). Authors Parecki,
McGuinness, Campbell. A profile of RFC 8693 letting an app obtain an ID-JAG JWT from a shared IdP
and redeem it at *another* app's AS with no per-user consent/redirect, the mechanism behind
**Cross-App Access (XAA)**. Explicit AI-agent use case in Appendix A.4 ("AI Agent using External
Tools"). Builds on `draft-ietf-oauth-identity-chaining` (RFC 8693 + RFC 7523).

## 2. Token Exchange for Agents / delegation
RFC 8693 `act` (delegation, nestable: Human → Agent A → Agent B → Service) and `may_act`.
**Critical limitation:** consumers must only consider top-level + *current* `act`; nested prior
actors are **informational only (audit)**, not enforceable, which is driving new drafts:
- `draft-niyikiza-oauth-attenuating-agent-tokens`: extends RFC 9396 for **monotonic permission
  reduction** at each hop.
- `draft-mw-spice-actor-chain`: **cryptographically verifiable actor chains**.
- `draft-mw-oauth-tls-session-bound-tokens`: binds tokens to an mTLS connection.
- Active OAuth-WG discussion (Feb 2026): "Delegation Chain Splicing in RFC 8693." Theme across
  the field: **delegation beats impersonation** (Red Hat).

## 3. SPIFFE / SPIRE
- **SPIFFE ID:** `spiffe://<trust-domain>/<workload>`. **SVID** = X.509-SVID (SPIFFE ID in URI
  SAN) or JWT-SVID. **Workload API** gives a workload its ID + short-lived key/cert with no prior
  self-knowledge. **SPIRE** = server + per-node agents + attestation pipeline.
- **Agent relevance:** an agent binary is a workload; SPIFFE gives it a verifiable, short-lived,
  auto-rotated identity for mTLS. This is the model **Google Cloud Agent Identity** adopts.

## 4. IETF WIMSE (Workload Identity in Multi System Environments)
Chartered 2024. Targets runtime workload identity for chained service calls. Names SPIFFE +
OAuth + JWT as the three foundations. Adopted drafts include `draft-ietf-wimse-arch-07`,
`-s2s-protocol-06` (defines **WIT** = JWS-signed JWT binding a public key to a workload identity,
and **WPT** = per-request proof-of-possession token in the `Workload-Proof-Token` header).

## 5. Transaction Tokens
`draft-ietf-oauth-transaction-tokens-08` (WG Last Call). Short-lived signed JWTs (`typ=txntoken+jwt`)
asserting a user/workload identity + an **immutable authorization context** that stays constant
as a call flows through a chain of internal workloads. Claims: `iat`, `aud`, `exp`, `txn`, `sub`,
`scope`, `req_wl`; optional `tctx`/`rctx`. Issued by a per-trust-domain TTS; the TTS **MUST NOT**
expand scope or alter `sub`/`txn`/`aud`: tamper-proof "who initiated, on whose behalf." (Our
per-call `event_id` + immutable-context framing is a lightweight analog.)

## 6. Vendor positions
**Anthropic (MCP):**
- MCP Authorization spec (draft, June 2026): server = **OAuth 2.1 resource server**; RFC 9728
  PRM (MUST), RFC 8707 Resource Indicators (MUST), OAuth Client ID Metadata Documents, RFC 9207,
  PKCE. Servers **MUST** validate token `aud` and **MUST NOT** pass along other tokens
  (confused-deputy defense).
- **Enterprise-Managed Authorization (EMA), shipped 18 June 2026**, a stable MCP extension
  **implementing ID-JAG**: during SSO the client gets an ID-JAG from the IdP, exchanges it for an
  MCP-server access token ("authorize once, inherit everywhere"). Launch: Okta (via XAA) as first
  IdP; clients Claude + VS Code; servers Asana, Atlassian, Canva, Figma, Linear, Supabase.
  ([EMA post](https://blog.modelcontextprotocol.io/posts/enterprise-managed-auth/))

**Google (Cloud Agent Identity):** built on SPIFFE; each agent gets a SPIFFE ID + X.509 certs
(auto-refresh 24h); access tokens cryptographically bound to the cert. Two authority models:
agent's own authority, and user-delegated (3-legged OAuth).

**Cross-industry:** Linux Foundation **Agentic AI Infrastructure Foundation** (AWS, Anthropic,
Google, Microsoft, OpenAI, Block, Bloomberg, Cloudflare). Google DeepMind's Delegation Capability
Tokens (macaroons). Academic: "Agent Identity Protocol (AIP)."

## Relevance to browser-mcp
Almost all of this is **v1-out-of-scope** (we do not wire in an IdP at runtime). The two things
worth keeping: (1) model audit identity as **delegation** (`act`/`may_act`), the industry
consensus; (2) **position relative to MCP EMA / XAA**: our manifest is the deployment-channel
equivalent of an ID-JAG assertion; complementary, not blind. If we ever want the *binary itself*
to carry a verifiable identity, **SPIFFE** is the model: a v2+ radar item, not now.

## Quick reference (exact identifiers)
- ID-JAG: `draft-ietf-oauth-identity-assertion-authz-grant-04`
- Identity chaining: `draft-ietf-oauth-identity-chaining-12`
- Transaction Tokens: `draft-ietf-oauth-transaction-tokens-08`
- WIMSE: `draft-ietf-wimse-arch-07`, `-s2s-protocol-06` (WIT+WPT), `-wpt-01`, …
- Core RFCs: 8693, 7523, 9396, 9728, 8707, 8414, 9207, 6750; OAuth 2.1 `draft-ietf-oauth-v2-1-13`
- SPIFFE: ID = `spiffe://trust-domain/path`; SVID = X.509 / JWT; SPIRE = server+agent+attestation

_Note: a couple of arXiv IDs from secondary sources carried future-dated identifiers (lower
confidence); underlying claims corroborated by vendor/standards sources._
