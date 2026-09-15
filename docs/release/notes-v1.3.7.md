# Ghostlight 1.3.7

Ghostlight 1.3.7 streamlines internal execution pipelines, modularizes the CLI,
centralizes catalog invariants, and adds autonomous visual settlement heuristics
and first-class workspace discovery and switching.

## Highlights

- Modular CLI decomposition: separated the monolithic orchestrator entry point into
  cohesive single-responsibility modules under `ghostlight::cli::` (`parse`, `setup`,
  `doctor`, `status`, and `desktop`) with unified intent dispatch (ADR-0177).
- Universal execution templates: standardized target lookup, authorization checks, and
  terminal error dispatch with `with_authorized_tab`, `with_authorized_target`, and
  `with_authorized_optional_target` across all operation handlers (ADR-0176, ADR-0177).
- Sane defaults for visual settlement: `browser_screenshot` and composite `browser_wait`
  default to autonomous visual settlement (`visual_settle: true`, ADR-0174).
- First-class workspace switching: 24th MCP catalog tool `browser_workspace` provides
  zero-argument discovery and dynamic runtime session rebinding (ADR-0175).
- Cross-workspace tab discovery: structured attribution and actionable guidance when tab
  handles belong to foreign workspaces (ADR-0172).
- Settle policy value objects & centralized catalog invariants: clean wire contracts
  and centralized tool counts across language and test surfaces (ADR-0176).

## Compatibility

Compatible with Chrome adapter 1.1.4 through 1.1.8. Chrome adapter 1.1.7 is
published in the Chrome Web Store; adapter 1.1.8 is in review.

## Install

Use `npx -y ghostlight@1.3.7`, the Windows installer, Debian package, or portable
archives. Release assets include checksums and GitHub build-provenance attestations.
