# arch-1: the second sweep, 2026-10-20

On 2026-10-20 the owner asked for "an overall quality pass. I noticed several magic
strings/numbers in the code, confusing namespaces/function names, this kind of thing. Now that
2.0.1 is stabilized, shouldn't we make sure the content is more maintainable? Identify convoluted
architecture takes and streamline them?" The owner then chose to hold 2.0.1 until the pass lands
("Refactor first, ship later"). The decisions are the
[ADR-0203 amendment of 2026-10-20](../../../adr/0203-one-owner-for-each-fact.md#amendment-2026-10-20-the-second-sweep),
and the lanes, machines and order are in [BOOTSTRAP.md](../BOOTSTRAP.md#the-second-sweep-2026-10-20).

## How it ran

Seven lenses, one per area, read the tree at 8f6a43ec. Each wrote its own file below. A fresh
skeptic per area then tried to refute every finding against the live code, and appended a verdict,
a class and the ADR-0203 slice the finding meets. No finding was refuted.

| Area | File | Findings | Confirmed | Partial |
| --- | --- | --- | --- | --- |
| `language/` | [language.md](language.md) | LANG-1 to LANG-12 | 7 | 5 |
| `install/` | [install.md](install.md) | INST-1 to INST-15 | 9 | 6 |
| `browser/` and the page assets | [browser.md](browser.md) | BROW-1 to BROW-15 | 9 | 6 |
| `work/`, `workspace/`, `governance/` | [work.md](work.md) | WORK-1 to WORK-15 | 9 | 6 |
| `service/`, `intake/`, `cli/` | [service.md](service.md) | SERV-1 to SERV-14 | 9 | 5 |
| `workbench/`, `ui/`, `crates/desktop` | [surface.md](surface.md) | SURF-1 to SURF-14 | 5 | 9 |
| `extension/` and the small crates | [extension.md](extension.md) | EXT-1 to EXT-15 | 9 | 6 |
| All | | 100 | 57 | 43 |

A finding's file is its specification, read with its skeptic's corrections. The class line says
whether a fix is a refactor (nothing a person or agent meets changes), a defect (behavior is wrong
today, so the fix carries a test), model-visible or visible (words or facts change on purpose),
persisted, frozen (a packaged extension file) or locked (`crates/edge` or `crates/relay`).

## Defects the sweep found

Each lands as its own fix slice with a regression test, before the refactors in its files. The
ones a skeptic found while checking another finding carry the id of that finding.

| Defect | Where | Lane |
| --- | --- | --- |
| An empty `XDG_CONFIG_HOME`, `APPDATA` or `CODEX_HOME` resolves client paths below the working directory, so an install writes there (with `CODEX_HOME=` set, a project's own `config.toml`) | INST-4 | install |
| A resume token near 64 KiB (about 270 tabs in one session) rides in an opening over the opening cap, so the reconnect is refused until the client restarts its server | SERV-1 | core-service |
| Uninstall waits 10 s for a quit that can take 15 s, and says the service did not stop | SERV-2 | install |
| After an update the relay's return window may be shorter than its backoff, which could start a second copy of an open browser; a test settles it first | BROW-9 | core-service |
| `ghostlight call` panics on an argument that is not UTF-8 | SERV-4 | install |
| A wheel scroll that leaves out `direction` is refused though the schema advertises "down", and a wheel scroll accepts any `amount` and ignores it | LANG-3 | core-language |
| A coverage note can carry a summary past 500 characters, and LANG-6's bound is one short | LANG-6 | core-language |
| "1 unreadable history entries were skipped." | LANG-9 | core-language |
| The check that replaces handle-like page text never matches a real `diag_` cursor or `invocation_` id | LANG-2 | core-language |
| An inline upload of exactly 5,000,000 bytes is refused, though LANGUAGE.md accepts it | WORK-1 | core-work |
| A flow step whose tab close the policy refuses shows the generic notice instead of the kept-tab one | WORK-8 | core-work |
| A selector wait leaves out the readiness fact the other waits give (a deliberate model-visible change) | WORK-13 | core-work |
| A policy draft near the limit is refused with "That request is too large.", never with the policy-size sentence; after Clear a live coverage change can drop older rows' details | SURF-5 | surface |
| The applied-rules toast reads "Your rules are applied: Your rules 2026-10-20." | SURF-3 | surface |
| A dialog that opens during world setup reads `browser_primitive_failed` instead of `dialog_open` | EXT-9 | package |

## Not taken

- **EXT-3 and EXT-4** (the edge's twin relays, stale relay docs): they edit `crates/edge` and
  `crates/relay`, so they need a relay revision, and the service refuses every edge older than its
  own revision. Every running 2.0.x edge would be refused. Decision 13 stands.
- **EXT-1** (fold `crates/bridge`): ADR-0203 declined it (V-11), and it would also amend ADR-0145
  Decision 4. It stays declined.
- **BROW-1's move of the registry into protocol 4**: browsers, families and protocol levels are
  plural (ADR-0114 D1, ADR-0196 Decision 3), so the registry stays in `browser/`; only protocol
  4's frames and hello validators move into it.
- **BROW-6's `mirror!` macro**: it fits only the simple enums. The ten test-only reverse
  conversions go.
- **BROW-13's move of `work.rs` and `documents.rs`**: it recreates the near-collision BROW-5 names.
  The golden path built from `CARGO_MANIFEST_DIR` and the `session.rs` rename are taken.
- **WORK-14**: browser events change leased workspaces through the store by design, and C4
  recorded the choice. The lease methods WORK-7 names are deleted.
- **WORK-8's typed coverage on `Terminal`**: ADR-0203's route, reading coverage from the audit
  projection, stands.
- **SERV-10's single browser handle**: about 34 tests swap in the fake port through the second
  handle. The one start function is taken.
- **SURF-9's rename of `ui/kit/kit.css`**: the packaged `extension/kit.css` names it in its header,
  so the rename rides with the package lane's resubmission or not at all.
