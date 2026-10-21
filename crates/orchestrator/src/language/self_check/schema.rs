//! A dependency-free JSON Schema checker for the keywords the catalog uses.
//!
//! It follows JSON Schema 2020-12 for `type`, `enum`, `const`, `minLength`, `maxLength`,
//! `pattern`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `items`,
//! `minItems`, `maxItems`, `uniqueItems`, `properties`, `required`, `additionalProperties`,
//! `oneOf` (exactly one branch matches) and `anyOf` (at least one does). Annotations never decide
//! validity. Any other keyword, and any pattern outside a small regular-expression subset, is
//! reported as a violation, so a catalog that outgrows this checker fails loudly instead of
//! passing unchecked.

use std::fmt::Display;

use serde_json::Value;

/// Keywords that describe a schema and never decide whether a value is valid.
const ANNOTATIONS: &[&str] = &["description", "default", "examples", "format", "title"];

/// Every way `value` breaks `schema`, each located by a JSON Pointer; empty when it is valid.
pub(super) fn violations(schema: &Value, value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    check(schema, value, "", &mut found);
    found
}

/// Whether `value` satisfies `schema`.
pub(super) fn is_valid(schema: &Value, value: &Value) -> bool {
    violations(schema, value).is_empty()
}

fn report(found: &mut Vec<String>, at: &str, message: impl Display) {
    let at = if at.is_empty() { "/" } else { at };
    found.push(format!("{at}: {message}"));
}

fn check(schema: &Value, value: &Value, at: &str, found: &mut Vec<String>) {
    let Some(rules) = schema.as_object() else {
        report(found, at, "schema is not an object");
        return;
    };
    for (keyword, rule) in rules {
        match keyword.as_str() {
            annotation if ANNOTATIONS.contains(&annotation) => {}
            "type" => match rule.as_str() {
                Some(expected) if has_type(value, expected) => {}
                Some(expected) => report(found, at, format!("{value} is not of type {expected}")),
                None => report(found, at, format!("unsupported type rule {rule}")),
            },
            "enum" => {
                if !rule.as_array().is_some_and(|values| values.contains(value)) {
                    report(found, at, format!("{value} is not one of {rule}"));
                }
            }
            "const" => {
                if rule != value {
                    report(found, at, format!("{value} is not the constant {rule}"));
                }
            }
            "minLength" | "maxLength" => {
                if let (Some(text), Some(bound)) = (value.as_str(), rule.as_u64()) {
                    let length = text.chars().count() as u64;
                    let fits = if keyword == "minLength" {
                        length >= bound
                    } else {
                        length <= bound
                    };
                    if !fits {
                        report(
                            found,
                            at,
                            format!("length {length} breaks {keyword} {bound}"),
                        );
                    }
                }
            }
            "pattern" => {
                if let Some(text) = value.as_str() {
                    let pattern = rule.as_str().unwrap_or_default();
                    match pattern_matches(pattern, text) {
                        Ok(true) => {}
                        Ok(false) => {
                            report(found, at, format!("{value} does not match {pattern}"));
                        }
                        Err(problem) => {
                            report(
                                found,
                                at,
                                format!("unsupported pattern {pattern}: {problem}"),
                            );
                        }
                    }
                }
            }
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
                if let (Some(number), Some(bound)) = (value.as_f64(), rule.as_f64()) {
                    let fits = match keyword.as_str() {
                        "minimum" => number >= bound,
                        "maximum" => number <= bound,
                        "exclusiveMinimum" => number > bound,
                        _ => number < bound,
                    };
                    if !fits {
                        report(found, at, format!("{value} breaks {keyword} {bound}"));
                    }
                }
            }
            "items" => {
                if let Some(items) = value.as_array() {
                    for (index, item) in items.iter().enumerate() {
                        check(rule, item, &format!("{at}/{index}"), found);
                    }
                }
            }
            "minItems" | "maxItems" => {
                if let (Some(items), Some(bound)) = (value.as_array(), rule.as_u64()) {
                    let count = items.len() as u64;
                    let fits = if keyword == "minItems" {
                        count >= bound
                    } else {
                        count <= bound
                    };
                    if !fits {
                        report(found, at, format!("{count} items break {keyword} {bound}"));
                    }
                }
            }
            "uniqueItems" => {
                if let (Some(items), Some(true)) = (value.as_array(), rule.as_bool()) {
                    let repeated = items
                        .iter()
                        .enumerate()
                        .any(|(index, item)| items[..index].contains(item));
                    if repeated {
                        report(found, at, "items are not unique");
                    }
                }
            }
            "properties" => {
                if let (Some(object), Some(schemas)) = (value.as_object(), rule.as_object()) {
                    for (name, field_schema) in schemas {
                        if let Some(field) = object.get(name) {
                            check(field_schema, field, &format!("{at}/{name}"), found);
                        }
                    }
                }
            }
            "required" => {
                if let Some(object) = value.as_object() {
                    for name in rule.as_array().into_iter().flatten() {
                        let name = name.as_str().unwrap_or_default();
                        if !object.contains_key(name) {
                            report(found, at, format!("missing required field `{name}`"));
                        }
                    }
                }
            }
            "additionalProperties" => {
                if let Some(object) = value.as_object() {
                    let declared = rules.get("properties").and_then(Value::as_object);
                    for (name, field) in object {
                        if declared.is_some_and(|declared| declared.contains_key(name)) {
                            continue;
                        }
                        match rule {
                            Value::Bool(true) => {}
                            Value::Bool(false) => {
                                report(found, at, format!("unknown field `{name}`"));
                            }
                            extra => check(extra, field, &format!("{at}/{name}"), found),
                        }
                    }
                }
            }
            "oneOf" | "anyOf" => {
                let branches = rule.as_array().map(Vec::as_slice).unwrap_or_default();
                let matched = branches
                    .iter()
                    .filter(|branch| is_valid(branch, value))
                    .count();
                if matched == 0 {
                    report(found, at, format!("{value} matches no {keyword} branch"));
                } else if keyword == "oneOf" && matched > 1 {
                    report(
                        found,
                        at,
                        format!(
                            "ambiguous oneOf: {value} matches {matched} branches, not exactly one"
                        ),
                    );
                }
            }
            other => report(found, at, format!("unsupported keyword `{other}`")),
        }
    }
}

