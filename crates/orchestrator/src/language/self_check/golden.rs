//! Golden files: compare, or rewrite on request.
//!
//! The catalog golden pins `tools/list`; the result goldens pin what a model receives from
//! `tools/call` for one success, one refusal, one find of repeated controls, one refusal of each
//! kind of stale handle and one call on a channel a policy turned off. Volatile values (handles, times) are replaced
//! by a placeholder so the files stay stable between runs. The audit line goldens reuse the same
//! comparison with their own update variable.

use serde_json::{json, Value};

/// The environment variable that rewrites the model-surface goldens instead of comparing them.
pub(crate) const UPDATE_VARIABLE: &str = "GHOSTLIGHT_CATALOG_UPDATE";

/// The exact `tools` array of `tools/list`, pretty-printed, relative to the repository root.
pub(crate) const CATALOG_GOLDEN: &str = "docs/1.0/catalog.json";

/// The complete `tools/call` result of one successful call, relative to the repository root.
pub(crate) const SUCCESS_GOLDEN: &str = "tests/golden/model/success.json";

/// The complete `tools/call` result of one `browser_find` whose matches repeat a role and name,
/// relative to the repository root.
pub(crate) const REPEATED_FIND_GOLDEN: &str = "tests/golden/model/find_repeated.json";

/// The complete `tools/call` result of one refused call, relative to the repository root.
pub(crate) const REFUSAL_GOLDEN: &str = "tests/golden/model/refusal.json";

/// The complete `tools/call` result of a call on a channel a policy turned off, relative to the
/// repository root.
pub(crate) const CHANNEL_REFUSAL_GOLDEN: &str = "tests/golden/model/channel_refusal.json";

/// The complete `tools/call` results of a click on a stale handle, one per word a stale handle can
/// give, relative to the repository root.
pub(crate) const STALE_REFUSALS_GOLDEN: &str = "tests/golden/model/stale_refusals.json";

/// The placeholder that stands for a value that differs on every run.
const VOLATILE_PLACEHOLDER: &str = "<volatile>";

/// Keys whose values are minted per run: handles, times and the resume token.
const VOLATILE_KEYS: [&str; 7] = [
    "invocation",
    "tab",
    "target",
    "replacement",
    "resume",
    "at",
    "timestamp",
];

/// The prefix of a target handle.
const TARGET_HANDLE_PREFIX: &str = crate::language::HandleKind::Target.prefix();

/// A path relative to the repository root, resolved from this crate.
pub(crate) fn repository_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// A pretty-printed golden: the value, then exactly one newline.
pub(crate) fn golden_text(value: &impl serde::Serialize) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("golden serializes");
    text.push('\n');
    text
}

/// Replace every volatile value with a fixed placeholder and keep every key.
fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                if VOLATILE_KEYS.contains(&key.as_str()) && !inner.is_null() {
                    *inner = Value::String(VOLATILE_PLACEHOLDER.into());
                } else {
                    normalize(inner);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize),
        Value::String(text) => *text = mask_target_handles(text),
        _ => {}
    }
}

/// The text with every target handle a sentence or a step quotes replaced by the placeholder.
fn mask_target_handles(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(TARGET_HANDLE_PREFIX) {
        let (before, found) = rest.split_at(at);
        masked.push_str(before);
        let suffix = &found[TARGET_HANDLE_PREFIX.len()..];
        let length = suffix
            .bytes()
            .take_while(|byte| byte.is_ascii_lowercase() || (b'2'..=b'7').contains(byte))
            .count();
        if length == crate::language::handle::WORKSPACE_SUFFIX_CHARS {
            masked.push_str(VOLATILE_PLACEHOLDER);
            rest = &suffix[length..];
        } else {
            masked.push_str(TARGET_HANDLE_PREFIX);
            rest = suffix;
        }
    }
    masked.push_str(rest);
    masked
}

/// Compare `actual` with the golden file, or rewrite the file when the update variable is set.
pub(crate) fn check_golden(relative: &str, actual: &str) {
    check_golden_with(relative, actual, UPDATE_VARIABLE, "the model surface");
}

/// Compare `actual` with the golden file, or rewrite it when `variable` is set.
///
/// `subject` names what the file pins, for the failure message.
pub(crate) fn check_golden_with(relative: &str, actual: &str, variable: &str, subject: &str) {
    let path = repository_path(relative);
    if std::env::var_os(variable).is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("golden directory is writable");
        }
        std::fs::write(&path, actual).expect("golden is writable");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    if expected == actual {
        return;
    }
    let line = expected
        .lines()
        .zip(actual.lines())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
    panic!(
        "{relative} differs from {subject} at line {}:
  file:   {}
  actual: {}
run the test with {variable}=1 to rewrite it when the change is deliberate",
        line + 1,
        expected.lines().nth(line).unwrap_or("<end of file>"),
        actual.lines().nth(line).unwrap_or("<end of file>"),
    );
}

/// The `tools/call` result the latest MCP revision gives a client for `envelope`.
///
/// The call is made on a real `McpSession`, so the result is rendered by the product's own
/// renderer, not rebuilt here.
pub(crate) fn rendered_result(
    tool: &str,
    arguments: Value,
    envelope: &crate::work::result::InvocationResult,
) -> Value {
    use crate::intake::mcp::{McpSession, Step, LATEST_REVISION};
    use crate::intake::session::{ServiceRequest, ServiceResponse};

    let mut session = McpSession::new();
    session.accept(
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": LATEST_REVISION,
                "clientInfo": {"name": "golden client", "version": "1"},
                "capabilities": {}
            }
        })
        .to_string(),
    );
    session.accept(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let steps = session.accept(
        &json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}
        })
        .to_string(),
    );
    let id = steps
        .into_iter()
        .find_map(|step| match step {
            Step::Forward(ServiceRequest::Invoke { id, .. }) => Some(id),
            _ => None,
        })
        .expect("the call is forwarded to the service");
    let lines = session.render(ServiceResponse::Result {
        id,
        text: envelope.model_text(),
        result: serde_json::to_value(envelope).expect("envelope serializes"),
        is_error: envelope.is_error(),
        content: Vec::new(),
    });
    let mut result = lines[0]["result"].clone();
    normalize(&mut result);
    result
}
