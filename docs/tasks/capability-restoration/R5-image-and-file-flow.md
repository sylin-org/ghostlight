# R5: Captured image and inline file flow

## Goal

Restore bounded client-supplied files and exact screenshot reuse without adding a second extension
pixel store.

## Required work

- Extend `browser_upload` to accept exactly one of absolute paths, bounded inline file objects, or
  one `image_` handle.
- Decode and validate inline base64 only after language validation and governance. Preserve one to
  five files and the 5,000,000-byte aggregate ceiling.
- Have each successful screenshot register one volatile generation-bound `image_` beside its
  `view_`. Keep at most one current image asset per workspace and never store more than the upload
  ceiling.
- Erase the asset on supersession, commit, ownership loss, workspace release, or service exit.
- Permit upload to a current file-input target or semantic selector. Permit an image source to be
  dropped at a current-view point through a distinct revisioned physical command.
- Keep file names, paths, media types, and bytes out of audit and presentation.

## Evidence

- Inline decode, duplicate, malformed, missing name, per-file, count, and aggregate bound tests.
- Image ownership, generation, supersession, memory ceiling, and cleanup tests.
- Credential preflight occurs before file loading or decoding that can be deferred.
- Extension tests for ordinary input attach and coordinate drop receipts.
- Process and live-browser journeys for inline attach, screenshot attach, and screenshot drop.

## STOP conditions

- Screenshot bytes must be persisted, written to a temporary file, or stored by the extension.
- A file is read or decoded before governance and credential preflight when it can be delayed.
- Coordinate drop cannot retain the existing `view_` stale-geometry protection.

## Commit

`feat(browser): restore captured image upload`

