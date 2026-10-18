# Latest result

## [0046] linux-claude: the open questions settled by the user-delight test; old leftovers gone

linux-claude, on test-01, at the owner's request ("delete unnecessary old stuff; use the user
delight point of view for all questions"). Details: [the ledger](../docs/tasks/linux-1/LEDGER.md),
"Follow-up, 2026-10-18". Each fix was deployed and checked live where the state can occur.

- 564bf635 `fix(language)`: the install's Next line is the step the window shows. With the
  extension in a profile and no browser connected it reads "Next: open Chrome, where Ghostlight is
  already added." instead of skipping to the agent step (ADR-0200 amendment).
- a241b633 `fix(browser)`: a browser that reports no name is Chromium, so plain Chromium is
  remembered as the last browser and the model reads "Chromium", not null (ADR-0149 amendment).
- 7cdd4031 `fix(language)`: an upload names the file input it used a "file field".
- dbd52fa6 `fix(install)`: every Linux install removes the folders 1.x left that nothing reads (the
  emptied `~/.local/state/ghostlight`, the 1.x window's `~/.local/share/org.sylin.ghostlight`).
  On Windows the 1.x window data may sit under `%LOCALAPPDATA%\org.sylin.ghostlight`; nothing
  removes it there yet.

For the owner, not changed: saved tab groups pile up in Chromium's bookmarks bar because Chromium
keeps every closed group and the extension API cannot see or reopen one; reuse (ADR-0137) works
only for open groups. Any fix is in the frozen extension.

Removed on test-01: the old home, the 0.x, 1.x and 0.5.7 leftovers, about 110 GB of stale build
folders and old `.tmp` evidence. Cline (CLI)'s 0.5.7 entry was replaced through Integrations' own
Fix first.

Please pull before touching `language/setup.rs` (install lines), `browser/mod.rs` (the browser
name), `work/forms.rs` (the upload subject) or `install/migration.rs` (leftovers).
