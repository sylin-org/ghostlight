# Identity: Okta Cross-App Access, ID-JAG, RFC 8693

**Date:** 2026-07-01 · **Track:** Governance · **Source:** research agent (verbatim report)

> Prior art on delegation modeling. Relevant to us mainly as a *concern surface*: the
> `act`/`may_act` delegation shape is a clean way to model "agent acting for a human" in audit
> records. NOT a call to integrate an IdP (explicitly out of v1 scope).

## Standards stack (most general → most specific)
1. **RFC 8693: OAuth 2.0 Token Exchange** (the primitive: subject/actor tokens, `act`/`may_act`
   delegation claims).
2. **draft-ietf-oauth-identity-chaining**, a profile composing RFC 8693 + RFC 7523 into a
   two-step cross-domain flow.
3. **draft-ietf-oauth-identity-assertion-authz-grant (ID-JAG)**, an enterprise-SSO
   specialization of identity chaining, defining a concrete token type + `typ`.

**Okta Cross-App Access (XAA)** = Okta's product brand for its ID-JAG implementation.

## Okta XAA
Closes a "recognized gap in OAuth 2.0": secure agent-to-app / app-to-app access with enterprise
IT oversight, shifting the access decision "from the application layer to the identity provider."
Okta positions XAA as "the recommended way to connect to MCP servers in the enterprise." The MCP
client calls the IdP token endpoint ("given this user session, I want access to that MCP
server") and gets a signed JWT assertion audience-bound to the MCP server, redeemed via a
JWT-bearer grant. Introduced **June 23, 2025**; Oktane 2025; **Early Access January 2026**; early
adopters include Anthropic, Zoom, Slack. Admin experience is at the *identity layer*: decide what
connects, enforce policy, see/audit, one-click revoke.

## ID-JAG token
- Token-type URN: `urn:ietf:params:oauth:token-type:id-jag`; JWT header `typ`: `oauth-id-jag+jwt`.
- Payload: `iss` (IdP), `sub` (end-user), `aud` (resource AS), `client_id` (acting app/agent),
  `jti`, `exp`, `iat`; optional `scope`, `resource`, `authorization_details`. Short-lived,
  single-use, audience-bound.
- Step A (token exchange at IdP): `grant_type=…:token-exchange`,
  `requested_token_type=…:id-jag`, `subject_token` = user's assertion, `audience` = resource AS.
- Step B (redeem at resource AS): `grant_type=…:jwt-bearer`, `assertion` = the ID-JAG.

## RFC 8693 delegation primitive (the reusable concept)
- **Impersonation:** A is indistinguishable from B (no `act`).
- **Delegation:** A keeps its own identity while acting for B, expressed via the **`act`** claim:
  ```json
  { "sub": "user@example.com", "act": { "sub": "admin@example.com" } }
  ```
  Nesting expresses a delegation chain (outermost = current actor). **`may_act`** states a party
  is authorized to become the actor for another (an authorization to delegate).

## What to borrow *conceptually* (modeling only, no IdP wiring)
1. **`act`/`may_act` delegation shape → audit records.** Model every governed tool call as
   delegation, not impersonation: audit `sub` = the human on whose behalf the agent acts;
   `act.sub` = the agent/client identity. Provenance without conflating agent and user. A
   manifest naming "which agent may act for a user" is exactly `may_act` semantics.
2. **Audience-binding.** Every authorization bound to one target (domain/origin), never ambient.
   Mirrors ID-JAG's single-audience rule.
3. **Assertion tuple `{iss, sub, aud, client_id, scope}`** is a clean schema for per-call
   governance context: who vouches, which user, which target domain, which acting agent, what
   scope.
4. **Two-domain separation of concerns** (decide vs. enforce at target) matches our "policy in
   the binary, execution in the extension" split.

_Caveat: Okta's blog uses "Identity Assertion Authorization Grant"; the IETF title is "…JWT
Authorization Grant." Same draft. Trust datatracker for revision numbers._

## Key URLs
- https://datatracker.ietf.org/doc/draft-ietf-oauth-identity-assertion-authz-grant/
- https://datatracker.ietf.org/doc/draft-ietf-oauth-identity-chaining/
- https://www.rfc-editor.org/rfc/rfc8693.html
- https://developer.okta.com/blog/2025/09/03/cross-app-access
- https://www.okta.com/solutions/cross-app-access/
- https://workos.com/blog/id-jag-cross-app-access
