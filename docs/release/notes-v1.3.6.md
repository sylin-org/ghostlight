# Ghostlight 1.3.6

Ghostlight now uses one serving installation per OS user. Running a package on a
development machine reaches the selected development authority and its connected
browser, instead of starting an unrelated authority with no browser.

## Fixed

- Package invocation, MCP and native messaging resolve the same durable selection.
- Package setup preserves development custody. Removing inactive artifacts cannot
  remove active browser registrations.
- History, diagnostics and policy locations survive selection and restart.
- CLI calls recover from stale runtime discovery and report startup failures.
- The bundled agent guide uses the current `browser_flow` catalog and no retired inputs.

## Development and upgrade

The first development deployment updates all three native binaries. Later ordinary
orchestrator changes retain the connectors. Doctor names the selected authority
and shared runtime. Development restore requires a complete compatible release.

Already published binaries cannot be retrofitted: upgrade the installed sibling
set and reconnect clients whose old stdio connector was replaced. Chrome adapter
1.1.4 is unchanged. Do not use archived pre-1.3.6 binaries to create new serving
installations alongside the selected installation.

## Install

Use `npx -y ghostlight@1.3.6`, the Windows installer, Debian package, or portable
archives. Release assets include checksums and GitHub build-provenance attestations.
