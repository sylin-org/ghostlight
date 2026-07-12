# ADR-0048: The development override -- a live dev instance shadows the default for unpinned clients

- Status: SUPERSEDED by [ADR-0064](0064-explicit-dev-isolation.md) (2026-07-12) -- the auto-shadow
  (an unpinned client preferring a live `dev` instance) is retired in favor of explicit dev
  isolation: the unpacked dev extension self-selects its OWN native host, a `dev` install registers
  that host, and every client pins exactly one instance. The NAMED-instance identity this ADR built
  on (ADR-0044) and the per-user hub-key survive; only the `Selection::Unpinned`/prefer-live-dev
  half is gone. Kept for historical context.
- Status (original): Accepted (ratified 2026-07-09)
- Deciders: project owner (design conversation, 2026-07-09)
- Amends: ADR-0044 (named instances -- the parallel-isolation DEFAULT is inverted for the
  dev-over-release case), ADR-0046 (adapter behavior)
- Touches: ADR-0045 (resolution happens per connect episode, riding the reconnect loop),
  ADR-0047 (the stable per-process guid is re-presented to whichever instance wins)
- Supersedes: the extension's installType-based host selection (2026-07-08, commit cd77bf5)

## Context

ADR-0044 modeled instances as PARALLEL, fully isolated stacks: two of everything (endpoint,
native host, MCP-client entry, supervisor), and every client explicitly bound to exactly one.
That is the right model for genuinely unrelated stacks. It is the wrong DEFAULT for the actual
common case it was built to serve: a developer running a dev build ON TOP OF a released install.

The failure that exposed it (2026-07-09): Claude Desktop (cowork) carried a stale default-instance
`ghostlight` MCP entry pointing at the pre-split `ghostlight.exe`, while the only working stack on
the machine was the `dev` instance -- reachable exclusively through a `ghostlight-dev` entry that
only Claude Code had. Nothing was "misplaced": the installer had correctly written per-instance
entries. The MODEL made the confusion: with parallel instances, every client must be re-pointed by
hand whenever the developer's attention moves between dev and release.

The owner's direction, which this ADR adopts as the design's premise: if a machine has both the
released and the dev versions, that implies the user is developing it -- ALL requests should be
handled by the dev version while it is up; once the dev version is out/off, the release version
should take over. Forcing a client to be bound to only one of them is the wrong default.

So the default inverts: instances become HIERARCHICAL for the one pair that matters. `dev`
SHADOWS the default. A client (or browser) that names no instance follows whichever is alive,
dev first. Explicit pinning remains for everyone who wants ADR-0044's parallel behavior.

The mechanism is nearly free: the agent adapter already re-dials per connect episode (ADR-0045's
reconnect loop, hardened by ADR-0047 D6), and the browser adapter is respawned by Chrome on every
native-messaging reconnect. Resolution simply happens at those existing seams.

## Decision 1 -- The shadowing pair

The reserved instance name `dev` is THE development override of the DEFAULT instance:

- An UNPINNED adapter (agent or browser) resolves its target at each connect episode: the dev
  instance if it is reachable, else the default instance.
- Only `dev` shadows, and only the DEFAULT is shadowed. Named instances other than `dev` never
  shadow and are never shadowed (they remain plain ADR-0044 parallel stacks).
- The service side is untouched: a dev service still owns exactly the dev endpoints
  (`org.sylin.ghostlight.dev.v1` and `...-adapter`), the default service the default ones.
  Shadowing is a CLIENT-SIDE resolution concept only.

## Decision 2 -- The selection grammar (adapter-side tri-state)

Both adapters resolve a `Selection` with three states; only adapters have the third:

1. `--instance <name>` (agent) or an inherited `GHOSTLIGHT_INSTANCE=<name>` (both), or a
   `ghostlight-adapter-browser-<name>` per-instance binary copy (browser, argv[0], ADR-0044
   Decision 4): PINNED to that named instance. No shadowing. (This includes `--instance dev`:
   pinning dev means "dev or nothing", the dev-loop posture.)
2. The reserved word `default` in the same positions: PINNED to the DEFAULT instance. No
   shadowing. Adapters special-case the word BEFORE `Instance::validate` (which continues to
   reject it as an instance NAME); the root `ghostlight` CLI is unchanged -- for `service`,
   `install`, and `doctor` there is no unpinned state, so omitting `--instance` already IS the
   pinned default and the existing "reserved; omit --instance" error stays correct there.
3. Nothing named anywhere: UNPINNED. Connect-time resolution, candidates `[dev, default]`.

Environment seams (adapters only, in precedence order above all of the ordinary grammar):

- `GHOSTLIGHT_ENDPOINT` (existing): a single pinned endpoint. Unchanged; every existing test and
  advanced deployment keeps its exact behavior.
- `GHOSTLIGHT_ENDPOINTS` (new): a comma-separated PINNED candidate list, tried in order. This is
  the integration-test seam (tests must never dial the machine's real
  `org.sylin.ghostlight*` names) and an escape hatch for exotic deployments.

