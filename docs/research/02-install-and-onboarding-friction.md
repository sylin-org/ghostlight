# Install & Onboarding Friction

**Date:** 2026-07-01 · **Track:** Delight (developer) · **Source:** research agent (verbatim report)

> The single-binary differentiator (validated with evidence), and the native-messaging landmine
> that a binary does *not* solve.

---

## TOP INSTALL PAIN POINTS (ranked by frequency across sources)

### 1. npx-on-Windows is fundamentally broken for MCP stdio
The most-repeated failure. On Windows `npx` is `npx.cmd` (a batch wrapper); spawned via
`cmd.exe /c`, the stdin/stdout pipes MCP's stdio transport depends on don't connect, so the
server "starts but hangs silently."
- Playwright MCP #1540: on Windows `npx` is `npx.cmd`; spawned via `cmd.exe /c` the stdio pipes
  don't connect. `claude mcp add` with `cmd /c npx` mis-parses `/c` as a path → `C:/`. Working
  fix: bypass npx, call `node` with an absolute path to `cli.js`.
  https://github.com/microsoft/playwright-mcp/issues/1540
- LM Studio #1301: "'npx' not a valid command"; MCP doesn't load.
  https://github.com/lmstudio-ai/lmstudio-bug-tracker/issues/1301
- chrome-devtools-mcp troubleshooting (official): "On Windows, running a Node.js package via
  `npx` often requires the `cmd /c` prefix."
  https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/troubleshooting.md
- A dedicated third-party repo exists *only* to translate chrome-devtools-mcp setup into
  Windows-native scripting. Its existence is itself evidence of systemic Windows friction.
  https://github.com/saifyxpro/chrome-devtools-mcp-windows-guide

### 2. Node/npx version + cache hell ("module not found")
- chrome-devtools-mcp requires Node v20.19+/v22.12+/v24+; wrong version or corrupt npx cache →
  `ERR_MODULE_NOT_FOUND`. Official fix: `rm -rf ~/.npm/_npx && npm cache clean --force`, and
  "make sure your MCP client uses the same npm and node version as your terminal" (clients often
  inherit a different Node).
- Recommended `--yes` to auto-accept the npx install prompt (the interactive prompt silently
  blocks startup otherwise).

### 3. Multi-component architecture never fully connects: AgentDeskAI browser-tools-mcp
Requires THREE separately-installed/-running pieces: (a) Chrome extension (manual zip + load
unpacked), (b) `npx @agentdeskai/browser-tools-mcp` in the IDE, (c)
`npx @agentdeskai/browser-tools-server` in a *separate terminal*.
https://github.com/AgentDeskAI/browser-tools-mcp
- Chronic failure: users verify every component yet tools still return "Not connected."
  Issue #101 (verified node server on 3025, extension connected, still "Not connected").
  https://github.com/AgentDeskAI/browser-tools-mcp/issues/101 · /issues/91 · /issues/145
- Cross-client failures: cline #2217; Windsurf #25; Cursor #209/#204.

### 4. Native messaging host registration (directly relevant to THIS project)
Chrome finds the host only via an exact registry key (Windows) or JSON manifest at an OS-
specific path. "Specified native messaging host not found" = key/manifest missing or misplaced;
**Windows requires the registry `(Default)` value to be a full absolute path.**
https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- **Claude Code #21426 is the most on-point evidence**: same binary+extension+native-messaging
  design. On Windows, even with registry key, manifest, `.bat` wrapper and correct extension ID
  all verified "100% correct," the extension never invokes the host: "No log file was ever
  created. This definitively proves the Chrome extension is NOT initiating the native messaging
  connection." All tools dead; 14 troubleshooting attempts failed.
  https://github.com/anthropics/claude-code/issues/21426
