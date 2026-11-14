# Ghostlight in Browser 2.0.2 -- store submission, 2026-11-14

The owner directed the submission on 2026-11-14 ("Let's bump, changelog, then push the store
submission."), and chose the locally packaged ZIP over the release workflow's ("Local package,
now").

## Artifact

- `dist/ghostlight-extension-v2.0.2.zip`, built by `scripts/package-extension.ps1` on the owner's
  Windows machine from commit `1ec01c2b` (chore(release): Ghostlight in Browser 2.0.2).
- SHA-256 `bca8719d53842aff10691366a53a4a3eda21473d0d35d01daa50365c1ddecfbd`.
- `tests/golden/extension-2.0.0.files.sha256` records this ZIP and its files.

This departs from the custody rule in [RELEASE.md](../RELEASE.md), which names the release
workflow's artifact as the one a submission uploads. Two packagers can give different bytes for
the same tree, so a later workflow build of this commit may not match this hash. The uploaded
bytes are the ones recorded here.

## Checks before upload

- `npm test` in `extension/`: 559 passed.
- `node --test tests/guards/*.test.mjs`: 16 passed, the freeze guard included.
- `scripts/adapter-compatibility.ps1`: source 2.0.2 with adapter 2.0.2 compatible; public 2.0.2
  with adapter 2.0.1 compatible.
- `tests/nested-frame-browser-journey.mjs` on the 2.0.2 build, Chrome for Testing 156: both
  promises held.
- Live on the owner's machine with the unpacked 2.0.1-numbered build of the same source: the
  stuck-tab sequence left the tab clean and the first PowerPoint capture was complete
  ([field session record](office-web-field-session-2026-11-13.md)).

## Store operations (`scripts/publish-extension.ps1`, Chrome Web Store API V2)

| Step | Result |
| --- | --- |
| Plan | API automation ready; version 2.0.2; the hash above; no request made |
| Status before | Published 2.0.1 at 100 percent; no submitted revision |
| Upload | `SUCCEEDED`; draft version 2.0.2 |
| Submit | `PENDING_REVIEW`; publish type `STAGED_PUBLISH` |
| Status after | Published 2.0.1 at 100 percent; submitted 2.0.2 `PENDING_REVIEW` |

Approval does not publish this revision. Public publication is a separate step the owner
authorizes, followed by `scripts/reconcile-chrome-store.ps1 -WriteObservedState` before any public
claim changes. `docs/public-status.json` is unchanged: the store still serves 2.0.1.
