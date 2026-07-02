# 0011. Truthful engine + secret redaction as a governance-config key

- Status: Accepted
- Date: 2026-07

## Context
`read_page` renders the accessibility tree, which includes form field values. Emitting a
field's raw value unconditionally leaks passwords, one-time codes, and card numbers into
the tree that travels to the (cloud) MCP client. The fix must not turn the engine into a
content inspector: SPEC sec 9.5 says the binary governs structurally (which domains,
which tools), not semantically (what data is on the page). The design stance is layered:
the engine is always truthful, and governed behavior is an overlay keyed on typed
configuration with a safe-by-default "Minimal" preset.

## Decision
The engine stays truthful. In the extension (content.js) `read_page` emits real
input/textarea values and truthful `<select>` options; a field the page itself marks
secret (input `type=password`/`hidden`, or a sensitive `autocomplete` token) is emitted
with a neutral `secret_value="..."` marker: a fact, not a decision. No policy lives in
the extension.

Redaction is a governance overlay in the binary. `src/policy/redact.rs` rewrites the
marker before the result leaves the binary: the marker is ALWAYS stripped (the model
never sees it), and when `content.security.secrets.redact` is enabled the value becomes
`value="[value redacted]"`, otherwise the raw value is preserved. The key is registered
in `src/policy/mod.rs` (the single home for governance keys) and defaults to `true`
under the "Minimal" preset. The overlay keys only on the page's own structural secret
markers, so it does not inspect content semantically, staying within SPEC sec 9.5.
`javascript_tool` remains the intentional unconstrained escape hatch. (Commit 310ae44.)

## Consequences
Positive: the raw value travels extension->binary (local, trusted) and is stripped
before the MCP client (cloud). Toggling the key needs no engine change: off yields the
raw truth, on yields safe-by-default output. Keeping every governed behavior as a typed
key in one registry makes the policy surface introspectable for future config UIs.

Negative / follow-ups: with `--debug` the raw pre-redaction result is still written to
the local per-PID debug file; redacting there is a deferred item. The classification is
only as good as the page's own field markup, and `javascript_tool` can still read raw
values by design.
