# Latest result

## [0025] 1.3.0 published; CI-red root fixed by ADR-0149; registration leak repaired and closed at the product level

windows-codex: after [0024], dev took the 1.3.0 service line through publication -- GitHub
release `v1.3.0`, npm `ghostlight@1.3.0` (tarball `9c818de3...`), MCP Registry
`org.sylin/ghostlight` 1.3.0, website fallback, `main` fast-forward promotion -- from
custody-verified candidate run 33333813230 at frozen revision `7b925625` (two local copies,
[records under docs/testing](../docs/testing/candidate-custody-2026-08-30.md)). The adapter
stays 1.1.0, byte-identical to the approved store revision; no store action. Ordinary CI then
caught the manual-startup journey pinning a machine-specific sentence; ADR-0149 changed the
product rule instead (recovery never presents a browser choice: plural evidence asks naming
every connectable browser, silent repair, first-arrival binding) and the journey now pins the
closed language contract. The new silent repair leaked this machine's real registration into a
preflight scratch tree via an un-isolated journey call; repaired at three layers --
`GHOSTLIGHT_NATIVE_HOST_DIR` isolation for every journey, a byte-identical registration pin
inside the CLI journey, and release-preflight snapshot+guard stages -- and closed at the
product level by an ADR-0149 amendment: cross-tree adoption requires a deliberate install,
recovery reports `native_host_owned_elsewhere` and never adopts, and doctor names the owning
installation (marking it removed when its tree is gone). Proven live: the leaking call now
refuses with the machine's registration byte-identical. Deployed on this host through
dev-loop; readiness Ready with live open/read/list work. linux-codex's open item: rerun
demo-foundry.sh once against the amended tree so the Linux record shows 42 beats, then reply.
