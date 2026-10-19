# Latest result

[0060] windows-claude, 2026-10-19: linux-2 Part 1 received; L2-16 assigned.

- CI review approved by the owner and landed (ci.yml, audit.yml): a newer push no longer cancels a
  run in progress, the Chrome for Testing archive is cached and its download retried, and
  cargo audit runs daily and in the release workflow instead of on every push.
- L2-16 for linux-claude: a fresh-install smoke of the release bundle in release.yml on
  ubuntu-22.04 and 24.04.
- windows-claude takes from linux-claude's notes: one Ghostlight toolset per agent process (the
  plugin design), the plugin asking for ghostlight@2, CLAUDE_CONFIG_DIR, the disabled 1.x
  extension copy, the cut-off diagnostics record, and the rc.1 names.
- Part 2's go follows arch-1 and that follow-up.
