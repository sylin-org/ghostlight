# ADR-0028: Tripwire licensing, tiers, and the Continuity Promise

- Status: accepted (2026-07-03); Decision 5 amended 2026-07-09 (see the note under the
  tier table: the `development` tier is renamed `evaluation`, the paywall wording moves
  from development/production to evaluation/operational use, and the small-team free
  grant lands in the license text as LICENSE-GOVERNANCE v1.1 grant (e)); Decisions 2 and
  8 amended 2026-07-10 by Decisions 10 and 11 (issuance pipeline pinned after a two-brief
  prior-art pass; production generations become composite Ed25519 + ML-DSA-65; the
  armored paste-block becomes a first-class license form)
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
- a `license` field stamped onto TOOL-CALL audit records while an abnormal license state holds
  (scoped to tool-call records; see the amendment note under Decision 3).

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

Stamp decision table (REFINED 2026-07-10; see the note below):

| License state | governance operational | Stamp |
|---|---|---|
| no license file | false | none (all-open / personal use is quiet; no license required) |
| no license file | true | `"unlicensed"` |
| present but invalid (bad sig, unknown keygen, malformed, wrong products) | true | `"invalid"` |
| valid but expired | true | `"expired"` |
| valid, tier `development` | true | `"development"` |
| valid production tier, in date | true | none |
| any state | false | none (the licensing engine is dormant when governance is not operating) |

REFINEMENT (2026-07-10, owner-directed, before any key shipped): the gate column changes
from `org_present` (`org_policy_path()` exists on disk) to **`governance operational`** --
an ORG-DEPLOYED policy is loaded and IN EFFECT in this session. Two consequences, both
deliberate: (a) the license engine emits NOTHING (no stamp, no startup warn) in the free
all-open path -- it is dormant unless governance is actually operating, so the audit stream
of a free deployment is byte-identical to today's; (b) the signal is org-policy ORIGIN, not
any active manifest -- a user `--manifest` / `GHOSTLIGHT_MANIFEST` never triggers a stamp,
because the binary cannot distinguish a free solo developer's own manifest from a paying
org's (it never phones home and never counts seats), and the admin-installed system-location
org policy file is the only reliable "an ORGANIZATION deployed central governance" signal.
Erring toward org-policy origin makes a false positive (wrongly stamping a free individual
`"unlicensed"`) impossible, at the cost of a false negative on the rare org that governs via
an env manifest instead of the org policy file -- the correct trade for a brand that promises
generosity to individuals. This supersedes the `org_present` phrasing above and keeps the
engine confined to one module (`governance::license`); the recorder receives only an opaque
stamp string, resolution/decision/formatting all live in that module (SoC per the owner).
Resolution itself still happens once at startup; `doctor` and `license status` are separate
read-only CLI invocations that display the resolved state and NEVER stamp or warn.

The stamped `license` field is APPENDED to the serialized record (after `held`) and is
entirely ABSENT when the state is normal. This deliberately diverges from the record
convention that absent values serialize as `null`: the stamp is an exceptional-state marker,
not a regular field, and its absence keeps licensed and personal audit streams byte-identical
to today's format.

AMENDMENT (2026-07-10): the stamp rides TOOL-CALL records ONLY, not session-event records.
Decision 1 originally said "tool-call and session-event lines alike", but session-event
records (`config_changed`, the panic kill switch, session lifecycle) carry a FROZEN shape
(ADR-0025 / PINS.md CS4) that external audit consumers may parse positionally, and appending a
field there breaks that contract for little gain -- the compliance signal is fully carried by
the tool-call stream, which is where governed agent work actually appears (and session events
are rare administrative lifecycle by comparison). So `Recorder::write_serialized` appends the
`license` field only for `kind == "tool_call"`; session-event lines stay byte-identical to
their frozen format. This surfaced when the licensing engine's stamp added a 7th key to the
`config_changed` session-event in `tests/manage_web_enable_remote` (that test injects an org
policy, making governance operational, so the stamp fired).

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

AMENDED 2026-07-09 (owner review of PRICING.md, before any license key shipped, so the
rename costs nothing): the `development` tier is renamed **`evaluation`** -- the old name
conflated the deployment stage with the paywall axis, and "never production" misread as
"production means you pay" when the actual trigger is a CONJUNCTION (a for-profit
organization of MORE THAN FIVE people, running governance CONFIGURED, operationally).
The wording across PRICING.md/LICENSING.md now uses "operational use" for the paid side
(defined: the organization relies on the policy for real work; internal tools count) with
"production" kept once as a parenthetical alias. The `community` small-team grant, which
the pricing page advertised but the license text never granted, is now granted explicitly:
LICENSE-GOVERNANCE v1.1 adds free-use grant (e), organizations of at most five people,
including production use. Two generosity accommodations join the pricing page as standing
policy: a hardship valve (email us; includes free) and an outgrew-the-tier grace (finish
the year free; nothing owed retroactively).

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

### 10. Issuance pipeline: MoR checkout as delivery only; signing stays air-gapped
(2026-07-10; amends Decision 8)

Pinned after a two-brief prior-art pass (merchant-of-record platforms; dedicated
licensing infrastructure and real offline issuers), both run 2026-07-10:

