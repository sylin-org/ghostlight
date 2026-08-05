// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Script-step reference resolution (ADR-0035 Decision 2, PINS.md SS6).
//!
//! [`resolve_refs`] walks a step's arguments immutably and returns a NEW [`Value`] with every
//! `$prev` / `$N` reference substituted from a prior step's structured result (ADR-0038
//! `structuredContent`), or a corrective error string naming the unresolved reference and the
//! `$$` escape. Only STRING leaves are inspected; numbers, booleans, arrays, and objects recurse.
//! References resolve against structure ONLY -- rendered text is never parsed.
//!
//! Grammar: a string is a reference when it matches `^\$(prev|[1-9][0-9]*)(\.[^.]+)*$` -- a head
//! (`$prev` for the previous step, `$N` for step N, 1-indexed) followed by zero or more dot-separated
//! path segments. A numeric segment indexes an array; any other segment names an object key. A bare
//! `$prev`/`$N` substitutes the whole structured value. A leading `$$` is the literal-`$` escape
//! (so `"$$1.50"` becomes `"$1.50"`); any other `$`-string that does not match the grammar passes
//! through unchanged.

use serde_json::Value;

/// Resolve every `$prev`/`$N` reference in `args` against `structured` (where `structured[i]` is
/// step `i+1`'s `structuredContent`, or `None` if that step failed, was skipped, or its tool
/// declares no vocabulary). Returns the new args `Value`, or an `Err` carrying the corrective
/// message for the first unresolvable reference.
pub(crate) fn resolve_refs(args: &Value, structured: &[Option<Value>]) -> Result<Value, String> {
    resolve_value(args, structured)
}

fn resolve_value(value: &Value, structured: &[Option<Value>]) -> Result<Value, String> {
    match value {
        Value::String(s) => resolve_string(s, structured),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_value(item, structured)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), resolve_value(v, structured)?);
            }
            Ok(Value::Object(out))
        }
        // Numbers, booleans, and null carry no reference syntax -- pass through unchanged.
        other => Ok(other.clone()),
    }
}

/// A compiled-ish view of a parsed reference: which step it targets and its dot-path segments.
struct ParsedRef {
    step: usize, // 1-indexed
    is_prev: bool,
    path: Vec<String>,
}

fn parse_ref(s: &str) -> Option<ParsedRef> {
    // The head is `$prev` or `$<digits starting 1-9>`. `$0` does NOT match (index must be >= 1).
    let body = s.strip_prefix('$')?;
    let (step, is_prev, rest) = if let Some(rest) = body.strip_prefix("prev") {
        // `$prev` -- the previous step, resolved by the caller against `structured.len()`.
        (0, true, rest)
    } else {
        // `$N` where N is [1-9][0-9]*. A leading '0' (or non-digit) is not a valid index, so the
        // whole string is not a reference and passes through unchanged.
        let mut digits = String::new();
        let mut chars = body.chars();
        let first = chars.next()?;
        if !('1'..='9').contains(&first) {
            return None;
        }
        digits.push(first);
        for c in chars.by_ref() {
            if c.is_ascii_digit() {
                digits.push(c);
            } else {
                // The path begins here; re-attach c and the rest.
                let remainder = format!("{c}{}", chars.as_str());
                let step: usize = digits.parse().ok()?;
                let path = parse_path(&remainder);
                return Some(ParsedRef {
                    step,
                    is_prev: false,
                    path,
                });
            }
        }
        let step: usize = digits.parse().ok()?;
        return Some(ParsedRef {
            step,
            is_prev: false,
            path: Vec::new(),
        });
    };
    // Reached only for the `$prev` head: any remainder must be a dot-path.
    let path = if rest.is_empty() {
        Vec::new()
    } else {
        parse_path(rest)
    };
    Some(ParsedRef {
        step,
        is_prev,
        path,
    })
}

/// Split `.a.b.0` (with a leading dot) into `["a", "b", "0"]`. The grammar guarantees segments are
/// non-empty and dot-separated; an empty segment would mean the regex did not match.
fn parse_path(rest: &str) -> Vec<String> {
    rest.split('.')
        .filter(|seg| !seg.is_empty())
        .map(str::to_string)
        .collect()
}

