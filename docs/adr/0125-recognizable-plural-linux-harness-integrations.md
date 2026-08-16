# ADR-0125: Recognizable plural Linux harness integrations

- Status: Accepted
- Date: 2026-08-15
- Amends: ADR-0071, ADR-0096 Decision 2, ADR-0102 Decision 6 and Amendment A3, ADR-0107
  Decision 4, and ADR-0117
- Builds on: ADR-0103, ADR-0119, ADR-0123, and Research 26

## Context

Ghostlight's fixed harness registry safely merges one owned MCP registration into nine known client
configurations. A fresh Zed installation exposed two experience gaps. First, the CachyOS package
uses `zeditor`, so one missing executable alias hid an otherwise supported product. Second, Zed
rendered a working custom server with a generated initial because its current custom MCP schema has
no icon field.

The workbench also treats every harness as one path and one button. That is too narrow for products
such as Cline, whose CLI and editor extensions can coexist with separate configuration files. A
user whose harness is not detected has no useful action, while a user whose automatic setup fails
cannot copy the one value every stdio harness needs: the absolute connector command.

Research 26 surveyed current official Linux and MCP contracts. It found six stable JSON or JSONC
targets ready for direct support, two important YAML targets that require a lossless mutation seam,
and one successor product whose recent migration still needs live evidence. It also found no
single icon source that clears every product's trademark and redistribution conditions.

The design must improve coverage and recognition without adding a plugin system, recursive machine
scan, arbitrary URL opener, generic shell runner, or new service boundary.

## Decision

### 1. Keep one fixed registry, with products and concrete targets

`HarnessRegistry` remains a closed orchestrator-owned list. Each entry carries product identity,
official download destination, detection aliases, config dialect, manual fragment, and one concrete
configuration target. Several entries may share one product id. The workbench groups those entries
into one card and shows each detected target independently.

This is a data split, not a plugin interface or trait hierarchy. Existing inspection,
serialization, ownership recognition, backup, atomic replacement, and idempotency remain shared.

An installation action always names a concrete target id. It never guesses which coexisting target
the user meant.

### 2. Expand the accepted roster in two proven dialect groups

The JSON and JSONC group is accepted immediately:

- GitHub Copilot CLI;
- Cline CLI plus detected Visual Studio Code, Cursor, and Windsurf extension instances;
- Kiro CLI and IDE through their shared user config;
- Qwen Code;
- Junie CLI and JetBrains plugin through their shared user config; and
- Kilo Code.

Goose and Continue join only when one YAML seam proves that it preserves comments, ordering,
unrelated values, file mode, and exact no-op bytes while inserting, updating, and removing only
Ghostlight's entry. This gate may be completed in the same implementation batch. It is not a reason
to use a lossy serializer.

Antigravity is admitted after installed build 1.1.13 consumed the documented
`~/.gemini/config/mcp_config.json`, started the exact Ghostlight connector, and completed MCP
startup in the release rehearsal. The retired Gemini CLI is not added as a new target.

Every admitted target requires a real Linux process proof. A descriptor-only implementation is not
support.

### 3. Use setup language and a closed action vocabulary

The workbench uses these actions:

- `Set up` adds Ghostlight to one detected target;
- `Update` replaces an exact stale Ghostlight-owned registration;
- `Remove` removes only an exact owned registration;
- `Install` opens the product's official Linux download page when the product is not detected;
- `Locate` asks the person for an executable or settings file when normal detection missed it;
- `Copy MCP command` copies the exact installed connector path; and
- `Copy setup` copies the smallest target-specific configuration fragment.

Connect and Disconnect are retired from this surface because the mutation is durable configuration,
not a live socket toggle.

The workbench refreshes harness detection after it regains focus. It keeps an explicit re-check
action. No filesystem watcher or polling service is added.

### 4. Make the manual route universal

Every target summary carries the absolute current `ghostlight-mcp-connector` path and a manual
fragment generated from the same typed dialect as automatic setup. The workbench exposes both even
when automatic setup is available. A failed automatic setup leaves them visible and names the
expected settings path.

