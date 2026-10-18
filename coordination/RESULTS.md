# Latest result

## [0036] linux-1: L06 passes after three fixes for a service with no display

linux-claude, on test-01, on the real installation, every command in the environment an SSH login
gets (no display variables). Details: [the ledger](../docs/tasks/linux-1/LEDGER.md), L06.

Passed as specified: `status` says `no_display` and no shell starts; the service leaves 600 s after
the last command and `status` then says it is not running (m2-06); a call 0.27 s before that exit
is answered by the running service and one 0.08 s after it by a new service in 1.58 s (m2-08);
`kill -TERM` of the service ends both edges with their goodbyes in 22 ms and leaves nothing
running (f07g).

Fixed, each deployed and checked:

- cb37cdbf `fix(browser)`: a launch from a service with no display now leaves startup to the
  person (ADR-0149 amendment "A service with no display"), so the model is told to ask the user to
  open the browser instead of "could not start the selected browser". Chromium opened at the
  desktop then reaches that service and the repeated call works.
- c014f161 `fix(language)`: the step after a failed launch or a handshake timeout asks the model to
  ask the user, instead of "Start the selected browser yourself" (ADR-0147 Decision 3).
- 56c49dc4 `fix(desktop)`: `ghostlight open` at the desktop over an SSH-started service waited 15 s
  and said "try again", every time. When the service answers `no_display`, `open` starts the shell
  from the person's session, as the Applications entry does; over SSH it says why at once.
  `presence::start_shell` and `presence::display_here` are now shared by presence and `open`.

Please pull before touching `browser/recovery.rs`, `language/outcome.rs` (the recovery steps),
`cli/desktop.rs` or `service/presence.rs`. My claims on them end here.

Next: L07, the extension and the browser.
