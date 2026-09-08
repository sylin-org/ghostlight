# ADR-0163: Persistent history compatibility

- Status: Accepted
- Date: 2026-09-08
- Amends: ADR-0156, ADR-0159

## Decision

The owner requires product-wide resilience through leniency: tolerate harmless variation and
nonessential failure while preserving useful work. This ADR applies that general directive to
history. It does not claim to audit or repair every compatibility boundary in the product.
The Linux downgrade failure is a resilience defect, not something documentation alone fixes.
Public 1.3.4 rejects the added `permissions` field in 1.3.5 history and fails during startup.
Changing current source cannot change that already-published reader.

Historical receipts are observations, not executable commands or current authority. Readers
ignore unknown top-level receipt and gap fields while retaining known fields. The writer remains
typed and content-minimized: ignored data never enters a new receipt or a human/model projection.
Reading never rewrites, deletes, migrates in place, or truncates the source history.

Known required fields and nested semantic types still validate. An unsupported variant or damaged
record is omitted from the bounded projection, counted in existing history health, and preserved
on disk. Valid neighbors remain available. Such an omission must not prevent service startup,
controls, or new work. Require audit continues to depend on the ability to save new receipts;
unreadable old history is not a current write failure. Policy and live protocol inputs remain
strict. Compatibility must never turn an unknown policy restriction into permission.

Additive fields are the default evolution path; new optional fields need reader defaults. A
semantic change that cannot be read safely needs an explicit migration or isolated versioned
storage before release, preserving original evidence. Do not add such a mechanism speculatively.

## Acceptance

Run the actual service and connector against history written by the published predecessor, then
restart and prove new work and saving still succeed with original bytes intact. Test unknown
fields and unsupported records separately, including readable neighbors and omission reporting.
Fabricated future records test the compatibility rule but do not establish real-version acceptance.

Supported downgrade acceptance must run the actual predecessor on successor-written state. The
known public 1.3.4 failure remains open until an explicit compatible delivery/storage strategy is
implemented and verified; a passing current reader must not be reported as fixing that binary.
