# Linux `Set up everything` live evidence -- 2026-08-30

## Scope

This pass exercised the aggregate harness action through the deployed Ghostlight workbench on the
KDE Wayland development host. It then asked the installed clients' own MCP commands for status
where those commands exist. The person completed ZCode's account login for its final live lane;
the verification neither requested nor stored credential material. Other login and provider setup
boundaries stayed closed.

Deployed orchestrator SHA-256:
`a67209edadad9ed482f8119f5d5813a868c18ec2c844ef16571b61426f492359`.

## Installed client sample

Fourteen real supported products were executable on this host:

| Product | Version |
| --- | --- |
| Codex | 0.145.0 |
| Claude Code | 2.1.251 |
| Zed | 1.17.2 |
| ZCode | 3.10.1 |
| OpenCode | 1.18.25 |
| Crush | 0.91.2 |
| GitHub Copilot CLI | 1.0.80 |
| Cline | 3.0.60 |
| Kiro | 2.20.1 |
| Qwen Code | 0.21.12 |
| Junie | 26.8.10 |
| Kilo Code | 7.4.22 |
| goose | 1.46.0 |
| Continue | 1.5.47 |

The official ZCode AppImage could not mount because this host does not provide legacy
`libfuse.so.2`. Extracting it into a private AppDir and invoking its bundled `AppRun` with an
explicit `APPDIR` produced a working self-contained installation. ZCode opened its account-choice
screen successfully.

## Workbench journey

The pre-action workbench showed `Set up everything` enabled. The actual bundled button was reached
with keyboard focus under the person's one-time KDE remote-input approval and activated with
Enter. No command-line install route was used for this proof.

The visible outcome was:

`Ghostlight set up 3 and updated 13 MCP integrations. Restart or reconnect those clients to load
the tools. 1 target still needs attention.`

The button disabled after refresh. `doctor --json` then reported:

- 16 installed targets;
- 1 target needing attention; and
- 5 not-detected targets.

The one blocked target was Cline CLI's pre-existing commandless `ghostlight` entry. It remained
untouched and visible with evidence. Cline's independent Visual Studio Code target became current.
The absent Claude Desktop, Cursor, Windsurf, Cline Cursor, and Cline Windsurf targets remained
not detected and were not prepared speculatively.

ZCode's resulting `mcp.servers.ghostlight` entry contains the exact deployed connector command,
empty args, and empty env. After the person completed account login, the exact live process chain
was `ZCode -> zcode-host -> zcode-cli -> ghostlight-mcp-connector`. A read-only ZCode task asked for
`policy_explain`; ZCode rendered Ghostlight's `status: "succeeded"` authority report and stated that
nothing changed because the tool is read-only. This proves config consumption, MCP startup,
catalog discovery, model selection, tool invocation, and result rendering through ZCode itself.

## Client-native observations

The following results came from the clients, not from Ghostlight's parser:

- Claude Code reported Ghostlight `Connected` after its MCP health check.
- OpenCode, Qwen Code, and Kilo Code each reported Ghostlight `connected`.
- Codex reported the exact connector enabled.
- GitHub Copilot CLI listed Ghostlight as a user-local server.
- Cline listed the foreign commandless entry as stdio, demonstrating why Ghostlight's stricter
  ownership inspection remains necessary.
- Kiro exposed `mcp list` and `mcp status` but refused both before login.
- goose refused its doctor before provider configuration.
- ZCode spawned the exact connector through its own CLI child and completed a real
  `policy_explain` model invocation in its visible task surface.
- Crush, Zed, Junie, and Continue expose no non-interactive local MCP health command in the tested
  versions. Their current registrations are proven by Ghostlight's parser and writer tests here,
  not by a real client model path in this pass.

This evidence proves the deployed aggregate UX, the ownership boundary, four live client health
checks, two further client-native config reads, and one complete ZCode model path. It does not
claim all fourteen clients reached a model invocation.
