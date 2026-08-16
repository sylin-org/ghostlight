# Linux harness roster and integration delight

Date: 2026-08-15

Status: Accepted by ADR-0125. This document records the research input for the roster, interaction
contract, visual-asset policy, path override behavior, and bounded desktop permissions.

## The short answer

Ghostlight should keep one fixed product-owned harness registry. It should make that registry more
expressive, not replace it with plugins or a discovery framework.

Each integration should have:

- a bundled, locally rendered product mark with recorded source and license provenance;
- one or more detected installations when a product has several Linux surfaces;
- a product-owned download destination;
- explicit Set up and Remove actions for a detected installation;
- Locate when normal detection misses an executable or configuration file;
- a copyable absolute path to `ghostlight-mcp-connector` at all times;
- a copyable harness-specific configuration fragment when automatic setup is unavailable or fails.

The first roster expansion should add GitHub Copilot CLI, Cline, Kiro, Qwen Code, Junie, and Kilo
Code. Goose and Continue are important, but their shared YAML mutation seam should be admitted only
with comment- and formatting-preserving evidence. Antigravity became eligible after its current
CLI, configuration path, and MCP startup were proved locally.

## Method and limits

No public source measures Linux agent-harness market share directly. GitHub stars are not users,
editor extension installs omit CLI use, and vendor user counts rarely separate operating systems.
This research therefore uses four bounded signals:

1. visible adoption, using repository activity and stars as one imperfect proxy;
2. official Linux availability;
3. first-class local stdio MCP support;
4. a stable user-scoped configuration that Ghostlight can merge and remove safely.

The GitHub API snapshot on 2026-08-15 included Cline at 66,253 stars, goose at 52,845, Continue at
35,493, Qwen Code at 27,047, Kilo Code at 26,887, and GitHub Copilot CLI at 11,099. All six had
current repository activity. The number is triage input, not a product ranking.

## Current roster

Ghostlight currently owns explicit registrations for Codex, Claude Code, Claude Desktop, Cursor,
Visual Studio Code, Windsurf, Zed, OpenCode, and Crush. That roster already covers the largest
general editors and several leading terminal agents. The missing high-value group is not another
editor shell. It is the Linux-native CLI and extension products that keep their own MCP settings.

The live CachyOS rehearsal also proved why each descriptor needs a short candidate list. Zed's
native package launches as `zeditor`, while another distribution may expose `zed`. Adding the
second executable name fixed detection without adding a detector type or another write target.

## Recommended additions

| Product | Linux MCP contract | Recommended posture |
| --- | --- | --- |
| GitHub Copilot CLI | `~/.copilot/mcp-config.json`; keyed `mcpServers`; official add, list, and remove commands | Add now. Stable JSON and a large installed ecosystem make this the clearest omission. |
| Cline | CLI uses `~/.cline/data/settings/cline_mcp_settings.json`; editor extensions have installation-specific storage | Add now, but model CLI and detected editor instances as separate targets below one Cline card. |
| Kiro | IDE and CLI use `~/.kiro/settings/mcp.json`; `kiro-cli mcp` also manages global entries | Add now. One stable JSON target covers both official Linux surfaces. |
| Qwen Code | `~/.qwen/settings.json`; keyed `mcpServers`; `qwen mcp` is first class | Add now. Stable JSON and direct Linux CLI support. |
| Junie | CLI and JetBrains plugin share `~/.junie/mcp/mcp.json` | Add now. One user JSON file reaches both surfaces without JetBrains installation probing. |
| Kilo Code | `~/.config/kilo/kilo.jsonc` or `kilo.json`, with legacy `opencode.jsonc` or `opencode.json` fallback; top-level `mcp`; `kilo mcp` is first class | Add now through the existing OpenCode-style JSONC seam, with Kilo's separate roots and ownership. |
| goose | `~/.config/goose/config.yaml`; stdio servers live below `extensions` | Add after one lossless YAML seam is proved. Do not rewrite a user's commented configuration with a lossy serializer. |
| Continue | `~/.continue/config.yaml`; `mcpServers` is an array | Add with the same lossless YAML seam. Its CLI and IDE discovery behavior still differs, so the global file is the only honest shared target. |
| Antigravity CLI | `~/.gemini/config/mcp_config.json` after the Gemini CLI transition | Add after live proof. Version 1.1.13 created and consumed this path, authenticated, and completed a real MCP startup during the implementation rehearsal. |

