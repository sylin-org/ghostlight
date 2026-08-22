# Ghostlight 1.0 extension release preparation -- 2026-08-22

Status: local package and disclosure preparation passed; no store or website mutation performed.

## Boundary

This record compares the exact published 0.8 extension artifact with the current 1.0 source and
records the local 1.0 package produced after fixing the differences that were actual release
issues. It does not claim Chrome review, public website synchronization, store installation, or
GitHub build provenance.

The shipped-byte baseline is release commit
`993135b048b60622157266b53b21f1719c9df4b3` and the public GitHub asset
`ghostlight-extension-v0.8.0.zip`:

- SHA-256: `9c7f16feaa0a7f068b440af107463162f8f467229f3daab8ad2f4f759bc9dfae`
- length: 137,673 bytes
- entries: 37

The 1.0 preparation began from local revision
`81ae40246c284841d03c85de0212e6d6a0c48314` on `dev`. The release-preparation edits recorded here
must be committed before a provenance-bound candidate can name its final source revision.

## 0.8 continuity result

| Surface | Result |
| --- | --- |
| Product identity | Name, description, minimum Chrome version, toolbar title, popup, options page, and keyboard command are preserved. |
| Development identity | The unchanged manifest key still derives extension id `cjcmhepmagomefjggkcohdbfemacojoa`. |
| Store identity | The existing public item remains `lejccfmoeogmhemakeknjjdhkfkgncdl`; store packaging removes the development key as 0.8 did. |
| Manifest icons | The 16, 32, 48, and 128 pixel PNGs are byte-identical to the 0.8 release source. |
| Other artwork | The inherited SVG, mascot, and 512 pixel source assets remain byte-identical in the repository. They are not manifest entries. |
| Popup and options | Pause/resume, end/start session, connection state, captions, effects, diagnostics, and the established visual identity remain; 1.0 adds the preserve-tabs interlock, missing-install recovery, and local debugger release. |
| Native host | The fixed host name and both extension identities are preserved. The old repository template is intentionally replaced by installer-owned generation and ownership checks. It was never in the 0.8 store ZIP. |
| Presentation | The old indicator and broker files are intentionally replaced by the current content-free presentation module and its protected palette, motion, accessibility, and lifecycle tests. |
| Permissions | The unused 0.8 `scripting` permission is gone. HTTP/HTTPS replaces broader `<all_urls>`. `webNavigation`, `downloads`, and `offscreen` serve tested 1.0 browser responsibilities. |
| Historical coverage | The checked recovery inventory still dispositions all 1,388 historical entries in 12 groups and all 34 named process scenarios. |

No deleted 0.8 extension file was found to represent an unowned current product responsibility.
Restoring those implementation files would violate the clean-room boundary; their observable
contracts are represented on current seams.

## Real issues fixed

1. `downloads` and `offscreen` were declared but lacked matching store justification blocks.
2. The planned 1.0 privacy policy had dropped recording and diagnostics handling. It now describes
   browser-local frames and encoding, the three recording destinations, bounded volatile state,
   Chrome downloads, client delivery, diagnostic minimization, and the audit host/name exceptions.
3. The store instructions implied a version bump required new assets. They now preserve accurate
   existing assets and require a change only when an asset misrepresents the submitted extension.
4. The 1.0 packager copied the complete `extension/icons` source directory. That included the
   unreferenced 1,331,931-byte `mascot.png`, `icon512.png`, `ghost-mark.svg`, and another mascot.
   The store package now carries only the four manifest icons, while every source asset remains in
   the repository.
5. The extension README named two absent historical files. It now describes the current
   presentation and installer-owned native-host seams.
6. Store instructions said to reconcile after staged review acceptance. Reconciliation now occurs
   only after public publication is observable and names the real `-WriteObservedState` switch.
7. Repository integrity now derives the expected Chrome permission-justification headings from
   `extension/manifest.json`, so another manifest/document mismatch fails CI.

## Prepared local artifact

`dist/ghostlight-extension-v1.0.0.zip` was built twice from the same source. The two files were
byte-identical:

- SHA-256: `ccb48577a93995b1eaaf9b13fab75313a347483553782d178187e1ea8ceb0923`
- length: 85,835 bytes
- entries: 30
- manifest version: `1.0.0`
- development key present: no

The four packaged icon hashes are:

| Size | SHA-256 |
| ---: | --- |
| 16 | `95d754348d4fabfb0412e32319226dd52615864ae511b8b492bef739f555d224` |
| 32 | `645b2b436975da68006fcf5bf89242f55f2988e468cd6278679cffb31a3b2dc8` |
| 48 | `9cdf8201880b2aec05f22d1dbb68822187dbe39f25d425b57960a6affca064de` |
| 128 | `153e65ae92af61a7cd2dcbe38c59e6875287a9e3f0208fb4e73f781292327a67` |

The package contains the root manifest, popup, options, setup fallback, service worker, content
script, offscreen encoder, current runtime libraries, vendored encoder and license, four manifest
icons, and Apache and MIT license texts. Its internal checker rejects any extra or missing entry,
a source-version mismatch, a development key, or repository-only test and package files.

## Checks passed

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- all 359 Rust workspace tests;
- all 116 extension tests;
- changed PowerShell parser checks;
- repository integrity, including exact permission/justification correspondence;
- all 809 historical artifact dispositions;
- all 1,388 historical behavior dispositions and 34 process-scenario dispositions;
- deterministic package construction across two builds;
- packaged version, key absence, exact entry count, and exact icon hashes;
- offline public-surface truth; and
- the Chrome publication adapter in non-mutating `Plan` mode with an explicitly absent credential
  fixture.

## Remaining owner-controlled boundaries

- The canonical policy is ready in `docs/legal/PRIVACY.md`, but the public
  `https://sylin.org/ghostlight/privacy/` fallback remains the 0.8 text in the separate website
  repository. Update that fallback and rebuild the website before Chrome submission. This is an
  outward website action and was not performed here.
- Review the signed-in Chrome dashboard fields against the prepared listing, privacy, and
  permission text. Reuse the current screenshots and promotional assets unless inspection finds a
  material inaccuracy.
- Build or adopt the exact ZIP from the final frozen, committed revision through the candidate
  workflow before claiming GitHub provenance. The local ZIP is ready for byte comparison but is
  not itself a provenance attestation.
- Upload, submit for staged review, and publish only with explicit owner approval. No Chrome Web
  Store request was made in this preparation.