Clipboard writes cross one closed desktop command. JavaScript supplies a target id and a closed
choice of command or fragment, never arbitrary clipboard contents. Clipboard data is not audited.

### 5. Download and Locate stay closed at the desktop edge

Download receives a product id. Rust resolves that id through a fixed table of official HTTPS
destinations and passes only the resulting product-owned URL to the operating-system opener. The
WebView never submits an address. Products without a Linux download do not expose the action.

Locate uses a native file picker reached through a closed command. A selected executable records
detection evidence only. A selected settings file is accepted as a write target only when its name
and top-level shape match the selected harness. The normalized override is shown before any later
Set up action.

Overrides live in one Ghostlight-owned per-user state document. They contain paths and target ids,
never credentials. A missing override becomes visibly stale and falls back to ordinary detection;
it never redirects a write to a guessed neighboring path.

The Tauri WebView receives no generic dialog, filesystem, opener, clipboard, or shell permission.
The Rust adapter registers native plugins for its own bounded commands only.

### 6. Package visual identity with provenance

Every product card renders one packaged local visual artifact plus the product name and text state.
No icon is fetched at runtime. Each third-party asset has a tracked provenance record containing
source, retrieval date, license or brand guidance, and file hash. When redistribution is unclear,
Ghostlight ships a neutral product-owned fallback rather than copying a mark without provenance.

Artwork and color are never the only state signal. Missing artwork degrades to a bundled neutral
mark without hiding the integration.

Simple Icons is one possible asset source, not blanket trademark clearance. Its CC0 copyright
license does not waive trademark rights.

### 7. Do not invent a Zed extension that cannot meet the goal

Zed's current custom MCP schema has no icon field. Its MCP extension mechanism does not document a
server icon and is planned for deprecation in favor of the MCP registry. The separate icon-bearing
ACP extension surface does not apply to an MCP server.

Ghostlight therefore packages Zed identity in its own workbench and retains the proved direct
`context_servers` registration. It will use Zed's official registry identity path when that path
exposes supported visual metadata. A separate Zed extension is not added merely to move the same
generic badge behind another lifecycle.

### 8. Restore compatible MCP revision negotiation

The installed Junie 26.8.10 client requested MCP `2025-03-26`, started the exact packaged
connector, and rejected Ghostlight's `2025-11-25` counteroffer before listing tools. This is direct
release evidence that ADR-0096 Decision 2's removal of older-revision negotiation excludes a
current supported harness.

Ghostlight again echoes `2024-11-05`, `2025-03-26`, `2025-06-18`, or `2025-11-25` when a client
requests one of those known compatible revisions. An unknown revision receives the latest
`2025-11-25` counteroffer. This restores ADR-0049 Decision 1's pure negotiation rule, but not its
pre-initialize behavior. The connector retains one tools-only `mcp_2025_11_25` state machine, one
lifecycle, and one product surface. No compatibility handler, protocol registry, SDK, or service
branch is added.

The clean-room connector also restores the `2026-07-28` `server/discover` compatibility probe
required for stdio fallback. It returns orchestrator-owned identity and instructions, a private
zero-TTL result, and exactly the four initialized revisions Ghostlight actually serves. It does
not advertise `2026-07-28`. A client with an initialized-revision implementation can therefore
fall back without interpreting EOF as a broken server. Antigravity 1.1.13 used this route and then
listed all 22 Ghostlight tools.

This deliberately narrows ADR-0096 Decision 2's claim that the clean-room 1.0 tree contains a full
stateless `mcp_2026_07_28` handler. It does not. The historical implementation record describes a
pre-rewrite tree and is not current source authority. Claiming that revision before its
per-request metadata, explicit workspace, cache, result, and cancellation contracts are all
present would be worse than a standards-compliant compatibility fallback.

### 9. Keep tool input schemas portable at the root