Primary configuration sources:

- [GitHub Copilot CLI MCP configuration](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers)
- [Cline CLI MCP support](https://github.com/cline/cline/blob/main/docs/cline-cli/overview.mdx)
- [Kiro MCP configuration](https://kiro.dev/docs/cli/mcp/configuration/)
- [Qwen Code MCP configuration](https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/)
- [Junie MCP configuration](https://junie.jetbrains.com/docs/junie-cli-mcp-configuration.html)
- [Kilo Code MCP configuration](https://github.com/Kilo-Org/kilocode/blob/main/packages/kilo-docs/pages/code-with-ai/platforms/cli-reference.md)
- [goose configuration](https://github.com/aaif-goose/goose/blob/main/documentation/docs/guides/config-files.md)
- [Continue CLI configuration](https://docs.continue.dev/cli/configuration)

### Products not promoted now

- Gemini CLI has very high visible adoption, but Google moved consumer users to Antigravity CLI in
  June 2026. Adding a newly retired default would create churn rather than coverage. See
  [Google's transition notice](https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/).
- OpenHands is usually a container or service deployment, and its own template describes local
  stdio MCP servers as a development path. A host-browser connector should not be promised across
  that container boundary without a distinct network and trust design.
- Pi has a large repository audience, but MCP is supplied through packages rather than one stable
  core user configuration.
- Roo Code is archived. Aider still has no stable core MCP client contract.
- Visual Studio Code already covers Copilot Chat's editor-level `mcp.json`. Copilot CLI needs a
  separate target because it reads a separate user file.

## The integration card contract

One card represents a product. A compact list inside the card represents concrete targets when the
product can be installed more than once. Cline is the motivating case: the CLI and extensions in
Visual Studio Code, Cursor, or Windsurf can coexist and read different files.

| Observed state | Primary action | Secondary actions |
| --- | --- | --- |
| Not detected | Install, opening the official Linux download page | Locate, Copy MCP command |
| Detected, not configured | Set up | Locate, Copy manual setup |
| Configured and current | Remove | Copy MCP command, show exact target |
| Ghostlight-owned but stale | Update | Remove, Copy manual setup |
| Malformed or foreign entry | Review | Locate, Copy manual setup; never overwrite |
| Automatic setup unsupported | Copy setup | Open settings location, Copy MCP command |

`Install` means open the vendor's official page. Ghostlight must not download or execute another
vendor's installer. The desktop receives only a harness id and resolves it through a closed table
of HTTPS destinations. The webview never supplies an address.

Refresh should run when the workbench regains focus and remain available explicitly. This makes the
download round trip feel immediate without adding a background watcher.

Set up and Remove are better words than Connect and Disconnect. Ghostlight is changing durable
configuration, not toggling a live socket. A harness may require a restart after either action.

## Locate without turning paths into policy

Locate should accept either an application executable or that harness's settings file:

- An executable proves detection. It does not silently change the canonical config destination.
- A settings file becomes a candidate target only after its filename and top-level shape match the
  selected harness. The UI shows the normalized path before Set up can write it.
- Located paths live in a small Ghostlight-owned per-user state file. They are machine facts, not
  project configuration, and contain no credentials.
- A missing located path falls back to ordinary detection and becomes visible as stale. It does not
  redirect writes to a guessed neighbor.

This adds one closed override seam. It does not add arbitrary recursive scanning, a plugin API, or
per-harness detector objects.

## The universal manual route

The workbench should always show and copy the absolute installed connector path, for example:

```text
/home/alex/.ghostlight/bin/v1.0.0/ghostlight-mcp-connector
```

Every descriptor also owns a minimal manual fragment generated from the same dialect used by the
automatic installer. The fragment must contain the exact current connector path and no secrets.
When setup fails, Ghostlight expands this section automatically and names the expected settings
file. A successful Copy action confirms what was copied without putting clipboard contents in the
audit log.

The connector path is the guaranteed floor. A harness-specific fragment is the delightful route.
Both remain useful for portable installs, unusual profiles, remote development, and harness
versions Ghostlight has not seen yet.

## Visual identity

Integration artwork must be packaged. The workbench must never fetch icons at runtime.

Use one small SVG or PNG per product plus a provenance manifest recording:

- product id and accessible name;
- upstream source URL and retrieval date;
- upstream license or brand-guideline URL;
- file hash and any permitted adaptation;
- a neutral Ghostlight-owned fallback mark when redistribution is unclear.

Simple Icons is useful input, but not a blanket clearance. Its graphics are CC0 while its own
license explicitly leaves trademark rights untouched, and its contribution guide excludes some
vendors. Each mark still needs an individual provenance decision. See the
[Simple Icons license](https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md).

The card should pair the mark with the product name, state text, and action labels. Color or artwork
must never be the only state signal.

### The Zed boundary

Zed's custom local MCP configuration has no icon field. Zed's MCP extension mechanism also does not
document a server icon and is planned for deprecation in favor of the official MCP registry. The
separate ACP agent extension mechanism does accept an SVG icon, but Ghostlight is an MCP server,
not an ACP agent. Building a second extension lifecycle would therefore not prove that the generic
MCP badge changes.

For 1.0, bundle Zed's product mark in Ghostlight and retain the proved `context_servers` setup. Track
the official registry identity path and use it when Zed exposes a supported visual field. An
upstream request can be drafted separately, but publishing it requires owner confirmation.

- [Zed custom and extension MCP setup](https://zed.dev/docs/ai/mcp)
- [Zed MCP extension deprecation notice](https://zed.dev/docs/extensions/mcp-extensions)
- [Zed extension auto-install setting](https://zed.dev/docs/reference/all-settings#auto-install-extensions)

## Lean implementation shape

Keep `HarnessRegistry` and the fixed descriptor list. Split the current one-row definition into
three small values:

1. `HarnessProduct`: id, name, packaged artwork key, official download destination, and platform
   availability.
2. `HarnessTarget`: a resolved config path, label, dialect, and evidence that found it.
3. `HarnessSupport`: executable names, normal target resolver, and optional user-located override.

The existing generic inspector and ownership-checked mutations remain the center. JSON, JSONC, and
TOML keep their current seams. YAML is one new dialect only after round-trip tests prove comments,
ordering, unrelated values, modes, atomic replacement, backup, idempotency, and owned removal.

Do not invoke a harness CLI for ordinary setup when direct ownership-aware editing is available.
CLI output and flags change, and a subprocess cannot prove which unrelated values it preserved.

## Acceptance evidence

Each added product needs the same bounded proof:

1. absent application reports Not detected and offers Install, Locate, and the connector path;
2. executable-only detection reports Detected without creating configuration;
3. Locate persists one validated override and stale overrides recover visibly;
4. Set up preserves unrelated bytes where the dialect promises preservation;
5. repeat Set up changes zero bytes;
6. a real harness process starts the exact installed connector and completes MCP initialization;
7. Remove deletes only Ghostlight's owned entry and repeat Remove changes zero bytes;
8. a foreign `ghostlight` entry is preserved and reported for review;
9. the packaged icon renders offline with an accessible product name;
10. manual command and fragment copy the exact current connector path.

This is enough evidence to earn a roster entry. A descriptor and an icon without a live MCP start
are not support.