- **Extension-ID / host-name collision** (Claude Code #20887): with both Claude Desktop and
  Claude Code installed, the extension requests host name
  `com.anthropic.claude_browser_extension` and always binds to Desktop's host, so Code's tools
  fail. A cautionary tale about host-name namespacing.
  https://github.com/anthropics/claude-code/issues/20887

### 5. Extension loading friction (MV3 unpacked, dev mode, ID discovery)
- `allowed_origins` in the host manifest must contain the exact `chrome-extension://<id>/`,
  **no wildcards**, and the ID is only knowable *after* loading the unpacked extension, a
  chicken-and-egg step.
- Mitigation to copy: webpage-mcp's installer auto-discovers local unpacked extension IDs from
  browser profiles and writes them into `allowed_origins`.
  https://github.com/mcpland/webpage-mcp

### 6. Browser-binary download step (Playwright/Python stack)
- `playwright install` fails behind firewalls; startup-time browser download "often leads to
  timeouts, crashes, or setup failed errors"; run manually first. browser-use inherits this.

---

## DELIGHT SIGNALS (easy install)

- **Single static binary, zero deps, GitHub-Releases download for mac/Linux/Windows** is the
  emerging "it just works" bar (e.g., codebase-memory-mcp markets exactly this).
- **Anthropic's MCPB (formerly DXT) desktop bundles** = one-click install in Claude Desktop,
  cross-OS, language-agnostic: "ideal for users who shouldn't have to handle npm."
  https://www.speakeasy.com/mcp/distributing-mcp-servers
- No source praised an npx-based browser MCP for *easy* setup; praise clusters on compiled-binary
  / one-click distribution.

---

## Is "single self-contained binary, zero runtime deps" a real differentiator? YES

Strongest single source: a first-hand Node→Go MCP rewrite (dev.to/zoharbabin):
- "npx spawns deeply nested process trees. When the parent MCP client crashes, the Node.js
  process doesn't receive a signal. It keeps running" (orphaned processes; unsolvable in Node).
- Go single binary: "No runtime process tree. EOF on stdin = immediate exit… The entire problem
  category disappeared." Also 430MB→~25MB idle, 2-4s→<100ms startup.
  https://dev.to/zoharbabin/from-nodejs-to-go-rebuilding-an-mcp-server-for-production-oil

A single binary directly eliminates pain points #1, #2, #6 and shrinks #3.

**Caveat: the binary does NOT eliminate #4 and #5** (native-messaging registration +
extension-ID/`allowed_origins` wiring). Those are inherent to any browser-extension + native-
host design, and Claude Code #21426 / #20887 prove even Anthropic ships broken on Windows and in
the dual-install collision case. The differentiator only fully lands if the binary **also ships
an installer that programmatically writes the registry key/manifest with absolute paths and
auto-discovers/injects the unpacked extension ID into `allowed_origins`** (the webpage-mcp
pattern). That combination (zero-runtime binary + self-registering native-messaging installer)
would be genuinely novel against the field.

## Sources
[playwright-mcp #1540](https://github.com/microsoft/playwright-mcp/issues/1540) ·
[lmstudio #1301](https://github.com/lmstudio-ai/lmstudio-bug-tracker/issues/1301) ·
[chrome-devtools-mcp troubleshooting](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/troubleshooting.md) ·
[saifyxpro windows guide](https://github.com/saifyxpro/chrome-devtools-mcp-windows-guide) ·
[browser-tools-mcp](https://github.com/AgentDeskAI/browser-tools-mcp) ·
[#101](https://github.com/AgentDeskAI/browser-tools-mcp/issues/101) ·
[claude-code #21426](https://github.com/anthropics/claude-code/issues/21426) ·
[claude-code #20887](https://github.com/anthropics/claude-code/issues/20887) ·
[Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging) ·
[webpage-mcp](https://github.com/mcpland/webpage-mcp) ·
[Node→Go rebuild](https://dev.to/zoharbabin/from-nodejs-to-go-rebuilding-an-mcp-server-for-production-oil) ·
[Speakeasy MCPB/DXT](https://www.speakeasy.com/mcp/distributing-mcp-servers)
