//! Script evaluation in the page's main world.
//!
//! A script can do anything the page can, so a script that threw is not a script that did
//! nothing: the browser may already have acted before the exception. The service reports it as
//! a script that threw ([`BrowserError::ScriptThrew`]), an unknown effect that keeps the
//! description the page gave, so the agent can read what its script threw.
//!
//! The page evaluates in REPL mode, which awaits its own result envelope and not a promise that
//! is the script's value, so a script that is one expression and evaluates to a promise would
//! read as `{}`. The service sends such a script awaited. It decides this by reading the text
//! itself, never by asking the page first, so the script still reaches the page at most once.

use super::super::link::{Answer, Link};
use super::super::scope::Scope;
use super::super::wire::EvaluateMain;
use crate::browser::adapter::{BrowserOutcome, ScriptForm};
use crate::browser::BrowserError;
use crate::language::SCRIPT_MAX_CHARS;

/// How long the page may settle after a script, in milliseconds.
const SETTLE_MS: u64 = 100;
/// What an awaited script is wrapped in: the text before it.
const AWAIT_OPEN: &str = "await (\n";
/// What an awaited script is wrapped in: the text after it. The line break keeps a trailing line
/// comment in the script from swallowing the parenthesis.
const AWAIT_CLOSE: &str = "\n)";
/// Words that begin a statement or declaration. A script that begins with one is not an
/// expression, and in parentheses it could become a different program (a function declaration
/// would turn into a function expression).
const STATEMENT_WORDS: &[&str] = &[
    "function", "class", "let", "const", "var", "if", "for", "while", "do", "switch", "try",
    "throw", "return", "import", "export", "with",
];
/// The word that, before `function`, makes an async function declaration.
const ASYNC_WORD: &str = "async";
/// The word that declares a function.
const FUNCTION_WORD: &str = "function";
/// The word that ends a function body early when it stands at the top of a script.
const RETURN_WORD: &str = "return";

/// Evaluate an explicit script in the top document's main world.
///
/// The scope's guard rides on the evaluation, so a page that changed since the request was
/// authorized is not touched. A script that [`single_expression`] proves to be one expression
/// is sent awaited. If the page refuses that form as invalid before it evaluates anything, the
/// script is sent once more as written, and that answer is the result. No other failure is
/// retried, so the script runs at most once.
///
/// # Errors
///
/// Returns the failure of the primitive, and [`BrowserError::ScriptThrew`] with the page's
/// description when the script threw.
pub fn evaluate(
    l: &mut Link<'_>,
    scope: &Scope,
    tab_id: u64,
    script: &str,
    max_result_chars: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let guard = Some(scope.guard());
    let request = |script: &str, timeout_ms: u64| EvaluateMain {
        tab_id,
        script: script.to_owned(),
        max_result_chars,
        guard: guard.clone(),
        settle_ms: SETTLE_MS,
        timeout_ms,
    };
    let awaited =
        single_expression(script).filter(|wrapped| wrapped.chars().count() <= SCRIPT_MAX_CHARS);
    let (evaluated, form) = match awaited {
        Some(wrapped) => {
            let timeout_ms = l.budget_ms();
            match l.send_unless_refused(request(&wrapped, timeout_ms))? {
                Answer::Done(evaluated) => (evaluated, ScriptForm::Awaited),
                Answer::Refused => {
                    let timeout_ms = l.budget_ms();
                    (l.send(request(script, timeout_ms))?, ScriptForm::AsWritten)
                }
            }
        }
        None => {
            let timeout_ms = l.budget_ms();
            (l.send(request(script, timeout_ms))?, ScriptForm::AsWritten)
        }
    };
    if let Some(description) = evaluated.exception {
        return Err(BrowserError::ScriptThrew(description));
    }
    Ok(BrowserOutcome::ScriptEvaluated {
        tab: l.tab(tab_id)?.into(),
        value: evaluated.value,
        truncated: evaluated.truncated,
        form,
        committed_urls: l.committed(),
    })
}

