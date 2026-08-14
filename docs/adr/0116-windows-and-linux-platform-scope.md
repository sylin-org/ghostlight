# ADR-0116: Windows and Linux platform scope

- Status: Accepted
- Date: 2026-08-13
- Supersedes: the wider platform assumptions in ADR-0026, ADR-0095, ADR-0102, and ADR-0115

## Context

Ghostlight's active release plan and owner direction require one explicit operating-system
boundary. Keeping source branches, jobs, package formats, launchers, and unverified release gates
for an unsupported target creates maintenance work without proving a user promise.

## Decision

Windows and Linux are the complete supported operating-system matrix for Ghostlight 1.0.

- Runtime and installer code contains only Windows and Linux layouts.
- Continuous integration runs Rust, extension, launcher, and process gates on Windows and Linux.
- Release construction builds one Windows native package, one Debian package, their two portable
  archives, and six raw binaries.
- The npm launcher resolves only Windows x86_64 and Linux x86_64.
- The Claude Desktop MCPB carries only the supported Windows executable set.
- Package-manager metadata is generated only for Scoop and WinGet.
- Source compilation fails clearly outside the supported matrix.

Historical records remain evidence of earlier decisions, but they do not expand this active
platform contract.

## Consequences

- CI and release jobs match the operating systems Ghostlight intends to ship.
- Candidate assembly contains 17 artifacts instead of 27.
- Native-host registration and legacy-supervisor retirement have fewer conditional paths.
- A future operating-system target requires a new ADR, implementation, packaging, and native live
  evidence before it can enter the supported matrix.
