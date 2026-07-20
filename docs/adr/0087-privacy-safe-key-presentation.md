# ADR-0087: Privacy-safe key presentation

Status: Accepted

Date: 2026-07-20

## Context

ADR-0083 removed literal values from `computer.type` feedback but deliberately left printable
`computer.key` review as a follow-up. The key action accepts a space-separated sequence of chords.
Its original visual message copied the complete `text` argument into the page renderer, which then
displayed every token. A model can use `computer.key` for a standalone printable key or a sequence
of printable keys, so this path could disclose text even when the page masks it.

At the same time, named keys and real shortcuts are useful user-facing information. `Enter`,
`ArrowDown`, and `Ctrl+A` communicate recognizable actions. Replacing every key action with a
generic keyboard badge would lose that clarity.

## Decision

### D1. Execution syntax and presentation syntax are separate

The existing key parser continues to receive the complete tool argument and drive Chrome. Before
the presentation event is created, the worker derives a separate bounded key-cue structure. The
original string never enters the page presentation message.

The derived structure contains at most six chord groups, at most five labels per group, a bounded
structural target class, and a boolean that says whether later groups were omitted. It contains no
target content, field value, element reference, event key, or raw fallback token.

### D2. Printable identity follows the actual trusted event target

Immediately before CDP key dispatch, the content mechanism arms a capturing `keydown` observer.
It records only one of three structural classes for each trusted event:

- `ordinary` when the actual event target is observable and is not a natively protected field;
- `protected` for password inputs and native input or textarea fields carrying the platform's
  sensitive autocomplete tokens for credentials, one-time codes, or payment data;
- `unknown` when the event target cannot be observed reliably.

The observer ignores synthetic page events. It is bounded, removed after dispatch, and never
records the key, target element, field value, or text. Classification uses the event target after
focus resolution, not a preflight `document.activeElement` guess.

The worker then applies these presentation rules:

- named control and navigation keys remain visible, including `Enter`, `Escape`, arrows, paging,
  deletion, and function keys F1 through F24;
- Ctrl, Alt, Meta, Cmd, and Win mark a command shortcut, so one printable shortcut key may remain
  visible, such as `Ctrl+A`, `Cmd+Shift+P`, or `Alt+/`;
- a printable key on an ordinary target remains visible, so `A` and `Shift+B` read literally;
- a printable key, including Space, on a protected or unknown target becomes an unlabeled glowing
  keycap; modifiers may remain beside it;
- an unknown or multi-character key token becomes the same unlabeled keycap.

If repeated execution observes different target classes for one chord, protected wins, then
unknown. A missing, extra, or overflowed event makes printable identity unknown for the sequence.
This is mechanism classification for disclosure minimization, not a governance decision. It does
not inspect page content or infer intent.

### D3. The renderer revalidates and renders only text nodes

The key-domain module is loaded before the renderer on both declared and on-demand activation
paths. The renderer rejects any cue outside the bounded allowlist. It creates chord, separator,
and label elements with `textContent`; key messages cannot inject markup.

Multiple space-separated chords remain distinct groups with a small `then` separator. An unlabeled
keycap is a styled empty element, not the word `Key`. The old renderer incorrectly displayed every
token as if it belonged to one simultaneous chord.

### D4. The tool contract and execution result do not change

The trained `computer` schema, accepted key syntax, Chrome input dispatch, and model-facing tool
result remain unchanged. Only optional user-facing presentation is minimized.

The lozenge keeps its existing lifetime, capture hiding, effects preference, reduced-motion
behavior, pointer transparency, and `ghostlight-` ownership.

## Consequences

- Printable keys remain delightful and explicit on ordinary page targets.
- Printable keys cannot be reconstructed from the presentation lane when the actual target is a
  native protected field or cannot be observed reliably.
- Useful named keys and recognizable command shortcuts remain inspectable.
- The content observer carries only bounded structural classes; the presentation event carries
  structured, bounded labels rather than arbitrary input text.
- The shared key module is loaded into both content and renderer isolated-world paths. It remains
  mechanism only and makes no governance decision.

## Rejected alternatives

- Keep sending raw text and mask only in the renderer. Rejected because sensitive text would still
  cross into a page-local presentation boundary.
- Hide every printable key identity. Rejected because ordinary keys are useful user-facing action
  feedback when the actual target is not protected.
- Inspect the focused field before dispatch. Rejected because focus can change before the key event
  lands, creating a time-of-check/time-of-use disclosure race.
- Record event keys or target values in the content observer. Rejected because the observer needs
  only a structural target class.
- Treat custom editor semantics as protected by inference. Rejected because the extension does not
  inspect content or guess application intent; unobservable targets fall back to unknown.

## Related decisions

- ADR-0005: policy-free extension.
- ADR-0012: UI and input fidelity.
- ADR-0081: document-aware Presentation Broker.
- ADR-0083: unified action signatures and the deferred printable-key review.
