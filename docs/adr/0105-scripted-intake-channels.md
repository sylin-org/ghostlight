# ADR-0105: Scripted intake -- a CLI edge, observed channel attribution, and signer-gated admission

- Status: Accepted
- Date: 2026-08-11
- Builds on: ADR-0013, ADR-0027, ADR-0028, ADR-0096, ADR-0102, ADR-0104

## Context

Driving Ghostlight requires an MCP client today. That is the right default for agent work and the
wrong shape for a scripted job: a deploy check, a smoke test, a scheduled report. Those callers want
one browser action and an exit code, not a protocol handshake and a model.

The capability already exists. ADR-0096 made the service protocol-neutral, so MCP lives entirely in
`crates/mcp-connector` and the orchestrator speaks its own typed vocabulary over authenticated
loopback TCP: `Hello`, `Catalog`, `Invoke`, `Cancel`. Anything that can open a socket and write
line-delimited JSON can already drive the browser. What is missing is a supported way to do it, and
an answer to what governance should say about a caller that is not a model.

Two facts shape that answer.

**A new local channel grants no new authority.** Anyone who can run a Ghostlight CLI can also start
`ghostlight-mcp-connector` by hand and speak stdio at it, reaching the same capability ceiling.
The pipe is not the boundary.

**Attribution today is a claim.** `client_label` arrives in `Hello` and reaches both the audit record
and the browser tab-group name. Nothing verifies it. With two first-party connectors that has been
academic; a documented scripting path makes it a question an auditor can ask.

## Decision

### 1. A first-party CLI edge on the existing executable

`ghostlight call <tool> <json>` invokes one catalog tool and prints one terminal result. It is an
edge in the same shape as the MCP connector: it owns argument parsing, rendering, and exit codes,
and makes no product decision. It resolves the runtime file, honors the lifetime lease, and inherits
demand-start from ADR-0104, so a script starts the authority the same way a client does.

It is a subcommand rather than a fourth executable. The binary already resolves runtime discovery
and speaks the service bridge for workbench activation, and a separate process would duplicate both
for no separation of concern.

Exit codes carry the terminal status so that shell control flow is truthful: success is `0`, a
governed refusal and a decisive failure are distinct non-zero codes, and an uncertain effect is
never `0`.

Workspaces are admitted per connection, so handles do not survive between separate `ghostlight call`
processes. A single process may therefore carry more than one call. Named, resumable workspaces are
deliberately not introduced; `browser_run_sequence` already composes multi-step work, and a naming
scheme would collide with per-connection admission.

### 2. Channel and peer are attribution, never authorization

Every session records the channel it arrived on, from a closed vocabulary (`mcp`, `cli`). Where the
operating system can identify the socket peer, the peer executable's file name is recorded beside
it -- the name only, never the path, bounded and lowercased, exactly as an observation records a
landed host and never the rest of a URL.

The audit record therefore carries claimed and observed attribution as separate fields. An auditor
can tell which is which, rather than reading one string that might be either.

Neither field is an input to any decision. A process image name is excellent attribution and
worthless authorization: renaming a binary defeats it. This is the same reason the extension's
observations never govern.

### 3. Signer-gated admission, on the peer, narrowing only

A policy layer may restrict which signers a channel admits:

```json
{
  "version": 1,
  "allow_capabilities": ["read", "action"],
  "channels": { "cli": { "signers": ["sha256:AB12...", "teamid:XYZ123"] } }
}
```

Absent `channels`, no restriction applies, so an unconfigured Ghostlight admits every channel and
all-open stays first-class (ADR-0013). Layers compose by intersection like `allow_host_layers`: a
managed layer narrows a local one, and a local layer can never re-admit a signer the managed layer
excluded. A signer allowlist decides *who may connect*; it never lifts an admitted caller above the
capability ceiling every other caller has.