/// The script as an awaited expression, when a lexical scan proves it is one expression.
///
/// A valid program wrapped in parentheses either parses as the same expression or fails to
/// parse, and a failure is harmless because the page refuses it before evaluating anything. The
/// scan exists for the other case: an invalid program must never become a valid, running one.
/// `1); x(); (2`, `a)` then a line break and `;(b`, and `f( <!-- )` all parse once wrapped. So the
/// scan declines whatever it cannot read with certainty: a statement or declaration, an
/// unterminated string, template or comment, a `/` that is not a comment (it could be a regular
/// expression), HTML-style comments, line separators, a `;` or `return` at the top, and
/// brackets that do not pair.
///
/// One trailing `;` is dropped. The result is `await (` and the expression on separate lines.
fn single_expression(script: &str) -> Option<String> {
    let trimmed = script.trim_matches(is_script_space);
    let body = trimmed
        .strip_suffix(';')
        .unwrap_or(trimmed)
        .trim_matches(is_script_space);
    if body.is_empty()
        || body.starts_with("#!")
        || body.contains("<!--")
        || body.contains("-->")
        || body.contains(['\u{2028}', '\u{2029}'])
    {
        return None;
    }
    reads_as_one_expression(&body.chars().collect::<Vec<_>>())?;
    Some(format!("{AWAIT_OPEN}{body}{AWAIT_CLOSE}"))
}

/// Whether `c` is white space or a line break to the script grammar. Only these are trimmed, so
/// a character the grammar rejects is never trimmed away.
fn is_script_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{b}'
            | '\u{c}'
            | '\r'
            | ' '
            | '\u{a0}'
            | '\u{feff}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// What the scan is inside of.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Frame {
    Paren,
    Bracket,
    Brace,
    /// The `${ ... }` of a template literal.
    Interpolation,
    /// The text of a template literal.
    Template,
}

/// How far the scan is from the script's first token.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Lead {
    /// No token seen yet.
    Unseen,
    /// The first token was the word `async`.
    AfterAsync,
    /// The first tokens are past judging.
    Done,
}

impl Lead {
    /// Take the next token and say whether the script may still be one expression. A token is a
    /// word, or one punctuation character.
    fn admit(&mut self, token: &str) -> bool {
        let (next, admitted) = match *self {
            Self::Unseen if token == "{" || STATEMENT_WORDS.contains(&token) => (Self::Done, false),
            Self::Unseen if token == ASYNC_WORD => (Self::AfterAsync, true),
            Self::AfterAsync if token == FUNCTION_WORD => (Self::Done, false),
            Self::Unseen | Self::AfterAsync | Self::Done => (Self::Done, true),
        };
        *self = next;
        admitted
    }
}

