# Ghostlight Continuity

Continuity is the question of what happens to your deployment if the license lapses, the
policy source goes dark, or the vendor ceases to exist. Ghostlight's answer is structural: the
software keeps working regardless, and this page shows why and how to prove it.

## The Continuity Promise

The Continuity Promise is binding on all tiers. Its normative wording, from ADR-0028, is:

> **The Continuity Promise.** Ghostlight never phones home and license state never affects
> behavior. Enforcement, audit, and your production workflows are never interrupted,
> degraded, or disabled by license expiry, by the vendor's unavailability, or by the
> vendor ceasing to exist. An expired license changes exactly one thing: license-state
> notices appear in `doctor`, `license status`, and your own audit records until it is
> renewed. Your deployment works forever, offline, as-is.

## Why this holds structurally

The promise is not a policy that could be reversed; it is a property of how Ghostlight is
built. License state never gates behavior: enforcement and audit run identically whether the
license is valid, expired, or absent. No vendor runtime sits in your critical path, so there
is no vendor service whose outage could degrade you. Central policy operates from a
last-known-good cache that keeps enforcing when the source is unreachable, and there is no
cache auto-expiry, because validity is anchored in the policy signature rather than a clock.
When nothing valid is available at all, Ghostlight fails closed to the protective state and
never fails open to unrestricted. Every one of these is a design invariant, not a runtime
check that could be toggled off.

## Verify it yourself

You do not have to take the promise on faith. Each of the following runnable scenarios
exercises one leg of it:

    cargo run -p ghostlight-lightbox -- run continuity-source-unreachable

This proves enforcement continues from the last-known-good cache when the policy source cannot
be reached.

    cargo run -p ghostlight-lightbox -- run fail-closed-cold-boot

This proves a cold boot with no policy available fails closed to the protective state rather
than opening up.

    cargo run -p ghostlight-lightbox -- run rollback-guardian

This proves a stale or downgraded but validly signed policy is refused, so protection cannot
be silently weakened.

## If the vendor ceases to exist

Your deployment keeps working, exactly as it did the day before. The automation engine is
licensed Apache-2.0 OR MIT and the governance module is source-available, so you hold the
actual code rather than a promise about it, which is stronger than a source-escrow arrangement
that releases only on a trigger. Everything you were doing, you can keep doing: enforcement,
audit, central policy from your own endpoint, all unchanged. The one thing that changes over
time is that there are no new releases. This page makes no commitment to future maintenance, a
successor maintainer, or a foundation handoff; it commits only to the property that what you
already run continues to run.

See [ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md) and the
[licensing guide](../guides/licensing.md).

Last reviewed: 2026-07-10 against v0.5.4 | Contact: support@sylin.org