Kiro CLI 2.18.1 completed MCP startup through the exact connector, then its Bedrock model path
rejected `browser_tabs` because the tool input schema used `oneOf` at the root. The diagnostic was
explicit: top-level `oneOf`, `allOf`, and `anyOf` are unsupported. This made a standards-valid
schema unusable in an admitted current harness.

Every advertised input schema therefore remains a typo-closed top-level object with properties,
required fields, and examples, but no root-level composition keyword. Conditional field variants
may use composition inside a property. For a conditional multi-field call, the top-level schema is
a safe teaching envelope: it advertises every accepted field and the fields common to every valid
branch. The orchestrator's typed decoder remains the exact authority and rejects irrelevant,
missing, or ambiguous combinations before governance or browser dispatch.

This amends ADR-0107 Decision 4's requirement that every conditional relation live in a
discriminated root-level `oneOf`. Exact runtime acceptance, typo closure, defaults, bounds,
examples, and property guidance remain. A client-name profile or Kiro-specific catalog would add
more machinery and make identical Ghostlight calls depend on harness identity, so none is added.

## Consequences

- The workbench becomes useful before detection succeeds and after automation fails.
- Products with several local installations no longer collapse into a singleton claim.
- Coverage grows by rows and dialects inside one installer rather than by per-harness services or
  plugins.
- Native dialog and clipboard libraries enter the desktop executable, but their generic commands
  remain unavailable to the WebView. The clipboard plugin's Windows implementation introduces
  permissive BSL-1.0 dependencies `clipboard-win` and `error-code`; dependency policy records
  narrow package exceptions instead of allowing that license for unrelated future dependencies.
- YAML support carries a higher proof burden because user-authored comments are product data.
- A harness vendor's own UI remains authoritative for what it can display. Ghostlight does not
  claim it can force artwork into a schema that has no artwork field.

## Acceptance evidence

1. Every product card renders from packaged assets with no network request and has an accessible
   text name.
2. The WebView cannot open an arbitrary URL, choose an arbitrary file outside the bounded Locate
   command, write arbitrary clipboard text, or execute a process.
3. A missing product offers Install, Locate, Copy MCP command, and Copy setup where a target shape
   is known.
4. A detected target offers Set up. A current target offers Remove. A stale owned target offers
   Update. A foreign or malformed target is preserved and offers manual recovery.
5. Located executable and config overrides survive workbench reconstruction. Invalid, mismatched,
   and missing selections are refused or shown stale without changing harness configuration.
6. Every manual fragment names the same exact connector used by automatic setup.
7. Cline's coexisting CLI and editor targets can be configured and removed independently.
8. Repeat Set up and repeat Remove change zero bytes. Unrelated config and foreign Ghostlight
   entries remain untouched.
9. Any accepted YAML target preserves comments, ordering, unrelated values, and file mode through
   setup and removal.
10. Each newly supported product is installed on Linux and starts the exact connector through its
    real MCP lifecycle. The evidence records version, config target, process chain or equivalent
    startup fact, idempotency, and ownership-safe removal.
11. Browser connector, bridge, and extension source diffs remain empty. The MCP connector diff is
    limited to the compatible revision negotiation required by Decision 8.
12. Initialization tests cover every compatible revision and the latest-version counteroffer. A
    current Junie build completes MCP startup and lists Ghostlight's tools.
13. A process transcript starts with `2026-07-28` `server/discover`, receives only the supported
    initialized revision set, initializes `2025-11-25`, and lists the same 22 tools. Antigravity
    1.1.13 completes that fallback through the exact packaged connector.
14. No tool has a root-level `oneOf`, `allOf`, or `anyOf`. Kiro CLI 2.18.1 starts the exact
    connector, submits the complete catalog through its Bedrock model path, and receives a normal
    model response.

## Research

[Research 26](../research/26-linux-harness-roster-and-integration-delight-2026-08.md) records the
official configuration sources, adoption limits, candidate dispositions, visual-asset boundary,
and proposed interaction model accepted here.
