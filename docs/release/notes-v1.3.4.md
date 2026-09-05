# Ghostlight 1.3.4

Agents can now use local HTTP(S) development servers. Localhost, loopback, and link-local
destinations follow ordinary host policies, with no built-in ban or local-access toggle.

This release includes the composed-page behavior and integration repair prepared in the
unpublished 1.3.3 candidate. Chrome adapter 1.1.1 is unchanged.

## Fixed

- **Local browser destinations follow policy (ADR-0155).** Removed the address-specific
  restrictions, including IPv4-embedded IPv6 cases. Host grants, RAWX capabilities, request
  restrictions, observe/enforce modes, and policy-defined never-touch destinations apply to
  local and remote HTTP(S) work alike. Non-HTTP(S) schemes retain their existing boundary.

## Also included from the 1.3.3 candidate

- **Composed full-page behavior (ADR-0151 through ADR-0153).** Default reading spans the top
  document, open shadow roots, assigned slots, and injected HTTP(S) frames. Find, document
  inspection, text waits, accessible names, iframe geometry, point receipts, and coordinate
  drops follow the same observable boundaries. Explicit article reading remains available.
- **Explicit integration repair (ADR-0154).** A parseable foreign command occupying Ghostlight's
  key offers a confirmed per-target `Fix`. It re-checks the entry, backs up the file, and replaces
  only Ghostlight's entry. Automatic setup still preserves foreign entries.

## Install

Use `npx -y ghostlight@1.3.4`, the NSIS installer, the Debian package, or the portable archives.
Install Chrome adapter 1.1.1 from the Chrome Web Store. Artifacts are checksum-bound and
provenance-attested; see `SHA256SUMS` and `release-candidate.json` in the release assets.