Environment normalization: a pinned NAMED selection writes its name back into
`GHOSTLIGHT_INSTANCE` (as today); pinned-default and unpinned REMOVE the variable. Consequence
(accepted): an unpinned adapter's own observability files live under the DEFAULT instance's
directories regardless of which service it ends up relaying to.

## Decision 3 -- Agent-side resolution mechanics

`relay_adapter` takes the ORDERED candidate endpoint list instead of one endpoint:

- A connect episode (the first connect, and each reconnect tick) tries every candidate IN ORDER
  with the full connect-plus-handshake attempt. No pre-probe: a dial on an absent pipe/socket
  fails instantly, so trying dev first costs nothing when dev is down.
- The supervisor kick on first failure is unchanged in shape and, because it derives its task
  name from the (normalized) environment, targets the DEFAULT instance's task when unpinned --
  exactly right: dev never has an auto-start supervisor (docs/DEV-LOOP.md).
- The bounded retry windows are unchanged (fail-fast first connect; patient 120s reconnect,
  ADR-0045); each retry tick walks the whole candidate list, dev first.
- Observability: when more than one candidate is in play, the adapter notes which one won, once
  per successful connect: `override resolution: connected to candidate <i>/<n>`.

Failover and failback granularity (deliberate):

- Failover dev -> default is IMMEDIATE: the moment dev drops, the very next reconnect tick lands
  on the default (if it is up). "Once the dev version is out/off, the release version takes over"
  -- at reconnect speed, with the MCP handshake replayed (ADR-0045), no client reload.
- Failback default -> dev happens at the NEXT CONNECT EPISODE only (a session reconnect or a
  client restart), never mid-session. A session is never silently migrated off a healthy service.
- Consequence for the dev loop: if a default service is ALSO running (auto-start), a dev rebuild
  gap fails the unpinned session over to the release binary until its next reconnect. A developer
  who wants "dev or nothing" pins dev (Decision 2.1) -- that is precisely what pinning is for.

Identity across a cross-instance failover: the adapter's per-process `SessionGuid` (ADR-0047 D2)
is re-presented to whichever service wins. To the other service this is an ordinary first
presentation (a fresh admit); per-session browser state (tab ownership, groups) does not
transfer, which is honest -- it IS a different brain. The browser side converges on the same
winner via Decision 4, so both halves of a session land on the same instance after a drop.

## Decision 4 -- Browser-side resolution

`relay_native_host` takes the same ordered candidate list. Because the browser-side `connect()`
deliberately retries for ~30s (startup-ordering patience), the browser adapter must not serially
WAIT on a dead dev candidate; it PROBES first:

- The first candidate whose endpoint EXISTS right now (`probe_endpoint(...) != Absent`; a busy
  pipe is still a live service) is chosen, then dialed with the existing patient `connect()`.
- If every candidate probes Absent, the LAST candidate (the default instance in the unpinned
  order) is dialed anyway, preserving today's wait-for-startup behavior toward the canonical
  target.
- Resolution happens once per adapter process. That IS per connect episode: when the relay ends
  (service died, extension reconnects), Chrome spawns a fresh adapter, which resolves fresh.

## Decision 5 -- One browser surface

- The extension ALWAYS connects to the host name `org.sylin.ghostlight`. The installType-derived
  dev-host selection (2026-07-08, cd77bf5) is superseded and removed: with adapter-side
  resolution, an extension-side static label would claim to know which service traffic reaches,
  and it cannot. The popup/options connection indicators drop the instance suffix for the same
  reason (they show connection truthfully; they no longer name an instance).
