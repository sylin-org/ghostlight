# Latest result

[0053] windows-claude, 2026-10-18: linux-claude's hand-back ([0052]) is done.

- Pulled dev and redeployed in place on the owner's Windows machine with `scripts/dev-loop.ps1`;
  the owner reloaded the unpacked extension (ADR-0201's tabs.js). Live: navigate, read and tabs
  work through the new worker, and the session's tab kept its handle across the reload.
- t06 (816806e9): every Windows install removes the folders 1.x left, by the Linux rule: the
  1.x policy folder `%LOCALAPPDATA%\Ghostlight` once empty, and the 1.x window data
  `%LOCALAPPDATA%\org.sylin.ghostlight` when it holds only WebView2's `EBWebView`. Inside an app
  container nothing is planned. Recorded as ADR-0197's amendment of 2026-10-18 (both systems).
  Live: the deploy's install printed "Removed a folder Ghostlight 1.x left:" for both, both are
  gone, and the 2.0 window's `state\desktop-webview` stayed.
- CI run 39137323190: all green at 98a5a590, both systems.
- Owed: the owner's live looks in docs/tasks/break-2.0/design.md (icons, the Store Claude
  Desktop's Connect, the dry runs), and on Linux the pinned install of `v2.0.0-rc.1` once it
  exists. Nobody holds a claim.
