# ADR-0091: Store-only end-user extension installation

- Status: Accepted
- Date: 2026-07-31
- Amends: ADR-0070 Decision 1, Decision 3, and consequences

## Context

ADR-0070 allowed the canonical browser handoff to offer a release archive while the Chrome Web
Store listing was pending. The listing is now public. Keeping the temporary path in current docs
creates two journeys, asks ordinary users to enable developer controls, and makes agents explain a
choice that no longer helps them.

Source builders still need to exercise the repository extension against a local binary without
waiting for a store release. That is a development workflow with a different audience and purpose.

## Decision

### 1. Packaged and public installs use the store

Every end-user installation surface points to the public Chrome Web Store listing. This includes
the README fast path, agent guide, installation guide, service-first website handoff, website agent
documents, and troubleshooting copy.

Release extension archives remain packaging and store-submission artifacts. They are not an
end-user installation option.

### 2. Source builders may load the local extension

Documentation for people building Ghostlight from source may explain how to load the repository's
`extension/` directory as a development extension. Keep that instruction inside an explicitly
labeled source-development section so it cannot be mistaken for the supported packaged journey.

### 3. The canonical handoff no longer selects a distribution fallback

The stable service-first page from ADR-0070 links directly to the Chrome Web Store. It may add
another official browser store later, but it does not fall back to an archive or a development
extension.

### 4. Checks hold the boundary

Repository and website checks reject alternate end-user installation language on current public
surfaces. The checks allow the source-development exception and do not rewrite historical ADRs,
research, or task ledgers.

## Consequences

- End users and installation agents see one browser-extension action.
- The source tree remains immediately testable.
- The website can go offline after installation without affecting the local runtime.
- ADR-0070's bidirectional handoff, one-time opening behavior, privacy boundary, and `--no-open`
  control remain unchanged.
