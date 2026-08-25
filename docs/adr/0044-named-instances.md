# 0044. Named instances: one identity parameter for dev/prod coexistence

- Status: Accepted (2026-07-08)

## Relationship to other decisions

- REFINES ADR-0030 (the hub): the hub is singular PER IDENTITY -- one service owns the one
  browser link, the extension talks to one native host. This ADR makes that identity a single
  named parameter instead of scattered hardcoded constants, so a machine can run more than one
  isolated stack (a `dev` alongside the default).
- PAIRS WITH ADR-0045 (the resilient adapter): 0045 is what makes the dev loop actually delightful
  and settles the freshness question. Because the adapter is a stateless conduit that reconnects to
  the service, ONLY the service's binary version matters, so a rebuild plus a service restart is
  live with no client reload. That fact drives Decision 4's launch-path split below.
- BUILDS ON the existing env-seam convention (`GHOSTLIGHT_ENDPOINT`, `GHOSTLIGHT_USER_CONFIG_DIR`,
  `GHOSTLIGHT_LOG_DIR`, `GHOSTLIGHT_WEBAPI_PORT`): the instance follows the same "resolve from the
  environment at the point of use" pattern, adding `GHOSTLIGHT_INSTANCE`.
- HONORS ADR-0019/0020 (layered config, org policy): org policy stays fixed-per-instance and
  never flag-overridable. An instance is a separate install, not a bypass.

## Context

One developer machine needs two things at once: a fast local loop (edit -> build -> restart,
never gated on the Chrome Web Store) and the ability to validate the real deployed path
(npm-installed binary, store extension). Today the stack's identity is hardcoded in ~6 places
(endpoint, native-host name, MCP server name, three supervisor names, config dir), all singular,
so a second stack cannot coexist without colliding.

Two clarifications remove most of the apparent difficulty:

1. **Build profile is orthogonal.** `debug` vs `release` is only optimization plus whether
   observability is on. It never determines a clash. The only thing that clashes is IDENTITY.
2. **The clash cannot happen by accident.** An MCP client's config is keyed by server name, so a
   second `ghostlight` entry overwrites the first -- it does not run in parallel. And the browser
   link is singular by construction. A true two-stack situation only exists when someone
   DELIBERATELY creates a second identity. This ADR makes that deliberate act clean.

The extension dev loop is already solved and orthogonal: load `extension/` unpacked (its id is
pinned by the committed manifest `key`), edit, reload at `chrome://extensions` -- instant. The
store build has a different id, so both can be installed at once.

### Prior art (2026-07-08 research)

The design matches how established tools let a variant coexist with a default: VS Code
(`code` vs `code-insiders`) and Chrome channels (`Chrome` vs `Chrome Beta`) both fan ONE channel
identity across binary name, data dir, bundle id, and single-instance mutex, and both keep the
DEFAULT bare and unsuffixed -- only the variant carries a suffix. Docker Compose derives every
container/network/volume name from one project name; systemd template units (`foo@instance`)
inject identity through `%i`. So "one name derives all identity, default byte-identical" is a
well-trodden pattern, not a novel risk.

The one Windows-specific finding reshaped Decision 4. A Chrome native-messaging host manifest
`path` CAN point at a `.bat`, but doing so opts back into Chrome's pre-113 `cmd.exe` launcher --
the exact fragile path Chrome 113+ was built to escape (it breaks under a non-`cmd` `COMSPEC`,
a RUNASADMIN policy, or an `&` in the path). Every surveyed production native-messaging host
(Bitwarden's `desktop_proxy`, KeePassXC's proxy, browserpass) instead ships a compiled `.exe` and
isolates parallel installs by exactly this ADR's scheme: a distinct host name plus a distinct
manifest path per stack. So the non-default native host is a per-instance-named copy of the one
binary, NOT a wrapper script.

## Decision

### Decision 1: identity is one named parameter

