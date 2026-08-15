# The policy surface a governed person actually reads

Date: 2026-08-14

Status: Research input. This document records prior art, the current tree baseline, and the design
patterns worth adopting for the workbench policy surface. It does not by itself change a product
contract. [ADR-0122](../adr/0122-readable-policy-destination-and-authored-user-layer.md) is the
decision it feeds.

## Why this document exists

[03-governance-enterprise-prior-art.md](03-governance-enterprise-prior-art.md) surveyed governance
prior art in 2026-07, but it looked at the problem from the administrator's chair: manifest
ergonomics, resolution semantics, deployment templates, SIEM alignment. It settled the questions an
organization asks.

Nothing has surveyed the other chair. The person Ghostlight governs is not the person who wrote the
policy. They open the window because an agent did something unexpected, or refused to, and they want
to know what the rules are and who set them. On the current tree they cannot find out. That is the
gap this document researches.

## The current tree, as of this writing

Three read-only pieces, all in the Status page or the band above it:

- A state chip in the lamp band ([index.html:22-26](../../crates/orchestrator/ui/index.html#L22-L26)),
  computed by `policyState()` in
  ([view.js:266-300](../../crates/orchestrator/ui/lib/view.js#L266-L300)). It folds four booleans
  into one of `All open`, `Policy applied`, `N policies applied`, or `Policy issue`.
- An "Authority sources" card grid ([view.js:437-449](../../crates/orchestrator/ui/lib/view.js#L437-L449)):
  three fixed cards showing configured / active / valid per source.
- The managed Policy Passport ([view.js:451-471](../../crates/orchestrator/ui/lib/view.js#L451-L471)):
  organization, freshness, verified sequence, source class, last check, rationale, contacts.

The data behind them is `ConfigurationSummary`
([workbench/mod.rs:958-978](../../crates/orchestrator/src/workbench/mod.rs#L958-L978)), built from
`GovernanceDiagnostics` ([governance/mod.rs:1100-1120](../../crates/orchestrator/src/governance/mod.rs#L1100-L1120)).
Every field there is deliberately content-free: booleans and provenance, never a rule.

Consequences of that baseline, stated plainly:

1. A user can learn that a policy exists and is healthy. They cannot learn what it does.
2. A user under an organization policy cannot see a single rule, cannot find out which rule blocked
   their agent, and cannot connect a denial to the contacts the passport already displays.
3. A user with no organization policy has no way to create one from the window. The only path is to
   hand-write schema-3 JSON, set `GHOSTLIGHT_POLICY_FILE`
   ([governance/mod.rs:1230](../../crates/orchestrator/src/governance/mod.rs#L1230)), restart
   Ghostlight, and inspect the result from a terminal with `ghostlight policy explain`.

## Prior art

### 1. Tailscale's visual policy editor: the closest match

Tailscale is a text-policy-file product (HuJSON) that added a visual editor in 2026 without
abandoning the file. Their design decisions are the ones this problem needs:

- The editor complements the file rather than replacing it. Policy stays HuJSON underneath and users
  toggle between the two ways of working.
- The visual output is copy-pasteable into an editor, a Terraform config, or a GitOps-managed file.
- Comments in the policy file are preserved and displayed in the visual interface.
- When the policy is locked by GitOps or Terraform, it remains available in read-only mode rather
  than vanishing from the UI.
- A "Preview rules" tab answers who can reach what before a save.
- Tests declared in the policy file must pass before a create or update is applied.

The stated audience is two groups at once: people who prefer forms, and people who work in text but
want help with syntax occasionally. That is the correct framing for Ghostlight, where the CLI is a
real surface with real users and must not be demoted.

**Take:** the editor is a generator over the canonical schema-3 file, never a replacement. The file
and its path are always visible. A file Ghostlight does not own is shown read-only rather than
hidden.

### 2. Windows Resultant Set of Policy: the answer to "who decided?"

Group Policy has the same overlay problem Ghostlight has, at much larger scale, and solved the
presentation of it decades ago. `gpresult` and the Group Policy Results wizard show the settings
actually applied, from which GPOs, in what order. The Settings tab names the **winning GPO** for
each individual setting, and a Precedence tab lists every GPO that tried to set that setting, in
precedence order, with the winner at the top.

The important part is the posture: the tool does not hand the user a stack of layers and ask them to
compute the answer. It gives the answer first and the derivation second, on demand.

**Take:** the compiled result is the headline. Every line carries the layer that decided it. The
full precedence chain is one interaction away, not the default view.

### 3. chrome://policy: restraint and escape hatches

Chrome's policy page displays only policies that are not at their default value; anything unset is
simply absent. Each row carries source (platform, cloud, enterprise default), scope (machine or
user), level (mandatory or recommended), and status. There is a name filter, a "Reload policies"
button, and JSON export.

Two lessons. First, silence for defaults: a page that lists everything teaches nothing. Second, the
raw export always exists, so a power user is never trapped in the rendering.

Worth noting that Ghostlight's schema already carries the same mandatory/recommended vocabulary in
`SettingLevel` ([manifest.rs:96-115](../../crates/orchestrator/src/governance/manifest.rs#L96-L115)),
and the GUI currently shows none of it.

### 4. chrome://management and Apple supervision: organization identity, plainly worded

Chrome shows a "managed by your organization" signal and a dedicated management page so an
enterprise user can see that policies are active and what is managed. Apple puts the equivalent at
the top of Settings: "This iPhone is supervised and managed by Company, Inc.", with the profile
detail one tap away. Both make the organization a named party rather than an anonymous force.

The value here is as much emotional as informational. The user is being restricted by someone. Being
told who, in a sentence, and being given somewhere to go, is what separates a governed product that
feels trustworthy from one that feels hostile. Neither Chrome nor Apple makes the user hunt for it.

**Take:** organization identity belongs at the top of the policy destination, as a sentence, in the
organization's own words. Ghostlight already carries `org_name`, `rationale`, and typed contacts in
the signed managed presentation
([governance/mod.rs:1160-1175](../../crates/orchestrator/src/governance/mod.rs#L1160-L1175)); the
manifest itself carries none, so a plain organization policy file is anonymous.

### 5. AWS IAM Access Analyzer and the Policy Simulator: consequences before commitment

Access Analyzer generates a least-privilege policy from recorded CloudTrail activity: run the
workload permissively for a representative period, then generate a policy that reflects what was
actually used. The Policy Simulator is the matching half, evaluating a candidate policy against
specific actions to show what it would allow or deny.

The documented weaknesses are as instructive as the strengths: generated policies are over-specific
and need human editing before production, and the log does not capture everything. The pattern works
as a **starting draft**, not as an answer.

**Take:** seed a new user policy from the hosts in the user's own audit history, presented as a
draft to trim rather than a finished policy. Then run the candidate against real recorded history
before applying it. Ghostlight has both halves already:
`ghostlight policy simulate` replays audit through the production decision engine
([inspection.rs:146-187](../../crates/orchestrator/src/governance/inspection.rs#L146-L187)), and the
workbench holds a bounded history of up to 500 records.

### 6. VS Code settings scopes: the cautionary tale

VS Code has the same shape of overlay: default, user, remote, workspace, with narrower scopes
overriding wider ones and an effective value computed per key. It is the most-used example of this
pattern, and its long-standing complaints are precisely about the presentation, not the semantics:
inherited values are not shown in the narrower scope, so while editing a workspace setting you
cannot see what you are actually going to get ([vscode#58038](https://github.com/microsoft/vscode/issues/58038),
[vscode#80243](https://github.com/microsoft/vscode/issues/80243)).

**Take:** while editing the user layer, the organization ceiling must be visible inline, on the
control being edited. A banner at the top of the page is what VS Code effectively has, and it is not
enough. A user rule that has been made redundant by an organization rule must say so in place.

### 7. Host pattern grammar: a readback is not optional

Research 03 established that Ghostlight's host pattern semantics are non-standard in three
directions at once: Chrome's URLBlocklist treats a bare host as host-plus-subdomains and a leading
dot as exact, agent-browser's `*.example.com` also matches the bare `example.com`, and Ghostlight's
matches subdomains only. That finding stands, and 1.0 shipped Ghostlight's own semantics
([ADR-0121](../adr/0121-restore-rawx-policy-and-managed-fetch.md) Decision 2).

If the grammar cannot be changed, the interface must remove the ambiguity by other means.

**Take:** every host pattern the user types gets an immediate plain-language readback stating
exactly what it matches. The user is never asked to hold a grammar in their head.

## The five patterns that recur

Across every system above:

1. **Answer first, derivation second.** Show the effective result; make the reasoning reachable.
2. **Name the decider on every line.** Ambiguity about who set a rule is the main source of user
   frustration and support load.
3. **Never hide the underlying file.** A GUI that conceals its own output loses the power user and
   creates a second source of truth.
4. **Preview before commit.** The systems people trust let you see the consequence of a change
   against real data before it takes effect.
5. **Say nothing about defaults.** Only what is actually set earns a line.

## What the user is actually asking

Framed as the questions a person arrives with, in the order they arrive:

| Question | Where it is answered today |
| --- | --- |
| Can the agent do things right now, or is something stopping it? | Partly. One chip word. |
| What exactly is it allowed to do? | Nowhere. |
| Who decided that, my company or me? | Nowhere. |
| Why was that specific action blocked? | Audit file only, by denial id, outside the window. |
| Who do I ask about it? | Contacts card, unlinked to any denial. |
| Can I make it stricter for myself? | CLI, environment variable, restart. |
| What happens if I change this? | `ghostlight policy simulate`, if you know it exists. |
| How do I undo it? | Delete a file, unset a variable, restart. |

Six of eight have no answer in the window. The two that do are the two that matter least.

## Assets Ghostlight already has

Worth stating plainly, because the delight here is mostly presentation of work already done, not new
machinery:

- **Layer intersection with deny-overrides** is implemented. A user layer already cannot widen an
  organization layer. Editing is safe by construction (ADR-0121 Decision 2).
- **Grant attribution and deterministic `D-` denial ids** already exist on every enforced denial.
  The data needed for "which rule blocked this" is recorded.
- **The decision engine is reusable offline.** Simulation runs the production path with no audit
  side effects.
- **Catalog projection** already computes the exact surviving tool set under current authority, so
  "what can it do right now" is a projection of an existing computation.
- **Atomic reload** already happens: `PolicySource::refresh` re-reads on every snapshot, so a written
  file applies to future invocations with no new machinery.
- **Signed organization presentation** already carries name, rationale, and typed contacts.
- **Bounded audit history** in the workbench is already the corpus a dry run needs.

## Anti-patterns to avoid

- **Do not build an admin console.** ADR-0102's amendment deliberately trimmed the window to a small
  number of destinations. Grant reordering matrices, signing helpers, and bundle publication belong
  to the CLI, which admins already script.
- **Do not let a GUI action fail the user closed.** A configured source with no valid policy fails
  closed by design. A save button must validate before it replaces anything, and write atomically.
- **Do not make revert ceremonial.** Reversibility is what makes people willing to try a policy at
  all. One click back to the previous state, no typed confirmation.
- **Do not invent vocabulary.** The user should never have to learn "grant", "manifest", "schema",
  "RAWX", or "capability set" to use the surface.
- **Do not present ordered rules as unordered.** Grant resolution is first-match-wins and that is
  decided. An editor that hides shadowing turns a documented footgun into a silent one.

## Open questions handed to the ADR

1. Does organization identity move into the manifest, and if so at what schema cost, given that
   manifests are typo-closed with `deny_unknown_fields`?
2. Where does a workbench-authored user policy live, and what happens when
   `GHOSTLIGHT_POLICY_FILE` already points somewhere else?
3. Should an organization be able to forbid a user layer at all, given that a user layer can only
   subtract authority?
4. Does policy become its own destination in the window?
5. Does the same compiled projection get a model-facing rendering, closing the gap between
   ADR-0121 Decision 3's "always-available policy explain operation" and a tree where explain is a
   CLI command over a file path?

## Sources

- [Tailscale visual policy editor beta](https://tailscale.com/blog/visual-editor-beta)
- [Tailscale visual policy editor reference](https://tailscale.com/kb/1587/visual-editor-reference)
- [Tailscale tailnet policy file management](https://tailscale.com/docs/features/tailnet-policy-file/manage-tailnet-policies)
- [Group Policy Modeling and Results](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/group-policy/group-policy-modeling-results)
- [Using RSoP to check group policy settings](https://activedirectorypro.com/how-to-use-rsop-to-check-and-troubleshoot-group-policy-settings/)
- [View a device's current Chrome policies](https://support.google.com/chrome/a/answer/9024365)
- [Chrome policy precedence](https://cloud.google.com/blog/products/chrome-enterprise/understanding-policy-precedence-for-chrome-browser)
- [Check if your Chrome browser is managed](https://support.google.com/chrome/answer/9281740)
- [About Apple device supervision](https://support.apple.com/guide/deployment/about-device-supervision-dep1d89f0bff/web)
- [IAM Access Analyzer policy generation from access activity](https://aws.amazon.com/blogs/security/iam-access-analyzer-makes-it-easier-to-implement-least-privilege-permissions-by-generating-iam-policies-based-on-access-activity)
- [VS Code user and workspace settings](https://code.visualstudio.com/docs/configure/settings)
- [Chromium URL blocklist filter format](https://www.chromium.org/administrators/url-blocklist-filter-format/)