fn has_type(value: &Value, expected: &str) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "number" => value.is_number(),
        // JSON Schema counts 1.0 as an integer: the mathematical value decides, not the spelling.
        "integer" => {
            value.is_i64() || value.is_u64() || value.as_f64().is_some_and(|n| n.fract() == 0.0)
        }
        _ => false,
    }
}

/// One pattern atom: any character but a line terminator, or one literal character.
#[derive(Clone, Copy)]
enum Atom {
    Any,
    Literal(char),
}

impl Atom {
    fn admits(self, character: char) -> bool {
        match self {
            Self::Any => !matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}'),
            Self::Literal(literal) => character == literal,
        }
    }
}

/// How many times an atom may repeat, as the `?`, `*` and `+` quantifiers say.
#[derive(Clone, Copy)]
enum Repeat {
    Once,
    Optional,
    Many,
    OneOrMore,
}

/// Whether `text` contains a match for `pattern`, as ECMA-262 would decide.
///
/// Only the subset the catalog uses is supported: literal characters, `.`, a backslash before a
/// punctuation character, the `?`, `*` and `+` quantifiers, and `^` and `$` at the ends. Classes,
/// groups, alternation, counted repeats and class escapes are reported as errors.
fn pattern_matches(pattern: &str, text: &str) -> Result<bool, String> {
    let mut source: Vec<char> = pattern.chars().collect();
    let anchored_start = source.first() == Some(&'^');
    if anchored_start {
        source.remove(0);
    }
    let escaped_end = source.len() >= 2 && source[source.len() - 2] == '\\';
    let anchored_end = source.last() == Some(&'$') && !escaped_end;
    if anchored_end {
        source.pop();
    }
    let mut tokens: Vec<(Atom, Repeat)> = Vec::new();
    let mut characters = source.into_iter();
    while let Some(character) = characters.next() {
        let atom = match character {
            '.' => Atom::Any,
            '\\' => match characters.next() {
                Some(escaped) if !escaped.is_ascii_alphanumeric() => Atom::Literal(escaped),
                Some(escaped) => return Err(format!("escape `\\{escaped}` is not supported")),
                None => return Err("the pattern ends in a lone backslash".into()),
            },
            '?' | '*' | '+' => {
                let repeat = match character {
                    '?' => Repeat::Optional,
                    '*' => Repeat::Many,
                    _ => Repeat::OneOrMore,
                };
                match tokens.last_mut() {
                    Some((_, current @ Repeat::Once)) => {
                        *current = repeat;
                        continue;
                    }
                    _ => return Err(format!("quantifier `{character}` has nothing to repeat")),
                }
            }
            '^' | '$' | '[' | ']' | '(' | ')' | '{' | '}' | '|' => {
                return Err(format!("`{character}` is not supported here"));
            }
            literal => Atom::Literal(literal),
        };
        tokens.push((atom, Repeat::Once));
    }
    let text: Vec<char> = text.chars().collect();
    let last_start = if anchored_start { 0 } else { text.len() };
    Ok((0..=last_start).any(|start| matches_from(&tokens, &text[start..], anchored_end)))
}

fn matches_from(tokens: &[(Atom, Repeat)], text: &[char], anchored_end: bool) -> bool {
    let Some((&(atom, repeat), rest)) = tokens.split_first() else {
        return !anchored_end || text.is_empty();
    };
    let (least, most) = match repeat {
        Repeat::Once => (1, 1),
        Repeat::Optional => (0, 1),
        Repeat::Many => (0, usize::MAX),
        Repeat::OneOrMore => (1, usize::MAX),
    };
    let available = text
        .iter()
        .take(most)
        .take_while(|character| atom.admits(**character))
        .count();
    (least..=available)
        .rev()
        .any(|taken| matches_from(rest, &text[taken..], anchored_end))
}
