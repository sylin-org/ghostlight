# S6: progressive depth and repair

## Objective

Make control and technical depth available without making either a prerequisite for ordinary use.
The GUI and CLI should expose the same underlying truth at different levels of detail.

## Prompt outline

1. Organize workbench depth from At a glance into controls and preferences, integrations and
   authority, then diagnostics and records. Reuse existing destinations where they remain honest;
   do not create a destination for every subsystem.
2. Present the few accepted behavior choices in user terms. Keep their registered keys and raw
   values available in detail, not as the primary label.
3. For every degraded state, prefer automatic recovery, then one bounded `Fix` action, then an
   exact manual instruction. Never expose a generic command runner or configuration editor.
4. For every owned integration, show what Ghostlight found, what it owns, what it would change, and
   how to copy the exact connector path or equivalent CLI command.
5. Align CLI and GUI projections for readiness, browser startup mode, page presence, runtime
   control, recovery results, and diagnostics. Add structured output only through existing command
   seams.
6. Remove stale duplicated status, settings, and repair language. Keep advanced information
   searchable and accessible without letting it dominate the common journey.

## Completion evidence

- Ordinary use remains possible without opening the workbench.
- At a glance reaches every deeper fact through a short, coherent navigation path.
- Every accepted preference has one owner and consistent GUI/CLI behavior.
- Repairs are ownership-safe, previewable where consequential, and idempotent.
- A power user can find exact paths, provenance, configuration, and structured status without
  reading source code.
- No new generic settings, installer, or diagnostic framework was introduced.

## Stop conditions

- A preference exists only to expose an internal mechanism.
- GUI and CLI would maintain separate state or wording authorities.
- A repair would overwrite foreign bytes, install third-party software, or broaden authority.
- Navigation growth is substituting for a missing aggregate projection.
