# ADR-0055: managed:// central policy distribution -- consumer design, transport-agnostic signed trust, and delight principles

Status: Accepted (design + delight principles; 2026-07-10; owner: "Capture all in an ADR", and on
register "for a corporate solution ... keep professional while presenting a clean, elegant,
easy-to-understand UX ... give the org enough customization options so it's presented neatly and
clearly, and give users clear communication channels"). Implementation deferred: `managed://` stays
reserved-but-unsupported in code (`SourceError::ManagedNotSupported`) until its own batch. Realizes
the `managed://` item deferred by ADR-0026 and named as a paid-governance feature by ADR-0027
Decision 3; the license gate at `crates/core/src/hub/mod.rs` already anticipates
`ManifestOrigin::Managed`. The two previously flagged sub-decisions are now resolved (owner,
2026-07-10): D4 auth roadmap is token-v1 / enroll-v2; D6 adopts the anti-rollback sequence,
conditioned on the delight ergonomics in D6 and D9.

## Context

Today an organization deploys Ghostlight governance policy as a local FILE, dropped by its existing
management channel (GPO / Intune / MDM) to a system location, loaded once at startup (ADR-0023) and
hot-reloadable in place (ADR-0025). `managed://` is the deferred next step: the endpoint instance
FETCHES its policy manifest from the organization's OWN network endpoint instead of reading a local
file. It was named as a not-yet item in ADR-0026, listed as a paid-governance-module feature in
ADR-0027 Decision 3 ("central management including `managed://`"), and reserved in code as a precise
unsupported error (`crates/core/src/governance/manifest/source.rs`).

`managed://` is unique among Ghostlight features in one respect that shapes the whole design: its
buyer and its user are different people with opposed interests. The organization (buyer) wants
central control over what the agent may do in the employee's authenticated browser. The developer
(user) wants freedom and privacy. Every other Ghostlight capability serves both at once; here they
pull apart. A design that silently serves only the buyer produces the default enterprise-policy
experience -- opaque restriction the user bumps into with no explanation. This ADR chooses instead
to make Ghostlight the governed person's honest broker: it enforces the org's policy faithfully AND
stays visibly on the user's side while doing so.

Two constraints bound the mechanism: SPEC section 10 excludes "No remote policy service (HTTP
manifest source)", and ADR-0028 Decision 9 is a permanent, marketing-visible promise to never phone
home. A prior-art sweep (2026-07-10, four parallel research agents across OPA/OPAL/Envoy-xDS,
enterprise policy distribution, feature-flag systems, and Vault-Agent/osquery/Puppet/SPIFFE) found
four unrelated domains converging on one consumer-side
design, which this ADR adopts.

## Decision

### 1. Reconciliation with SPEC section 10: managed:// is a distinct mechanism; the exclusion stands.

SPEC section 10 rejected a VENDOR-operated remote policy service -- a hosted control plane that
authors or deploys org policy in place of file-over-MDM. This is the same scope ADR-0020's amendment
already drew for its sibling "no remote policy service / no SaaS control plane" line: the rejection
was about who OPERATES the surface. `managed://` is a client that fetches from the CUSTOMER's own
endpoint; Ghostlight operates and hosts nothing. Different actor, never in scope. SPEC section 10's
text stands unchanged; this decision records why `managed://` does not violate it. The rejected
alternative -- declaring the exclusion superseded -- is weaker: it reads as loosening a public
promise when the honest fact is that we are simply not building the excluded thing. The prior art
reinforces the reading: every system surveyed is either customer-operated or vendor-operated, and
being the customer-operated one is precisely our differentiator (see D7).

### 2. Never-phone-home boundary: managed:// is the inbound mirror of a configured audit destination.

