//! `computer` -- mouse, keyboard, and screenshots. A single MCP tool with an `action` parameter
//! spanning **13 actions**: `left_click`, `right_click`, `double_click`, `triple_click`, `type`,
//! `screenshot`, `wait`, `scroll`, `key`, `left_click_drag`, `zoom`, `scroll_to`, `hover`.
//!
//! Per-action classification (enforced by the v1.5 overlay in the binary, not at the schema
//! level): **Observe** = `screenshot`, `wait`, `zoom`; **Mutate** = everything else. Screenshots
//! are returned only for `screenshot`/`scroll`/`zoom`; other actions return a text confirmation.
//! Implemented in Phase 2/3.
