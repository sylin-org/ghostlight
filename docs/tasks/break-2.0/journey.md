# Break 2.0: install journey ledger

The journey lane carries [ADR-0200](../../adr/0200-the-install-journey.md): one install journey in
three steps (This computer, Your browser, Your agent) across the extension's setup page, the popup,
the window's setup view and the install output. It follows the seams ledger ([seams.md](seams.md))
and the shell ledger ([shell-m2.md](shell-m2.md)); ADR-0199 wins over ADR-0197 where they differ and
ADR-0200 wins for the journey. The wire is [adapter-protocol-4](../../design/adapter-protocol-4.md).
The ground rules are the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md) plus the gate and rules
below. Slices, in order: j01-adr, j02-kit, j03-show-event, j04-setup-page, j06-popup, j07-options,
j08-toolbar-icon, c01-carriers, c02-get-scripts, x01-store-submission, j09-first-request,
j10-profiles, j11-setup-language, j11b-agent-outcomes, j12-setup-part, j13-setup-actions,
j14-window-view, j15-install-cli, j17-docs. A slice whose owner answer is Pending stops and says so
here.

## Gate

Run from the repository root (E:/repo/github/sylin-org/ghostlight) in Git Bash with
CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-journey on every cargo command.
Run each check the slice's files require, and record in the ledger which ones ran.

- (1) Any Rust, Cargo.toml, Cargo.lock or rust-toolchain.toml change: cargo fmt --all -- --check
  (fix with cargo fmt -p <package>); cargo clippy --workspace --all-targets -- -D warnings (always
  workspace-wide); cargo test -p ghostlight-service, or cargo test --workspace when crates/bridge,
  crates/relay, crates/edge, crates/win-peer or any manifest changed.
- (2) Model surface changed on purpose (j11b only): GHOSTLIGHT_CATALOG_UPDATE=1 cargo test -p
  ghostlight-service the_catalog_matches and GHOSTLIGHT_CATALOG_UPDATE=1 cargo test -p
  ghostlight-service work::tests::golden, then review the golden diff and run the tests again
  without the variable.
- (3) extension/ or tests/golden/ changed: npm test in extension/.
- (4) Page assets changed: node --test
  "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js", plus cargo
  test -p ghostlight-service and npm test in extension/.
- (5) extension/ or scripts/package-extension.ps1 changed: pwsh -NoProfile -File
  scripts/package-extension.ps1 -OutputPath .tmp/lane-journey/extension.zip -Force.
- (6) crates/orchestrator/ui or the workbench test fixtures changed: node
  tests/workbench-surface.mjs and node --test tests/workbench-transport.test.mjs
  tests/workbench-settings.test.mjs.
- (7) packaging/ changed: npm test --prefix packaging/npm and node --test
  packaging/mcpb/test/launcher.test.js.
