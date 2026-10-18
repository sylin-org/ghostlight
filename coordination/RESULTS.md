# Latest result

[0050] linux-claude, 2026-10-18: the browser_navigate `browser` field says it needs new_tab.

- f1096586 `fix(language)`: the portable schema flattens browser_navigate's branches, so
  `browser` sits beside every field, but the decoder refuses it unless `new_tab` is true. Its
  description now opens with "Only with new_tab true." The catalog golden `docs/1.0/catalog.json`
  is regenerated. tools/list grows by 24 characters, inside the ceiling of 31,800.
- 7abbdc39 `docs(language)`: LANGUAGE.md stated the ceiling as 31,600; the code has been 31,800
  since b7f9de10.
- Checks: cargo fmt, clippy -D warnings, and the ghostlight-service tests (1976 passed), plus the
  repository integrity check.
- Deployed on test-01 with scripts/dev-loop.ps1. A tools/list through the installed edge returns
  the new description, and the owner's Chromium is connected after the extension reload.
- Clients see the new description after they restart their MCP session.
