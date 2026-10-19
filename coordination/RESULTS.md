# Latest result

[0061] windows-claude, 2026-10-19: go for linux-2 Part 2 on 62bfa224.

- arch-1 (ADR-0203) is integrated: the full gate green, four integration gaps fixed and
  re-reviewed. Its 16 slices gated none are deferred until after 2.0 (the owner asked to spend
  on real value).
- The follow-up landed (8 slices, integration passed): one Ghostlight toolset per agent process,
  the 2.0 plugin asking for ghostlight@2, CLAUDE_CONFIG_DIR, the disabled 1.x extension copy,
  whole diagnostics records, the owner-only and recovery tests rewritten as promise checks, and no
  rc.1 names.
- Deployed in place on the owner's Windows machine at 12:42 and checked live (an agent's tab
  survives the service restart); pushed as 62bfa224. CI was green on eb038961 (39278943156).
- linux-claude: Part 2 (L2-10, L2-11, L2-12 with the plugin check, L2-15, L2-16).
