# ADR-0028: Tripwire licensing, tiers, and the Continuity Promise

- Status: accepted (2026-07-03)
- Deciders: founder (Leo Botinelly), in session with the frontier assistant
- Extends: ADR-0027 (open-core business model); ADR-0026 (release maturity)

## Context

ADR-0027 established open-core: Apache-2.0 OR MIT engine, source-available governance
module under the Ghostlight Commercial License, boundary at `src/governance/**`. What it
deliberately left open was activation mechanics and go-to-market: how a commercial license
manifests at runtime, what tiers exist, and how a solo developer with a two-day-old product
and zero adopters turns the license into revenue without an enforcement apparatus.

The founder's concern, raised and resolved in session on 2026-07-03: with no brand and no
legal capacity to pursue violators, what stops a company from using the source-available
governance module without paying? The resolution: the buyer persona (compliance and
security teams) self-polices through its own SCA scanners, procurement gates, and audit
processes; the license needs a runtime tripwire that makes unlicensed use *visible and
deliberate*, not impossible. Closing the source or moving governance behind a plugin
boundary was considered and rejected (it removes the auditability trust lever that an
unknown vendor needs most, and the Elastic X-Pack precedent shows the industry moved away
from closed plugins toward in-repo source-available).

The founder ratified four principles verbatim:

1. Never gate safety.
2. The key gates central control, not governance itself.
3. Tripwire, not DRM.
4. License failure never weakens enforcement. Never phone home. Always behave and work.
   Trust the corporate compliance mechanisms to react to the noise.

## Decision

### 1. Licensing is purely observational: zero behavioral gating

License state changes NOTHING about what the binary does. No feature is enabled, disabled,
degraded, or delayed by license state, ever. The license manifests exclusively as
*observability*:

- a startup `tracing::warn!` when the state warrants a stamp,
- a `License:` section in `ghostlight doctor`,
- a `ghostlight license status` CLI report,
- a `license` field stamped onto audit records (tool-call and session-event lines alike)
  while an abnormal license state holds.

This is the purest implementation of the founder's principles: nothing to fail open,
nothing to fail closed, no failure modes at all. The pressure mechanism is that a
compliance team cannot tolerate `"license":"unlicensed"` lines flowing into their own SIEM.
The binary never phones home; there is no telemetry, no activation server, no network I/O
of any kind in the license path.

### 2. License artifact: offline Ed25519, key generations, dev generation 0

A license is a JSON envelope file:

    {
      "v": 1,
      "keygen": 0,
      "claims": "<base64 of the claims JSON bytes>",
      "sig": "<base64 of the 64-byte Ed25519 signature over those exact claims bytes>"
    }

The signature covers the exact base64-decoded claims bytes; there is no canonicalization
step (the bytes ARE the message). Claims JSON:

    {
      "id": "<uuid v4>",
      "licensee": "<display name>",
      "org": "<slug>",
      "tier": "development" | "community" | "founding" | "team" | "enterprise",
      "seats": <u32>,
      "products": ["browser"],
      "issued": "YYYY-MM-DD",
      "expires": "YYYY-MM-DD"
    }

- Verification uses `ed25519-dalek` `verify_strict` against an embedded table of verifying
  keys indexed by `keygen`. The table ships with room for rotation from birth.
- **Generation 0 is the development key and is deliberately public.** Its 32-byte seed is
  the ASCII string `ghostlight development key gen0!` (exactly 32 bytes), committed in the
  source. Anyone can self-sign a `development`-tier license and exercise the entire
  licensed surface with zero interaction with the vendor. Dev-signed licenses stamp audit
  records with `"license":"development"`, so they are useless for quiet production use.
  This makes evaluation frictionless while preserving the tripwire (using a self-signed
  license in production is as deliberate an act as patching the check out).
- **Generation 1+ are production keys**, generated offline by the founder (a 32-byte seed
  via `openssl rand`, stored offline with one encrypted backup, never committed and never
  placed in CI). Adding a production verifying key is a one-line constant addition.
- `seats` and `licensee` are legal terms carried in the claims, never enforced at runtime
  (runtime seat counting would require phone-home; Decision 1 forbids it).
- Expiry comparison is lexicographic on the `YYYY-MM-DD` strings against the current UTC
  date (ISO dates compare correctly as strings); a malformed date is an invalid license,
  not a panic.

### 3. License resolution and the stamp decision table