/// Whether `chars` reads as exactly one expression with every bracket, string, template and
/// comment closed. See [`single_expression`] for what it declines.
fn reads_as_one_expression(chars: &[char]) -> Option<()> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut lead = Lead::Unseen;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if stack.last() == Some(&Frame::Template) {
            match c {
                '\\' => at += 1,
                '`' => {
                    stack.pop();
                }
                '$' if chars.get(at + 1) == Some(&'{') => {
                    stack.push(Frame::Interpolation);
                    at += 1;
                }
                _ => {}
            }
            at += 1;
            continue;
        }
        match c {
            c if is_script_space(c) => {}
            '/' => match chars.get(at + 1) {
                Some('/') => {
                    while chars
                        .get(at + 1)
                        .is_some_and(|next| !matches!(next, '\n' | '\r'))
                    {
                        at += 1;
                    }
                }
                Some('*') => {
                    let close = (at + 2..chars.len().saturating_sub(1))
                        .find(|&index| chars[index] == '*' && chars[index + 1] == '/')?;
                    at = close + 1;
                }
                _ => return None,
            },
            '\'' | '"' => {
                if !lead.admit("'") {
                    return None;
                }
                at = end_of_string(chars, at)?;
            }
            '`' => {
                if !lead.admit("`") {
                    return None;
                }
                stack.push(Frame::Template);
            }
            '(' | '[' | '{' => {
                if !lead.admit(&c.to_string()) {
                    return None;
                }
                stack.push(match c {
                    '(' => Frame::Paren,
                    '[' => Frame::Bracket,
                    _ => Frame::Brace,
                });
            }
            ')' | ']' | '}' => {
                let closes = matches!(
                    (c, stack.pop()?),
                    (')', Frame::Paren)
                        | (']', Frame::Bracket)
                        | ('}', Frame::Brace | Frame::Interpolation)
                );
                if !closes {
                    return None;
                }
            }
            ';' if stack.is_empty() => return None,
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let end = (at..chars.len())
                    .find(|&index| !is_word_char(chars[index]))
                    .unwrap_or(chars.len());
                let word: String = chars[at..end].iter().collect();
                if !lead.admit(&word) || (word == RETURN_WORD && stack.is_empty()) {
                    return None;
                }
                at = end - 1;
            }
            other => {
                if !lead.admit(&other.to_string()) {
                    return None;
                }
            }
        }
        at += 1;
    }
    stack.is_empty().then_some(())
}

/// Whether `c` can continue a word.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The index of the closing quote of the string literal that opens at `start`, or none when it
/// has no closing quote before the end or a line break.
fn end_of_string(chars: &[char], start: usize) -> Option<usize> {
    let quote = chars[start];
    let mut at = start + 1;
    loop {
        match *chars.get(at)? {
            '\\' => at += 2,
            c if c == quote => return Some(at),
            '\n' | '\r' => return None,
            _ => at += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::single_expression;

    #[test]
    fn a_script_that_is_one_expression_is_awaited() {
        for script in [
            "fetch('/a').then(r => r.json())",
            "new Promise(r => setTimeout(() => r(42), 50))",
            "`a${b}c`",
            "x;",
            "f(\"/\")",
            "a // note",
            "({a: 1})",
            "(() => { return 1; })()",
            "`a${ {b: `c${d}`}.b }e`",
            "async () => 1",
            "/* lead */ a",
            "document.title",
        ] {
            let wrapped = single_expression(script).unwrap_or_else(|| panic!("wraps {script:?}"));
            assert!(wrapped.starts_with("await (\n"), "{script:?}");
            assert!(wrapped.ends_with("\n)"), "{script:?}");
        }
        assert_eq!(
            single_expression("  x ;  ").as_deref(),
            Some("await (\nx\n)")
        );
        assert_eq!(
            single_expression("a // note").as_deref(),
            Some("await (\na // note\n)"),
            "the line break keeps the comment from eating the parenthesis"
        );
    }

    #[test]
    fn anything_that_is_not_certainly_one_expression_is_left_as_written() {
        for script in [
            "",
            "   ;",
            "let a = 1",
            "return 1",
            "const u = 1; f(u)",
            "a / b",
            "/x/.test(s)",
            "{a: 1}",
            "/* c */ {a: 1}",
            "// c\nfunction f() {}",
            "function f() {}",
            "async function f() {}",
            "async /* c */ function f() {}",
            "class C {}",
            "#!x",
            "1); x(); (2",
            "a)\n;(b",
            "f( <!-- )",
            "a\n--> b",
            "'unterminated",
            "'line\nbreak'",
            "`${",
            "`open",
            "/* open",
            "(a",
            "a)",
            "(]",
            "[a)",
            "x;;",
            "a; b",
            "a\u{2028}b",
            "import('x')",
            "if (a) b",
            "x = 1; return x",
            "a return b",
            "`${a)`",
        ] {
            assert_eq!(single_expression(script), None, "declines {script:?}");
        }
    }
}