Introduce a single `Instance` concept (`src/instance.rs`), resolved ONCE per process, that is the
single source of truth today's scattered constants fold into. This is a net REDUCTION in identity
surface: many hardcoded literals collapse into one derivation.

Resolution precedence (highest first):

1. `--instance <name>` -- a global CLI flag (like `--manifest`), used by the human running
   `install` / `uninstall` / `service` / `doctor`.
2. `GHOSTLIGHT_INSTANCE` -- the env seam, for tests, the e2e harness, and the value `main` folds
   the flag into so every point-of-use derivation in the process agrees.
3. `argv[0]` basename -- when the executable is named `ghostlight-<name>[.exe]`, the instance is
   `<name>`. This is the ONLY signal Chrome's native-host launch can carry (see Decision 4).
4. Default -- the canonical unnamed instance.

### Decision 2: the default is byte-identical

The default instance MUST reproduce every current identifier exactly, or the published product,
existing installs, and the install/fidelity tests break. The derivation is defined so the default
yields today's literals unchanged, and the default takes a SEPARATE branch (never string-concat
with an empty name, which would emit a stray `ghostlight-` / `org.sylin.ghostlight.`):

| Identifier | Default instance (unchanged) | A non-default instance `<n>` |
|---|---|---|
| IPC endpoint | `org.sylin.ghostlight.v1` | `org.sylin.ghostlight.<n>.v1` |
| Native host name | `org.sylin.ghostlight` | `org.sylin.ghostlight.<n>` |
| MCP server name | `ghostlight` | `ghostlight-<n>` |
| Supervisor task (Win) | `Ghostlight Service` | `Ghostlight Service (<n>)` |
| Supervisor label (mac) | `org.sylin.ghostlight.service` | `org.sylin.ghostlight.<n>.service` |
| Supervisor unit (linux) | `ghostlight.service` | `ghostlight-<n>.service` |
| User config / log dir leaf | `ghostlight` | `ghostlight-<n>` |
| Org policy path | machine-wide, instance-INDEPENDENT (see Decision 3) | same machine-wide path |
| Native-host launcher exe | `ghostlight` (the bare binary) | `ghostlight-<n>` (a per-instance copy) |

`Instance::default()` returns the literals above, verified by a pinned test
(`src/instance.rs` `default_instance_is_byte_identical`).

The instance NAME is validated at the boundary (it flows into paths, socket names, registry keys,
and OS unit names): lowercase ASCII letters, digits, and hyphens; must start with a letter and not
end with one; length `1..=32`; the word `default` is reserved. Hyphens are allowed (`qa-staging`)
because the derivation is one-way -- nothing ever reverse-parses the leaf back into base plus name,
so `ghostlight-qa-staging` is unambiguous. Uppercase is rejected to avoid the Linux
case-sensitive vs Windows/macOS case-insensitive collision trap.

### Decision 3: user state isolates per instance; org policy stays machine-wide

A non-default instance suffixes the USER config dir leaf and the LOG/observability dir leaf
(`ghostlight` -> `ghostlight-<n>`), so its user config and its debug/observability files never
touch the default's. That is the isolation a dev instance wants.

The ORG POLICY path, by contrast, stays MACHINE-WIDE and instance-INDEPENDENT
(`%ProgramData%\ghostlight\policy.json` on Windows, `/Library/Application Support/ghostlight/policy.json`
on macOS, `/etc/ghostlight/policy.json` on Linux -- unchanged), and EVERY instance reads it. This is
a deliberate security property, not an oversight: if each instance had its own org-policy path, a
user on a governed machine could escape a MANDATORY org policy simply by running
`ghostlight --instance escape` into a fresh, policy-free (all-open) instance. Making the org policy
machine-wide closes that hole -- `--instance` selects which user-facing install you are, it never
bypasses the machine's org governance. It also matches how the policy is deployed: an admin drops
ONE file at the fixed system path (ADR-0020), and it governs all instances at once. A personal dev
machine has no such file, so a dev instance there is all-open -- the right default. The path remains
never flag- or env-overridable (the ADR-0020 property is preserved).

