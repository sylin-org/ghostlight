# ADR-0122: A policy destination the governed person can read, and a user layer they can author

- Status: Accepted
- Date: 2026-08-14
- Amends: ADR-0102's amended destination list (A3), ADR-0121 Decision 2 (an additive manifest block)
  and its registered setting list
- Builds on: ADR-0013, ADR-0060, ADR-0079, ADR-0102, ADR-0103, ADR-0119, ADR-0121
- Research input: [24-policy-surface-user-delight-2026-08.md](../research/24-policy-surface-user-delight-2026-08.md)

## Context

Ghostlight's 1.0 governance engine is complete and its workbench shows almost none of it. The window
reports that a policy exists and is healthy through four booleans per source, and nothing else. A
person governed by an organization policy cannot see one rule, cannot learn which rule refused their
agent, and cannot connect a refusal to the contact details the passport already displays. A person
with no organization policy cannot create one from the window at all: the only path is hand-written
schema-3 JSON, an environment variable set before startup, a restart, and a terminal.

That is backwards for a product whose whole claim is that browser work stays visible and under the
user's eye. Governance the user cannot read is indistinguishable, from where they sit, from a
malfunction.

The engine is not the problem. Layers already compose by intersection with deny-overrides, so a user
layer can only subtract; grant attribution and deterministic `D-` denial ids are already recorded on
every enforced denial; the decision engine already runs offline against recorded audit; catalog
projection already computes the exact surviving tool set under current authority. The missing work
is almost entirely presentation of facts the orchestrator already holds, plus one narrow, guarded
write path.

Prior art is settled and consistent (see the research document). Every mature system that presents
layered policy to the person it governs does the same five things: answers with the effective result
first, names the deciding layer on every line, keeps the underlying file visible, previews
consequences before committing, and stays silent about defaults. The one widely used counter-example,
VS Code's settings scopes, is chiefly known for the failure this ADR must avoid: inherited values
that are invisible in the scope being edited.

## Decision

### 1. Policy is a destination, and the state chip is its entrance

The window gains a fourth destination, `Policy`, between `Status` and `About` in the tab order. The
lamp band's policy state chip moves from the left of the tabs to sit immediately after `Status` and
before `About`, and navigates to that destination.

This supersedes the three-destination list in ADR-0102's 2026-08-11 amendment (A3). That amendment's
reasoning was that home, sessions, and history were one dataset at three ages and did not deserve
three pages. Policy is not that: it is a distinct dataset, with a distinct question behind it, that
currently has no page at all. The rest of A3 stands.

Status keeps diagnostics, the runtime session control, and notifications. The three authority-source
cards and the managed passport leave Status for the new destination; Status keeps one line stating
whether configured authority is valid, which is a health fact rather than a policy fact.

### 2. The compiled policy is orchestrator-owned, and it is the headline

The orchestrator gains one typed projection, the effective-authority view, assembled from the same
immutable snapshot that authorizes work. The surface renders it and computes nothing.

