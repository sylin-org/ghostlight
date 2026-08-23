# R4: Rich document reading

## Goal

Restore useful long-form text and bounded hierarchical page-state observation.

## Required work

- Add article-first and visible-text modes to `browser_read`; keep article-first as the default and
  raise only the maximum ceiling to 50,000 characters.
- Fall back to visible document text when article extraction has no useful result.
- Retain current target reads and governed landing checks.
- Add document mode to `browser_inspect` with optional target subtree, bounded depth, and a
  50,000-character result ceiling. Preserve current target-list mode.
- Introduce a generation-bound `snapshot_` handle for semantic structure only and return a bounded
  diff when a current prior snapshot is supplied.
- Supersede prior snapshots for the same tab and invalidate them on commit, ownership loss, and
  workspace release.

## Evidence

- Article, fallback, target, truncation, Unicode-character, and landing-governance tests.
- Tree depth, output bound, subtree ownership, open-shadow-root, hidden-content exclusion, and
  credential-value exclusion tests.
- Snapshot unchanged, added, removed, changed, stale generation, foreign workspace, and eviction
  tests.
- Process journey reads article text, inspects a subtree, mutates the fixture, and reads the diff.

## STOP conditions

- The tree would include editable values, credentials, arbitrary hidden DOM, or unbounded page
  strings.
- Snapshot comparison requires durable storage or extension persistence.
- Existing `target_` generation checks cannot safely scope a subtree.

## Commit

`feat(browser): restore rich document reading`

