//! Small phrasing helpers every language module shares: counts, articles and page text bounded
//! to one line. They only shape words; what a sentence says stays in its own module.

/// What a sentence says for a field that holds nothing.
pub(super) const NO_VALUE: &str = "no value";

/// Page-authored text as one bounded line: controls and direction overrides become spaces,
/// double quotes become single ones, and anything over `max_chars` ends in an ellipsis.
pub(super) fn one_line_bounded(value: &str, max_chars: usize) -> Option<String> {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                ' '
            } else if character == '"' {
                '\''
            } else {
                character
            }
        })
        .collect();
    let one_line = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    let count = one_line.chars().count();
    if count <= max_chars {
        return Some(one_line);
    }
    let mut bounded: String = one_line.chars().take(max_chars - 3).collect();
    bounded.push_str("...");
    Some(bounded)
}

/// The indefinite article for a noun: "an" before a vowel letter, "a" otherwise.
pub(super) const fn article(noun: &str) -> &'static str {
    match noun.as_bytes().first() {
        Some(b'a' | b'e' | b'i' | b'o' | b'u') => "an",
        _ => "a",
    }
}

/// A count and its noun, singular or plural, with the digits grouped by thousands.
pub(super) fn counted(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{} {noun}", grouped(count))
}

/// "no policy layers" rather than "0 policy layers", where an absence is the usual case.
pub(super) fn counted_or_none(count: usize, singular: &str, plural: &str) -> String {
    if count == 0 {
        format!("no {plural}")
    } else {
        counted(count, singular, plural)
    }
}

fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut rendered = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            rendered.push(',');
        }
        rendered.push(digit);
    }
    rendered
}
