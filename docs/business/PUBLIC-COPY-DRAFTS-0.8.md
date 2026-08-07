# Ghostlight 0.8 public copy drafts

Status: Internal publication packet. Checked 2026-08-05. No external edit or submission is
authorized by this file.

These drafts adapt the [0.8 message architecture](../design/public-message-0.8.md) to package,
store, registry, directory, and showcase fields. Current release and adapter facts still belong
to [`docs/public-status.json`](../public-status.json). Recheck that file and the destination before
using any draft.

## Shared factual core

Keep these facts intact even when a field requires shorter copy:

- Ghostlight works in the signed-in Chromium profile the person chose.
- Browser work stays visible in a dedicated workspace and the person can interrupt or take over.
- The MCP connector, persistent service, browser connector, and extension run locally.
- Compatible local stdio MCP clients can use the same service. Do not claim every MCP client.
- Personal and all-open use is complete. Capability, domain, and audit boundaries are optional.
- Browser-created child continuity is bounded to an exact, unambiguous opener relationship.
- The extension is required, stays policy-free, and is installed from the Chrome Web Store.
- Ghostlight is open-core. Do not describe the complete distribution as MIT-only or wholly open
  source.

Product evidence for those claims is mapped as C1 through C10 and C13 in the
[copy kit](../design/public-message-0.8.md#claim-to-evidence-matrix).

## Local candidate metadata

These values are committed in the 0.8 source candidate. Their external publication remains part
of the owner-gated release.

### npm package

The npm `description` is 127 characters:

> Installer and launcher for visible local browser automation in your signed-in Chromium profile,
> with optional policy and audit.

This describes the npm artifact's job instead of implying that JavaScript contains the runtime.
The repository and homepage fields route to deeper product context. npm recommends a custom
description because it is used for package discovery; see the
[official package.json documentation](https://docs.npmjs.com/cli/configuring-npm/package-json/#description).

### Official MCP Registry

The `server.json` description is 87 characters, below the registry's 100-character limit:

> Visible local browser automation in signed-in Chromium, with optional policy and audit.

Publish this candidate only with the matching npm 0.8 package and normal registry release step.
The [official registry quickstart](https://modelcontextprotocol.io/registry/quickstart) owns the
publication flow.

### MCPB and Claude directory

Short description, 94 characters:

> Use signed-in Chromium from Claude with visible local execution and optional policy and audit.

Long description:

> Ghostlight lets Claude work in the signed-in Chromium profile you already use. Browser work
> stays visible in a dedicated workspace, and you can interrupt or take over. It can read pages,
> navigate, fill forms, handle files and dialogs, follow supported browser-created child tabs,
> and inspect page, console, and network evidence. The MCP connector, persistent service, browser
> connector, and extension run locally, with no Ghostlight account, telemetry, or hosted control
> plane. Personal use is complete without a policy manifest; optional capability and domain
> policy can add structured local audit for work that needs stronger boundaries. Ghostlight in
> Browser from the Chrome Web Store is required.

Do not submit the MCPB until the licensing eligibility and released-asset gates in
[`DIRECTORY-SUBMISSIONS.md`](DIRECTORY-SUBMISSIONS.md#claude-connectors-directory) are resolved.

## Chrome Web Store draft

Do not edit or resubmit the store listing while adapter 0.8.0 is pending review. A copy change can
reset or extend review. After the pending package is accepted, the owner must choose whether to
publish it first or submit a later listing update.

Item name:

> Ghostlight in Browser

Summary, 115 of 132 characters:

> Let AI agents work in your signed-in Chromium browser while you watch, stay in control, and keep
> the runtime local.

Detailed description:

> Ghostlight in Browser completes the browser side of Ghostlight MCP. It lets compatible AI
> clients work in the signed-in Chromium profile you already use, inside a dedicated visible
> workspace.
>
> With Ghostlight, an agent can:
>
> - read and navigate pages;
> - fill forms and handle files or dialogs;
> - continue into a supported browser-created child tab;
> - inspect page, console, and network evidence; and
> - show visible feedback while you watch, interrupt, or take over.
>
> The MCP connector, persistent service, browser connector, and extension run locally as the
> current user. Ghostlight needs no account, telemetry service, or hosted control plane. Personal
> use is complete without policy. Optional capability and domain policy plus structured audit can
> add boundaries for work that needs them; that policy stays in the service, not the extension.
>
> Install the Ghostlight service on the same machine, add this extension, and run `ghostlight
> doctor` to verify the full chain. Chromium 116 or newer is required. Current platform and
> adapter compatibility: https://sylin.org/ghostlight/install.md
>
> Privacy and data flow: https://sylin.org/ghostlight/privacy/

Chrome defines the summary as plain text with a maximum of 132 characters and recommends a concise
overview followed by the main features in the detailed description; see the
[official listing guidance](https://developer.chrome.com/docs/webstore/best-listing).
[`STORE_LISTING.md`](../legal/STORE_LISTING.md) remains the current submitted source until the
owner approves a later store edit.

## Directory drafts

### Glama

Compact overview:

> Ghostlight MCP lets AI agents work in your signed-in Chromium browser while you watch and stay
> in control. It runs locally, supports compatible MCP clients, recovers across browser changes,
> and can add capability policy and audit.

Full overview:

> Ghostlight MCP connects compatible AI clients to the Chromium profile you already use. Agents
> can read and navigate pages, fill forms, handle files and dialogs, compose multi-step work, and
> inspect page, console, and network evidence in a dedicated visible workspace. Browser-created
> tab continuity and explicit recovery help work survive ordinary browser changes. The runtime
> stays local with no Ghostlight account or telemetry. Personal use is complete without policy;
> optional capability and domain grants add structured audit when the job needs boundaries.

After the website and 0.8 release are public, request or trigger a repository rescan through the
verified owner account. Do not configure Glama hosting or a gateway for this local-only server.
Current listing: [Glama](https://glama.ai/mcp/servers/sylin-org/ghostlight).

### mcpservers.org

Description:

> Ghostlight MCP gives compatible AI clients visible, local browser automation in the signed-in
> Chromium profile you already use. It supports page work, forms, files, browser evidence,
> bounded child-tab continuity, and useful recovery. Personal use is complete; optional policy
> and audit add local boundaries when needed.

Install field or final line:

> Install with `npx -y ghostlight install`, add Ghostlight in Browser from the Chrome Web Store,
> then run `npx -y ghostlight doctor`.

Ask the directory to refresh its copied repository text only after canonical 0.8 surfaces are
public. Current listing: [mcpservers.org](https://mcpservers.org/servers/sylin-org/ghostlight).

### Cline marketplace issue 1989

Replace the stale Additional Information section, or add this as a dated maintainer update after
0.8 is public:

> Ghostlight MCP lets Cline work in the signed-in Chromium profile you already use. Browser work
> stays visible in a dedicated workspace, and you can interrupt or take over. The current surface
> supports page reading and navigation, forms, files and dialogs, browser evidence, multi-step
> work, and bounded browser-created child-tab continuity.
>
> `npx -y ghostlight install --client cline` installs the native runtime and registers Cline.
> Ghostlight in Browser from the Chrome Web Store completes the browser side; `npx -y ghostlight
> doctor` verifies the chain. The MCP connector, persistent service, browser connector, and
> extension run locally. Personal use is complete without policy; optional capability and domain
> policy plus structured local audit add boundaries when needed.
>
> Windows and Linux are verified with live browsers. macOS builds and passes the full suite in CI;
> live-browser verification is still owed. Current release and adapter state:
> https://sylin.org/ghostlight/install.md

Keep the tested-install checkboxes and repository/logo fields unchanged. Do not add a comment or
edit [issue 1989](https://github.com/cline/mcp-marketplace/issues/1989) without owner approval.

### awesome-mcp-servers PR 11306

Replacement Browser Automation row:

> - [sylin-org/ghostlight](https://github.com/sylin-org/ghostlight)
> [![sylin-org/ghostlight MCP server](https://glama.ai/mcp/servers/sylin-org/ghostlight/badges/score.svg)](https://glama.ai/mcp/servers/sylin-org/ghostlight)
> [Rust] [Local] [Windows] [Linux] - Visible local browser automation in the signed-in Chromium
> profile you already use, with useful recovery and optional policy and audit.

When applying this draft, preserve the directory's emoji tokens for Rust, local, Windows, and
Linux in place of the bracketed words above. The source repository requires ASCII, so this packet
does not copy those emoji. Do not update
[PR 11306](https://github.com/punkpeye/awesome-mcp-servers/pull/11306) without owner approval.

### GitHub MCP Registry

Description:

> Visible local browser automation in signed-in Chromium, with optional policy and audit.

Use the canonical npm-backed `server.json` record. Owner notes say the one-time publisher approval
completed, but no public Ghostlight catalog entry was independently located on 2026-08-05. Locate
the public item and its update path before claiming discoverability or sending an edit.

### mcp.so and PulseMCP

Use the Glama compact overview, repository URL, website URL, npm install command, and local stdio
transport. These are new submissions, not refreshes. Recheck each form and its terms immediately
before an owner-approved submission; neither surface was submitted as of 2026-08-05.

## Showcase update drafts

The existing Codex and Zed posts are project-authored distribution evidence, not testimonials.
Post these only after 0.8 is public, and label the update with the actual release date.

### Codex Show and tell

> Update for Ghostlight 0.8: the browser workflow is still local and visible, but the runtime now
> has explicit MCP connector, persistent service, and browser connector roles. It also keeps exact
> workspace identity, follows an unambiguous browser-created child tab, and gives the agent useful
> recovery guidance when page, tab, or transport state changes. Install for Codex with `npx -y
> ghostlight install --client codex`, add the store extension, restart Codex, and try the read-only
> first task at https://sylin.org/ghostlight/.

Current post: [Codex discussion 36424](https://github.com/openai/codex/discussions/36424).

### Zed Show and tell

Replace the old topology block with:

```text
Zed -> agent harness -> ghostlight-mcp-connector -> ghostlight service
    -> ghostlight-browser-connector -> extension -> Chromium
```

Then add:

> Update for Ghostlight 0.8: the MCP edge now has exact local stdio state machines for revisions
> `2025-11-25` and `2026-07-28`, while the persistent service stays protocol-neutral. Exact
> workspace identity, bounded browser-created child-tab continuity, and concrete recovery guidance
> make longer browser work easier to resume. Install for Zed with `npx -y ghostlight install
> --client zed`, add the store extension, restart Zed, and try the read-only first task at
> https://sylin.org/ghostlight/.

Current post: [Zed discussion 62035](https://github.com/zed-industries/zed/discussions/62035).

## Publication gates

No draft above should be sent merely because it is ready. Before each external action:

1. Confirm 0.8 is public where the draft says it is public.
2. Re-read `docs/public-status.json` for service, adapter, platform, and review state.
3. Recheck the destination field, terms, and existing content.
4. Obtain owner approval for that exact edit, comment, submission, or publication.
5. Record the resulting URL, timestamp, and factual drift in the E6 reconciliation log.

The non-publishing website branch prepared in E5 is source work only. Pushing it does not authorize
a merge into the website deployment branch.