- (8) tests/golden/, tests/guards/, crates/relay, crates/edge, any Cargo.toml or Cargo.lock, or any
  packaged extension file changed: node --test tests/guards/*.test.mjs (the connectors guard
  tests/guards/connectors.test.mjs and, once x01 lands, the extension freeze guard).
- (9) Changed runtime JavaScript or .mjs that no test runs: node --check <file>. Changed .sh: sh -n
  <file>. Changed .ps1: a parse-only check, pwsh -NoProfile -Command
  "[System.Management.Automation.Language.Parser]::ParseFile('<file>',[ref]$null,[ref]$e) >
  $null; if ($e) { $e; exit 1 }".
- (10) Always: pwsh -NoProfile -File scripts/check-repository-integrity.ps1 (ASCII and links).

Process hygiene: never search a whole drive (search only inside the repository or the session
scratch directory); never run a Ghostlight binary, the dev loop, an installer, a get script, a
journey under tests/ that starts Ghostlight, or a browser (the coordinator runs every live check);
never touch ~/.ghostlight; never read local/, private/ or saps/; leave no process running when the
slice ends (no background job, watcher, server or node process you started may outlive the slice).

## Rules

1. Never run a Ghostlight binary, journey, install, get script, the dev loop or a browser. The
   coordinator runs every live check; record each under Owed live checks.
2. Never submit, publish, push or tag. Never edit `CHANGELOG.md` or `docs/STATUS.md`.
3. Commit only your own paths: `git add -- <paths>`, then `git commit -s -m '<subject>' -m '<body>'
   -m '<the Co-Authored-By trailer the coordinator gives>' -- <paths>`. Retry while
   `.git/index.lock` exists. Never `git add -A`, `git add .`, `git commit -a`, stash, reset,
   `checkout --`, clean or rebase.
4. Another lane rewrites files under `tests/` in this checkout. If a file you must edit there has
   uncommitted changes that are not yours, wait and tell the coordinator.
5. ASCII only. A module doc on every new module and a doc on every public item whose name does not
   state its contract. Person-facing service sentences live in `crates/orchestrator/src/language/`.
6. Tests are behavioral, never source-text pins (ADR-0197 Decision 24).
7. Never read `local/`, `private/` or `saps/`. Never copy from `reference/`.
8. After x01, a packaged extension file changes only with a recorded owner decision to withdraw and
   resubmit.
9. Update this ledger in the same commit as the slice: a Landed row, RESUME HERE, owed live checks
   and every place the tree differed from the spec. Line numbers in specs drift: find code by name.
10. Never stage `docs/testing/independent-user-delight-2026-10-02.md` or
    `docs/testing/quiet-success-local-deployment-2026-10-02.md`.

## Owner answers

Q1 and Q2 were answered 2026-10-15 (see [seams.md](seams.md) Notes). The owner delegated JQ1 to
JQ11 to the coordinator on 2026-10-16 with one test, "what would be delightful for the user?", and
ruled JQ2 directly. The full wording is in ADR-0200.

| Question | Answer | Date |
|---|---|---|
| Q1 the install-time page | The extension's own bundled `setup.html`, opened only when its first connection finds the service absent. | 2026-10-15 |
| Q2 the setup page's command | Get-script one-liners, no Node.js. | 2026-10-15 |
| JQ1 grey toolbar icon while Ghostlight does not answer | No. The badge and the popup already show it. | 2026-10-16 |
| JQ2 embedded page-asset fallback (Chrome and Edge) | No. The page code is local, so the store's remote-code answer stays No (owner ruling). | 2026-10-16 |
| JQ3 host permissions | Drop http and https host permissions if the live check shows nothing needs them. | 2026-10-16 |
| JQ4 frozen URLs and the pre-submission test release | `irm https://sylin.org/ghostlight/get.ps1 \| iex`, `curl -fsSL https://sylin.org/ghostlight/get.sh \| sh`, documentation link https://sylin.org/ghostlight/. Both redirect to the latest GitHub release assets; the release attaches `get.ps1` and `get.sh` (c02). The website redirect is the owner's to publish; x01 waits until both answer. The test release is made in a fork. | 2026-10-16 |
| JQ5 a Watch tab during setup | Shows the ordinary workbench. | 2026-10-16 |
| JQ6 installed but not running | "Open Ghostlight on this computer." and where it is (Start menu or Applications), on the setup page and in the popup. | 2026-10-16 |
| JQ7 scope of 2.0.0 and the cutover | 2.0.0 waits for the service half (j09 to j15). | 2026-10-16 |
| JQ8 Add and Open <browser> | Ghostlight starts that browser. | 2026-10-16 |
| JQ9 Edge | Add from the Chrome Web Store until an Edge listing exists. | 2026-10-16 |
| JQ10 window on the first no-browser refusal | None. The agent's answer carries the link. | 2026-10-16 |
| JQ11 Skip for now | Kept, quiet; the tray brings setup back until done. | 2026-10-16 |

## RESUME HERE

j02-kit has landed. Next: j03-show-event.

## Landed

| Slice | What | Commit |
|---|---|---|
| j01-adr | ADR-0200 is filed and indexed with Decisions 1 to 16 and the owner answers above. The ADR index marks ADR-0070, 0091, 0095, 0126 and 0177 as amended by it. The adapter-protocol-4 note gains a marked amendment superseding the section 7 rows on publishing after services spread and the unchanged level-3 path. This ledger starts; `docs/tasks/README.md` and the seams ledger point here. | see `git log` |
| j02-kit | `extension/kit.css` is the one stylesheet for the extension pages: the sylin.org night tokens and base rules under `.gl-page`, the kit components, the setup layout, the companion with its two-segment stepper, and the popup. `scripts/package-extension.ps1` ships `kit.css` and `icons/ghostlight-mascot.png` and throws when a staged page names a local file the package lacks. The source-grep test for `setup.html` is deleted. | see `git log` |

## Owed live checks

- j01: none. Docs only.
- j02: none. The page markup that links the kit lands in j04, j06 and j07; the live look is checked there.

## Owed to the owner

- Publish the sylin.org redirects `/ghostlight/get.ps1` and `/ghostlight/get.sh` to the latest
  GitHub release assets (separate repository). x01 waits until both answer.
- The Edge listing is not requested; Edge installs from the Chrome Web Store (JQ9).

## Notes

- j01: the tree and the spec differed in these places.
  (1) The spec's dashboard line in Decision 15 ("the remote-code answer changes from No to Yes")
  is overridden by the owner ruling of 2026-10-16: the page code is local and never pulled from a
  remote source, so the answer stays No for 1.x and 2.0 and the 1.x listing is not corrected. The
  ADR still withdraws the sentence "All extension logic ships in the reviewed package" and its
  copies (`docs/legal/PERMISSION_JUSTIFICATIONS.md`, `docs/trust/faq.md`,
  `docs/trust/questionnaire.md`) because the extension works with the locally installed
  application, which supplies the page code (Edge policy 1.2.3 requires naming the dependency);
  j17 does that pass.
  (2) The ADR-0199 amendment of 2026-10-16 is titled "a start outlasts a service that is leaving on
  idle"; idle exit itself is Decision 12. The ADR cites both by those names.
  (3) Citations checked against their ADRs: 0070 D1 to D5, 0091 D3, 0126 D10 measure 2 (the
  "Second machine" line), 0177 D2 (`cli/setup.rs`, "extension walkthrough handoff"), 0095 D2
  (`--no-open`), 0196 D5 and 0199 D23. All say what the ADR claims.
  (4) `install/handoff.rs` still holds `EXTENSION_INSTALL_URL` and opens it once per home until
  j15. The page assets, the Settings kit (`ui/kit/kit.css`) and the loopback routes exist as the
  spec describes.
  (5) `docs/design/adapter-protocol-4.md` also gets one sentence on its store-fallback row,
  answered by JQ2.
- j02: ran the gate checks the files require: (3) `npm test` in `extension/` (464 pass), (5) the
  packager to `.tmp/lane-journey/extension.zip`, (8) `node --test tests/guards/*.test.mjs`, (9) a
  PowerShell parse of the packager, (10) the repository integrity script. The packager check was
  proven once by pointing `setup.html` at `missing.css`: it threw "Packaged page setup.html
  references missing.css, which the package does not include." The page was restored and the
  probe never committed.
  The tree and the spec differed in these places.
  (1) Links use `.gl-page :where(a)` and `.gl-page :where(a):hover`, not `.gl-page a`. A plain
  prefix would out-rank `.gl-btn--primary` and turn a link-button's text the accent colour. The
  focus ring is `.gl-page :where(:focus-visible)` for the same reason: at class weight, later
  component rules such as `.gl-btn` and `.gl-copy` keep their own corner radius when focused.
  (2) The companion has no body wrapper. The step names (`gl-stepper__names`) follow the stepper as
  a sibling and carry the old body padding as `padding-top`. The names use flat element classes,
  `gl-stepper__t` and `gl-stepper__w`. Segments are `gl-stepper__seg--1` and `--2`; segment N joins
  disc N-1 and disc N, and the discs are `--0`, `--1`, `--2`.
  (3) `is-celebrating` goes on `.gl-companion` (as the mockup did on its card). The kit's
  `.gl-mascot.is-celebrating`, `.is-asleep` and `.gl-mascot--sm` are not carried over: the companion
  and `data-light` replace them.
  (4) `.gl-pop` carries `width: 284px` (the mockup got it from viewer chrome). Popup, window and
  page share one `--wash` rule: `[data-light]` waiting .22, off 0.
  (5) The reduced-motion block drops the ember and notch resting rules. With animation off the
  ember already rests on its `--es` and `--eo` values, and the notch no longer exists. The halo
  resting look (.7) stays, and the waiting and off states still dim it.
  (6) `.gl-hr`, `.gl-wrap`, `.gl-stack` and the typography classes are kept as written; no
  extension page uses them yet, and j04 to j07 may drop what they leave unused.
  (7) The kit has no `--r-5` radius token: only the hero card used it.
- Release clock (coordinator, 2026-10-16): the owner's week of daily use starts 2026-10-16, now
  that the shell is complete. Only a change to the service or shell lifecycle (start, stop,
  presence, session end, update) restarts it. Install-time work does not.
- The owner pushed `dev` on 2026-10-16 (832d1abe..8a21f8d1).
