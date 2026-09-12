# ADR-0167: One user installation and one serving authority

- Status: Accepted
- Date: 2026-09-12
- Supersedes: ADR-0124 installation-local production discovery and ADR-0150
  installation-local election and deferred authority diagnostics
- Amends: ADR-0065, ADR-0104, ADR-0115, ADR-0149, ADR-0145, ADR-0166

## Mandate and evidence

Ghostlight usage MUST be delightful. Deployment architecture must not make a
person repair a browser that is already connected and working.

On September 12, an agent invoked `npx -y ghostlight@1.3.5 open` after failing
to find Ghostlight on PATH. The launcher started a second authority beside its
downloaded binaries. Chrome and every configured MCP connector remained attached
to the development authority. The new authority reported no browser. The owner
requires one serving pipeline, including package invocation on development machines.

## Decision

There is one production installation per OS user. Binary archives, package
caches and build directories are artifacts, not independent serving installations.
Separate OS users retain separate authenticated browsers and policy authority.

All three executables resolve one user-owned control directory independently of
their own location. It contains one durable installation selection, one runtime
document, one lifetime lease and one startup admission lock. Windows uses the
profile's `.ghostlight` directory, outside redirected AppData. Linux uses the same
home-relative location, including supported home-visible Flatpak callers; sandbox
XDG remapping must not elect a different authority. Missing user identity fails
clearly rather than falling back to a shared temporary production authority.

The durable selection names the serving binary directory and whether development
owns the selection. The development loop selects its deployed build explicitly.
Package installation preserves development ownership and remembers the available
release. Ordinary package invocation only connects or demand-starts the selected
authority. Merely downloading or executing a different copy never changes an
existing selection. Development restore selects the remembered release.

A release upgrade while its authority is running is staged. The running directory remains
selected until its lifetime lease is released; the next startup selects the remembered release.
Setup run from the selected development directory does not register that directory as a release.

Selection is serialized with startup. Only the selected executable may initialize
the desktop and service. Other copies route through the existing authenticated
bridge. The OS lifetime lock enforces one authority even across simultaneous
launches. Deployment quiescence belongs to the selected authority, not the caller.
Existing bounded recovery, compatibility negotiation and no-replay rules remain.
No supervisor, process registry, discovery scan or additional wire protocol is added.

Installers and registration surfaces describe and register the selected installation.
Uninstalling inactive artifacts must preserve active registrations. A selected
development build is not silently replaced by a release after a startup failure.
Malformed selection data is preserved and reported; it never authorizes a guessed
executable. Paths are absolute and selected sibling sets must be complete.

History and diagnostics belong to the user installation. Migration preserves the
previous state's location when adopting an existing installation, so switching
binary directories cannot hide history or toggle diagnostics. Policy sources and
explicit authority constraints remain intact. Selection records contain no tokens.
Runtime authentication tokens remain in the private ephemeral runtime document.

Doctor names the selected authority, selection source and runtime location separately
from the invoking binary's version. Readiness always describes the shared runtime.
CLI catalog retrieval and tool execution continue through the serving authority.

`GHOSTLIGHT_RUNTIME_FILE` remains an explicit isolated test/deployment seam. Normal
installation and development do not depend on environment propagation. Tests must
elect their own runtime and must not register against a person's real browser.

Already published binaries cannot be changed retroactively. In-place migration
updates the actual registered sibling set, stops verified legacy authorities and
retains their state before starting the canonical service. Explicit execution of
an archived legacy release is outside the new executable contract; do not claim
host containment or remove arbitrary copies to conceal that limitation.

## Required evidence

- Different executable directories converge, both warm and cold, without overrides.
- Package installation/invocation preserves development selection and browser access.
- Concurrent connector, CLI and direct launches create one selected authority.
- Development replacement and release restore preserve state and reconnect shores.
- Missing/invalid selection, failed startup and incompatible peers cannot split service.
- Inactive removal preserves active registrations; explicit isolated tests stay isolated.
- The installed Windows browser and MCP path works after migration; reboot and Linux
  physical acceptance are reported separately until actually exercised.
