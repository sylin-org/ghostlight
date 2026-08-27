# What is in docs/

A map of the folders, so that thirteen directory names are not the first thing you have to decode.
It says what lives where and nothing about current state, because state belongs in exactly one
place and would rot here.

**Start with [MEMORY.md](MEMORY.md)** if you are an agent or a new contributor: it carries the
owner's standing preferences, the durable learnings, and a pointer index by need. **Start with
[STATUS.md](STATUS.md)** if you want to know where the project stands right now. The repository's
onboarding document is [AGENTS.md](../AGENTS.md) at the root.

## Files here

| File | What it is |
| --- | --- |
| [MEMORY.md](MEMORY.md) | Cross-agent project memory: standing preferences, durable learnings, pointer index. Read first. |
| [STATUS.md](STATUS.md) | The mutable snapshot: what is implemented, what is verified, what is owed. |
| [DEV-LOOP.md](DEV-LOOP.md) | Build, run, restart, and validate on a dev machine. Read before touching a live stack. |
| [RELEASE.md](RELEASE.md) | The planned release procedure. |
| [COMPARISON.md](COMPARISON.md) | How Ghostlight compares to other browser-control approaches, as a decision guide. |
| [SPEC.md](SPEC.md) | The original 2026-07-01 design. Historical: ADRs and `1.0/` supersede it where they differ. |

## Folders

| Folder | What it is |
| --- | --- |
| [1.0/](1.0/) | The current implementation contract: intent, model-facing language, architecture, acceptance. These four govern implementation work. |
| [adr/](adr/README.md) | Every design decision, one per file, immutable. A new decision supersedes an old one; it never rewrites it. Read a subsystem's ADR before changing it. |
| [guides/](guides/README.md) | Task-oriented guides for people: installing, configuring governance, collecting audit, the licensing boundary. |
| [trust/](trust/README.md) | The open trust center: security overview, data flows, control mappings, and procurement material. Every claim here is red-teamed against the tree, so change the code before softening a claim. |
| [design/](design/) | Design notes and reviews that are not decisions: visual language, observation design, experience reviews, demo material. |
| [tasks/](tasks/README.md) | Task batches authored for unattended execution, each with its own ledger. All but one predate the 1.0 internals rebuild. |
| [testing/](testing/) | Test plans that a person runs rather than a machine: first-success gates, live lifecycle checks. |
| [research/](research/) | Dated research and landscape work. Evidence of what was true when it was written, not a live claim. |
| [business/](business/) | Business and go-to-market records. |
| [legal/](legal/) | Legal and entity records. |
| [licenses/](licenses/) | License texts (MIT; the root `LICENSE` is the Apache-2.0 text). |
| [assets/](assets/) | Images and artwork used by the documentation. |

## Two rules that keep this readable

- **State lives once.** A document points at the owner of a fact rather than restating it. When you
  find the same fact in three places, two of them are already drifting.
- **History is preserved, not edited.** Superseded material gets a marker saying what replaced it
  and stays where it is. Nothing here is deleted because it stopped being current.
