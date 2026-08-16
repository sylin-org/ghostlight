# Linux harness roster evidence -- 2026-08-15

Status: development-host pass; release candidate and Ubuntu gates remain open

This record covers ADR-0125's expanded MCP harness roster on the CachyOS development host. All
configuration writes and harness credentials used a disposable home rooted at
`/tmp/ghostlight-roster-probe-20260815`. No authentication value is recorded here.

The tested Ghostlight siblings came from `.target-harness-roster/debug`. Every registration named
the exact absolute `ghostlight-mcp-connector` in that directory. This is current-source process
evidence, not a packaged-candidate or clean-machine result.

## Installed roster and live result

| Product | Version | User configuration | Live result |
| --- | --- | --- | --- |
| GitHub Copilot CLI | 1.0.80 | `~/.copilot/mcp-config.json` | `/mcp show ghostlight` reported Connected and 22 of 22 tools through the exact connector. |
| Cline | 3.0.55 | `~/.cline/data/settings/cline_mcp_settings.json` | A real headless Cline process spawned the exact connector and completed MCP startup before its deliberately dummy model-provider boundary. |
| Kiro CLI | 2.18.1 | `~/.kiro/settings/mcp.json` | Authenticated `chat --require-mcp-startup` started the exact connector, submitted all tool schemas to Bedrock, and returned the requested normal model response. |
| Qwen Code | 0.21.12 | `~/.qwen/settings.json` | `qwen mcp list` reported Ghostlight connected through the exact connector. |
| Junie | 26.8.10 (2651.6) | `~/.junie/mcp/mcp.json` | `/mcp` reported Ghostlight Active; its log recorded a completed connection through the exact connector. |
| Kilo Code | 7.4.22 | `~/.config/kilo/kilo.json` | `kilo mcp list` reported Ghostlight connected through the exact connector. |
| goose | 1.46.0 | `~/.config/goose/config.yaml` | A real `goose run` reached ready state through the exact connector and exited successfully. |
| Continue CLI | 1.5.47 | `~/.continue/config.yaml` | `cn` spawned the exact connector; its log recorded successful MCP service initialization and ready services. |
| Antigravity CLI | 1.1.13 | `~/.gemini/config/mcp_config.json` | `/mcp` listed all 22 Ghostlight tools after standards-defined discovery fallback through the exact connector. |

The candidate packages were installed from their current official package or installer channels.
The table records process behavior, not an endorsement of any vendor or a market-share claim.

## Compatibility failures found and closed

Junie requested MCP revision `2025-03-26`. The prior connector counteroffered `2025-11-25`, which
Junie rejected. The connector now echoes each compatible initialized revision in the set
`2024-11-05`, `2025-03-26`, `2025-06-18`, and `2025-11-25`; an unknown value receives the latest
counteroffer. Junie then became Active and listed Ghostlight normally.

Antigravity began with `server/discover` for revision `2026-07-28`. The prior connector treated it
as an illegal pre-initialize request and exited. The connector now answers the standards-defined
discovery probe with a private zero-TTL result and only the four initialized revisions it actually
serves. Antigravity selected `2025-11-25`, initialized, and listed all 22 tools. Ghostlight does
not claim full stateless `2026-07-28` support.

Kiro completed MCP startup but its Bedrock path rejected `browser_tabs` because the input schema
used root-level `oneOf`. Every Ghostlight tool now advertises a typo-closed top-level object without
root-level `oneOf`, `allOf`, or `anyOf`. The typed decoder still enforces exact conditional input
shapes before governance or dispatch. After the running pre-fix service was replaced by the rebuilt
binary, the same authenticated Kiro command accepted the complete catalog and returned normally.

## Ownership and repeatability

One explicit setup pass selected all nine new target ids. A second pass reported every registration
current. SHA-256 fingerprints taken before and after the second pass proved every target file was
byte-identical.

One explicit removal pass removed only Ghostlight's owned entry from all nine targets. A second
removal pass changed no target byte. The same fingerprint set included absent-file state, so a file
that contained only Ghostlight could not be silently recreated. Re-adding all nine targets then
restored their current registrations.

Unit coverage separately proves foreign-entry preservation, malformed-input refusal, JSONC comment
and trailing-comma preservation, YAML comments and ordering, file mode, exact no-op bytes, stale
located-path recovery, mismatched Locate refusal, independent Cline targets, and preservation of a
sibling's own nested YAML key or list item named Ghostlight.

## Remaining boundary

This pass closes current-source Linux process compatibility for the expanded roster. It does not
replace the provenance-bound release artifact, Debian and Ubuntu package evidence, the visible
Ubuntu GNOME Wayland lifecycle, the matching store extension, login/reboot, or publication gates.