fn resolve_string(s: &str, structured: &[Option<Value>]) -> Result<Value, String> {
    // Leading `$$` is the literal-`$` escape: replace and stop (no further reference processing).
    if let Some(rest) = s.strip_prefix("$$") {
        return Ok(Value::String(format!("${rest}")));
    }
    // Only a string that is EXACTLY a reference (head + optional dot-path) resolves. A `$`-string
    // that does not match the grammar (e.g. "$hello", "$0.x") passes through unchanged.
    let Some(parsed) = parse_ref(s) else {
        return Ok(Value::String(s.to_string()));
    };

    // Resolve the target step index (1-indexed). `$prev` targets the most-recent step.
    let target = if parsed.is_prev {
        structured.len()
    } else {
        parsed.step
    };
    if target == 0 || target > structured.len() {
        let ran = structured.len();
        return Err(format!(
            "unresolved reference \"{s}\": references step {target}, but only {ran} step{} ha{} run",
            if ran == 1 { "" } else { "s" },
            if ran == 1 { "s" } else { "ve" },
        ));
    }
    let Some(source) = &structured[target - 1] else {
        return Err(format!(
            "unresolved reference \"{s}\": step {target} has no structured result; only tools with a declared result vocabulary can be referenced"
        ));
    };

    // A bare `$prev`/`$N` (no path) substitutes the whole structured value.
    if parsed.path.is_empty() {
        return Ok(source.clone());
    }
    let mut current = source;
    for seg in &parsed.path {
        current = match current {
            Value::Object(map) => {
                if let Some(v) = map.get(seg) {
                    v
                } else {
                    return Err(format!(
                        "unresolved reference \"{s}\": step {target} has no field \"{seg}\". If you meant a literal string starting with \"$\", write \"$${}\".",
                        s.strip_prefix('$').unwrap_or(s)
                    ));
                }
            }
            Value::Array(items) => {
                let Ok(idx) = seg.parse::<usize>() else {
                    return Err(format!(
                        "unresolved reference \"{s}\": step {target} has no field \"{seg}\". If you meant a literal string starting with \"$\", write \"$${}\".",
                        s.strip_prefix('$').unwrap_or(s)
                    ));
                };
                let Some(v) = items.get(idx) else {
                    return Err(format!(
                        "unresolved reference \"{s}\": step {target} array index {idx} is out of bounds. If you meant a literal string starting with \"$\", write \"$${}\".",
                        s.strip_prefix('$').unwrap_or(s)
                    ));
                };
                v
            }
            _ => {
                return Err(format!(
                    "unresolved reference \"{s}\": step {target} has no field \"{seg}\". If you meant a literal string starting with \"$\", write \"$${}\".",
                    s.strip_prefix('$').unwrap_or(s)
                ));
            }
        };
    }
    Ok(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_prev_path() {
        // `$prev.results.0.ref` over the previous step's structured result.
        let args = json!({"ref": "$prev.results.0.ref"});
        let structured = vec![Some(
            json!({"results": [{"ref": "ref_12", "x": 5}], "more": false}),
        )];
        assert_eq!(
            resolve_refs(&args, &structured).unwrap(),
            json!({"ref": "ref_12"})
        );
    }

    #[test]
    fn double_dollar_escapes() {
        // `$$` -> a single literal `$`; the remainder is untouched.
        assert_eq!(resolve_refs(&json!("$$1.50"), &[]).unwrap(), json!("$1.50"));
    }

    #[test]
    fn non_grammar_dollar_passes_through() {
        // `$hello` does not match the reference grammar -> unchanged.
        assert_eq!(
            resolve_refs(&json!("$hello"), &[]).unwrap(),
            json!("$hello")
        );
    }

    #[test]
    fn money_value_errors_with_escape_hint() {
        // `$1.50` parses as step 1, path `50`; step 1's structured result has no field `50`.
        let args = json!("$1.50");
        let structured = vec![Some(json!({"tabId": 3}))];
        let err = resolve_refs(&args, &structured).unwrap_err();
        assert!(
            err.contains(r#"unresolved reference "$1.50": step 1 has no field "50". If you meant a literal string starting with "$", write "$$1.50"."#),
            "got: {err}"
        );
    }

    #[test]
    fn forward_reference_errors() {
        // `$2.tabId` when only 1 step has run.
        let err = resolve_refs(&json!("$2.tabId"), &[Some(json!({"tabId": 3}))]).unwrap_err();
        assert!(
            err.contains("references step 2, but only 1 step has run"),
            "got: {err}"
        );
    }

    #[test]
    fn unstructured_step_errors() {
        // `$prev.tabId` when the previous step has NO structured result.
        let err = resolve_refs(&json!("$prev.tabId"), &[None]).unwrap_err();
        assert!(
            err.contains("has no structured result; only tools with a declared result vocabulary can be referenced"),
            "got: {err}"
        );
    }

    #[test]
    fn zero_index_passes_through() {
        // `$0.x` is not grammar (index must be >= 1) -> unchanged.
        assert_eq!(resolve_refs(&json!("$0.x"), &[]).unwrap(), json!("$0.x"));
    }

    #[test]
    fn bare_prev_substitutes_whole_structured_value() {
        // A bare `$prev` (no path) substitutes the whole structured value.
        let args = json!({"tabId": "$prev"});
        let structured = vec![Some(json!({"tabId": 7}))];
        assert_eq!(
            resolve_refs(&args, &structured).unwrap(),
            json!({"tabId": {"tabId": 7}})
        );
    }

    #[test]
    fn non_string_leaves_pass_through() {
        // Numbers, booleans, arrays, nested objects recurse but primitives are never references.
        let args = json!({"n": 5, "b": true, "arr": ["$prev.x", 9], "nested": {"k": "$1.tabId"}});
        let structured = vec![Some(json!({"x": 1, "tabId": 42}))];
        let out = resolve_refs(&args, &structured).unwrap();
        assert_eq!(
            out,
            json!({"n": 5, "b": true, "arr": [1, 9], "nested": {"k": 42}})
        );
    }
}
