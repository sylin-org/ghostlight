# ADR-0154: Explicit Fix may replace a foreign harness entry

- Status: Accepted (implemented in this revision)
- Date: 2026-09-04
- Amends: ADR-0135 Decision 3 and ADR-0146 Decision 2
- Builds on: ADR-0102, ADR-0117, ADR-0125, and ADR-0135

## Context

Ghostlight detects when another command occupies the `ghostlight` key in a supported MCP client
configuration. ADR-0135 made that state judgeable by showing the bounded command Ghostlight found,
but deliberately left only read-only actions. The card could explain the exact problem and the
replacement configuration, yet the person still had to edit the file by hand.

The ownership rule was too broad for this user journey. An automatic setup pass must not replace
uncertain content. A person looking at the evidence and explicitly choosing Fix is a different
intent: replace this one conflicting entry with Ghostlight's current registration.

Malformed and unreadable files are not the same case. Ghostlight cannot isolate one entry safely
when it cannot understand the surrounding document.

## Decision

1. A supported target whose parseable configuration contains a foreign entry under Ghostlight's
   own key advertises `can_fix`. Its card offers one primary `Fix` action. Malformed, unreadable,
   missing, current, and Ghostlight-owned stale entries do not advertise that action.
2. Fix requires a visible confirmation. The confirmation states that only the entry under
   Ghostlight's key will be replaced and that the configuration is backed up first.
3. Fix re-reads the file inside the serialized harness mutation boundary. The writer proceeds only
   if that exact read still contains a foreign Ghostlight entry in a shape it can replace
   losslessly. If the entry changed, disappeared, became owned, or became malformed, the action
   refuses and asks for a re-check.
4. The existing JSON/JSONC, TOML, and YAML writers perform the replacement. They preserve unrelated
   entries and supported comments and formatting, write through symlinks, create the existing
   `.ghostlight-backup`, and atomically replace the resolved file. Fix grants no generic file or
   command capability.
5. Ordinary Set up, Update, Remove, `Set up everything`, and CLI setup retain their existing
   ownership rules. They never replace a foreign entry. Fix is a separate per-target local-human
   action and is never inferred or run in bulk.
6. The action travels through the existing closed `manage_harness` workbench command and
   `WorkbenchFacade`. No MCP, service, browser, connector, bridge, or extension contract changes.

## Consequences

- A misconfigured Cline or another supported client can be repaired from its evidence card without
  manual JSON, TOML, or YAML editing.
- The overwrite is deliberate, narrow, recoverable, and stale-state checked.
- Malformed and unreadable documents remain manual because rebuilding them would risk unrelated
  user configuration.
- Aggregate setup remains safe to repeat and cannot silently consume the new overwrite authority.

## Rejected alternatives

### Let Set up overwrite foreign entries

Rejected because one aggregate convenience action cannot express review of each conflicting
command. It would turn discovery into destructive authority.

### Replace malformed configuration files

Rejected because Ghostlight cannot preserve content it cannot parse. A backup does not justify
silently discarding unrelated settings.

### Add a generic editor

Rejected because the product already knows the exact entry and desired replacement. General file
mutation would widen the workbench boundary without improving this repair.

## Acceptance

- Projection tests distinguish fixable foreign entries from malformed or non-lossless shapes.
- Installer tests prove Fix preserves siblings, creates a byte-exact backup, reaches Current state,
  and refuses a repeated or stale Fix without changing bytes.
- JSON/JSONC, TOML, Goose YAML, and Continue YAML fixtures exercise the replacement seam.
- The workbench journey proves only `can_fix` targets render Fix and that confirmation precedes the
  closed `fix` action.
- Existing tests continue to prove ordinary install, uninstall, and aggregate setup leave foreign
  entries untouched.
