# Ghostlight Data Processing Addendum (Template)

> **DRAFT -- template pending counsel review. Not an offer.** This document is published for
> transparency and early review. It becomes binding only when executed in writing by both
> parties after legal review.

This addendum is short because the fact it records is short: the Vendor processes no
Customer personal data. It is shaped against the Common Paper Data Processing Agreement v1.1
(a counsel-drafted standard form published under CC BY 4.0), reduced to its degenerate case:
every processing term resolves to none, stated outright instead of left as blank fields.

## 1. Recitals

The Ghostlight software runs entirely on Customer infrastructure. The Vendor receives, stores,
and processes no Customer personal data through the software: there is no vendor-side service in
the path of Customer's use, and no data flows to the Vendor, as established in
[data-flows.md](data-flows.md) and foreclosed by ADR-0028 Decision 9 (never phone home). This
addendum records that fact rather than papering over a data flow that does not exist.

## 2. Controller and processor roles

Because the Vendor processes no Customer personal data, the controller-to-processor clauses of a
conventional DPA are not engaged. There is no processing by the Vendor to instruct, restrict, or
audit. Customer remains the controller of any personal data Customer processes on its own
systems using the software; that processing is Customer's, not the Vendor's.

## 3. If vendor processing were ever introduced

If a future Ghostlight service ever introduced Vendor-side processing of Customer personal data,
the parties would execute a conventional DPA covering that processing before it began. The
parties pre-commit to the shape of that future DPA now: processing only on Customer's documented
instructions; no subprocessor without Customer's prior approval; security-incident notification
to Customer without undue delay and no later than 72 hours after discovery; and, for any
restricted international transfer, the then-current EEA Standard Contractual Clauses and UK
Addendum (or their successors). This addendum does not authorize any such processing.

## 4. Subprocessors

None. The Vendor engages no subprocessors, because it processes no Customer personal data. See
[sub-processors.md](sub-processors.md).

## 5. International transfers

None. No Customer personal data is transferred to the Vendor, so no cross-border transfer of
such data to the Vendor occurs.

## 6. Breach notification

The Vendor holds no Customer personal data to breach. For the vendor-side compromise scenario
that does apply (the build, signing keys, or update channel), the advisory commitment is stated
in [security-overview.md](security-overview.md).

## 7. Audit and due diligence

In place of the processor audit a conventional DPA provides, Customer has something stronger on
the axis that matters here: the governance module is source-available, so the code that would
do any processing is readable at any time, and [data-flows.md](data-flows.md) plus the
CAIQ-shaped [questionnaire.md](questionnaire.md) serve as the due-diligence record. The Vendor
will answer one due-diligence questionnaire per year on request.

## 8. Term

This addendum runs concurrently with the agreement it accompanies and terminates with it. No
post-termination data return or deletion obligations arise, because the Vendor holds no
Customer personal data to return or delete.

Last reviewed: 2026-07-10 against v0.7.3 | Contact: support@sylin.org