ADR-0028 D9 forbids network I/O for a "vendor-serving purpose" and explicitly permits "the user's
own configured audit destinations." A `managed://` fetch is user-configured, user-serving governance
I/O in exactly that permitted category -- the inbound mirror of the outbound audit destination.
Nothing flows to the vendor. Invariant: the free / all-open path stays network-silent. `managed://`
activates only when an org configures it (like the org policy file); a default install makes zero
network calls, preserving the never-phone-home promise byte-for-byte for every non-configured user.

### 3. Consumer architecture: persist-and-pull around the machinery we already own.

The four researched domains converge on one loop, which we adopt: boot -> load last-known-good from
disk (verify) -> serve immediately -> fetch on a bounded interval (conditional / ETag) -> verify ->
atomic-swap -> persist -> on ANY failure keep last-known-good. Pull, not push: the endpoint
initiates, so it works behind NAT and firewalls with no inbound listener, survives source downtime,
and reaches out only when it chooses -- which is the never-phone-home posture by construction.
Conditional fetch (ETag / `If-None-Match`) makes a no-change poll cost a 304, so short intervals stay
cheap, and the last-good ETag doubles as a version handshake.

We already own the two hard pieces: signature verification (the licensing stack) and atomic apply
(manifest hot-reload, ADR-0025). `managed://` is mostly the fetch-loop + write-through cache +
capped backoff wrapper around them; a fetched, verified bundle is just a new manifest source fed to
the existing loader (ADR-0023) and reload path. In `ManifestOrigin` terms a `Managed` variant joins
`{OrgPolicyFile, UserFile, UserEnv}`, and the last-known-good cache is the last verified bundle on
disk. This is the lean-internals commitment: no new subsystem, a wrapper over verified machinery.

### 4. Auth: static bearer token (v1), enrollment to a per-device credential (v2), skip SPIFFE.

Owner decision (2026-07-10): token as v1, enroll as v2.

v1 -- a static hardened bearer token. The management channel (GPO / Intune / MDM) drops a token
file exactly as it drops today's policy file, the same mental model as a user-configured audit
destination. The client sends `Authorization: Bearer` and PINS the org endpoint's CA (does not trust
WebPKI) -- a cheap defense against DNS/MITM of the policy source, harvested from osquery
`tls_server_certs`. This is defensible ONLY because bundles are signed (D7): the token guards read
access and rate-limiting; the signature guards authenticity. A stolen token can read or replay stale
policy but cannot forge one or fail the box open. The on-disk secret is encrypted at rest and
machine-bound (DPAPI / TPM). The customer endpoint stays a dumb HTTPS host -- v1 is a
plain conditional GET against any host the fleet can already reach.

v2 -- an enrollment exchange: the endpoint trades a broadcast bootstrap secret ONCE for a
per-endpoint credential it then uses for every subsequent fetch (Chrome enrollment-token -> DM
token; osquery enroll-secret -> `node_key`; SPIRE attestation -> SVID). This buys per-device
revocation and a small blast radius (the broadcast secret is never the long-lived one), and the
bootstrap secret is deleted after first read so a copied disk cannot replay. The natural form of the
per-endpoint credential is an mTLS client cert minted through the org's existing machine PKI (Intune
SCEP / AD CS): the private key never leaves the box and rotation is the cert lifetime. The cost v2
accepts over v1: the customer runs a REGISTRATION endpoint, not just a dumb file host. We SKIP full
SPIFFE/SPIRE -- it is the right reference SHAPE for unattended enrollment and mTLS rotation, but a
SPIRE server plus per-node agent plus attestor plugins is far too much stack to fetch a policy file;
borrow the shape (enroll -> per-device cert, and authenticate with a platform identity the box
already holds where one exists), not the dependency. Because v1 signing already removes the forge and
fail-open risks, v2 is a hardening upgrade (per-device revocation), never a correctness prerequisite.

### 5. Caching and Continuity: write-through last-known-good, signature-gated validity, two failure modes.