- **Checkout and tax: Polar.sh** (a true merchant of record), with Lemon Squeezy as the
  fallback. The platform's own license-key feature is NEVER used for validation: every
  MoR built-in key (Polar, Lemon Squeezy, Gumroad) is validated by calling the vendor's
  API, which Decision 9 forbids. The platform carries payment, tax, subscription state,
  and the delivery envelope for our own signed license, nothing else. Stripe alone is
  rejected (not a merchant of record; the founder would carry worldwide VAT), and Paddle
  is rejected (native license fulfillment deprecated; approval-gated onboarding).
- **The purchase webhook files an order-intent; it never signs.** A minimal endpoint (or
  a manual step at first) records identity fields from the buyer (licensee, org) plus
  commercial fields from the purchase itself (tier, seats, expires) into the private
  `ghostlight-licensing` ledger repo. Nothing online ever holds the signing seed.
- **The founder batch-signs offline** on the air-gapped machine with the feature-gated
  `license sign` CLI, commits the claims to the ledger, and delivers the signed license
  (armored block by email, or a Polar file benefit). Because licensing is observational
  (Decision 1), the hours-scale signing delay is invisible: the buyer's binary already
  works; the license file only silences a stamp.
- **Rejected mechanisms, with reasons, so they are not re-proposed:** a serverless
  function that signs on purchase (parks the air-gapped seed in an online function);
  Keygen.sh Cloud (the vendor holds the signing key -- verified 2026-07-10 -- which
  breaks the air-gap non-negotiable; Keygen self-host CE remains the named escape hatch
  only if self-service issuance at scale ever becomes real); any client-side phone-home
  (Decision 9 stands).
- **Revocation is expiry.** Annual terms aligned to annual billing; a lapsed or refunded
  license is simply not renewed. No CRL, no kill switch -- the norm among offline
  issuers (JetBrains offline codes, Keygen offline mode) and the only mechanism
  consistent with Decisions 1 and 9. Runtime seat/machine binding stays out (it exists
  to gate behavior, which Decision 1 forbids).

### 11. Composite signatures (Ed25519 + ML-DSA-65) and the armored license block
(2026-07-10; amends Decision 2)

- **Every production key generation (keygen 1 and up) is a composite scheme from
  birth:** Ed25519 plus ML-DSA-65 (FIPS 204, security category 3). The embedded
  generation table declares each generation's scheme. Generation 0 (the public
  development key) stays pure Ed25519, unchanged.
- **Composite envelope** (v stays 1; the scheme rides on `keygen`):

      {
        "v": 1,
        "keygen": 1,
        "claims": "<base64 of the claims JSON bytes>",
        "sig": "<base64 of the 64-byte Ed25519 signature>",
        "sig_mldsa": "<base64 of the 3309-byte ML-DSA-65 signature>"
      }

  Both signatures cover the SAME exact decoded claims bytes. Verification for a
  composite generation requires BOTH to pass (AND-composition): a missing or invalid
  `sig_mldsa` on a composite generation is Invalid, as is a stray `sig_mldsa` that fails.
  An Ed25519-only envelope verifies only against a generation whose declared scheme is
  Ed25519 (today: generation 0 only).
- **Why composite, not a swap:** the two algorithms fail differently. Ed25519's exposure
  is quantum (Shor); ML-DSA's exposure is youth -- roughly fifteen years of lattice
  cryptanalysis versus forty for elliptic curves (the Rainbow and SIKE breaks happened
  mid-NIST-process, by classical attacks), and the pure-Rust ML-DSA crates are pre-1.0
  and unaudited. Under AND-verification a forger must break both, so the composite is as
  strong as the stronger algorithm whichever way history breaks. This mirrors the
  IETF LAMPS composite-signature direction and the hybrid pattern TLS and SSH adopted
  for their post-quantum rollouts.
- **Implementation:** a pure-Rust ML-DSA implementation (required: the four-target
  cross-compile matrix must not grow a C toolchain); the concrete crate (fips204 or
  RustCrypto ml-dsa) is pinned at build time after checking release state. Verification
  cost is microseconds, once, at startup; the license file grows to roughly 5 KB, which
  stays comfortably inside the paste-block form below.
- **The armored block is a first-class license form.** Format:

      -----BEGIN GHOSTLIGHT LICENSE-----
      <base64 of the exact envelope JSON bytes, wrapped at 64 columns>
      -----END GHOSTLIGHT LICENSE-----

  `license sign` emits both the envelope JSON file and the armored block; `license
  install` accepts a file path or a pasted armored block. The armored payload decodes to
  the exact envelope JSON bytes (no transformation), so both forms verify identically.

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
- 2026-07-10 (Decisions 10 and 11, decided in session before any license key was ever
  issued): the owner asked for prior-art research on the licensing ENGINE (issuance
  mechanics), which produced the two briefs Decision 10 cites. The composite
  post-quantum scheme and the armored paste-block are both owner ideas from the same
  session (owner on the quantum flex, after hearing the cost was a pure-Rust dependency
  and microseconds at startup: "the flex is totally worth it!"); the AND-composition
  rationale and the sign/install verb split are the assistant's refinements the owner
  ratified. The owner also directed that the Enterprise-tier promise ("security
  questionnaires, MSA, DPA...") be backed by a ready-to-go document pack that leads with
  the offline/no-phone-home/post-quantum posture; that pack extends the Decision 8
  template set and owes a legal skim before first use.

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
