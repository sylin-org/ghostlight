# 0032. Test at pure seams; inject config sources at the composition root

- Status: Accepted
- Date: 2026-07-05

## Relationship to other decisions

- BUILDS ON ADR-0030 Decision 2 (the `src/hub` composition root and `ServiceContext`): that
  decision established a single place where shared per-service state is constructed once and handed
  to every session. This ADR extends that root to also own config-source resolution, which the
  loader currently reaches for on its own via fixed-path free functions.
- PRESERVES ADR-0019's layered-configuration security property (the org policy file lives at a
  fixed, non-user-redirectable path; `governance::config::load::org_policy_path` carries the
  invariant "no flag, environment variable, or config key relocates or bypasses this path"). This
  ADR does NOT weaken that invariant. Injection here happens at the trusted, compile-wired
  composition root, never through a user-facing override. In production the injected org-policy
  path IS `org_policy_path()`, unchanged.
- RECONSIDERS the K5 `GHOSTLIGHT_USER_CONFIG_DIR` env override (`docs/tasks/console`,
  `user_config_path`): that override was added so a spawned-process test could relocate the user
  config file. Under this ADR the behaviour it was protecting is tested in-process instead, so the
  env override stops being the isolation mechanism of record. It may remain as a deployment
  convenience but is no longer the reason the tests pass.
- ALIGNS WITH ADR-0026 (release maturity): the release gate runs the test suite on Linux, macOS,
  and Windows. This ADR is what lets that gate be honestly green on all three rather than green
  only where an OS env var happens to redirect a fixed path.

## Context

The v0.2.0 release did not publish. The release workflow's `test` job failed on Linux, its
`release` job was therefore skipped, and no GitHub Release was created. The failing assertion was
`tests/console_config_api.rs::config_api_reflects_a_locked_org_mandatory_key`: it expected an
org-mandatory key to serialise with `source: "org_mandatory"` and got `source: "builtin"` on Linux.

The proximate cause is platform-specific. That test isolates an org policy by spawning a real
`ghostlight` service with the `ProgramData` environment variable pointed at a temp directory. On
Windows `org_policy_path()` resolves through `%ProgramData%`, so the override lands; on Linux and
macOS `org_policy_path()` is hardcoded (`/etc/ghostlight/policy.json`,
`/Library/Application Support/ghostlight/policy.json`), so the override is silently ignored, no org
policy loads, and the key falls back to its builtin default. `cargo test`'s fail-fast then stopped
the suite at that binary, so the true cross-platform status of every alphabetically-later test
binary was unknown -- the suite had never actually been proven green on Linux or macOS.

The proximate cause is one test. The architectural cause is general. Consider what that test is
actually asserting: that a `Resolution` carrying an org-mandatory entry serialises to a particular
JSON shape. That is pure logic. Yet the only way the test can reach it is to spawn a real process,
make that process read a policy file from a fixed OS path, bind a TCP port, and perform an HTTP
round-trip. The HTTP handler couples two unrelated jobs -- a pure transform (`Resolution` -> JSON)
and transport (bytes -> socket) -- and it reaches its input through `ctx.store`, whose
`Resolution` was produced by reading a fixed OS path deep inside `load_initial_with_policy`. Pure
logic is only reachable through the entire process stack.

Every downstream pain follows from that one coupling:

- a combinatorial family of spawn helpers (`spawn_service`, `spawn_service_with_program_data`,
  `spawn_service_with_webapi_port`, `spawn_service_with_program_data_and_webapi_port`,
  `spawn_service_with_user_config_dir_and_webapi_port`,
  `spawn_service_with_program_data_user_config_dir_and_webapi_port`, ...), one per isolation
  dimension, doubling with each new dimension;
- flakiness, because every behaviour test spawns a real process that binds a real port and races
  its own startup;
- a hard cross-platform failure, because org-policy isolation is only possible where a fixed path
  is redirectable by an OS env var;
- `#[cfg(windows)]` gating proposed as the "fix", which is a band-aid: it hides the coupling,
  abandons Linux/macOS coverage for the gated behaviour, and leaves the flakiness and the helper
  explosion untouched.

The codebase already contains the counter-example. `governance::config::reload.rs`'s own unit tests
construct an `OrgMandatory` `Resolution` entirely in memory (via the pure `layers::resolve`) and
assert on it -- no process, no file, no platform dependency. The good seam exists one layer down;
the HTTP layer simply does not expose a pure entry point that a test can call.

## Decision

Four parts. Sequenced so the first, smallest part unblocks the release on its own and every later
part is a pure quality improvement.

### Decision 1: test behaviour at the narrowest pure seam, not through a spawned process

