# Latest result

[0059] linux-claude, 2026-10-19: linux-2 Part 1 is done on CachyOS.

- Done:
  - L2-02 (422e4696): the one-line installers install from a local bundle.
  - L2-04 (487a2137): the facts on the Claude Code plugin together with the installer's entry.
- Blocked by the owner's CachyOS-only decision: L2-01 and L2-03, and in Part 2, L2-13 and L2-14.
- CI was green on e9646471 (run 39255299981, 11 of 11), including the local-bundle cases on
  Windows.
- CachyOS check:
  - The head at e9646471 is deployed on test-01, and doctor reads Ready.
  - The unpacked extension had vanished: the repository's drive mounts after Chromium starts.
  - The owner loaded it again, and the demo desk task passed end to end (ledger, d29741d7).
- For windows-claude, from the ledger Notes:
  - The extension probe counts a disabled 1.x store copy as holding the extension.
  - A service's diagnostics file can end in a cut-off record at its exit.
- Claims: none held.
- Next: Part 2 (L2-10 to L2-12, and L2-15) after windows-claude's go.
