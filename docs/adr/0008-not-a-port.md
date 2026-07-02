# 0008. Not a port: harvest intent and techniques, not code

- Status: Accepted
- Date: 2026-07

## Context

Two bodies of prior art exist for extension-based browser automation: the
community reference (open-claude-in-chrome, a Node.js reimplementation) and the
official Claude-in-Chrome extension (v1.0.78). Both are instructive, but neither
is a codebase to fork. The community reference is a lossy proxy that ships its own
bugs (docs/research/12). The official extension is Anthropic proprietary, and
this repository is intended open-source, so its code cannot enter our tree. Prior
art is a concern-surface (the hazards and questions others hit), not a feature
catalog to copy (README; SPEC 1.3, 1.4).

## Decision

We study prior art for two things only: the observable interface (tool
names, parameters, enums, description strings) and battle-tested techniques (CDP
command sequences, algorithms), and we reimplement both leanly in Rust and a thin
extension. We never copy proprietary code into the repo; the beautified official
files are studied in a throwaway scratchpad and are never tracked. The only thing
preserved verbatim is the external tool-schema contract (ADR-0007). Everything
behind that contract is our own, kept to fewer and more meaningful moving parts
than the reference's five-process pipeline.

## Consequences

- Lean internals we fully own and can reason about, instead of inheriting a
  larger codebase's structure and defects.
- We fix inherited bugs rather than porting them: commit 0deef1c corrected six
  extension bugs the reference either shipped or dropped (e.g. populating the DOM
  `code` on key events where "the reference dropped this; we do better"; a
  truncation message `find` had promised but not emitted).
- Legal and licensing cleanliness: no Anthropic-proprietary code in an
  open-source repo.
- Cost: reimplementation is more work than copy-paste, and techniques must be
  re-derived from minified/beautified sources that we deliberately keep
  ephemeral, so the harvest is reproducible only via the documented re-extract
  recipe (docs/research/12), not from anything committed.