- The unified host manifest allows BOTH shipped extension identities by default: the Web Store id
  `lejccfmoeogmhemakeknjjdhkfkgncdl` and the pinned unpacked-dev id
  `cjcmhepmagomefjggkcohdbfemacojoa` (ADR-0016's committed manifest key). `--extension-id`
  becomes an OPTIONAL extra origin (validated, deduplicated) instead of a required flag; the
  `MissingExtensionId` error is retired. One registration serves a store install, a dev checkout,
  or both at once.

## Decision 6 -- Installer

- The DEFAULT install is byte-compatible in shape and becomes the whole story: unified host
  manifest (both origins), host registrations, the `ghostlight` MCP-client entry with NO
  `--instance` args -- which the new adapters read as UNPINNED. Existing default client entries
  already have empty args, so every previously-installed client becomes override-aware the moment
  the binaries update, with no reinstall.
- `ghostlight --instance dev install` THINS to MCP-client entries only (the pinned
  `ghostlight-dev` entry): no host manifest, no host registration, no per-instance binary copy,
  and no supervisor (a dev service runs in a terminal; DEV-LOOP). Browser traffic for dev rides
  the unified default host.
- `ghostlight --instance dev uninstall` keeps FULL cleanup: it still removes any legacy
  per-instance host registration, manifest, and `ghostlight-adapter-browser-dev` copy left by
  pre-0048 installs, plus the pinned client entries.
- Named instances other than dev keep ADR-0044's full install/uninstall behavior, with the
  documented caveat that the shipped extension only reaches dev/default (their per-instance hosts
  are reachable only by a separately-packaged extension).

## Decision 7 -- Doctor visibility

`ghostlight doctor` for the DEFAULT instance gains a `Development override:` section: it probes
the dev instance's adapter endpoint and prints whether unpinned clients currently route to a live
dev instance or to the default. This makes the exact confusion that motivated this ADR visible in
one line.

## Decision 8 -- Non-decisions (rejected here, on purpose)

- NO arbitrary priority chains (`a > b > c`): one pair with a real user; chains add config
  surface without one.
- NO mid-session failback or stickiness heuristics: failback is next-episode only (Decision 3).
- NO config-file priority list and no new behavioral config: the override is a fixed convention
  of the reserved `dev` name (instances stay identity-only, ADR-0044 Decision 3).
- NO extension-side instance selection of any kind (superseding cd77bf5, Decision 5).
- NO service-side changes: no cross-instance awareness, no delegation, no proxying.
- Unpinned adapters log under the default observability directories (Decision 2, accepted).

## Consequences

- The stale-entry class of failure disappears: one client entry, one host, valid on every machine
  state (dev only, release only, both, neither -- the last fails with today's clear error).
- The dev "install" shrinks to: run `ghostlight install` once, load the unpacked extension, run
  the dev service in a terminal. No per-client re-pointing, ever.
- A probe connection against a live dev service briefly wins an accept slot and shows as one
  phantom connect/disconnect in that service's debug counters (same disclosed side effect
  `doctor`'s probe already has).
- `GHOSTLIGHT_ENDPOINTS` is a power seam; it is documented as test/advanced only.
- Risk: a user who runs a default service with auto-start AND a dev service, then kills dev
  mid-session, silently lands on release for the rest of that session (Decision 3). Mitigation:
  the doctor line (Decision 7), the resolution debug note, and pinning.

## Migration

- Binaries update -> existing default client entries (empty args) become unpinned automatically.
- Stale pre-split entries (a `ghostlight` entry pointing at `ghostlight.exe`) are fixed by
  running `ghostlight install` (same-name entries are overwritten; that path already existed).
- Legacy dev-instance artifacts (the `org.sylin.ghostlight.dev` host registration and the
  `ghostlight-adapter-browser-dev` copy) are inert once the extension targets the unified host;
  `ghostlight --instance dev uninstall` removes them.
- Pinned `ghostlight-dev` client entries keep working unchanged (they are Decision 2.1 pins).

## Provenance (settled questions -- do not relitigate in implementation)

- Shadowing pair is dev-over-default only: owner direction, 2026-07-09.
- Failback is next-episode, not mid-session: decided here for session integrity; revisit only
  with a new ADR.
- The extension carries no instance logic: decided here; cd77bf5 is superseded.
- `--extension-id` optional with both shipped ids baked in: decided here (Decision 5).
- Dev install thins; dev uninstall keeps full legacy cleanup: decided here (Decision 6).

## Amendment (post-execution, 2026-07-09) -- the anti-squat hub-key is per-user, not per-instance

Live verification surfaced a defect the execution batch's tests masked. The ADR-0030 Decision 8
anti-squat handshake keys its HMAC proof on a `hub-key` secret that the implementation stored under
the per-INSTANCE `observability::log_dir()` (`<data-local>/ghostlight` for default,
`<data-local>/ghostlight-dev` for dev). The development override, by design, has an UNPINNED adapter
(default identity, hence the default hub-key) connect to a live `dev` service (which proves with the
dev hub-key). The two secrets differ, so every override connect failed the proof with the pinned
refusal ("the Ghostlight service on this endpoint is not the one this user installed") -- exactly
the cowork symptom this ADR set out to fix. A pinned `ghostlight-dev` client worked only because it
resolved the matching dev key.

Resolution: the hub-key moves to an instance-INDEPENDENT `observability::shared_data_dir` (the
default leaf, regardless of the current instance; `GHOSTLIGHT_LOG_DIR` still overrides it for test
isolation), so all of a user's instances share ONE key. This is strictly correct for the threat
model -- the anti-squat defends against CROSS-USER squatting (the key file is user-ACL'd), and
per-instance separation bought no same-user defense, since any same-user process can already read
any of that user's key files. The `antisquat.rs` module doc already called the secret "per-user";
this aligns the implementation with that stated intent.

Why the batch missed it: `tests/adapter_override.rs` (and `adapter_reconnect.rs`) give every process
one shared `GHOSTLIGHT_LOG_DIR`, so every process shared one hub-key regardless of instance -- the
cross-instance-dir case never arose. The regression guard is now a pure unit test on the
instance-independent resolver (`observability::shared_data_dir_from`).