**The subject of verification is the socket peer, never its parent.** Windows lets a process choose
its parent through `PROC_THREAD_ATTRIBUTE_PARENT_PROCESS` given a `PROCESS_CREATE_PROCESS` handle,
which a same-user process can usually obtain, so parent-derived identity is forgeable. The kernel's
answer for who owns a connection is not.

That has a consequence worth stating plainly: a governed integration is an organization's own signed
program speaking the service bridge, not a signed program invoking `ghostlight call`. The CLI is a
human convenience whose peer is always Ghostlight's own binary.

Verification happens once at connect and is frozen on the session. Revocation checking is disabled,
because it is network I/O and Ghostlight never phones home (ADR-0028); chain validation without
revocation is the honest ceiling, and a revoked certificate with a valid chain will pass. Where an
operating system offers no verification, a policy that requires signers cannot be satisfied and the
channel is denied, the same way invalid authority denies.

### 4. The service bridge becomes a documented versioned contract

An organization cannot write a signed integration against an undocumented internal protocol. The
`Hello`/`Catalog`/`Invoke`/`Cancel` vocabulary, its framing, runtime discovery, and
`service_bridge_major` negotiation are published as a supported contract.

This is the largest decision here, because it converts something freely changeable into a promise.
It is accepted because a signer allowlist is meaningless without a caller to allow, and because the
alternative -- organizations reverse-engineering the same wire and depending on it anyway -- is
worse and unversioned.

## Amendment 2026-08-11 (signer gating deferred; plain channel admission ships instead)

Decision 3 is not implemented as written, and Decision 4 is not yet acted on. What ships is the
admission switch without the signer allowlist:

```json
{ "version": 1, "channels": { "cli": {} } }
```

An absent `channels` map restricts nothing, so all-open is untouched. Naming a channel is how a
layer takes control of it, and taking control means saying yes explicitly: `{}` and
`{"enabled": false}` both refuse the channel, `{"enabled": true}` admits it. Layers still compose by
intersection, so a managed refusal cannot be undone locally. A misspelled channel name is a decode
error and fails closed, like every other policy typo. The refusal lands at admission, before a
workspace exists, so nothing is invoked and no audit record is written; the caller gets the stable
`channel_denied` reason and a non-zero exit.

The deferral is not a change of mind about Decision 3's shape. It is that its precondition cannot be
met today: signature checking is only meaningful against the socket peer, identifying the socket
peer needs `GetExtendedTcpTable`, verifying a signature needs `WinVerifyTrust`, and both are raw
Win32 FFI while this workspace sets `unsafe_code = "forbid"` -- which, unlike `deny`, no scoped
`#[allow]` can override. Reaching stage 3 therefore requires an owner decision that is larger than
the feature: relax that invariant for one audited module, or take a dependency that wraps those
calls, on a security-sensitive path, in a project whose trust material advertises supply-chain
discipline.

Until that decision is made, nothing may treat the channel as an authenticated identity. It remains
what Decision 2 says it is: attribution. The switch above governs whether an intake may open a
session at all, which is a different and weaker claim than knowing who is calling.

## Amendment 2026-08-24 (stage 2 lands through one audited FFI crate; stage 3 stays deferred)

The owner revisited the deferred question on 2026-08-24 and decided it in two halves.

**Stage 2 (observed attribution) ships.** The owner chose to relax the invariant for one audited
module and to take no new third-party dependency on a security-sensitive path. Both halves are
implemented literally in one new workspace member crate, `ghostlight-win-peer`, whose manifest
deliberately does not inherit the workspace lints table -- Cargo applies inherited lints as
command-line flags no in-source `allow` can override, so a per-crate opt-out alongside inheritance
is impossible and relaxing the shared table would relax everywhere. The crate declares its foreign
functions by hand (`GetExtendedTcpTable`, `OpenProcess`, `QueryFullProcessImageNameW`) against
system link libraries, carries a `// SAFETY:` note at each call site, and is confined by a
repository guard test that fails if raw memory access ever appears in any other crate. At hello,
before any workspace exists, the orchestrator resolves the connection quadruple to the owning
process and keeps only the executable's bounded lowercase file name. The name rides the workspace
state and lands beside the claimed channel as its own audit field -- name only, never the path,
and never an authorization input -- exactly as Decision 2 specified. Where identification cannot
answer, the field stays absent and admission behaves exactly as before.

