# Issuing a Ghostlight license (founder runbook)

How the founder creates, signs, and delivers a license. This is the offline,
air-gapped side of the engine; the customer-facing side is
[docs/guides/licensing.md](../guides/licensing.md). The design and the non-negotiables
are ADR-0028 (Decisions 10 and 11); this page is the operational recipe.

**The one rule that never bends: the signing seeds live only on an air-gapped machine.**
They never enter the repository, CI, a server, or a synced folder. Everything online (the
public keys, the customer claims, the finished signatures) is safe to publish; the seeds
are the whole security of the scheme.

## Two things to know first

- **Generations.** Every key has a generation number. **Generation 0 is public** (the seed
  `ghostlight development key gen0!` is in the source) so anyone can self-sign an
  *evaluation* license; you never issue those. **Generations 1 and up are yours**, private,
  and **composite**: each license is signed with BOTH an Ed25519 key and an ML-DSA-65
  (post-quantum) key, and both must verify. You sign production licenses with a private
  generation.
- **There is no revocation.** The binary never phones home, so a license cannot be killed
  remotely; it simply expires. Terms are annual. A leaked or refunded license is handled by
  not renewing it (and, if a *signing seed* leaks, by rotating to a new generation -- see
  the end). This is deliberate (ADR-0028 Decisions 1 and 9).

## Build the authoring tool (air-gapped machine)

The `sign` and `pubkey` subcommands are gated behind the `license-admin` feature and are
**never** in a release build. On the air-gapped machine:

    cargo build --release --features license-admin
    # then run target/release/ghostlight, or use `cargo run --features license-admin --`

## One-time: create a production generation

Do this once to stand up generation 1 (repeat with the next number to rotate later).

1. Generate two 32-byte seeds and back them up offline (an encrypted drive, one copy):

       openssl rand 32 > gen1-ed.seed
       openssl rand 32 > gen1-mldsa.seed

2. Print the two public keys:

       ghostlight license pubkey --seed gen1-ed.seed --mldsa-seed gen1-mldsa.seed
       # ed25519   <64 hex chars>
       # ml-dsa-65 <hex...>

3. Embed those public keys in `crates/core/src/governance/license/crypto.rs`, in the
   `verifying_key` table. There is a commented `Composite` arm for generation 1 showing the
   exact shape; paste the two keys in as byte constants and uncomment it. This is a normal
   code change: commit it, and ship a release. From then on, any binary at that release or
   later can verify generation-1 licenses. (The public keys are safe to commit; the seeds
   are not, and never are.)

Nothing to do again until you rotate.

## Per customer: sign and deliver

1. Write the claims (one JSON object). `id` is a fresh UUID v4; `products` is `["browser"]`;
   dates are `YYYY-MM-DD`; `tier` is one of `evaluation`, `community`, `founding`, `team`,
   `enterprise`. `seats` and `licensee` are carried for the record and never enforced at
   runtime.

       {
         "id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
         "licensee": "Acme Corp",
         "org": "acme",
         "tier": "team",
         "seats": 25,
         "products": ["browser"],
         "issued": "2026-07-10",
         "expires": "2027-07-10"
       }

   You set `tier`, `seats`, and `expires` from the actual purchase; the buyer only supplies
   their identity (`licensee`, `org`). The signature makes it impossible for them to edit any
   field after the fact.

2. Sign it (composite, generation 1):

       ghostlight license sign \
         --seed gen1-ed.seed --mldsa-seed gen1-mldsa.seed \
         --keygen 1 --claims acme-claims.json --out acme-license.json

   This writes `acme-license.json` and prints the armored `-----BEGIN GHOSTLIGHT LICENSE-----`
   block to stdout. Either form installs (see the customer guide).

3. Deliver the armored block (or the `.json`) to the customer by email, and **commit the
   claims JSON** -- not the seed, not the license file, just the claims -- to the private
   `ghostlight-licensing` ledger repo as the record of what was issued.

Because licensing is observational, the customer's binary already works; the signed license
only silences the audit marker, so a few hours between purchase and delivery is invisible.

## The issuance pipeline

At volume, a Polar.sh purchase webhook files an "order intent" (identity + tier + expiry)
into the private ledger repo; you drain that queue on the air-gapped machine, batch-sign,
and reply. The webhook never signs -- the seed stays offline (ADR-0028 Decision 10). Until
volume warrants it, issuing by hand as above is the whole process; at ~ten organizations a
year it is minutes a month.

## Renewals

A renewal is a fresh signature with a later `expires` and the same generation. A scheduled
GitHub Action in the ledger repo opens a reminder issue at T-30 and T-7 before each expiry;
sign the renewal, deliver it, and the customer installs it over the old one. Lead with the
Continuity Promise: nothing stops working; renew when procurement is ready.

## Rotating a generation

If a signing seed is ever exposed, do NOT try to revoke (you cannot). Instead:

1. Create generation 2 exactly as above and ship a release embedding its public keys.
2. Sign all new and renewing licenses with generation 2.
3. Leave generation 1 in the `verifying_key` table so existing gen-1 licenses keep verifying
   until they expire; drop it in a later release once they all have.

Never remove a generation while valid licenses still depend on it, and never enable the
`license-admin` feature in a published build.
