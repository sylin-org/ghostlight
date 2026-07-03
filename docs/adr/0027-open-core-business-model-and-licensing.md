# 0027. Open-core business model and licensing

- Status: Accepted
- Date: 2026-07
- Supersedes the whole-repo permissive license stance in ADR-0021 (brand and
  family decisions there are untouched).

## Context

Ghostlight is built by essentially a solo developer, and its most valuable
surface, the governance overlay, is interesting to companies, not to
individuals. The founder's goal is that individuals and open-source initiatives
use Ghostlight for free while companies pay. A permissive license (the dual
Apache-2.0 OR MIT that ADR-0021 set as the org convention) cannot express that:
it lets a large company use everything for free.

Two verified research passes (license mechanics, and solo-maintainer
sustainability) reached the same conclusion. First, no OSI-approved open-source
license can compel a class of user to pay for the identical code (Open Source
Definition points 5 and 6 forbid discrimination against persons or fields of
use), so any "large companies must pay" condition means the affected code is
"source-available," not OSI open source. Second, the only mechanism that reliably
turns enterprise value into a solo maintainer's income is a commercial
transaction, not donations: an enterprise-critical library on 75 to 80 percent of
the top-100 websites (core-js) still raised only about 2,500 USD per month in
donations against a stated need near 30,000 USD per month. The proven shape for a
self-hosted tool run by one person is open-core: a free open core plus a
commercial module or add-on that companies buy for a concrete reason (Sidekiq,
run solo, reached revenue "closer to 10M than 1M" USD; SQLite sells a closed
add-on and support tiers; HashiCorp Sentinel, a policy-as-code governance layer,
is sold only in the paid tier and is the closest structural analog).

Ghostlight's architecture already draws the seam. ADR-0013 separates an
unconstrained engine from a separable governance overlay, and the governance code
is already physically isolated under `src/governance/**`. The thing individuals
do not want (org policy, central audit, never-touch enforcement, central
management) is exactly the thing only a company will pay for, so the split is a
self-selecting paywall rather than a crippled core. Governance, audit, and
policy are an established paid-enterprise tier: GitLab's "buyer-based open core"
reserves audit and compliance for paid editions, and Percona's Peter Zaitsev, an
open-core critic, endorses reserving "Security, Compliance, and Enterprise
Complexity" (naming SSO and auditing) for the paid edition.

## Decision

### 1. Adopt open-core along the engine/governance seam

Ghostlight is licensed as open-core. The automation engine and all-open mode are
free and open to everyone, including large companies. The governance overlay is a
commercial, source-available module. Ghostlight as a whole is therefore open-core,
not OSI open source; the engine is open source, the governance module is not.

### 2. Engine license: dual Apache-2.0 OR MIT (OSI open source)

The engine and all shared code, everything outside `src/governance/**`, is
licensed dual Apache-2.0 OR MIT (the ADR-0021 convention, now scoped to the open
core). A downstream consumer may satisfy either; the Apache-2.0 half carries a
patent grant enterprises prefer, and this matches the domain norm (Playwright is
Apache-2.0; MCP reference servers use MIT or Apache-2.0). Repo-root
`LICENSE-APACHE` and `LICENSE-MIT` carry the texts. This tier must always be a
complete, genuinely useful product: full browser automation, the 13 trained tools
and `explain`, everything an individual or open-source user needs. A bug fix, a
security fix, or a core automation capability is never moved behind payment. This
is the adoption flywheel and the trust anchor, and degrading it is the Caddy
mistake (a non-commercial restriction on the free binary that had to be reversed
after backlash).

### 3. Governance module license: source-available Ghostlight Commercial License

`src/governance/**` (identity-bound grants with host polarity, org policy locks,
structured audit and its destinations, sacred never-touch domains, observe /
shadow / enforce modes, `explain`, central management including `managed://`) is
licensed under a source-available Ghostlight Commercial License. Its terms:
personal, non-production, and evaluation use is free (an individual may run and
inspect the full governance layer); organizational or production governance use
requires a paid commercial license. The source stays published and inspectable,
because this is the code that enforces a customer's security policy and writes
their audit trail, and buyers want to verify it, not receive an obfuscated blob;
source-available, not secrecy, is the accepted norm here (PostHog `ee/`, Cal.com
`ee/`, n8n). The license is perpetual with no automatic conversion to open source:
governance is ongoing value, not a temporary head start, so a time-delayed
converting license (BSL or FSL, which convert after a fixed period) is the wrong
mechanism and is rejected for this module. To reduce legal risk the license text
is adapted from a proven base rather than drafted from scratch: the GitLab
Enterprise Edition license template, as instantiated by the n8n Enterprise
License (`LICENSE_EE.md`), the authentik Enterprise Edition License, and the
Infisical Enterprise License. That template is the family that natively encodes
this module's boundary (free for development, testing, and personal use; paid for
production or organizational use), and it is what this ADR's own cited precedents
(PostHog, Cal.com, n8n) actually run on their commercial directory. A verified
license catalog (2026-07-03) rejected the two bases an earlier draft named, the
n8n Sustainable Use License and the Elastic License 2.0: both gate only competing
use (reselling or hosting the software to third parties), which a self-hosted tool
with no hosted service never triggers, so they would leave all organizational
production use free and monetize no one. The chosen skeleton needs one intentional
widening (the EE templates exempt only dev and test, so graft the explicit
personal / non-production / evaluation free carve-out this module grants) and two
trims for a solo-dev self-hosted tool (drop per-user seat metering in favor of a
per-organization or per-instance trigger, and drop the vendor-owns-your-
modifications clause). Repo-root `LICENSE-GOVERNANCE` carries the text, referenced
by SPDX id `LicenseRef-Ghostlight-Commercial` (the EE-template licenses have no
SPDX-list id, so a `LicenseRef-` is correct).

