# Extension store submission -- 2026-08-24

Status: SUBMITTED. The 1.0.0 extension package rebuilt from current source replaced the stale
pending review on the existing Chrome Web Store item. This is long-lead review traffic, not
publication: the public listing still serves 0.8.0, and the staged submission cannot go live
without an explicit owner publication action.

## Package

```text
date_utc: 2026-08-24
version: 1.0.0
source_revision: 70869631 (dev tip; includes the foundry-sprint extension fixes:
                 reply-before-dispatch activation, fill submit-leg early reply,
                 activationPlan button-index correction)
sha256: f7b9a6adbf94bf5b1dcc158a3548501ff230ad4d39e72a5c878bde8d2d284d68
entries: 32
development_key: absent
determinism: byte-identical across two packaging runs
```

Built with `scripts/package-extension.ps1`. The hash supersedes both `97bd4816...` (R9
candidate, pre-foundry-fixes) and `ccb48577...` (the stale staged review's bytes).

## Submission

```text
item: lejccfmoeogmhemakeknjjdhkfkgncdl (Ghostlight in Browser, publisher Sylin.org)
api: CWS v2 publishers/{id}/items/{id}:upload then :publish
upload: SUCCEEDED, draft version 1.0.0
submission state: PENDING_REVIEW
publish_type: STAGED_PUBLISH
public_listing: unchanged, still serving adapter 0.8.0
```

Executed through `scripts/publish-extension.ps1 -Action Upload/Submit -Execute` after the
owner directed store-first submission and supplied the publisher id from the dashboard URL.
The OAuth refresh path was verified healthy before the upload.

## Operational note

`chrome.google.com/webstore` is a Chromium-protected origin: content scripts cannot inject
and debugger attach is refused ("The extensions gallery cannot be scripted"), so the
developer dashboard is human-driven territory by platform design. Ghostlight can open and
navigate to it, but cannot read or act inside it. Submission automation therefore runs
through the CWS API, which is exactly what the credential file and this script support.

## Consequences

- Google's review of the 1.0 permission set (`offscreen`, `downloads`) starts now, outside
  any release clock.
- If the extension source changes before G0 freeze, this submission goes stale and must be
  replaced (accepted tradeoff for early policy review; resubmission is one command).
- G3's "exact candidate bytes" rows remain open until the frozen revision's ZIP is packaged
  and matches this item's draft.
