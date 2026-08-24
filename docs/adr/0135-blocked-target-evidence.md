# ADR-0135: Blocked-target evidence on integration cards

- Status: Accepted
- Date: 2026-08-24
- Builds on: ADR-0125 Decision 2 (card roster and its evidence requirement), ADR-0129
  Decision 4 (the gap that outlived every layout attempt)
- Superseded provenance: ADR-0130 Decision 4 specified the first evidence shape and was
  superseded in full by ADR-0129 before it governed any shipped surface. This ADR re-decides
  the substance independently; ADR-0130 remains history and does not govern.

## Context

A blocked integration target asserts a conclusion with no evidence. Today one conflated
sentence -- "The configuration is malformed or has a foreign ghostlight entry; it was left
untouched." -- covers two different causes, and the projection discards everything it saw:
the parse-failure reason dies in the error arm of `inspect`, and a foreign entry's existing
command dies in the `Foreign` match arm. A person is told something is wrong under
Ghostlight's own key and gets nothing to look at, nothing to compare, and no way to judge
whether to intervene.

ADR-0125 Decision 2 and the reference-experience epic require showing what Ghostlight found,
what it owns, and what it would change. The evidence was built once alongside the rejected
layout attempts and reverted with them; ADR-0129 records that the gap, not the shape, is the
substance.

## Decision

### 1. Evidence rides the existing card

ADR-0129 returns the destination to the compact status-sorted card roster, and this ADR does
not reopen layout. A blocked card gains one optional evidence paragraph below its detail
sentence. Nothing about ordering, categories, or actions changes.

### 2. The orchestrator authors the words

The projection carries one optional precomposed `evidence` string on `HarnessSummary`,
authored in Rust like every other fixed detail sentence. The WebView renders it verbatim and
adds no vocabulary of its own. It is absent (`skip_serializing_if`) for every state that is
not blocked.

### 3. Two causes, two sentences, both bounded

Malformed and foreign stop sharing a sentence:

- Foreign entry: names the found command as bounded, whitespace-normalized text capped at 200
  characters, states that Ghostlight maintains its own connector command there, and states
  that nothing was changed.
- Malformed configuration: names the bounded parse-failure reason and states that nothing was
  changed.

Only the entry under Ghostlight's own key travels -- never the rest of the document. Control
characters and bidi overrides are stripped from anything disclosed. Ownership rules are
untouched: a foreign entry is still never overwritten or removed by any automatic path,
`can_install` stays false while blocked, and the offered actions are unchanged.

### 4. Additive wire shape

`HarnessSummary` gains one optional serialized field. Older surfaces ignore it; the preview
fixture and surface journey learn it together with the renderer.

## Consequences

Blocked cards become judgeable: a person can see the intruding command or the parse failure
beside the claim. The conflation defect -- one sentence for two causes -- is closed at the
seam that had the information and dropped it.

Tests pin the composition: foreign entries disclose their found command bounded and
normalized across JSON, TOML, and YAML dialects; malformed configurations disclose a bounded
reason; unblocked states never carry evidence.