Two sub-choices in this decision, recorded with their alternatives so they are not
silently re-litigated: (a) personal use of governance is free rather than the
governance module being fully commercial, because it costs no revenue (individuals
were never buyers) and preserves goodwill and inspectability; (b) the license is
perpetual rather than time-converting, for the ongoing-value reason above.

### 4. The license boundary is the `src/governance/**` directory

The open/commercial line follows the existing physical seam. When the Cargo
workspace is split (ADR-0021 already anticipated extracting shared crates), the
boundary becomes a crate boundary: engine and shared crates set
`license = "Apache-2.0 OR MIT"`; the governance crate sets `publish = false` and
`license-file = "LICENSE-GOVERNANCE"`. Until that split lands, the single crate
carries `publish = false` and a repo-root `LICENSE` notice stating that everything
outside `src/governance/**` is dual Apache-2.0 OR MIT and that `src/governance/**`
is the Ghostlight Commercial License.

### 5. Contributor terms: DCO for the engine, CLA for governance

Contributions to the engine and shared code use the Developer Certificate of
Origin (inbound equals outbound under the permissive license). Contributions to
`src/governance/**` require a Contributor License Agreement granting the relicense
rights needed to distribute that code commercially, because only the copyright
holder can sell a commercial license. The founder owns all copyright today, so
this is single-vendor by default; the CLA is put in place before the first
outside pull request into the governance module. This mirrors the GitLab pattern
(DCO for core, CLA for the enterprise directory).

### 6. Revenue path: self-serve commercial licensing, not donations

The commercial license is sold self-serve through a merchant-of-record checkout
with built-in license keys (for example Polar.sh, Lemon Squeezy, or Paddle), so
one person does not carry worldwide tax, VAT, or PCI burden, with published flat
tiers and no mandatory sales call; human procurement is reserved for large deals
only. Enforcement is honor-system plus enterprise self-audit backed by copyright,
not a detection or litigation operation the maintainer cannot staff; the target
buyers are governance-conscious organizations with procurement and audit
functions, the population most likely to true up. Donations and sponsorship are
modeled as supplementary signal income only, never the revenue line. An optional
support or assurance tier (business-hours response SLAs, priced high) is a second
revenue line that does not touch the code boundary.

## Consequences

- Positive: who pays (the enterprise) and what is valuable (governance, the stated
  moat) are the same population by construction, so a maximally generous free
  engine does not cannibalize revenue. This is the property core-js lacked.
- Positive: the engine stays permissive and OSI open source forever, so there is
  nothing to relicense out from under users later; the 2023 to 2025 relicensing
  backlash (OpenTofu, Valkey, OpenSearch) was triggered by permissive-to-restricted
  rug-pulls, which this decision forecloses for the engine.
- Positive: the commercial line is a proven solo-runnable model (Sidekiq, SQLite,
  HashiCorp Sentinel) with a self-serve path that keeps the work build-dominant
  rather than sales-dominant.
- Negative and accepted: the core trade-off is that Ghostlight monetizes
  governance, not automation. A large company content to run the bare, ungoverned
  engine pays nothing; revenue rests entirely on the governance layer being
  something an enterprise cannot compliantly operate without. If the goal were to
  charge large companies for the bare engine too, this model cannot deliver it (a
  size-gated license such as PolyForm Small Business was the alternative for that
  goal, and was rejected because it makes the whole codebase non-OSI, trips
  enterprise `cargo-deny` and procurement allowlists, and repels the adoption the
  business depends on).
- Negative: Ghostlight as a product is no longer OSI open source. Free-software
  purists will object to the governance module, and the README and SPEC language
  "intended open-source" must be replaced with an accurate open-core statement
  (open engine, commercial governance).
- Negative: a self-hosted governance module is inspectable and bypassable; the
  accepted answer is source-available licensing plus the value of a maintained,
  supported, legally-licensed, audit-trustworthy product, not technical lockout. A
  careless small user who bypasses it was never a buyer.
- Negative: open-core structurally tempts a vendor to starve the free core; the
  standing commitment is to keep shipping engine improvements and never withhold a
  fix. The CLA on the governance module adds contribution friction and is read by
  some as a relicensing-risk signal, mitigated by keeping the engine contribution
  path DCO-only and frictionless, and by a public commitment that the engine stays
  OSI open and will not be relicensed.
- Relationship to ADR-0021: this narrows ADR-0021's whole-repo permissive stance
  to the engine and shared crates only, and gives its anticipated workspace split
  an earlier, licensing-driven reason to happen (to place the license boundary
  between engine and governance). ADR-0021's brand, family, and naming decisions
  are unaffected.
- Execution is scheduled by ADR-0026 Decision 1 (the LICENSE files, the crate
  license fields, and replacing the stale "TBD (intended open-source)" strings).
