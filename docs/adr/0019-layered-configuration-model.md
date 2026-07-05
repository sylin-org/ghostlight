# 0019. Layered configuration: typed key registry, presets, organization locks

- Status: Accepted
- Date: 2026-07

## Context

Hardening work introduced the first tunable (the bounded first-call wait,
ADR-0017), and the staged governance layer will introduce many more. Two
audiences pull in different directions: individuals want a simple posture
choice, not a settings maze; organizations want to lock some settings (the
grant list, redaction, audit destinations) while leaving others open, per
user or per org, with a full or reduced range of options.

Mature prior art exists for exactly this shape, and per ADR-0008 we harvest
the intent, not the code:

- **Chromium enterprise policy**: the closest complete model. Policies have
  levels (mandatory vs recommended), scopes (user vs machine), and sources
  merged under a fixed precedence; `chrome://policy` shows every effective
  value with its source and any conflict.
- **Windows Group Policy / ADMX**: machine policies live where only
  administrators can write; the policy-vs-preference distinction separates
  enforced-and-reverting settings from defaults that merely tattoo.
- **macOS managed preferences**: MDM profiles layer above user defaults.
- **Firefox policies.json**: a single cross-platform policy file.
- **GNOME dconf**: system databases plus explicit lock files that mark
  individual keys read-only; the cleanest open-source key-level lock.
- **VS Code**: a typed settings registry drives the UI, documentation, and
  the default-user-workspace precedence chain.
- **chrome.storage.managed**: organizations already have a deployment pipe
  that delivers read-only configuration to extensions.

## Decision

1. **One typed key registry.** The registry in `src/policy/mod.rs` is the
   single source of truth for every configurable behavior. `KeyDef` grows
   beyond booleans to typed values (bool, integer with range, enum, string
   list), each with a description and per-preset default. Nothing
   configurable is hardcoded; `engine.connection.first_call_wait_ms` is the
   first non-boolean key.
2. **Layered resolution, one precedence.** Effective value = org-mandatory,
   else user, else org-recommended, else preset default, else built-in
   Minimal. Every resolved value carries its source and a locked flag.
3. **Presets are UX, not machinery.** "Fully Open", "Safe", and "Restricted"
   are named bundles that write the user layer only. "Safe" is today's
   Minimal. A preset is a starting point the user can then edit key by key.
4. **The org layer is a machine-scope file.** It lives at an
   admin-writable-only path (ProgramData, /etc, /Library) and is delivered by
   the org's existing deployment channel (GPO, Intune, Jamf), consistent with
   deployment-channel identity binding (SPEC 8.1). Each entry sets a value at
   level mandatory (locked) or recommended (overridable default). Range
   constraints (allow a key but bound its values) are a later extension.
   Honesty rule: file ACLs plus the deployment channel are a usage-surface
   guard, not a cryptographic boundary; manifest signing stays excluded
   (SPEC 10).
5. **Surfaces.** The CLI is the source of truth: `config list | get | set`
   shows and edits effective values with source and lock (the
   `chrome://policy` analog), and `doctor` reports health. The extension
   options page and popup are the friendly surface for the same data: they
   render status and settings, and submit edits to the binary over the
   existing native messaging channel. The binary resolves layers and rejects
   writes to locked keys, so the extension remains policy-free presentation
   (ADR-0005); locked fields render read-only with a "managed by your
   organization" badge. No embedded HTTP server in v1: it would add an attack
   surface and a moving part to a security-sensitive binary. Revisit only if
   the product family (ADR pending on the family vision) needs a shared local
   dashboard; Syncthing is the precedent to study then.

## Consequences

- Positive: individuals get one-click postures, organizations get per-key
  locks, and both ride the same mechanism; no second config system later.
- Positive: one registry drives the CLI, the extension UI, and generated
  documentation, so the three cannot drift apart.
- Positive: locked-field UX falls out of the model instead of being bolted on.
- Negative: key names become a public, stable API surface; renames need
  deprecation handling.
- Negative: three platform-specific org-file paths to document and test.
- Follow-up: implement resolution and the CLI in stage 2 step 1 (alongside the
  audit recorder, ADR-0018), since audit destinations are the first keys an
  organization will want to lock.

## Amendment (2026-07-05, ADR-0030)

This is the anticipated revisit, not a reversal: ADR-0030 (Ghostlight Hub) is the "product family"
ADR this decision's Decision 5 named as the precondition ("Revisit only if the product family...
needs a shared local dashboard; Syncthing is the precedent to study then"). Its Decision 9
introduces a loopback-pinned embedded HTTP server (the local web API) and, on top of it, "the
Console" -- a read-mostly, provenance-aware view of exactly the effective-value/source/lock data
this ADR's Decision 2 already models, the `chrome://policy` analog this ADR's Decision 5 named as
the CLI's own point of comparison. `config list | get | set` remains the source of truth; the
Console renders the same registry, never a second one. See ADR-0020's own amendment for why this
does not reopen that ADR's "no web console" non-goal for organization policy authoring.
