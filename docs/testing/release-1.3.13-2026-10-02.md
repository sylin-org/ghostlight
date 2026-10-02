# Ghostlight 1.3.13 release -- 2026-10-02

The owner authorized bump, changelog, commit, push and publication across configured
channels. This release uses the committed `f2e42368` browser/workbench candidate as
its base and adds ADR-0193's fixed-path download installation. Unfinished workspace
UI edits are excluded. Public state is reconciled from observed channel delivery.

## Local source gates

The isolated release checkout passes formatting, strict workspace clippy, workspace
tests, 404 extension tests, 11 npm tests and four MCPB tests. Versions are 1.3.13
across the workspace, desktop, adapter, npm launcher and MCPB. No dependency changed.
The public truth guard still observes release/adapter 1.3.12. Repository integrity
passes after restoring a referenced review and identifying temporary evidence paths
as local references, without removing their recorded results.

Freshly built Windows binaries pass fixed-path legacy migration, running-image
replacement, unchanged connector continuity, runtime reacknowledgement, retained
history, offline PowerShell download rejection and the complete process journey.

The Chromium 152.0.7977.82 browser lane passes script, real MV3 frames, history and
modern/legacy quiet-coexistence fixtures. One fixture now asserts the specific
`script_exception` cause while retaining unknown-effect and no-replay checks. The
visual fixture uses its own observer session after cancellation releases the
product's debugger lease. No production mechanism was changed for these corrections.

The deterministic adapter ZIP hash is
`2f850ca96a35bb54e5a84b8917bb641e196b25632768aaa45c7088eb00bacf5e`.
This is local packaging evidence until the workflow candidate binds and attests
the same bytes. Local `cargo audit` was unavailable; the required CI dependency
job must pass before publication. The workflow also runs Linux fixed-installation
and downloader fixtures and Windows release-binary installation checks.

No everyday browser profile, installed service, settings or grants were deployed
by these isolated source gates. Synthetic native framing and test Chrome profiles
remain distinct from installed-user and clean-machine acceptance.

## Publication

Pending the provenance-bound candidate and independent channel reconciliation.
GitHub, npm, Chrome and MCP Registry access checks succeeded. Candidate, publication
results, package-manager submissions and website delivery will be recorded here.
