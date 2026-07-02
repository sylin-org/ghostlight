//! Tab-group tools: **`tabs_context_mcp`** (Observe -- get the MCP tab-group context) and
//! **`tabs_create_mcp`** (Mutate -- create a tab in the group).
//!
//! Note the `_mcp` suffix -- the exact reference names (see `reference/ANALYSIS.md` sec 8.5). The
//! extension manages the "MCP" tab group via `chrome.tabs`/`chrome.tabGroups`. Implemented Phase 2.
