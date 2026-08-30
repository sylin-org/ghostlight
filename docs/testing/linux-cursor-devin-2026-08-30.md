# Linux Cursor and Devin harness evidence -- 2026-08-30

This report records the current-client acceptance pass for ADR-0148 on x86_64 CachyOS. It separates
installation and configuration proof from an authenticated model-path claim.

## Official artifacts

- Cursor 3.14.27 x86_64 AppImage, reached through Cursor's official Linux download redirect:
  SHA-256 `75e8a51d1b4812645b2396873747f75e363e9db2aa3e4f6eee73d897ea60b426`.
- Windsurf's official stable Linux x64 redirect served Devin 3.8.20:
  SHA-256 `cc269e129c8159f4994199b8df59054308a9c998beb22a5c443663437f09acce`.

Cursor was extracted because AppImage FUSE is unavailable on this machine. Both applications use
user-local launchers and desktop entries. The started executable images were:

- `/home/test/.local/opt/cursor/Cursor-3.14.27.AppDir/usr/share/cursor/cursor`
- `/home/test/.local/opt/devin/Devin-3.8.20/devin-desktop`

## Ghostlight registration

The deployed 1.2.0 orchestrator included ADR-0148 at SHA-256
`af7897063037e19b44492ffa07ae43def24d1c3ef9faa119e40203a5186e3e33`. The unchanged live MCP
connector was:

`/home/test/.cache/workbench/worktrees/github/sylin-org/ghostlight/target/release/ghostlight-mcp-connector`

The real installer selected only `cursor` and `devin`. It wrote that exact command with empty args
and environment objects to:

- `/home/test/.cursor/mcp.json`
- `/home/test/.config/devin/mcp_config.json`

The refreshed doctor projection reported both target ids as `installed`. The historical Windsurf
row remained independent at `/home/test/.codeium/windsurf/mcp_config.json`.

Devin's shipped CLI exposes `--add-mcp <json>`. Its bundled application code resolves
`mcp_config.json` from the current Devin configuration directory. This agrees with the admitted
registry path and avoids the false proof of treating a `devin-desktop` executable as evidence for
the historical Windsurf file.

## Verification boundary

Both editors restarted successfully from their installed images after registration. No new
Ghostlight connector child appeared under either editor before its visible onboarding or
user-session boundary. This report therefore proves official installation, real client startup,
current config-path consumption, ownership-safe Ghostlight registration, and live doctor state. It
does not claim that an authenticated Cursor or Devin model listed or invoked Ghostlight tools.
