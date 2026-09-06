# Acorn 8.18.0

Ghostlight packages this MIT-licensed JavaScript parser for syntax-only script form selection
inside the extension worker. It does not execute scripts, fetch code, or make policy decisions.
The extension's existing packaging script includes this directory and its license notices.

- Upstream: [acornjs/acorn](https://github.com/acornjs/acorn).
- Exact package: [acorn-8.18.0.tgz](https://registry.npmjs.org/acorn/-/acorn-8.18.0.tgz).
- Archive SHA-512 (base64):
  `lGq+9yr1/GuAWaVYIHRjvvySG5/4VfKIvC8EWxStPdcDh/Ka7FG3twP6v4d5BkravUilhIAsG4Qj83t02LWUPQ==`.
- Upstream source: `package/dist/acorn.js`; license: `package/LICENSE`.
- Checked-in [acorn.js](acorn.js) SHA-256:
  `23dcd0dc2e4d2d1c2b65d8feb87a4489f3e1a92d568844324dc0efc2ebeb5aae`.
- [License](acorn.LICENSE.md): retained verbatim.

The only source transformation replaces each non-ASCII UTF-16 code unit with its JavaScript
`\uXXXX` escape (14 code units), satisfying the repository's ASCII rule. All other source bytes
are retained, with UTF-8 encoding, no BOM, and LF line endings. No minifier or dependency install
step is required to load or package the extension.

To reproduce, download the exact archive, verify its SHA-512, extract the two named members, and
apply that escape transformation to the JavaScript file. Check the resulting SHA-256. Do not
run package lifecycle scripts or substitute an unpinned latest release.

An update must retain the license and refresh both digests deliberately. Run evaluator regressions
and the real Chromium script journey; especially check await, resource declarations, nested
functions, bare returns, REPL declarations, and rejection before any effect.
