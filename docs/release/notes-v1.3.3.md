# Ghostlight 1.3.3

This patch release makes page semantics consistent across observable shadow DOM and iframe
boundaries, and adds an explicit repair for incorrect MCP client registrations. The matching Chrome
adapter advances to 1.1.1.

## Added

- **Explicit integration repair (ADR-0154).** A parseable foreign command occupying Ghostlight's
  key now offers a confirmed per-target `Fix` action in MCP integrations. Ghostlight re-checks the
  file, backs it up, replaces only its own entry, and preserves unrelated configuration. Automatic
  setup still never overwrites foreign entries.

## Changed

- **Composed full-page behavior (ADR-0151 through ADR-0153).** The shortest `browser_read` call now
  reads visible text across the top document, open shadow roots, assigned slots, and injected
  HTTP(S) frames under one global limit. Find, document inspection, text waits, accessible names,
  iframe geometry, point receipts, and coordinate drops follow those observable boundaries too.
- Explicit article reading remains available and falls back to the composed full page when no
  useful article exists. Negotiated capability revisions make older adapters refuse the stronger
  behavior instead of returning incomplete results.

## Install

Use `npx -y ghostlight@1.3.3`, the NSIS installer, the Debian package, or the portable archives.
Install Chrome adapter 1.1.1 from the Chrome Web Store. Every release artifact is checksum-bound
and provenance-attested; see `SHA256SUMS` and `release-candidate.json` in the release assets.
