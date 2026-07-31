# Developer-first repository and installation entry, 2026-07

Status: Implemented in the repository README and installation guide on 2026-07-14. Publication
outside this repository remains separately gated.

## Goal

Let a developer understand Ghostlight, install both local halves, and ask a first useful browser
question without first parsing organizational positioning. Preserve exact licensing and governance
facts, but place them where they answer the reader's next question.

This refines the public content contract in
[the July documentation review](public-documentation-review-2026-07.md). It does not authorize a
license change or publication outside this repository.

The non-author reviewer named https://github.com/anomalyco/opencode as a positive reference. Its
current repository order puts a short product promise and visual first, installation immediately
afterward, then working modes, documentation, and contribution. Ghostlight adopts the fast
orientation and install visibility, not the exact one-command shape: the extension remains an
honest second step and the local trust explanation remains near the entrance.

## Reader order

The README should follow this sequence:

1. The practitioner problem and the one-sentence product answer.
2. The visible local experience and a short proof artifact.
3. The shortest supported installation and first task.
4. Capabilities, supported clients, and current platform truth.
5. The local architecture, no-account explanation, and core license boundary.
6. Build, test, and contribution paths.
7. Organizational governance, Trust Center, and comparison detail.
8. Exact licensing, continuity, support, and procurement links.

The first screen should not use `open core` as shorthand. That phrase makes the reader infer a
future paywall before the document has stated what is actually licensed. State the concrete
boundary instead.

## Candidate opening

> Ghostlight gives the MCP client you already use a visible, local browser in the profile where
> you are already signed in.
>
> Ask it to work in your real Chromium session. You see what it reads and changes, keep control of
> the browser, and get compact results designed for an LLM instead of a stream of screenshots.

Then name fit directly:

- Use Ghostlight when Codex, Claude Code, Cursor, VS Code, or another local MCP client needs the
  browser session you already use.
- Do not use it when you want a headless, isolated, cloud-hosted, or unattended browser fleet.

An early factual reassurance can follow the first install proof:

> The local browser automation core is Apache-2.0 OR MIT. It runs without a Ghostlight account,
> activation, telemetry, or subscription. Organizational governance is a separately licensed
> layer; its exact terms are linked below.

Before publication, compare this wording line by line with ADR-0027, ADR-0028, the root license
files, and the current governance license. Do not promise that every feature is free, use
`permanently free`, or obscure the separately licensed layer.

## The install journey

Show one four-stage strip near the first command:

```text
[1 Install service] -> [2 Add extension] -> [3 Restart MCP client] -> [4 Ask a first task]
       automatic          visible step             once                 useful proof
```

`ghostlight doctor` is the recovery path, not a mandatory fifth stage. The installer may register
supported MCP clients automatically, but the browser extension remains an intentional user-visible
step and must never be represented as automatic.

The mascot can walk across the four stages as a guide. Completion states should use the shared
Ghostlight mark, color tokens, and vocabulary rather than separate illustrations that look like
different products.

## Browser extension path

The Chrome Web Store listing is public. Main product and repository entrances should link directly
to it and should not present a release archive as an end-user installation choice. Source builders
may load the repository's `extension/` directory as a development extension so they can test their
local build immediately. Keep that workflow inside source-development documentation.

## No-account explanation

Place this beside the architecture sketch, before organizational governance:

> Ghostlight has no hosted account to sign in to. The service and extension connect locally as the
> current OS user, and Ghostlight admits only the configured local relays and pinned extension
> origins. Website sessions stay in your existing browser profile. Connect only MCP clients you
> trust: a local agent can still ask the browser to perform powerful actions.

Follow with the honest threat boundary. Ghostlight governance can constrain an admitted MCP
client; it cannot recover a browser, extension, service, or OS account that is already compromised.
An extra Ghostlight cloud login would not repair that local compromise.

## Cross-surface continuity

Each owned surface should answer three questions without scrolling:

- `Where am I?` Ghostlight name, mark, and consistent header.
- `What is ready?` Local service, browser extension, or MCP client state.
- `What happens next?` One primary action with the destination named.

Recommended transitions:

| From | Primary copy | Destination cue |
| --- | --- | --- |
| Product page | `Install Ghostlight` | Local service installer |
| Service post-install | `Add the browser extension` | `Open Chrome Web Store` |
| Extension post-install | `Connect your MCP client` | Name detected clients and restart requirement |
| Doctor output | `Finish the browser step` | Print and optionally open the same stable guide |
| Ready state | `Try your first browser task` | Copyable client prompt |

The Chrome Web Store cannot share Ghostlight styling. Use a small screenshot or annotated
illustration before the transition so the user recognizes the listing and the Ghostlight mascot.

## First useful task

The final install stage should offer one copyable, read-only prompt that proves the connected real
session without asking the user to understand tools:

> In my current browser, summarize the active page and tell me which tab you used. Do not click or
> change anything.

After that succeeds, show one optional action task. The first proof should remain read-only so the
user can separate connection success from governance and write authority.

## Organizations and governance later in the README

The latter half should explain that Ghostlight also serves organizations that want RAWX-style
capability governance over local browser use. Lead with the operational question, not license
jargon:

> Need to control which MCP identities may read, act, write, or execute on which domains?

Then link to the governance guide, comparison, Trust Center, pricing, continuity promise, and exact
license. This gives an organizational evaluator depth without making a practitioner pass through a
procurement entrance.

The product site and Trust Center remain the natural homes for buyer proof, deployment controls,
legal terms, SBOM and release evidence, and security claims.

## Project support

If the owner chooses to accept financial support, place a small `Support Ghostlight` section near
the contributor material, not in the hero. Prefer GitHub Sponsors as the repository-native path;
Ko-fi may be a secondary option.

Before enabling `.github/FUNDING.yml` or publishing buttons, decide:

- the receiving person or entity;
- accounting and tax handling;
- whether recurring and one-time support are both wanted; and
- the public explanation of how funds support maintenance.

Support must not change feature access, governance rights, release priority, or security response.
Use `support` or `sponsorship`; do not imply tax-deductible charitable donation status.

## Acceptance checks

The repository content implementation is not considered done until:

1. a new visitor can find the current extension artifact from the main entrance in one click;
2. the complete four-stage install path is visible before opening the detailed guide;
3. a reviewer recognizes every owned step as Ghostlight and understands each third-party handoff;
4. `no account` is understood as local identity and ownership, not no authentication;
5. the core and governance license boundary is accurate and does not feel like a hidden conversion;
6. the first prompt succeeds through a supported MCP client on a clean machine; and
7. mobile and narrow layouts preserve the stage order and primary action.

The new Linux host should first run the current baseline journey. Record where it diverges before
testing revised copy, so installation defects are not mistaken for documentation defects.