Extract the pure transform out of each web-API handler into a standalone function that maps
already-in-hand data to a `serde_json::Value`, leaving the handler responsible only for obtaining
`ctx` state and writing bytes:

- `config_payload(resolution: &layers::Resolution) -> serde_json::Value`
- `sessions_payload(summaries: &[session::SessionSummary], live_count: u64) -> serde_json::Value`
- an analogous pure shape for the enable-remote success/refusal bodies

These functions are unit-tested directly. A test builds a `Resolution` in memory (the same way
`reload.rs`'s existing tests do, through `layers::resolve` over hand-built `LayerInputs`), calls the
payload builder, and asserts on the JSON. No process, no file, no TCP, no platform dependency,
microsecond runtime.

Config facts that are about resolution rather than serialisation (for example "an entry declared
mandatory resolves with `source = OrgMandatory` and `locked = true`") are proven once, at
`layers::resolve`, where they already partly live. The HTTP tests assert only the serialisation
contract on top of a given `Resolution`.

### Decision 2: real-process tests are for end-to-end smoke only

A spawned-process integration test earns its cost only when the thing under test IS the process
boundary: that the binary boots, claims its endpoints, serves, and returns a well-formed response
on the happy path. That set is small and mostly all-open (no policy needed). It is de-flaked with a
readiness-retry on connect rather than a bare connect-immediately-after-spawn. Individual smoke
tests MAY be platform-scoped when the mechanism they exercise is genuinely platform-specific, and
that scoping is stated with its reason.

Behaviour matrices -- every config key, every provenance source, every denial shape -- are NOT
expressed as spawned-process tests. They are Decision 1 unit tests.

### Decision 3: inject config sources at the composition root; never hijack an OS env var for isolation

The set of paths and source strings the config layer reads -- the org policy path, the user config
path, and the optional user manifest source -- is bundled into one `ConfigSources` value that is
constructed at the composition root and threaded into `ConfigStore::load_initial_with_policy` and
the reload path, and stored on `ConfigStore` so `reresolve` uses the SAME sources it started with
rather than re-deriving them from free functions.

- In production, `main.rs`/`run_service` builds `ConfigSources` from the real OS paths
  (`org_policy_path()`, `user_config_path()`) exactly as today. Behaviour is unchanged.
- In-process tests build `ConfigSources` pointing at temp files they control, on any platform, with
  no environment variable involved.

This preserves the ADR-0019 security invariant: `org_policy_path()` remains fixed and
non-overridable by any user-facing input. The injection point is the trusted composition root, not
an env var or flag exposed to an untrusted caller. The distinction is exactly the one that makes
this safe: a value wired in at compile/startup by the program's own root is not an override channel
an attacker can reach.

### Decision 4: collapse the spawn helpers into a builder

Whatever real-process tests remain (Decision 2) configure their child through one builder --
`ServiceSpawn::new(endpoint).webapi_port(p).manifest(m).spawn()` and so on -- replacing the
combinatorial `spawn_service_with_*` family with a single composable surface. New isolation
dimensions become one method, not a doubling of the helper count.

## Consequences

- The behaviour assertions that blocked v0.2.0 move to pure unit tests that are green on all three
  platforms. The release unblocks correctly, not by gating.
- The suite gets dramatically faster and stops being flaky for the migrated cases: no process
  spawn, no port bind, no startup race.
- The `ProgramData` / `APPDATA` env-hijack pattern is retired as an isolation mechanism. Test
  isolation becomes explicit constructor injection.
- Cross-platform coverage becomes honest: the migrated behaviour is exercised identically
  everywhere, rather than on Windows only.
- Migration is phased and each phase leaves a green tree. Phase 1 is Decision 1 (and the retirement
  of the org-policy-dependent integration tests it replaces) and is sufficient to unblock the
  release; Phases 2 and 3 (Decisions 3 and 4) are pure internal quality with no behaviour change.

## Provenance

- Why not simply `#[cfg(windows)]`-gate the failing tests. That hides the coupling behind the
  platform where the accident works, permanently abandons Linux/macOS coverage of that behaviour,
  and leaves the flakiness and the helper explosion in place. It treats the symptom.
- Why not make `org_policy_path()` env-overridable so the existing spawned tests isolate on Linux
  too. That reintroduces a user-facing override of the org policy location, which is exactly the
  ADR-0019 security property being defended. The org policy must live where the org put it, not
  where a caller's environment says. Injection at the composition root is safe precisely because it
  is not reachable as an override.
- The seam this ADR asks for already exists one layer down: `reload.rs`'s unit tests build an
  `OrgMandatory` `Resolution` in memory today. This ADR makes the HTTP layer expose the same kind
  of pure entry point so its tests can do likewise, rather than re-proving resolution facts the
  expensive way through a spawned process.
