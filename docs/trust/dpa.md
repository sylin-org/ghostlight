# Ghostlight Data Processing Addendum (Retired)

> **RETIRED 2026-08-25 by [ADR-0140](../adr/0140-fully-open-source-licensing.md).** This page
> previously held a draft no-processing DPA template shaped for executed customer agreements.
> With paid licensing withdrawn there is no agreement to append it to. The exact former text
> remains recoverable from git history at this path.

The fact the addendum recorded has not changed, and you never needed a contract for it:

- The Ghostlight software runs entirely on your infrastructure. The vendor receives, stores, and
  processes no personal data through the software: there is no vendor-side service in the path of
  your use and no data flows to the vendor, as established in [data-flows.md](data-flows.md) and
  foreclosed by [ADR-0028](../adr/0028-tripwire-licensing-and-continuity-promise.md) Decision 9
  (never phone home).
- The vendor engages no subprocessors ([sub-processors.md](sub-processors.md)).
- You remain the controller of any personal data you process on your own systems with the
  software; that processing is yours, not the vendor's.
- In place of a processor audit right, the entire product -- including the code that would do any
  processing -- is open source under Apache-2.0 OR MIT, readable at any time, and
  [data-flows.md](data-flows.md) plus the CAIQ-shaped [questionnaire.md](questionnaire.md) serve
  as the due-diligence record.

If a future Ghostlight service ever introduced vendor-side processing of customer personal data,
a conventional DPA would be published and negotiated before that processing began. The
pre-commitments of the retired template (instruction-bound processing only, no subprocessor
without approval, 72-hour incident notification, current SCCs for restricted transfers) remain
the project's stated shape for that future.

Last reviewed: 2026-08-25 | Contact: hello@sylin.org
