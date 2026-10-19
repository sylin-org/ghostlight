# Latest result

[0057] linux-claude, 2026-10-19: L2-04, Claude Code with both the plugin and the installer's entry.

- Scope: the owner narrowed linux-2 on this machine to CachyOS ([0056], ledger Notes). L2-01,
  L2-03, L2-13 and L2-14 are blocked by that decision.
- Method:
  - Claude Code 2.1.303 on test-01, in an isolated `CLAUDE_CONFIG_DIR`.
  - The plugin `ghostlight@sylin-org-plugins` 1.0.1, from this repository's marketplace.
  - The installer's entry exactly as `ghostlight install` writes it.
  - `npx` was a stand-in that runs this repository's 2.0 launcher, because npm serves 1.3.14.
  - A local Messages API stand-in on 127.0.0.1 recorded the tools offered to the model. No login
    was used.
- Facts:
  1. Both servers start, and the model is offered 46 Ghostlight tools: 23 per server.
  2. The plugin's server is dropped only when a configured server has the same command and args.
     The name and `env` do not count, `${HOME}` is expanded first, and the configured server wins.
  3. Parents: the installer's edge is a child of Claude Code. The plugin's edge sits under
     `npm exec ghostlight` and then the node launcher.
  4. `tools/list_changed` is honored as a re-list. Added tools reach the model on the next turn;
     removed tools do not. A duplicate keeps itself out only by an empty first `tools/list`.
- Side finding (install/mod.rs): the installer ignores `CLAUDE_CONFIG_DIR`.
- Details: docs/tasks/linux-2/LEDGER.md, L2-04.
