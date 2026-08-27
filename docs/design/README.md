# Design notes

Notes that are not decisions. A decision lives in an [ADR](../adr/README.md); the current contract
lives in [`1.0/`](../1.0/). These are the thinking around them: how something should look, why a cut
was chosen, what a review found on a particular day.

The folder mixes three different kinds of document, and filenames alone do not tell them apart.

## Living references

Maintained, and still describing the product as built.

| Note | What it covers |
| --- | --- |
| [visual-language.md](visual-language.md) | The shared visual vocabulary across the in-page renderer and the desktop workbench. |
| [visual-feedback.md](visual-feedback.md) | The feedback vocabulary: what each treatment means. Its rendered companion is [visual-feedback-dictionary.html](visual-feedback-dictionary.html). |
| [tool-visual-signatures.md](tool-visual-signatures.md) | Which visual signature each tool carries. |
| [action-observations.md](action-observations.md) | Where per-action facts belong, and the two structural owners that keep the account complete. |

## Dated reviews and plans

Snapshots of what was true and what was recommended on a date. They are evidence, not standing
commitments, and they were not rewritten as the product moved.

| Note | Date |
| --- | --- |
| [mcp-spec-currency-2026-07.md](mcp-spec-currency-2026-07.md) | MCP spec currency against the tree |
| [developer-first-entry-2026-07.md](developer-first-entry-2026-07.md) | Repository and installation entry review |
| [non-author-experience-review-2026-07.md](non-author-experience-review-2026-07.md) | Retrospective non-author experience review |
| [public-awareness-plan-2026-07.md](public-awareness-plan-2026-07.md) | Public awareness plan |
| [public-documentation-review-2026-07.md](public-documentation-review-2026-07.md) | Public documentation review |
| [visual-language-next-2026-07.md](visual-language-next-2026-07.md) | Visual-language refinement proposals |
| [verification-topology-evaluation.md](verification-topology-evaluation.md) | Fewer, more meaningful verification moving parts |

## Proposals, demos, and prior architecture

Written before or beside the 1.0 rebuild. Read them for intent, not for current shape; where they
describe topology or features, [`1.0/ARCHITECTURE.md`](../1.0/ARCHITECTURE.md) and
[`STATUS.md`](../STATUS.md) win.

| Note | What it is |
| --- | --- |
| [ghostlight-service-architecture.md](ghostlight-service-architecture.md) | Earlier service topology and family baseline. |
| [managed-mode-network-features.md](managed-mode-network-features.md) | Managed-mode network and identity features. The 1.0 runtime deliberately does not implement these. |
| [agent-journey-artifact-v0.md](agent-journey-artifact-v0.md) | A proposed journey-evaluation artifact. |
| [bounded-delegation-scenario.md](bounded-delegation-scenario.md) | Three delegation scenarios and a prototype script. |
| [demo-brief.md](demo-brief.md) | The launch-brief demo. |
| [tcg-foundry-demo.md](tcg-foundry-demo.md) | The Sylin Card Foundry demo. |
