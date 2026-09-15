# Adapter 1.1.8 custody -- 2026-09-15

Status: Packaged and staged; preflight verification passed; awaiting Chrome Web Store upload and review.

This revision packages the multiline contenteditable form verification fix,
the visual settlement sensor heuristics (GhostlightSensor), and background
quiet-window stability. Service 1.3.6 and permissions are unchanged;
adapter compatibility covers service 1.3.4-1.3.7. The Chrome Web Store
currently serves adapter 1.1.7 publicly at 100 percent distribution.

## Cause and validation

1. Multiline contenteditable verification:
   `verifyTargetValue` in `extension/content.js` previously read `textContent` directly,
   which does not preserve newlines for contenteditable elements. It now checks both
   `innerText` and `textContent` fallback, normalizing CRLF and lone CR to LF, matching
   native textarea behavior.

2. Visual settlement sensor heuristics:
   `GhostlightSensor` in `extension/lib/sensor.js` observes layout bounding metrics,
   scroll geometry, and running CSS transitions/animations via `document.getAnimations()`.
   Continuous infinite animation spinners (loading indicators) are bypassed safely to
   prevent deadlocks. Metrics settle over consecutive requestAnimationFrame ticks with
   a quiet-window fallback.

- `npm test --prefix extension`: 283 tests pass cleanly.
- `node tests/stress-journey.mjs`: five real Chromium tests pass against headless Chromium,
  verifying static layout settlement, finite CSS transitions, spinner immunity, layout shift
  timeout, and 20 parallel animation targets.
- `node tests/keyboard-browser-journey.mjs`: six Chromium text replacement cases pass,
  confirming newline preservation, model consistency, trusted events, and zero unexpected submissions.
- JavaScript syntax check (`node --check`) passes across all extension scripts.
- Compatibility check (`pwsh scripts/adapter-compatibility.ps1`) passes: source 1.1.8 and
  public 1.1.7 both cover service 1.3.6.

## Artifact and Chrome status

- Path: `dist/ghostlight-extension-v1.1.8.zip`.
- SHA-256: `55169dc5f115d4d1a00b14c16171b64f4257e4d85715727d97f429599e3a3e7d`.
- Packager: `scripts/package-extension.ps1 -Force`.
- Chrome Web Store public adapter: 1.1.7 (reconciled in `docs/public-status.json`).