This is a correction as well as an addition. Today the window derives its policy words in JavaScript
from four booleans ([view.js:266-300](../../crates/orchestrator/ui/lib/view.js#L266-L300)), which
puts product language in a disposable surface and contradicts the invariant that the orchestrator
owns model-facing and user-facing language (ADR-0103). One projection, rendered by whoever asks, is
the shape every other Ghostlight fact already uses.

The view carries, in this order:

1. **One sentence** naming the current situation and, when an organization layer is active, naming
   the organization. Fixed cases: nothing applied, organization only, user only, both, and refusing
   all governed work because a configured source is invalid.
2. **The capability answer**, one line per independent capability, each stating whether it is
   available, unavailable, or available on specific sites, and naming the layer that decided it.
3. **The rules behind each line**, on demand: the host sets that produced the answer, each tagged
   with its deciding layer and, where authored, the rule's own description.
4. **The permanent ceilings**, always present and never editable: non-HTTP(S) schemes, localhost and
   its subdomains, loopback, link-local, and any sacred destinations in force.
5. **Provenance** for an organization layer: the passport as it exists today.

Every line that comes from a layer names that layer. The layer vocabulary is closed and reads in
plain words: the organization's name when one is known, "you", and "Ghostlight". The full precedence
chain for one line is reachable from that line, and is never the default view.

Silence for defaults. A capability nothing restricts says so in one line and expands to nothing.

### 3. Organization identity belongs in the policy document

Schema 3 gains one optional additive `organization` block: a display name, an optional statement in
the organization's own words, an optional URL, and optional typed contacts reusing the existing
contact shape (kind, value, optional label). The block is informational. It grants nothing, denies
nothing, and never participates in a decision.

When a signed managed bundle carries the existing presentation block, that presentation wins on
conflict. Both are covered by the signature envelope; the presentation block is the outer, explicitly
published statement, and bundles already in the field must keep behaving exactly as they do.

The block is optional, so every manifest valid today stays valid. The reverse is not true: manifests
are typo-closed with `deny_unknown_fields`, so a policy authored with an organization block is
rejected by a Ghostlight older than this change. That is the honest cost of a strict schema and it is
accepted rather than worked around. It is not a schema-4 event: the version number gates decision
semantics, and this block has none.

An organization layer still reaches a machine only through the administrator-provisioned signed
bootstrap, from a local file or an HTTPS source. This ADR does not introduce an unsigned
organization file.

### 4. The user layer is one layer, and the workbench may author it

There is exactly one user layer. Its document comes from one of two places, in this order:

1. `GHOSTLIGHT_POLICY_FILE`, when set. Ghostlight does not own that path and the workbench never
   writes to it. The destination shows the policy read-only and states plainly that an environment
   variable points Ghostlight at a file it does not manage.
2. Otherwise, one product-owned path in the existing per-user state root, beside the managed cache
   and status sidecar. This is the file the workbench writes.

No third layer is introduced. A user policy authored in the window occupies the same slot the
environment variable would, so the compiled view never has to explain two user layers.

Two bounded Tauri commands are added to the existing allowlist, and nothing else: one that applies a
complete typed user policy, one that removes it. Both are ordinary application-boundary mutations
under ADR-0102 Decisions 5 and 6, with no free path, no arbitrary write, and no shell.

Applying a policy validates with the production parser before anything is replaced, then writes
atomically. A rejected document changes nothing and returns why. **A GUI action must never be able to
fail the user closed**: the fail-closed rule for a configured source with no valid policy remains
exactly as it is for files Ghostlight does not own, but the window cannot produce that state through
its own write path.

Removing the user policy removes the file. Authority returns to the organization layer, or to
all-open when there is none. That is one click and no typed confirmation.

### 5. An organization may switch the user layer off, and that is an operational control

`policy.user.enabled` joins the registered settings. It is a boolean, it defaults to enabled, and
only an organization layer may author it.

It is not a security boundary and must never be described as one. A user layer can only subtract
authority, so forbidding it protects nothing. Its legitimate purpose is operational: an organization
that wants a predictable, supportable configuration, or that does not want a user narrowing their own
agent into a support ticket. Recording that distinction here prevents it from being mistaken for a
hardening feature later.

When it is off, the destination stays fully readable, the editor is absent rather than disabled, and
the reason is stated in the organization's own words using the statement and contacts from Decision 3.
An organization that switches this off without supplying a statement gets a plain fallback sentence.

### 6. The editor speaks sentences, not schema

The unit a person manipulates is one site rule, rendered as a complete sentence with editable parts:
on these sites, agents may do these things. The word "grant" never appears.

The design rules are normative, not decorative:

- **No magic words.** The primary surface never requires a person to read or type "grant",
  "manifest", "schema", "RAWX", or "capability set". Capabilities are named by what they do.
- **Readback on every pattern.** A typed host pattern immediately states what it matches, in plain
  words, before it is saved. Ghostlight's host grammar is deliberately its own and differs from both
  Chrome's and the nearest comparable product's; the interface removes that ambiguity by stating the
  result rather than by teaching the grammar.
- **Ceilings inline.** A capability an organization does not allow is shown unavailable on the
  control itself, with the organization named there, not in a banner at the top of the page. This is
  the specific failure VS Code's settings editor is known for and it is the one thing this design
  must not repeat.
- **Redundancy is visible.** A user rule that grants nothing beyond what the organization already
  denies is marked in place as having no effect.
- **Shadowing is visible.** Grant resolution is first-match-wins and that remains decided
  (ADR-0121 Decision 2). A rule that can never fire because an earlier rule covers it says so, in
  place, with a direct way to reorder. An ordered system presented as unordered turns a documented
  footgun into a silent one.
- **Two modes, in plain words.** The choice between observing and enforcing is a two-state switch
  described by what happens, not by the words `observe` and `enforce`.
- **The file is never hidden.** The destination always shows the document's path and its exact JSON,
  copyable, for both the user layer and any organization layer in force.

A new user policy is offered as a draft seeded from the hosts already present in this machine's own
audit history, ordered by frequency. It is presented as a starting point to trim, never as a finished
policy. Nothing is written until the person applies it.

### 7. Applying a policy shows its consequences first

Before a user policy is applied, the destination states what it would have changed, by replaying the
candidate through the production decision engine against the bounded audit history already held by
the workbench, exactly as `ghostlight policy simulate` does today. The result is stated as a count
and a short list of what would have been refused, or as a plain statement that nothing recently done
would have been refused.

This is audit-free and read-only. It creates no records and changes no authority.

### 8. A refusal leads to its reason

A refused action in the monitor leads to the rule that refused it: the deciding layer, the rule and
its description where authored, and the deterministic denial id already recorded. When the deciding
layer is an organization and that organization supplied contacts, those contacts appear there, at the
moment the person actually needs them, rather than only as a card on a page they had no reason to
open.

### 9. What this ADR does not do

- It does not change grant resolution, host specificity, layer intersection, mode composition,
  sacred-destination handling, denial identity, or the attention circuit. The engine's semantics are
  unchanged.
- It does not put policy logic, classification, or audit in the extension. The extension is untouched.
- It does not add an administrator console. Key generation, signing, publication, bundle inspection,
  and simulation against arbitrary audit files remain CLI surfaces that administrators script.
- It does not add a model-facing rendering of the effective-authority view. ADR-0121 Decision 3
  described an always-available policy explain operation; in the current tree that exists only as a
  CLI command over a file path, and the 22-tool catalog has no policy tool. The projection this ADR
  introduces is the natural source for one, and a later ADR should decide it. Until then that gap is
  recorded here rather than implied to be closed.
- It does not add a process, a listener, a credential, a wire protocol, or a network dependency.
  Ghostlight still never phones home.

## Consequences

The window gains its first authoring surface. That is a real widening of what the WebView can cause,
and it is bounded the same way harness installation already is: two allowlisted commands, typed
arguments validated at the boundary, one product-owned path, no free strings.

The compiled view creates a second consumer for facts that were previously private to the decision
path. Those facts stay orchestrator-owned; what changes is that a person can see them. Nothing in the
view exposes managed source addresses, bearer tokens, CA material, or signing keys, which remain out
of every projection.

Showing organization rules in full is a deliberate reversal of the content-free stance the passport
takes today. It is defensible because the verified bundle already sits on the user's own disk: hiding
the rules protects nothing and costs the user the ability to understand a refusal. Organizations that
consider their rule set sensitive should know that this was always true of a locally verified policy.

The additive `organization` block splits the manifest population by version: documents that use it
are rejected by older Ghostlight builds. Organizations publishing to a mixed fleet must either wait
or keep the block out until their fleet has moved.

The audit-history dry run is only as good as the history. It says what would have happened to
recorded work on this machine, and it must be worded so that nobody reads it as a guarantee about
work that has not happened yet.

Removing the authority cards from Status changes a page users may already know. The chip that used to
sit left of the tabs still leads to the same information, one destination further along.

## Acceptance evidence

1. The tab order is Monitor, Integrations, Status, Policy, About, and the state chip renders between
   Status and About and navigates to Policy.
2. The surface computes no policy language: every word describing policy state arrives in the
   projection, and a guard fails the build if the policy vocabulary reappears as literals in the view.
3. The effective-authority view names a deciding layer on every capability line and every rule line,
   and the permanent ceilings are present in every state including all-open.
4. All-open renders as a complete, non-nagging answer with no empty panels and no call to action.
5. A manifest carrying an `organization` block parses, its identity renders, and the same manifest
   without the block still parses byte-identically to today.
6. A signed bundle carrying the existing presentation block renders that presentation, unchanged,
   when the manifest also carries an `organization` block.
7. With `GHOSTLIGHT_POLICY_FILE` set, the destination is read-only, states why, and the apply and
   remove commands refuse.
8. Applying an invalid user policy changes no file, changes no authority, and returns the parser's
   own reason.
9. Applying a valid user policy replaces the file atomically and takes effect on the next invocation
   with no restart.
10. Removing the user policy returns authority to the organization layer, or to all-open, in one
    action.
11. No sequence of workbench actions can leave the product failing closed.
12. With `policy.user.enabled` false, no editor is reachable, the whole view stays readable, and the
    organization's statement is what explains it.
13. A user rule fully covered by an organization denial is marked as having no effect; a user rule
    shadowed by an earlier user rule is marked as unreachable.
14. Every host pattern accepted by the editor produces a readback that matches what the production
    matcher does with it.
15. The dry run reports the same refusals for a candidate policy and a recorded history that
    `ghostlight policy simulate` reports for the same two inputs.
16. A refused monitor row reaches its deciding rule, its denial id, and, when an organization decided
    it and supplied them, that organization's contacts.
17. The destination shows the exact document and path for every layer in force.
18. Diffs for `crates/mcp-connector`, `crates/browser-connector`, and `extension` are empty.

## Amendment (2026-08-14): one rule list, stated polarity, and authored restrictions

Status: Accepted. Extends Decision 2's capability line and Decision 6's editor. Decisions 1, 3, 4,
5, 7, 8, and 9 stand exactly as written.

### A1. Rules are one list, in evaluation order (refines D2 and D6)

D2 put the rules behind each capability line in a per-layer section, and D6 put the editable ones in
a second section below. In use that printed the same rules twice under the same heading, and the
page grew a scrollbar before it said anything the first section had not.

There is now one list. Organization rules come first because authority considers them first and
nobody can edit them here; this person's own follow. Each rule is one line: the sentence it reads
as, and at the right edge either the organization's name or the way in to edit it. Opening a line is
what reveals detail -- read-only for a rule this person cannot change, the editor for one they can.
Settings and the exact documents sit below the list rather than repeating per layer.

### A2. A capability line states polarity, not just breadth (supersedes D2's three states)

D2 gave a capability three states: available, site-scoped, refused. Site-scoped covered two opposite
situations -- an open baseline with sites blocked, and a closed one with sites allowed -- and
flattening them hid the only part a person can act on.

The compiled state is now four: available, some sites blocked, some sites allowed, and not
available. The two middle states name which way the rules point, in those words.

### A3. The editor authors restrictions, in the one direction they can mean anything

D6 described the editor in terms of site rules only. The registered settings
(`browser.tabs.allow_close`, `privacy.preserve_target_names`, `channels.mcp.enabled`,
`channels.cli.enabled`, `content.security.sacred_domains`) were readable on the destination and
authorable only by hand, which left the window unable to express things its own schema already had.

The editor now authors them. Each appears as a restriction to switch on, named by what it does, with
the consequence stated beneath it. Absence means no opinion; the permissive value is never authored,
because a user layer cannot hand authority back and offering it would imply otherwise. `level` is
not offered: both levels only tighten in 1.0 and nothing sits below this layer, so the choice would
be a word without a consequence. `policy.user.enabled` remains organization-only and is still
refused at the boundary (D5).

Sacred destinations are a list rather than a switch, with the same plain-words readback every host
pattern gets, and they continue to appear under the permanent boundaries once in force.

### Acceptance evidence added

19. Rules render as one list, organization first, each naming whose it is, with no detail pane until
    a row is opened.
20. A capability narrowed by a universal rule with holes reads "some sites blocked"; one narrowed by
    named hosts reads "some sites allowed"; the narrower of two layers wins and both are named.
21. Switching a restriction on authors only the tightening value; switching it off removes the entry
    rather than authoring permission.
22. An authored restriction is read back into the draft, and a restriction in force renders as a
    sentence rather than a registered key.
23. Only an organization ceiling disables a capability control; a capability merely absent from this
    person's own rules stays available to grant.

## Prior art

- [Tailscale visual policy editor](https://tailscale.com/blog/visual-editor-beta): a visual editor
  that generates the canonical text policy rather than replacing it, keeps the file visible even when
  externally managed, and previews rules before a save.
- [Group Policy Modeling and Results](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/group-policy/group-policy-modeling-results):
  effective settings with a named winning policy object per setting and the precedence chain one
  click away.
- [chrome://policy](https://support.google.com/chrome/a/answer/9024365): only non-default policies,
  each with source, scope, level, and status, plus a raw export.
- [Apple device supervision](https://support.apple.com/guide/deployment/about-device-supervision-dep1d89f0bff/web):
  the managing organization named in a sentence, at the top, with detail one tap away.
- [IAM Access Analyzer policy generation](https://aws.amazon.com/blogs/security/iam-access-analyzer-makes-it-easier-to-implement-least-privilege-permissions-by-generating-iam-policies-based-on-access-activity):
  a draft policy generated from recorded activity, explicitly a starting point rather than an answer.
- [VS Code settings scopes](https://code.visualstudio.com/docs/configure/settings) and its
  long-standing inherited-value complaints ([58038](https://github.com/microsoft/vscode/issues/58038),
  [80243](https://github.com/microsoft/vscode/issues/80243)): the counter-example this design exists
  to avoid.
