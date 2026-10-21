# tests-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: leo-desktop-02. Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Classified on 2026-10-21 (every test, read-only; the plan is under Notes, "The install area's plan") and cutting file by file. Budget 78 with the coordinator's 3 reserve tests. Next: the next file in the plan's order that has no row in Slices. The four product findings under Notes are this lane's to fix, each as its own slice with one test.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `install/`, `cli/`, `agent/`, `crates/bridge`, `crates/win-peer` | 626 (627 with I-D4's test) | 78 (75 and 3 reserve) | 214 |
| `crates/desktop` | 161 | 20 | 161 |

## Slices

One row per slice: what it cuts, its counts before and after, and what the kept tests protect
(promise, contract, defect, unsafe).

| Slice | Subject | Before | After | Kept for | Status |
| --- | --- | --- | --- | --- | --- |
| T-I1 | `install/layout.rs`: its three tests restated path literals; the edge command name's tie to the relay's executable name is now a compile-time assert | 627 | 624 | none (the assert costs no test) | landed (this commit) |
| T-I2 | `install/last_browser.rs`: its three tests repeated `service/mod.rs`'s B22 test, which remembers a connected browser and reads an unreadable record as none | 624 | 621 | none here (the service test proves it) | landed (this commit) |
| T-I3 | `install/setup_state.rs`: four tests of one record fold into one; the unknown-fields test goes (no reader writes them) | 621 | 618 | promise 1 (Desktop 24) | landed (this commit) |
| T-I4 | `install/browser_package.rs`: four tests of one classification fold into one, every case kept | 618 | 615 | promise 1 (Desktop 16, with defect 8ac06f75 as a case) | landed (this commit) |
| T-I5 | `install/container.rs`: five tests of the container probe fold into one, every assertion kept | 615 | 611 | defect 1 (53a04ef2) | landed (this commit) |
| T-I6 | `install/packaged.rs`: six tests fold into one rank rule; the path-mapping, absent-package and own-copy-without-file cases go to the writer golden's Store seeds | 611 | 606 | defect 1 (236485f4, 4fac15a7) | landed (this commit) |
| T-I7 | `install/entry.rs`: four path-spelling tests fold into one; its two entry-rule tests wait for the `cli/start.rs` slice, which absorbs them | 606 | 603 | defect 1 (664d0573) | landed (this commit) |
| T-I8 | `install/user_dirs.rs`: its seven tests fold into the harness-roots test in `install/mod.rs`, which now adds a relative value and the platform home rule | 603 | 596 | defect (I-D1, INST-4), counted under mod.rs | landed (this commit) |
| T-I9 | `install/apps.rs`: three location tests fold into one; the wrapper list goes to `provenance.rs`'s source test, which walks the wrappers | 596 | 593 | promise 1 (ADR-0161 connection source) | landed (this commit) |
| T-I10 | `install/browser_profiles.rs`: nine tests of the extension scan fold into one; four that restated the name cleaning and the key limit go | 593 | 581 | promise 1 (Desktop 23, 24; defect 68aa9656) | landed (this commit) |
| T-I11 | `install/user_assets.rs`: four tests fold into one; a content restatement and two routing arms go | 581 | 575 | defect 1 (00c6eccf), promise (owned bytes) | landed (this commit) |
| T-I12 | `install/desktop_entry.rs`: eight tests fold into one, a block per case; a routing arm goes | 575 | 567 | promise 1 (Desktop 15) | landed (this commit) |
| T-I13 | `install/windows_entry.rs`: nine tests fold into one; a byte-inequality check, a routing arm and a command-format restatement go; the AUMID pin stays inside it until the shared constant replaces it | 567 | 556 | promise 1 (Desktop 15) | landed (this commit) |
| T-I14 | `install/command_path.rs`: the Windows `Path` tests fold into one round trip; the Unix link tests go, as the plan ranked them below the budget line; a detail restatement and two routing arms go | 556 | 539 | promise 1 (the command on `Path` on Windows) | landed (this commit) |
| T-I15 | `install/user_path.rs`: twelve tests of the `Path` value fold into one, which runs on every system (one over the plan, which folded them into T-I14's Windows-only test; made back later in the area); two that only round-tripped the registry encoding through itself go | 539 | 526 | promise 1 (the value comes back byte for byte) | landed (this commit) |
| T-I16 | `install/icons.rs`: its seventeen tests go, with `IconCache::with_catalog` and `IconCache::version`, which only they used; the plan ranked the icon-cache and PNG-only folds below the budget line, and `agent/image.rs`'s tile golden still reads the cache | 526 | 509 | none here | landed (this commit) |
| T-I17 | `install/swap.rs`: six tests fold into one; three of the sweep's private details go (the moved-image stop in `deployment.rs` covers the listing) | 509 | 501 | promise 1 (ADR-0197 D14), defect fafb0cb5 as a case | landed (this commit) |
| T-I18 | `install/record.rs`: twenty tests fold into four: the record reads any release's file, payloads install only forward on the launchers' shared version vectors (the image hash as a case), a replace publishes whole or not at all, and its access is private or kept as asked; the refusal-words test goes (the refusal is proven in `deployment.rs`) | 501 | 485 | promise 1 (ADR-0197 D14), ADR-0203 D11.1 and D11.2 | landed (this commit) |
| T-I19 | `install/reconcile.rs`: sixteen tests fold into one: a reconcile points what it owns at `bin` once and leaves the rest, with another app's container naming instead of writing, the client's own container rewriting, and the Windows entries as a case; the tests of report words and dry-run naming go (wording, not a promise) | 485 | 470 | ADR-0197 D13 and D14, ADR-0199 D1 | landed (this commit) |
| T-I20 | `install/deployment.rs` and `install/version_directories.rs`: ninety-six tests fold into eight, one per promise of an install (place, repair or refuse; the edge rule; the development channel; the 1.x replacement with its policy; the replace order with the new instance's answer; older layouts retired; the quit-for-replacement wait; strays). The real stray now runs a copied system sleeper, not an ignored stand-in test; the quit wait proves the lock by order, not a 300 ms sleep; `v1.4` and `1.3.5` join the planted names that are not version directories. Gone: the version_directories unit tests (deployment proves them through an install), the Flatpak duplicate, the sleep-based held-lock test (nothing now proves two installs wait for each other; no ACCEPTANCE item names it) and the doctor-json mapping | 470 | 382 | ACCEPTANCE 14, ADR-0197 D14, the ADR-0199 development channel; defects e3e861b7, 26d6edba, 64164332, 83aa61f0, 80810b08, 31f2c835, bd29366c, 5b818ffc, a663e83d | landed (this commit) |
| T-I21 | `install/migration.rs`: forty-six tests fold into four: a 1.x home reaches the 2.0 home and a rerun does nothing; a migration that cannot finish safely refuses before changing anything; the retire rule settles each original against its copy; what 1.x left outside the home goes only while it is still Ghostlight's (its folders and the Flatpak route). Gone: the plan-serialization pin, the registration passthrough and the archive-label and copy-helper details (the end-to-end cases cover them) and `registrations_are_matched_by_directory`, which leaves `find_registrations` without a direct test | 382 | 340 | ACCEPTANCE C14, Desktop 24, ADR-0199; defects b4a5ad79, fafb0cb5, 58cf6cc1, 64164332, 83aa61f0, 80810b08, dbd52fa6, 816806e9, da955857 | landed (this commit) |
| T-I22 | `install/native_host.rs`: thirty-five tests fold into five: the browser table matches where each Chromium browser reads and names itself; recovery repairs only its own stale registration and only install adopts another; install is idempotent and uninstall removes only what Ghostlight owns (the root isolation as a case); install repoints Ghostlight's old registrations and leaves any other; inside an app container install writes the manifest and names what it cannot register. Gone: the link and no-temporary tests (`record.rs` proves the replace) and the table-order restatement, with their two helpers | 340 | 310 | Browser 22 and 23, ACCEPTANCE C14, the native-messaging contract; defects 47d26b0b, c51204ca, 53a04ef2, 85fbfa83, 2b251105 | landed (this commit) |
| T-I23 | `install/mod.rs`: ninety-one tests fold into nine: what makes an entry Ghostlight's; setting up everything touches only detected, changeable targets; evidence names only the program found; an update sets only Ghostlight's keys (no temporary left as a case); uninstall reaches every copy; each agent's file is the one it reads (Claude Code and located paths as cases); a Store app's own copy; the npm launcher entry; the agent catalog. Thirty-nine go that the writer golden already pins byte for byte (dialects, comments, foreign, Fix and backup, manual setup, the Store seeds) or that restate the roster, with the background-scan lock test, whose only proof was a 200 ms negative wait (the lock order is structural, arch-1 I6) | 310 | 228 | workbench gates 8 and 10, the ADR-0197 container amendments, the catalog contract; arch-1 I1 to I5; defects c51204ca, 1d220361, 0a0ef063, 567cf210, 218d386f, 5d589e1d, 4af49885, f51130d0, 765736e1, 41a3f8bf, 4fac15a7, 236485f4, d4bdaf9c | landed (this commit) |
| T-I24 | `cli/start.rs` and `install/entry.rs`: the entry rule is proven once, through `start` with a fake launcher: start.rs's thirteen tests fold into one, which gains a damaged-image row, and entry.rs's two `decide` tables go (every row is now a case of the start test); entry.rs keeps its path test, with its helpers moved into the Windows block that uses them | 228 | 214 | contract gate 14 (the entry rule) | landed (this commit) |

## Notes

- From the service lane's pilot (2026-10-21): `cli/desktop.rs` has a test near line 739 that
  repeats the service's deleted `show_reveals_the_existing_workbench` through a real host, with
  a 2 s sleep and a deadline under 1 s. Take it in this lane's `cli/` slice: keep it only as a
  condition-waiting test if no kept test proves the reveal.
- From the coordinator (2026-10-21): `crates/desktop/src/link.rs`'s golden test now checks the
  service's longest tray, its bearer and its address with the shell's own `validate()`,
  `well_formed_bearer` and `validate_workbench_url`, against the desktop goldens the service writes.
  Keep that test in this lane's `crates/desktop` cut; `tray.rs`'s `assert_eq!(MAX_TOOLTIP_LEN, 127)`
  only restates the number and can go.
- The install area's plan (2026-10-21), from a read of every test against ADR-0204 Decision 1,
  folded so each kept test is one behavior and keeps every meaningful assertion it absorbs:
  install/mod.rs and its neighbours 16; deployment, swap, record, reconcile and
  version_directories 14; migration, native_host, browser_profiles, browser_package, setup_state
  and last_browser 13; cli/ 15; icons, the path modules, agent/ and crates/bridge 12; crates/win-peer
  5 (one test per FFI function, as the coordinator decided); 75 in all. The 1.x retire rule is
  proven once, in migration.rs; the entry rule once, in cli/start.rs; I-D1 once, in the harness
  roots test; I-D2 becomes a compile-time assert. The coordinator's 3 reserve tests (budget 78):
  the `deployment install [--development|--restore] [--plan]` grammar that get.sh, get.ps1 and
  dev-loop call; an install.json golden the service writes and the npm and mcpb launchers read
  (the Rust side here); the dev extension id in the native-host manifest against
  extension/manifest.json's key. The AUMID `Sylin.Ghostlight` and the window class
  `org.sylin.ghostlight` become one constant each in ghostlight_relay, with no test (run the
  connectors guard).
- Product findings from the read, to fix in this lane (coordinator, 2026-10-21): (1) unverified:
  an empty, relative or unset home makes every default root relative to the working directory
  (UserDirs, and every resolver before INST-4); verify, and if real fix it with one test.
  (2) cli/deployment.rs `registration_lines` prints lines a person must act on to stdout, which
  get.sh and get.ps1 discard; show them where those scripts show output, with one test. (3) the
  writer golden records Fix on a host with no Ghostlight entry refusing with "the Ghostlight entry
  changed and is no longer eligible for Fix"; fix the words and regenerate the golden. (4)
  `HarnessRegistry::resymbol` has no caller (surface S3, known). Code that only deleted tests use
  goes with them (`HarnessRegistry::scans`, diagnostics' `MarkerWatcher` seam).
