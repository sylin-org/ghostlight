# Adapter 1.1.7 custody -- 2026-09-13

Status: Correction prepared; replacement submission not yet performed.

The owner reported another agent's multiline form failure immediately after the
authorized Chrome deployment submission. This revision continues that release
with the reproduced defect corrected. Service 1.3.6 and permissions are unchanged;
adapter compatibility remains service 1.3.4-1.3.6.

## Cause and validation

The production `replaceFocusedText` function converts LF to the named Enter key.
Its shared Enter descriptor supplied physical key identity but no text. A real
Chromium textarea received `First jobSecond job` for `First job\nSecond job`.
The browser test failed against the submitted implementation before the fix.

Enter now supplies carriage-return text and unmodified text on key down. Key up
and shortcut chords keep omitting text.
Textarea replacement and verification normalize CRLF and lone CR to LF.
This corrects the existing input mechanism without a new protocol or architecture.

Prior art: [Playwright's keyboard layout](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/usKeyboardLayout.ts)
declares Enter's carriage-return text. The
[CDP input contract](https://chromedevtools.github.io/devtools-protocol/tot/Input/#method-dispatchKeyEvent)
defines key-generated text separately from physical key identity. Only the
observable protocol technique informed this correction; no external source was copied.

- `node tests/keyboard-browser-journey.mjs`: six Chromium cases pass, running the
  production replacement function and expected-value helper. Checks cover DOM,
  input-listener model, trusted events, blur, and no submission. Cases include
  ordinary newlines, leading/trailing/blank lines, CRLF, lone CR, single-line text
  and clearing. The test uses a disposable component browser without a Ghostlight
  service or changes to the user's installed browser profile.
- `npm test --prefix extension`: 267 tests pass, including multiline worker packets
  and plain/Shift/Control Enter behavior.
- Changed JavaScript syntax and diff checks pass.
- Rust formatting, warnings-denied Clippy and workspace tests pass. Offline
  public-surface and repository-integrity checks pass.
- The earlier installed focus-retention proof remains evidence for unchanged
  focus code; it did not cover multiline values and cannot prove this correction
  is loaded in the user's existing browser. Extension reload is still needed there.

## Artifact and Chrome status

- Path: `dist/ghostlight-extension-v1.1.7.zip`.
- SHA-256: `3375a2599e3a3957c5a31c1d553659c993eb40f145b8642b7360baf4951d5715`.
- The deterministic packager reproduced this exact hash twice; its package-surface,
  license, manifest-version and development-key checks passed.
- Plan reports API ready with this version/hash. The independent pre-mutation
  status reports public 1.1.4 PUBLISHED and our 1.1.6 PENDING_REVIEW.
- The existing deployment authorization covers this correction to the same release.
  Preserve the prior artifact and submission record; replace only the matching
  pending revision after inspection.
