# Ghostlight continuity

Ghostlight has no vendor service in the runtime path. An installed copy keeps working without
telemetry, activation, an update server, or access to Sylin infrastructure. The source license is
contractual only: Ghostlight 1.0 has no license-status command, runtime license gate, or license
field in audit.

Signed managed policy also remains customer-controlled. A device verifies and caches the last
accepted bundle. Source failure, malformed bytes, a bad signature, or rollback leaves that bundle
active. A configured cold start without a valid source or verified cache fails closed. The cache
has no automatic expiry, so an organization outage cannot silently remove protection.

These behaviors are exercised by unit and service tests for:

- unreachable managed source at cold start;
- offline recovery from a verified cache;
- bad updates and rollback retaining last-known-good;
- signature verification on cache read; and
- the absence of policy network work when no bootstrap exists.

If the vendor ceases to exist, installed software and organization-hosted policy continue operating as
before. The entire product is Apache-2.0 OR MIT ([ADR-0140](../adr/0140-fully-open-source-licensing.md)),
so the source you run is the source you hold the rights to. This is a continuity property of the
shipped software, not a promise of future releases or successor maintenance.

See [the licensing guide](../guides/licensing.md),
[the governance guide](../guides/governance-configuration.md), and
[ADR-0121](../adr/0121-restore-rawx-policy-and-managed-fetch.md).

Last reviewed: 2026-08-25 against the 1.0 source candidate | Contact: hello@sylin.org