The last-known-good cache is the default operating state, not a fallback branch. Persist every
VERIFIED bundle to disk; load it at boot before the first fetch (in parallel with the fetch, never
gated on the network -- the Unleash pattern); keep enforcing it whenever the source is down (OPA
`persist`, Puppet `use_cache_on_failure`). This is the Continuity Promise mechanized, reinvented
independently across every domain surveyed.

There is no cache auto-expiry. Validity is gated by the SIGNATURE, not a TTL: an air-gapped or
disconnected box must enforce its last signed policy indefinitely, and an expiry would reintroduce a
fail-open path. Credential lifetime (short, rotating -- the SPIRE model for D4's mTLS) is decoupled
from policy lifetime (indefinite on last-known-good). Staleness is surfaced, not acted on (D9).

Two failure modes, one invariant. UNREACHABLE (source down -> keep the last-activated bundle -- OPA,
Puppet) and REACHABLE-BUT-BAD (a fetch that returns a malformed, bad-signature, or bad-schema bundle
-> reject it and keep the last VALID bundle -- the Envoy xDS NACK-and-keep-last-valid discipline)
both retain last-known-good. Signature and schema are verified BEFORE the atomic swap; any failure
keeps the old policy. There is no code path to unrestricted.

The on-disk cache is signed AND encrypted, and its signature is verified on load-FROM-cache, not
only on fetch. Every feature-flag SDK surveyed writes a plain-JSON backup that is trivially swapped
on disk to inject a permissive policy -- a back-door fail-open. Verifying the cache on load closes
that hole; the cache is as exposed as the wire and gets the same trust check.

### 6. Revocation and propagation: pull floor plus optional customer-hosted push; anti-rollback counter.

The guaranteed interval pull is the correctness path. Any immediacy is an OPTIONAL accelerator and
never a correctness dependency: a content-free "refetch now" wake (the simplest device-management
payload never transits the push cloud) delivered by long-poll / SSE / conditional-GET against the
CUSTOMER's own endpoint. Never third-party push clouds: they are external
dependencies, phone-home-shaped and off-limits. Absent a push accelerator, the pull interval plus the
signed-cache floor fully satisfies governance. Revocation is tightening: the customer's endpoint
serves a stricter policy on the next poll, aligned with ADR-0028's "revocation is expiry" model (no
CRL, no kill switch). Fetch failures back off with jitter, capped, and retry forever -- never
exit-open (Vault Agent 1s -> 5m).