The binary looks for `license.json` first in the org policy directory (the directory of
`load::org_policy_path()`, e.g. `%ProgramData%\ghostlight\` on Windows), then in the user
config directory (sibling of `config.json`). The first file that exists is THE license; no
merging. Resolution happens once at mcp-server startup (hot-reload of the license file is
explicitly out of scope for v1 and may be added later).

Stamp decision table (`org_present` means `load::org_policy_path()` exists on disk):

| License state | org_present | Stamp |
|---|---|---|
| no license file | false | none (personal use is quiet; no license required) |
| no license file | true | `"unlicensed"` |
| present but invalid (bad sig, unknown keygen, malformed, wrong products) | any | `"invalid"` |
| valid but expired | any | `"expired"` |
| valid, tier `development` | any | `"development"` |
| valid production tier, in date | any | none |

The stamped `license` field is APPENDED to the serialized record (after `held` on tool-call
records) and is entirely ABSENT when the state is normal. This deliberately diverges from
the record convention that absent values serialize as `null`: the stamp is an
exceptional-state marker, not a regular field, and its absence keeps licensed and personal
audit streams byte-identical to today's format.

### 4. Surfaces: exactly four, and explicitly not `explain`

License state appears in: `ghostlight license status`, `ghostlight doctor` (a `License:`
section), the startup warning, and the audit stamp. It does NOT appear in the `explain`
tool output (agent-facing directory, and its CLI goldens stay untouched) and it does NOT
appear in any MCP tool response. The license is an administrator concern; agents never see
it.

### 5. Tiers and initial pricing

| Tier | Price | Terms |
|---|---|---|
| `development` | free, self-signed | evaluation and development, any org size; never production |
| `community` | free, self-serve key | production use, organizations of 5 or fewer people |
| `founding` | free 12 months, then 50% of list forever | 10 slots; quarterly email questionnaire + reference (named case study preferred, anonymized accepted) |
| `team` | ~$12/user/month, billed annually, 5-seat minimum | org policy at scale, email support (2-business-day response) |
| `enterprise` | from ~$10k/year | team + procurement paperwork (questionnaires, MSA, DPA), 1-business-day support, deployment help, roadmap input |

Pricing is initial and founder-revisable until first publication on the pricing page;
tier NAMES and the claims enum are pinned now. Annual billing only. Every early customer
is grandfathered at their signup price permanently ("founding customer" applies to the
founding tier; grandfathering applies to all tiers).

### 6. The Continuity Promise (normative wording)

Published on the pricing page, quoted in every renewal email, and binding on all tiers:

> **The Continuity Promise.** Ghostlight never phones home and license state never affects
> behavior. Enforcement, audit, and your production workflows are never interrupted,
> degraded, or disabled by license expiry, by the vendor's unavailability, or by the
> vendor ceasing to exist. An expired license changes exactly one thing: license-state
> notices appear in `doctor`, `license status`, and your own audit records until it is
> renewed. Your deployment works forever, offline, as-is.

### 7. The founding program

Ten slots. Twelve months of enterprise-equivalent licensing free. In exchange: a reply
to a short quarterly questionnaire (5-8 topics, sent and answered by email; template in
`docs/business/templates/`) and a reference (named case study preferred; an
anonymized-but-quotable reference is acceptable for teams that cannot be named). No
calls, no meetings. Post-year price locked at 50% of then-current list, forever.
Applications by email (hello@sylin.org, the single sink address); a one-page agreement
per org (template in `docs/business/templates/`).

### 8. License operations

- A private repo (`ghostlight-licensing`) is the ledger of record: one committed claims
  JSON per issued license, plus the signing tooling. The production signing seed itself
  lives offline, never in the repo and never in CI.
- A daily scheduled GitHub Action in that private repo reads the committed claims and
  opens an issue at T-30 and T-7 before each expiry. The founder sends renewal emails
  personally (templates in `docs/business/templates/`); at ten-org scale this is minutes
  per month and the personal touch is deliberate.
- Renewal emails lead with the Continuity Promise. The tone is fixed: nothing stops
  working; renew when procurement is ready.
- Payments via Stripe payment links; key issuance is manual (within 24 hours) until
  volume forces automation.

### 9. Never phone home (normative, permanent)

No Ghostlight binary, extension, or module will ever initiate network traffic for
licensing, telemetry, analytics, update checks, or any other vendor-serving purpose. The
only network I/O the product performs is what the user's own tool calls and the user's own
configured audit destinations require. This decision is permanent and marketing-visible.

## Provenance (decided in session, 2026-07-03; do not re-litigate)

- Founder confirmed full accordance with the four principles, verbatim quote: "Never
  phone home. Always behave and work. Trust the corporate mechanisms to simply do their
  job properly to avoid compliance noise."
- Founder set the founding program at 10 slots and expects no revenue in year one.
- Founder chose zero-dollar infrastructure (GitHub Pages + sylin.org domain, Cloudflare
  email routing, Stripe pay links, private GitHub repo as license ledger).
- Founder chose to persist this plan in the public repo, explicitly including the fact
  that it was produced with agentic assistance.
- Closed-source plugin distribution was considered and REJECTED (see Context); do not
  reopen without new facts.
- Renewal-email posture ("rest assured everything keeps working") is the founder's own
  wording and is embodied in Decision 6.
- 2026-07-04 same-week revisions, before any external party relied on the terms
  (precedent: the in-place ADR-0026/0027 corrections): the founding-program ask was
  reduced from a quarterly 30-minute call to a short quarterly email questionnaire (5-8
  topics), founder's words: "we can reduce the floor even more: An email"; and the
  operative contact address became the single sink hello@sylin.org. Decisions 5 and 7
  updated in place.

## Consequences

- The engineering surface is small and safe: a verifier, a resolver, a stamp, three
  display surfaces, and a feature-gated signing CLI. No failure modes were added to the
  enforcement path. Execution package: `docs/tasks/licensing-1/`.
- The audit record gains one conditional field; the shared-format convention divergence
  (omitted-when-absent) is documented in Decision 3 and in the field's doc comment.
- `tests/audit_recorder.rs` and the explain goldens remain untouched by design; the stamp
  is invisible in every existing test scenario.
- The dev generation-0 key being public is a FEATURE (frictionless evaluation), not a
  leak; its licenses are loudly marked. Production trust rests solely on generation 1+.
- GTM execution, document pack, and channel plan live in `docs/business/PLAN.md`; founder
  personal actions in `docs/business/FOUNDER-TODO.md`.
