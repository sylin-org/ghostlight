// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Small protocol-neutral result builders shared by tool executors.

use serde_json::{json, Value};

/// Build a canonical tool result carrying a single text block:
/// `{ "content": [ { "type": "text", "text": ... } ] }`.
pub fn text_content(text: impl Into<String>) -> Value {
    json!({ "content": [ { "type": "text", "text": text.into() } ] })
}
