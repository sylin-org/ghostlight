# ADR-0124: User-writable runtime discovery for the Linux system package

- Status: Accepted
- Date: 2026-08-15
- Amends: ADR-0104 Decision 2 and ADR-0123 acceptance evidence 5
- Builds on: ADR-0115 and ADR-0123

## Context

The three sibling processes discover one running authority through a shared runtime document and
hold the authority lease beside that document. Development and portable installations keep those
files beside their executables. This isolates concurrent test and portable installations without
another registry.

The Debian package installs the same siblings in `/usr/bin`. An ordinary user can execute those
binaries but cannot create `ghostlight-runtime.json` or its lease in `/usr/bin`. The existing
package smoke asked only for `ghostlight --version`, so it did not cross the runtime-write seam.
A rootless Ubuntu 22.04 package install exposed the failure before the visible lifecycle:
`ghostlight --headless` exited while opening the authority lease with `Permission denied`.

Using `XDG_RUNTIME_DIR` only for the package would not make the siblings converge. Browser native
messaging and some MCP launchers may omit that session variable, while a direct desktop launch has
it. ADR-0082 records this class of scrubbed Linux launch environment in the earlier architecture.

## Decision

1. An explicit `GHOSTLIGHT_RUNTIME_FILE` remains authoritative for tests and controlled
   deployments.
2. Linux siblings running from the Debian package location `/usr/bin` share
   `$HOME/.cache/ghostlight/ghostlight-runtime.json`. The authority lease stays beside that
   document through the existing lifecycle helper. Deployment quiescence remains scoped to the
   sibling executable directory.
3. Other sibling layouts continue to keep the runtime document beside the executable. This
   preserves isolated development, portable, and versioned per-user installations.
4. If a packaged process has no usable `HOME`, runtime discovery retains the existing temporary
   directory fallback. It does not invent a service, global registry, package script, or new
   process role.
5. Package lifecycle evidence must start the installed authority as an ordinary user. A version
   query alone is not runtime evidence.

## Consequences

All three Debian-package siblings converge without requiring a graphical-session environment or
write access to system directories. The runtime document remains user-private through the existing
atomic writer, and normal per-user cache ownership protects its directory.

The system package and a portable copy owned by the same user may run independently because their
runtime documents remain in different locations. Explicit test isolation is unchanged.

The package path is deliberately exact. Supporting another system installation prefix requires a
separate packaging decision and corresponding runtime rule; heuristic permission probes are not
added.

## Acceptance evidence

1. Unit tests prove all three `/usr/bin` siblings select the same user-cache runtime document.
2. Unit tests retain installation-local convergence for portable siblings and explicit override
   precedence.
3. In a clean Debian-family guest, an ordinary user starts packaged `ghostlight --headless`, the
   runtime document and lease appear below that user's cache, and a packaged connector reaches the
   same authority.
4. The package install, remove, reinstall, and purge lifecycle still removes only package-owned
   system files. User runtime state is treated as ordinary retained user data.