**Stage 3 (signer-gated admission) remains deferred, for a new reason.** The original blocker was
the unsafe invariant; that door is now open. What closes it again is evidence: Ghostlight publishes
no signed Windows binaries, so there exists no artifact a signer allowlist could admit, no test or
live lane anywhere in this tree could exercise the verification success path, and shipping
security-critical FFI whose accepting branch has never executed would be theater, not control.
The practical gate today is also already covered: the runtime discovery token decides who may
connect, and the stage-1 channel switch plus capability ceilings decide what an admitted caller
may do. Stage 3 becomes worth building when Ghostlight signs its first release artifact; that
event reopens this ADR, and the audited-crate seam this amendment establishes is where the
`WinVerifyTrust` work belongs when it happens. An implementation attempt on 2026-08-24 confirmed
the integration cost is real -- a hand-rolled `WinVerifyTrust` call kept refusing valid inputs
with `TRUST_E_PROVIDER_UNKNOWN` while platform tooling verified the same binary -- and was
withdrawn rather than half-shipped; its working notes live in the batch ledger only.

## Consequences

- Governance is not bypassed. Every channel crosses the same executor, workspace aggregate,
  governance facade, browser port, and completion path, and writes the same payload-free record.
- Signature checking authenticates a binary, not intent. Allowlisting a signed **interpreter**
  allowlists everything a user can type into it, which is why Microsoft publishes a blocklist of its
  own signed binaries for WDAC. The guide must say so beside the policy key.
- Signature checking is not a sandbox. Injection into an already-signed process defeats it, and the
  trust center must not imply otherwise.
- Verification is Windows in practice. Linux has no deployed equivalent, so a signer
  requirement denies there.
- Controlled tabs group by client label. The CLI channel uses one stable label so a scripted session
  gets its own visible group rather than multiplying groups per calling program.
- The policy schema grows within `version: 1` rather than bumping. There is no installed 1.0 base to
  migrate, and the strict decoder means a stale build reading a new policy fails closed with
  `invalid_authority` -- a development footgun, not a user-facing one.

## Staging

Attribution and admission are separable, and the ordering is forced by the argument above.

1. The CLI edge and the declared channel. Both edges are first-party executables, so the channel is
   asserted by a Ghostlight binary and used only for attribution and presentation.
2. Peer identification from the socket, which upgrades attribution from asserted to observed.
3. Signer-gated admission, which requires 2, because unforgeable peer identity is exactly what makes
   a signature check mean anything.

Stage 1 is safe without 2 and 3 precisely because the channel is never decisive. Nothing in stage 1
may become an authorization input before stage 2 lands.

## Alternatives considered

- **Documenting the raw bridge as the scripting answer, with no CLI.** Rejected for stage 1: it
  makes every script carry framing, discovery, and negotiation. Accepted for signed integrations in
  Decision 4, where the caller must be the peer anyway.
- **A local HTTP endpoint.** Rejected. The loopback socket already exists, so HTTP adds convenience
  rather than capability, in exchange for a broader exposure mistake.
- **Allowlisting executable paths or names.** Rejected. Unenforceable on a machine where the caller
  is already the user, and a claim the trust center could not defend.
- **Per-channel tokens.** Rejected. Anything that can read the runtime file gets all of them.
- **Gating the CLI channel off by default.** Rejected. It taxes the ungoverned path, and the channel
  grants no authority that a local user did not already have.
- **Named, resumable workspaces for shell ergonomics.** Deferred. It collides with per-connection
  admission and with caller-supplied labels; multiple calls per process covers the need.
