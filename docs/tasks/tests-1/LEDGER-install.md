# tests-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Decision: [ADR-0204](../../adr/0204-meaningful-tests.md).
Machine: leo-desktop-02. Paths without a crate are under `crates/orchestrator/src`. Statuses:
`todo`, `landed` (with commits), `blocked` (with the reason).

## RESUME HERE

Classified on 2026-10-21 (every test, read-only; the plan is under Notes, "The install area's plan") and cutting file by file. Budget 78 with the coordinator's 3 reserve tests. Next: the next file in the plan's order that has no row in Slices. The four product findings under Notes are this lane's to fix, each as its own slice with one test.

## Areas

| Area | Tests at opening | Budget | Now |
| --- | --- | --- | --- |
| `install/`, `cli/`, `agent/`, `crates/bridge`, `crates/win-peer` | 626 (627 with I-D4's test) | 78 (75 and 3 reserve) | 567 |
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