### Decision 4: the multi-call binary (how the launch paths carry the instance)

The instance reaches each launch path differently, and the split is chosen so the ONE thing whose
version matters -- the service -- is always the freshly built binary:

- **Service, client-adapter, supervisor, and CLI carry `--instance <n>` on the command line**,
  pointing at the STABLE build-output path. A rebuild replaces that path in place, so a restart
  runs new code with no reinstall. (For the adapter and supervisor the flag is belt-and-suspenders,
  since argv[0] also resolves the instance; for the service it is what keeps the brain fresh.)
- **The native host is launched by CHROME with a bare path and no room for an argument**, so it
  uses the multi-call signal: for a non-default instance the installer places a per-instance copy
  of the binary named `ghostlight-<n>[.exe]` and points the manifest `path` at it; the binary reads
  its own `argv[0]` / `current_exe()` basename to self-identify. The DEFAULT instance points the
  manifest straight at the bare binary -- no copy, byte-identical. A stale copy is harmless because
  the native host is a stateless dumb pipe (ADR-0045): its version does not matter; only the
  service it relays to does.

This REPLACES the earlier draft's per-instance wrapper script. A `.bat` wrapper would reintroduce
Chrome's fragile pre-113 `cmd.exe` launcher; a compiled shim would be a second artifact. The
multi-call copy is arg-free, Chrome-113 direct-launched, cross-platform uniform, and matches every
production native-messaging host surveyed.

### Decision 5: CLI surface

- `--instance <name>` is a global flag (like `--manifest`), read before subcommand dispatch and
  folded into `GHOSTLIGHT_INSTANCE` so every point-of-use derivation agrees. `install` /
  `uninstall` / `service` / `doctor` / `status` all honor it.
- `doctor` reports the active instance name and its resolved paths, so "which stack is this?" is
  always answerable.

## Consequences

### If taken

- One machine cleanly runs the default (npm/store deploy validation) and `dev` (local build,
  unpacked extension) side by side; an agent sees `ghostlight` and `ghostlight-dev` as distinct
  servers with no tool-name ambiguity because they are distinct servers.
- The identity, today smeared across ~6 files, gets one home. Future identity questions have one
  place to look.
- Deleting a dev instance is `ghostlight --instance dev uninstall` plus removing its config dir.

### Cost

- The derivation module plus threading the instance to each former-constant site.
- The per-instance binary copy for the native-host role (non-default only).
- Tests: the byte-identical-default pin, and a dev-instance derivation test.

### Risks

- **Default drift.** If any default derivation diverges from today's literal, the product breaks.
  Mitigation: Decision 2's pinned test, plus the existing install/fidelity tests.
- **Stale native-host copy.** A non-default instance's `ghostlight-<n>` copy does not track a
  rebuild. Accepted: it is a dumb pipe, so its version is immaterial; the service (which carries
  code) is launched fresh via `--instance`. Re-running install refreshes the copy if ever needed.
- **Scope creep into "profiles".** An instance is only an identity plus isolated dirs. It is NOT a
  place to hang behavioral config (that is the layered config's job). Keep it to identity.

## Open questions

- Whether `doctor` should list ALL installed instances (scan the native-host dir) or only the
  active one. Presumably active-plus-a-hint; deferred.
- Whether a non-default instance should default observability ON (dev usually wants it). Leaning
  yes as an install-time default for non-default instances, but it stays orthogonal (a `--debug`
  choice), not baked into identity.

## Amendment (2026-08-25)

This record stands as written; the text above is the decision as it was made. The 1.0 candidate
narrows the supported operating-system matrix to Windows and Linux
([1.0/ACCEPTANCE.md](../1.0/ACCEPTANCE.md)). macOS mentions above describe the scope considered at
decision time; macOS remains a later row of the platform table, deferred for want of test hardware,
not removed from the product's future.