Anti-rollback (owner decision 2026-07-10, adopted through the delight lens -- "does it bring
delight?", see D9): a MONOTONIC PUBLISH SEQUENCE carried INSIDE the signed bundle. The endpoint
refuses any bundle whose sequence is below what it currently holds, so an attacker or a stale mirror
serving an OLD but validly-signed policy cannot downgrade a box to a more permissive past version --
the one gap neither bearer nor mTLS closes. It is adopted BECAUSE the guardian behavior it enables is
delightful (D9 invariant ii: the tool actively refusing to let protection be silently weakened), on
the condition of three ergonomics that keep it a guardian and never an opaque wall: (a) the sequence
is auto-incremented at SIGN time so a human never hand-maintains it and cannot footgun a legitimate
update into silent refusal; (b) a refused rollback is a DOOR, not a wall -- a plain guardian message
routed to the org contact channel and surfaced in the Passport and audit, never a silent drop; (c) a
monotonic publish SEQUENCE, not a wall-clock timestamp, so clock skew can never false-refuse a valid
update. Without those ergonomics the sequence is net-neutral-to-negative on delight and we would lean
purely on the expiry-as-revocation model above; with them it serves both the user (a visible
guardian) and the buyer (a stateable downgrade-resistance property). The sequence is a new field in
the signed policy payload.

### 7. Integrity: transport-agnostic, post-quantum signed policy.

The policy bundle is signed with the composite Ed25519 + ML-DSA-65 stack shipped for offline license
verification in v0.5.3, generalized to the policy payload. Because authenticity lives in the
SIGNATURE and not the connection, the transport becomes untrusted defense-in-depth rather than the
trust anchor. `managed://` can therefore point at any dumb HTTPS host, an S3 bucket, a corporate CDN,
a file share, or a USB stick, and AIR-GAP SNEAKERNET distribution uses the IDENTICAL trust model as a
network fetch -- the same signature check unifies the `file://` floor and `managed://`. This is the
strongest differentiator and it reuses crypto already shipped and verified. The contrast from the
prior art is sharp: OPA signs bundles with classical RS256 / ES256; Chrome, Intune, and device-management vendors anchor
trust in TLS plus server identity (so the channel MUST be trusted); the feature-flag SDKs do not sign
the payload at all. No surveyed system offers post-quantum-signed, transport-independent central
policy.

### 8. Licensing tie-in: ManifestOrigin::Managed is the strongest governance-operational signal.

When `managed://` is built, `ManifestOrigin::Managed` joins `OrgPolicyFile` in the
`governance_operational` match at `crates/core/src/hub/mod.rs` -- the one-line addition already
anticipated by the code comment there. This is not merely mechanical parity: a fetched, centrally
distributed, signed policy is the CLEANEST "an organization is operating governance" signal we have,
stronger than a local org-policy file (which a solo developer could hand-place). The license stamp
stays purely observational (ADR-0028): `managed://` changes what triggers the stamp, never behavior.

### 9. Delight principles (normative): Ghostlight is the governed person's advocate inside the governance.

Register is PROFESSIONAL and elegant on the enterprise surface (owner decision): delight here comes
from clarity, honesty, and respect, not theatrics. The mascot and visual-FX personality (the
sky-blue ghost and lantern) stay on the personal, user-first surfaces; the `managed://` surfaces are
restrained and clean. The buyer-vs-user tension is resolved by making the tool the governed person's
honest broker, expressed as four invariants the implementation is CHECKED AGAINST, not decoration:

- **(i) The governed can always see how they are governed.** Provenance (org identity, source, fetch
  time), freshness (FRESH / STALE / LAST_KNOWN_GOOD, borrowing OpenFeature's STALE state), and
  verification state are always visible via the Console (ADR-0030 D9 / ADR-0020 amendment) and
  `explain` (ADR-0022). Policy can never suppress them.
- **(ii) Governance can never secretly weaken protection.** Policy is signed and cannot be forged
  (D7), the free path is network-silent (D2), and every fetch and activation is auditable. No
  configuration makes governance opaque or fails the box open (D5).
- **(iii) Sacred stays sacred against everyone, including the org.** The sacred never-touch domains
  remain off-limits to automation under any policy; no `managed://` policy can widen them, and the
  passport says so plainly. This is the tool protecting the user even against their own employer.
- **(iv) Offline is never the user's problem.** Source-unreachable is communicated as calm
  reassurance (running on verified policy from <time>, still protected), never as an error the user
  must resolve.

Org-authored presentation customization, additive only. A policy MAY carry optional presentation
fields -- org display name, a one-line policy rationale, and contact channel(s) (email / chat /
ticket URL) -- that flow into the Policy Passport and into every denial, so governance feels like it
comes from the user's OWN organization: accountable and contactable. The governing rule: policy can
add voice, never remove visibility. The org authors the friendly parts (its name, its rationale, how
to reach a human); the tool retains SOLE authorship of the truth-telling parts (provenance,
freshness, verification, sacredness). A presentation field that could hide, forge, or contradict a
truth-telling surface is rejected at validation. These are optional additive manifest fields (no
change to the trained tool surface; the exact schema is a batch detail).

Two signature surfaces realize the invariants:

- **The Policy Passport** (Console + `explain`): one human-plain answer to who governs me, from
  where, how fresh, verified how, what it permits, what stays sacred, and how to reach a human. The
  org-authored fields make it read as the user's own organization speaking, not an opaque tool.
- **Denials are doors, not walls.** A denial names the deciding policy and its freshness, routes to
  the org-provided contact channel, and tells the agent what IS permitted so it pivots instead of
  flailing -- extending the structured-denial and teaching-reject discipline already in place
  (ADR-0031, ADR-0049).
- **The guardian moment.** A refused policy rollback (D6) is surfaced, not silent: the tool tells the
  user it declined an older-than-current policy and stayed on the verified one, routed to the org
  contact channel. This is invariant (ii) made visible -- the tool actively refusing to let
  protection be weakened -- and it is the reason the anti-rollback sequence is adopted at all (D6). It
  is also why the sequence's ergonomics (auto-bump at sign time, never a wall-clock timestamp) are a
  delight REQUIREMENT, not an implementation nicety: a legitimate update silently refused would be the
  exact opaque wall this ADR exists to abolish.

## Implementation decisions (batch details resolved pre-implementation, 2026-07-10)

Owner-approved the same session, refining the HOWs that D3-D7 and D9 left as batch details:

1. **The policy-bundle trust anchor is the ORG's key, not Ghostlight's.** A license is signed by
Ghostlight's embedded keys (`license::crypto::verifying_key`); a `managed://` policy cannot be,
because the customer operates everything and the vendor signs nothing (D1). The org holds its OWN
composite (Ed25519 + ML-DSA-65) keypair and signs its policy bundle with it; the endpoint is
provisioned with the org's PUBLIC key over the same MDM channel that drops the source URL and the
bearer token. Ghostlight embeds no policy key. Customers therefore get a signing tool (`ghostlight
policy keygen` / `sign` / `publish`), the customer-facing analog of the founder-only `license sign`.
Domain separation: policy bundles sign under the context `ghostlight/policy`, distinct from
`ghostlight/license`, so a signature minted in one domain can never verify in the other.

2. **The composite-signature primitive is lifted into a shared `governance::crypto` module,
context-parameterized.** `verify(key, ctx, claims, sig_ed, sig_mldsa)` and the `admin` sign
primitives move out of `license::crypto` and take the domain-separation context as a parameter;
`license::crypto` becomes a thin wrapper that passes `ghostlight/license` and keeps its embedded-key
table, so licensing behavior stays byte-identical and its existing tests are the refactor's
regression guard. The sign primitives become always-compiled (policy signing is customer-facing);
the `license sign` COMMAND stays `license-admin`-gated (founder-only).

3. **The signed policy bundle mirrors the license envelope.** On-disk/on-wire form
`{v, claims, sig, sig_mldsa?}`, where `claims` is base64 of the canonical signed content
`{seq, manifest, presentation?}`, ASCII-armored as `-----BEGIN GHOSTLIGHT POLICY-----`. `seq` is the
monotonic publish sequence (the D6 anti-rollback field). `presentation` carries the additive-only
org-authored fields (D9): display name, one-line rationale, and contact channel(s). The signature
covers the manifest, the sequence, AND the presentation, so an attacker cannot swap the org's contact
for a phishing address without breaking the signature.

4. **HTTP client (Phase 3): `ureq` + rustls, feature-gated behind `managed-fetch`, quarantined behind
`fetch_bytes`** (amended 2026-07-10 via the delight lens, superseding the earlier `reqwest` pick).
The network path is the smallest, most isolated thing it can be. `ureq` (a small, readable dependency
tree) rather than `reqwest` (~100 transitive crates) keeps the supply-chain audit surface tiny for a
product whose pitch is a clean supply chain; a periodic poll needs no async client, so blocking on a
`spawn_blocking` thread is fine. The ENTIRE HTTP/TLS dependency lives behind the one
`governance::managed::fetch_bytes` seam, so verify/cache/reconcile stay network-agnostic and testable
without a server. It is a CARGO FEATURE (`managed-fetch`, on by default in the shipped binary,
removable): `--no-default-features` yields a pure-Rust, no-C, air-gap-only build as a first-class
artifact, which DISSOLVES the no-C-toolchain tension -- the audited `ring`-backed TLS exists only in
the network build, never in the air-gap one, and the signature crypto stays pure Rust everywhere. CA
pinning is a one-root rustls trust store (trust exactly the org's CA), not a hand-rolled verifier. TLS
is never load-bearing: trust lives in the signature and availability in the cache, so a TLS or pin
failure is just `FreshError::Unreachable` (keep last-known-good), and a pin mismatch surfaces as a
guardian door (Phase 5), never an opaque stack trace.

5. **Cache (Phase 2): sign in v1, defer machine-bound at-rest encryption.** Verifying the cache
signature on load closes the fail-open-via-tampered-cache hole (the security-critical half, reusing
the bundle verify); confidentiality encryption is platform-specific and mainly protects the bearer
token, which gets OS-appropriate protection instead. Full machine-bound cache encryption is later
hardening.

6. **Phase decomposition.** Phase 1 (this batch) splits into: 1a the shared-crypto refactor; 1b the
signed-bundle format (verify + sign, pure); 1c org-key config + `managed://` source parsing +
local-path load (the air-gap path, no network); 1d the customer `ghostlight policy` CLI. Phases 2-5
follow per the plan above. Each sub-step lands on a green tree.

7. **Phase 4b as-built (2026-07-10): unified PolicySource; a live fail-open found and fixed.** The
ConfigStore resolves policy through an injected `PolicySource { SourceString | Managed }` on every
re-resolve; the file watcher and the managed poll timer share that ONE path, and a resolve error is
keep-last-good, never a swap to all-open. This fixed a live fail-open the delight review surfaced:
the watcher's re-resolve re-ran the source-string loader, so a routine `config set` under managed
governance replaced the managed policy with unrestricted. `spawn_managed_poll` re-resolves on
`poll_seconds` (default 900s, per-process jitter). Proven by the lightbox `no-clobber-on-reresolve`
scenario (ADR-0056).

8. **Phase 5 surfaces (owner-approved via the delight lens): ManagedStatus + a status sidecar
bridges the process boundary.** A first-class `ManagedStatus { freshness, seq, fetched_at,
presentation, last_error }` is retained on every managed resolve, and a small VERSIONED (`v: 1`,
because admins will script against it), no-secrets `managed-status.json` sidecar is written
atomically beside the cache on each poll. Surface map: the `explain` TOOL renders the live Policy
Passport (who governs me, freshness, sacred, org contact) from live status; `doctor` answers the
admin's "did it propagate?" from the sidecar (seq, fetched-ago, source OK / the guardian doors)
without needing a service session; the Console reads live state. The guardian moment renders in both
registers (user: "still protected on your verified policy"; admin: "update rejected, still enforcing
seq N"). Presentation validation is additive-only sanity (length and control-character limits) so
org voice can never spoof or crowd out the truth-telling lines. Refinement (2026-07-10, batch
authoring): the SIDECAR IS the ManagedStatus store -- ONE writer (`governance::managed::activate`,
which both startup and every re-resolve flow through) writes it atomically; the `explain` tool,
`doctor`, and the Console all READ it. No new ConfigStore state; one artifact, three readers.

9. **Strategic riders (owner-ratified 2026-07-10: "EXCELLENT suggestions -- add all of them").**
(a) **The bundle carries a `kind` discriminator** (claims field, default `"policy"`, unknown kinds
REJECTED at verification): the signed/seq/last-known-good machinery is artifact-agnostic, so
managed:// is deliberately a GOVERNED CONTENT CHANNEL -- a future `kind` distributes ADR-0039 saved
scripts (org-published vetted workflows: governance as enablement, not only restriction). The org
keypair is intentionally THE ORG TRUST ROOT for all future org-authored artifacts.
(b) **Break-glass signed exceptions** (future `kind`: signed, time-boxed, offline-verified emergency
grants, Passport-visible and audited) are the anticipated answer to the emergency-loosening
objection; PARKED for their own ADR before implementation.
(c) **Audit provenance:** the managed publish sequence is stamped on TOOL-CALL audit records (the
same additive channel as the ADR-0028 license stamp; session-event shapes stay frozen; absent unless
origin is Managed, so the all-open stream stays byte-identical). Every decision becomes "made under
org-signed policy version N".
(d) **Fleet visibility rides existing tooling:** the versioned sidecar is a deliberate integration
surface -- the org's own osquery/Intune inventory reads it; no vendor infrastructure, no phone-home.
Positioning/docs, not code.
(e) **The bundle format is a publication candidate for `open-spec/`** (PQ-signed, transport-agnostic
policy distribution with additive-only presentation); PARKED, founder timing.
(f) **NEVER GATE the `policy` CLI (normative, permanent):** `policy sign`/`pubkey`/`publish` ship
ungated in every build. Ed25519-only evaluation keys plus the ADR-0028 tripwire stamp ARE the
try-before-buy funnel; adding a license gate would break both the funnel and the never-gate
philosophy.

## Consequences

- `managed://` becomes a thin, verifiable addition: a fetch-loop, a signed-and-encrypted
  write-through cache, and backoff, wrapped around the existing loader, signature verifier, and
  hot-reload path. The new `ManifestOrigin::Managed` threads through source selection, the license
  gate, and the audit origin.
- The organization gets central, tamper-proof policy distribution with zero new vendor
  infrastructure and zero phone-home: a conditional GET against infrastructure it already runs and
  MDM already trusts. This is the anti-SaaS, bring-your-own-endpoint posture, and the same trust
  story that makes the Continuity Promise credible.
- Air-gapped and regulated deployments are first-class, not bolted on: the signed bundle makes a file
  mirror or a USB stick the same trust model as a live fetch. This lands directly on the
  regulated-enterprise buyer.
- The governed user gains a legible, honest governance experience -- the Passport, visible
  verification, sacred-line protection against their own org, and calm offline behavior -- that no
  competing enterprise-policy tool offers, because none is designed to be on the user's side.
- Costs and boundaries: a new optional signed-payload field (the anti-rollback counter, D6) and
  optional presentation fields (D9); a bearer-token secret (or client cert) provisioned per endpoint
  and kept encrypted at rest; a validation surface that enforces the additive-only customization
  rule. Out of scope: a vendor-hosted control plane (D1), federated identity / OIDC / SAML / LDAP
  (SPEC section 10; D4 uses token or machine-PKI cert), any third-party push cloud (D6), and any
  behavioral effect of the license stamp (D8, ADR-0028).

## Provenance

Design and delight direction set in session on 2026-07-10, immediately after the offline licensing
engine and v0.5.4 shipped. The owner opened the topic ("then we'll discuss managed://"), directed the
prior-art sweep ("research prior art ... any good practices we could harvest? any strategic
opportunities?"), then "Capture all in an ADR." On register the owner chose professional over
playful for the corporate surface: "for a corporate solution ... keep professional while presenting a
clean, elegant, easy-to-understand UX ... give the org enough customization options ... and give
users clear communication channels" -- which sharpened rather than softened the advocate-for-the-
governed thesis and produced the additive-only customization rule (D9). The consumer architecture,
auth recommendation, Continuity model, and integrity model (D3-D7) are backed by a four-agent
prior-art study of OPA/OPAL/xDS, enterprise policy distribution, feature-flag SDKs, and secret/config
agents; those defaults are research-backed recommendations. The two initially
flagged sub-decisions were resolved by the owner the same day: D4 = "token as v1, enroll v2"; and D6
was decided through the delight lens (the owner asked "does it bring delight?") -- adopt the
anti-rollback sequence because the guardian behavior is delightful, conditioned on the three
ergonomics (auto-bump at sign time, refusal-as-door, publish-sequence not wall-clock).
Reserved-but-unsupported in code until the implementation batch.
