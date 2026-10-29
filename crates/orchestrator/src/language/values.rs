//! Reading a call's values the way CSS reads a declaration (ADR-0205): a number, a number with its
//! unit, a keyword, or nothing at all.
//!
//! One pass runs before typed decoding. It rewrites each value it can read into the catalog's
//! canonical form, fills in a value the call's other fields imply, and drops a field that does not
//! apply. What it changed or left out is kept in a [`Reading`], which the result reports, so the
//! agent sees how its call was read. A value the pass cannot read is left as sent, and the typed
//! decoding refuses it in the catalog's words. Authority and content are never read generously:
//! `user_authorized_credentials`, typed text, form values, scripts and paths pass through as given.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::de::{Deserialize, Deserializer, Error as _};
use serde_json::{json, Map, Number, Value};

use super::bounds::{
    Range, DIAGNOSTIC_LIMIT, DOCUMENT_DEPTH, DURATION_MS, FIND_RESULTS, INSPECT_ITEMS, READ_CHARS,
    SCRIPT_RESULT_CHARS, WHEEL_TICKS,
};
use super::{
    LanguageError, ScrollAmount, ScrollDirection, Tool, WaitCondition, MAX_TIMEOUT_MS,
    MIN_TIMEOUT_MS, NAMED_KEYS,
};

/// The result fact that lists the fields Ghostlight did not use.
pub const IGNORED_FACT: &str = "ignored";
/// The result fact that maps each field Ghostlight read differently to the value it used.
pub const ADJUSTED_FACT: &str = "adjusted";

/// How one call was read: the fields left out and the values used instead of the ones sent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    ignored: Vec<String>,
    adjusted: Map<String, Value>,
}

impl Reading {
    fn ignore(&mut self, field: &str) {
        if !self.ignored.iter().any(|known| known == field) {
            self.ignored.push(field.into());
        }
    }

    fn adjust(&mut self, field: &str, value: Value) {
        self.adjusted.insert(field.into(), value);
    }

    /// Add this reading to a result's facts: `ignored` and `adjusted`, each only when it has
    /// something to say.
    pub fn report(&self, facts: &mut Value) {
        let Value::Object(object) = facts else {
            return;
        };
        if !self.ignored.is_empty() {
            object.insert(IGNORED_FACT.into(), json!(self.ignored));
        }
        if !self.adjusted.is_empty() {
            object.insert(ADJUSTED_FACT.into(), Value::Object(self.adjusted.clone()));
        }
    }
}

/// How far a page scroll goes: a step word, pixels, a share of the scroller's own size, or all the
/// way to its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDistance {
    /// One of the step words.
    Step(ScrollAmount),
    /// A number of CSS pixels.
    Pixels(u32),
    /// A percentage of the scroller's visible size.
    Percent(u32),
    /// As far as the scroller goes.
    End,
}

/// The word for a scroll that goes all the way.
const END_WORD: &str = "end";
/// The share of the visible size each step word moves, as the page runtime applies it.
const STEP_SHARE: [(ScrollAmount, f64); 4] = [
    (ScrollAmount::Small, 0.25),
    (ScrollAmount::Medium, 0.5),
    (ScrollAmount::Large, 0.75),
    (ScrollAmount::Page, 0.95),
];
/// About how many wheel ticks move one viewport, for a distance asked of a wheel at a point.
const VIEWPORT_TICKS: f64 = 8.0;
/// About how many pixels one wheel tick moves.
const TICK_PIXELS: f64 = 100.0;
/// The forms a scroll distance takes, for the refusal of one that reads as none of them.
const DISTANCE_FORMS: &str = "a number of pixels (600 or \"600px\"), a percentage of the view (\"50%\"), small, medium, large, page, top, bottom, or end";

impl ScrollDistance {
    /// The form the page runtime reads.
    #[must_use]
    pub fn wire(self) -> String {
        match self {
            Self::Step(step) => step.as_str().into(),
            Self::Pixels(pixels) => format!("{pixels}px"),
            Self::Percent(percent) => format!("{percent}%"),
            Self::End => END_WORD.into(),
        }
    }

    /// The distance `text` names, in any case and with or without its unit.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase();
        if text == END_WORD {
            return Some(Self::End);
        }
        if let Ok(step) = ScrollAmount::named(&text) {
            return Some(Self::Step(step));
        }
        if let Some(percent) = text.strip_suffix('%') {
            return whole(percent.trim()).map(Self::Percent);
        }
        whole(text.strip_suffix("px").unwrap_or(&text).trim()).map(Self::Pixels)
    }

    /// The wheel ticks that move about this far.
    fn ticks(self) -> u64 {
        let ticks = match self {
            Self::Step(step) => {
                STEP_SHARE
                    .iter()
                    .find(|(known, _)| *known == step)
                    .map_or(0.5, |(_, share)| *share)
                    * VIEWPORT_TICKS
            }
            Self::Pixels(pixels) => f64::from(pixels) / TICK_PIXELS,
            Self::Percent(percent) => f64::from(percent) / 100.0 * VIEWPORT_TICKS,
            Self::End => f64::MAX,
        };
        // Clamped to the small whole tick bounds, so the cast is exact.
        ticks
            .round()
            .clamp(WHEEL_TICKS.min as f64, WHEEL_TICKS.max as f64) as u64
    }
}

impl<'de> Deserialize<'de> for ScrollDistance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let written = Value::deserialize(deserializer)?;
        let distance = match &written {
            Value::String(text) => Self::parse(text),
            Value::Number(number) => number
                .as_u64()
                .and_then(|pixels| u32::try_from(pixels).ok())
                .map(Self::Pixels),
            _ => None,
        };
        distance.ok_or_else(|| {
            D::Error::custom(format!("amount must be {DISTANCE_FORMS}: got {written}"))
        })
    }
}

/// A whole number written as digits, perhaps with a fraction that rounds away.
fn whole(text: &str) -> Option<u32> {
    rounded(text.parse().ok()?)
}

/// `number` rounded to a whole `u32`, when it is finite, not negative and in range.
fn rounded(number: f64) -> Option<u32> {
    // Checked to be finite and within u32 before the cast.
    (number.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&number))
        .then(|| number.round() as u32)
}

/// What the catalog advertises for one tool, read once from the catalog itself.
struct Advertised {
    fields: Vec<String>,
    required: Vec<String>,
    words: HashMap<String, Vec<String>>,
}

fn advertised(tool: Tool) -> &'static Advertised {
    static ALL: OnceLock<HashMap<String, Advertised>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        super::catalog()
            .into_iter()
            .map(|definition| {
                let schema = definition.input_schema;
                let properties = schema["properties"]
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                let words = properties
                    .iter()
                    .filter_map(|(field, property)| {
                        let words = property["enum"].as_array()?;
                        Some((
                            field.clone(),
                            words
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect(),
                        ))
                    })
                    .collect();
                let required = schema["required"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                (
                    definition.name,
                    Advertised {
                        fields: properties.keys().cloned().collect(),
                        required,
                        words,
                    },
                )
            })
            .collect()
    });
    &all[tool.name()]
}

/// The fields `tool` takes, as the catalog advertises them.
pub(super) fn fields_of(tool: Tool) -> &'static [String] {
    &advertised(tool).fields
}

/// Arguments that were removed from the catalog and still get their own refusal, so they are
/// never dropped as unknown.
const RETIRED_FIELDS: &[&str] = &["restrict_hosts", "restrict_capabilities", "dry_run"];

/// Limits on how much a call reads, brought inside their bounds rather than refused.
const READ_LIMITS: &[(&str, Range)] = &[
    ("max_chars", READ_CHARS),
    ("max_results", FIND_RESULTS),
    ("max_items", INSPECT_ITEMS),
    ("max_depth", DOCUMENT_DEPTH),
    ("limit", DIAGNOSTIC_LIMIT),
    ("max_result_chars", SCRIPT_RESULT_CHARS),
];
/// Counts of actions, read as numbers and kept to their bounds by the decoder.
const COUNTS: &[&str] = &["repeat", "ticks", "click_count", "percent"];
/// Lengths in pixels.
const LENGTHS: &[&str] = &[
    "x", "y", "width", "height", "start_x", "start_y", "end_x", "end_y",
];
/// Booleans. `user_authorized_credentials` is not one of them: it states the user's permission
/// and takes only `true` as sent.
const FLAGS: &[&str] = &[
    "visual_settle",
    "full_page",
    "new_tab",
    "clear_first",
    "focused",
    "bypass_cache",
    "download",
];
/// Fields that take one closed word.
const WORDS: &[&str] = &[
    "action",
    "direction",
    "mode",
    "scope",
    "condition",
    "detail",
    "source",
    "on_error",
    "reuse",
    "beforeunload",
    "button",
];
/// Other names for pointer buttons.
const BUTTON_NAMES: &[(&str, &str)] = &[("left", "primary"), ("right", "secondary")];
/// Other names for modifier keys.
const MODIFIER_NAMES: &[(&str, &str)] = &[
    ("alt", "Alt"),
    ("option", "Alt"),
    ("opt", "Alt"),
    ("control", "Control"),
    ("ctrl", "Control"),
    ("shift", "Shift"),
    ("meta", "Meta"),
    ("cmd", "Meta"),
    ("command", "Meta"),
    ("win", "Meta"),
    ("windows", "Meta"),
    ("super", "Meta"),
];
/// Other names for named keys, compared without case, spaces, dashes or underscores.
const KEY_NAMES: &[(&str, &str)] = &[
    ("esc", "Escape"),
    ("return", "Enter"),
    ("up", "ArrowUp"),
    ("down", "ArrowDown"),
    ("left", "ArrowLeft"),
    ("right", "ArrowRight"),
    ("pgup", "PageUp"),
    ("pgdn", "PageDown"),
    ("pagedn", "PageDown"),
    ("del", "Delete"),
    ("spacebar", "Space"),
];
/// A duration wait's default unit is the millisecond; these suffixes say otherwise.
const SECOND_SUFFIXES: &[&str] = &["seconds", "second", "secs", "sec", "s"];
/// Hosts an address without a scheme reaches over plain HTTP, as the address bar does.
const LOOPBACK_HOSTS: &[&str] = &["localhost", "127.0.0.1", "[::1]", "0.0.0.0"];

/// Read one call's JSON as ADR-0205 says, before typed decoding.
///
/// # Errors
///
/// A required field that nothing in the call implies is refused with the words it takes.
pub(super) fn read_call(
    tool: Tool,
    input: Value,
    reading: &mut Reading,
) -> Result<Value, LanguageError> {
    let Value::Object(mut call) = input else {
        return Ok(input);
    };
    let advertised = advertised(tool);
    rename_fields(&mut call, &advertised.fields);
    for (field, value) in &mut call {
        read_value(field, value, reading);
    }
    match tool {
        Tool::Tabs => read_tabs(&mut call, reading),
        Tool::Navigate => read_navigate(&mut call, reading),
        Tool::History => drop_unless(&mut call, reading, "bypass_cache", "action", &["reload"]),
        Tool::Window => read_window(&mut call, reading),
        Tool::Read if call.contains_key("target") => drop_field(&mut call, reading, "mode"),
        Tool::Inspect => read_inspect(&mut call, reading),
        Tool::Scroll => read_scroll(&mut call, reading),
        Tool::PressKey => read_keys(&mut call, reading),
        Tool::Wait => read_wait(&mut call, reading),
        Tool::Dialog => read_dialog(&mut call, reading),
        Tool::Record => read_record(&mut call, reading),
        _ => {}
    }
    if let Some(Value::Object(expect)) = call.get_mut("expect") {
        read_expect(expect, reading);
    }
    if only_observes(tool, &call) {
        let unknown: Vec<String> = call
            .keys()
            .filter(|field| {
                !advertised.fields.contains(field) && !RETIRED_FIELDS.contains(&field.as_str())
            })
            .cloned()
            .collect();
        for field in unknown {
            drop_field(&mut call, reading, &field);
        }
    }
    if let Some(missing) = advertised
        .required
        .iter()
        .find(|field| !call.contains_key(field.as_str()))
    {
        let words = advertised
            .words
            .get(missing)
            .map(|words| format!(": {}", words.join(", ")))
            .unwrap_or_default();
        return Err(LanguageError::Invalid(format!(
            "{} needs {missing}{words}",
            tool.name()
        )));
    }
    Ok(Value::Object(call))
}

/// Whether `tool` only observes, so a field it does not know is dropped rather than refused.
fn only_observes(tool: Tool, call: &Map<String, Value>) -> bool {
    match tool {
        Tool::Read
        | Tool::Find
        | Tool::Inspect
        | Tool::Screenshot
        | Tool::Wait
        | Tool::Diagnose
        | Tool::PolicyExplain => true,
        Tool::Tabs => call.get("action").and_then(Value::as_str) == Some("list"),
        _ => false,
    }
}

/// Rename a field written in camel case or kebab case to the snake case the tool takes.
fn rename_fields(call: &mut Map<String, Value>, fields: &[String]) {
    let renames: Vec<(String, String)> = call
        .keys()
        .filter(|field| !fields.contains(field))
        .filter_map(|field| {
            let snake = snake_case(field);
            (fields.contains(&snake) && !call.contains_key(&snake)).then(|| (field.clone(), snake))
        })
        .collect();
    for (from, to) in renames {
        if let Some(value) = call.remove(&from) {
            call.insert(to, value);
        }
    }
}

fn snake_case(field: &str) -> String {
    let mut snake = String::with_capacity(field.len() + 4);
    for (index, character) in field.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 && !snake.ends_with('_') {
                snake.push('_');
            }
            snake.push(character.to_ascii_lowercase());
        } else if character == '-' || character == ' ' {
            snake.push('_');
        } else {
            snake.push(character);
        }
    }
    snake
}

/// Read one top-level value by the kind its field holds.
fn read_value(field: &str, value: &mut Value, reading: &mut Reading) {
    if field == "timeout_ms" {
        if let Some(milliseconds) = duration_ms(value) {
            let bounded = milliseconds.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);
            if bounded != milliseconds {
                reading.adjust(field, json!(bounded));
            }
            *value = json!(bounded);
        }
    } else if let Some((_, range)) = READ_LIMITS.iter().find(|(name, _)| *name == field) {
        if let Some(count) = count(value) {
            let bounded = count.clamp(range.min as u64, range.max as u64);
            if bounded != count {
                reading.adjust(field, json!(bounded));
            }
            *value = json!(bounded);
        }
    } else if COUNTS.contains(&field) {
        if let Some(count) = count(value) {
            *value = json!(count);
        }
    } else if LENGTHS.contains(&field) {
        if let Some(length) = length(value) {
            *value = length;
        }
    } else if FLAGS.contains(&field) {
        if let Some(flag) = flag(value) {
            *value = Value::Bool(flag);
        }
    } else if WORDS.contains(&field) {
        if let Some(word) = word(value) {
            let word = if field == "button" {
                renamed(&word, BUTTON_NAMES).unwrap_or(word)
            } else {
                word
            };
            *value = Value::String(word);
        }
    } else if field == "modifiers" {
        read_modifiers(value, reading);
    } else if field == "paths" {
        if let Value::Array(paths) = value {
            let before = paths.len();
            let mut seen = Vec::new();
            paths.retain(|path| {
                let fresh = !seen.contains(path);
                seen.push(path.clone());
                fresh
            });
            if paths.len() != before {
                reading.adjust(field, value.clone());
            }
        }
    }
}

/// A duration in milliseconds: a number, digits, or digits with `ms` or a seconds suffix.
fn duration_ms(value: &Value) -> Option<u64> {
    let (number, seconds) = match value {
        Value::Number(number) => (number.as_f64()?, false),
        Value::String(text) => {
            let text = text.trim().to_ascii_lowercase();
            if let Some(milliseconds) = text.strip_suffix("ms") {
                (milliseconds.trim().parse().ok()?, false)
            } else if let Some(seconds) = SECOND_SUFFIXES
                .iter()
                .find_map(|suffix| text.strip_suffix(suffix))
            {
                (seconds.trim().parse().ok()?, true)
            } else {
                (text.parse().ok()?, false)
            }
        }
        _ => return None,
    };
    let milliseconds = if seconds { number * 1000.0 } else { number };
    rounded(milliseconds).map(u64::from)
}

/// A whole count: a number or digits, and a percentage sign is allowed.
fn count(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64().or_else(|| {
            let float = number.as_f64()?;
            (float.fract() == 0.0)
                .then(|| rounded(float))
                .flatten()
                .map(u64::from)
        }),
        Value::String(text) => {
            let text = text.trim();
            whole(text.strip_suffix('%').unwrap_or(text).trim()).map(u64::from)
        }
        _ => None,
    }
}

/// A length in pixels: a number, digits, or digits with `px`.
fn length(value: &Value) -> Option<Value> {
    let Value::String(text) = value else {
        return None;
    };
    let text = text.trim().to_ascii_lowercase();
    let number: f64 = text
        .strip_suffix("px")
        .unwrap_or(&text)
        .trim()
        .parse()
        .ok()?;
    if !number.is_finite() {
        return None;
    }
    match rounded(number) {
        Some(whole) if number.fract() == 0.0 => Some(json!(whole)),
        _ => Number::from_f64(number).map(Value::Number),
    }
}

fn flag(value: &Value) -> Option<bool> {
    match value {
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// A closed word in the catalog's spelling: lower case, with spaces and dashes as underscores.
fn word(value: &Value) -> Option<String> {
    let Value::String(text) = value else {
        return None;
    };
    Some(text.trim().to_ascii_lowercase().replace([' ', '-'], "_"))
}

fn renamed(name: &str, names: &[(&str, &str)]) -> Option<String> {
    names
        .iter()
        .find(|(other, _)| *other == name)
        .map(|(_, canonical)| (*canonical).to_owned())
}

/// A modifier key in the catalog's spelling, from any case and its common names.
fn modifier(name: &str) -> Option<String> {
    renamed(&name.trim().to_ascii_lowercase(), MODIFIER_NAMES)
}

fn read_modifiers(value: &mut Value, reading: &mut Reading) {
    let Value::Array(modifiers) = value else {
        return;
    };
    let mut read = Vec::new();
    for written in modifiers.iter() {
        let Some(name) = written.as_str() else {
            return;
        };
        let canonical = modifier(name).unwrap_or_else(|| name.to_owned());
        if !read.contains(&canonical) {
            read.push(canonical);
        }
    }
    if read.len() != modifiers.len() {
        reading.adjust("modifiers", json!(read));
    }
    *value = json!(read);
}

/// A named key or a single character, from any case and the keys' common names.
fn key(name: &str) -> Option<String> {
    if name.chars().count() == 1 {
        return Some(name.to_owned());
    }
    let squashed: String = name
        .chars()
        .filter(|character| !matches!(character, ' ' | '-' | '_'))
        .collect::<String>()
        .to_ascii_lowercase();
    NAMED_KEYS
        .iter()
        .find(|named| named.to_ascii_lowercase() == squashed)
        .map(|named| (*named).to_owned())
        .or_else(|| renamed(&squashed, KEY_NAMES))
}

/// A chord such as `ctrl+a`: its modifiers and its key.
fn chord(written: &str) -> Option<(Vec<String>, String)> {
    let (held, last) = match written.strip_suffix("++") {
        Some(held) => (held, "+"),
        None => written.rsplit_once('+')?,
    };
    if held.is_empty() {
        return None;
    }
    let modifiers = held.split('+').map(modifier).collect::<Option<Vec<_>>>()?;
    Some((modifiers, key(last)?))
}

fn drop_field(call: &mut Map<String, Value>, reading: &mut Reading, field: &str) {
    if call.remove(field).is_some() {
        reading.ignore(field);
    }
}

/// Drop `field` unless `selector` names one of `when`.
fn drop_unless(
    call: &mut Map<String, Value>,
    reading: &mut Reading,
    field: &str,
    selector: &str,
    when: &[&str],
) {
    let applies = call
        .get(selector)
        .and_then(Value::as_str)
        .is_some_and(|chosen| when.contains(&chosen));
    if !applies {
        drop_field(call, reading, field);
    }
}

fn set(call: &mut Map<String, Value>, reading: &mut Reading, field: &str, value: Value) {
    reading.adjust(field, value.clone());
    call.insert(field.into(), value);
}

fn read_tabs(call: &mut Map<String, Value>, reading: &mut Reading) {
    if !call.contains_key("action") && !call.contains_key("tab") {
        call.insert("action".into(), json!("list"));
    }
    drop_unless(call, reading, "tab", "action", &["focus", "close"]);
}

fn read_navigate(call: &mut Map<String, Value>, reading: &mut Reading) {
    if let Some(Value::String(url)) = call.get("url") {
        if let Some(absolute) = with_scheme(url) {
            set(call, reading, "url", Value::String(absolute));
        }
    }
    let new_tab = call.get("new_tab") == Some(&Value::Bool(true));
    if !new_tab {
        drop_field(call, reading, "browser");
    }
    if new_tab || call.contains_key("browser") {
        drop_field(call, reading, "beforeunload");
    }
}

/// The address the browser's own address bar would load for `url` when it has no scheme.
fn with_scheme(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.contains("://") || url.contains(char::is_whitespace) {
        return None;
    }
    let rest = url.strip_prefix("//").unwrap_or(url);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = host
        .rsplit_once(':')
        .filter(|(_, port)| port.chars().all(|digit| digit.is_ascii_digit()))
        .map_or(host.as_str(), |(name, _)| name);
    if name.is_empty() || (!name.contains('.') && !LOOPBACK_HOSTS.contains(&name)) {
        return None;
    }
    let plain = LOOPBACK_HOSTS.contains(&name) || name.ends_with(".localhost");
    Some(format!("{}://{rest}", if plain { "http" } else { "https" }))
}

fn read_window(call: &mut Map<String, Value>, reading: &mut Reading) {
    if !call.contains_key("action") {
        if call.contains_key("percent") {
            call.insert("action".into(), json!("zoom"));
        } else if call.contains_key("width") || call.contains_key("height") {
            call.insert("action".into(), json!("resize"));
        }
    }
    drop_unless(call, reading, "percent", "action", &["zoom"]);
    drop_unless(call, reading, "width", "action", &["resize"]);
    drop_unless(call, reading, "height", "action", &["resize"]);
}

fn read_inspect(call: &mut Map<String, Value>, reading: &mut Reading) {
    let points_into = call.contains_key("root") || call.contains_key("max_depth");
    if points_into && !call.contains_key("scope") {
        call.insert("scope".into(), json!("document"));
    }
    drop_unless(call, reading, "root", "scope", &["document"]);
    drop_unless(call, reading, "max_depth", "scope", &["document"]);
}

fn read_dialog(call: &mut Map<String, Value>, reading: &mut Reading) {
    if !call.contains_key("action") {
        let action = if call.contains_key("text") {
            "respond"
        } else {
            "status"
        };
        call.insert("action".into(), json!(action));
    }
    drop_unless(call, reading, "text", "action", &["respond"]);
}

fn read_record(call: &mut Map<String, Value>, reading: &mut Reading) {
    if !call.contains_key("action") {
        call.insert("action".into(), json!("status"));
    }
    drop_unless(call, reading, "tab", "action", &["start"]);
    drop_unless(call, reading, "target", "action", &["save"]);
    drop_unless(call, reading, "download", "action", &["save"]);
}

fn opposite(direction: ScrollDirection) -> ScrollDirection {
    match direction {
        ScrollDirection::Up => ScrollDirection::Down,
        ScrollDirection::Down => ScrollDirection::Up,
        ScrollDirection::Left => ScrollDirection::Right,
        ScrollDirection::Right => ScrollDirection::Left,
    }
}

/// A scroll distance as written: how far, whether it points the other way, and the edge it
/// names, if it names one.
fn written_distance(written: &Value) -> Option<(ScrollDistance, bool, Option<ScrollDirection>)> {
    match written {
        Value::Number(number) => {
            let pixels = number.as_f64()?;
            Some((
                ScrollDistance::Pixels(rounded(pixels.abs())?),
                pixels < 0.0,
                None,
            ))
        }
        Value::String(text) => {
            let text = text.trim().to_ascii_lowercase();
            match text.as_str() {
                "top" => return Some((ScrollDistance::End, false, Some(ScrollDirection::Up))),
                "bottom" => return Some((ScrollDistance::End, false, Some(ScrollDirection::Down))),
                _ => {}
            }
            let (reversed, rest) = match text.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, text.as_str()),
            };
            Some((ScrollDistance::parse(rest)?, reversed, None))
        }
        _ => None,
    }
}

fn read_scroll(call: &mut Map<String, Value>, reading: &mut Reading) {
    if call.contains_key("target") {
        drop_field(call, reading, "direction");
        drop_field(call, reading, "amount");
        return;
    }
    let asked = call
        .get("direction")
        .and_then(Value::as_str)
        .and_then(|word| ScrollDirection::named(word).ok());
    let at_point = call.contains_key("view");
    if let Some((distance, reversed, edge)) = call.get("amount").and_then(written_distance) {
        if let Some(edge) = edge {
            if asked.is_some_and(|asked| asked != edge) {
                reading.ignore("direction");
            }
            call.insert("direction".into(), json!(edge.as_str()));
        } else if reversed {
            let flipped = opposite(asked.unwrap_or(ScrollDirection::Down));
            set(call, reading, "direction", json!(flipped.as_str()));
        }
        if at_point {
            call.remove("amount");
            if call.contains_key("ticks") {
                reading.ignore("amount");
            } else {
                set(call, reading, "ticks", json!(distance.ticks()));
            }
        } else {
            call.insert("amount".into(), json!(distance.wire()));
        }
    }
    if at_point {
        if !call.contains_key("ticks") {
            call.insert(
                "ticks".into(),
                json!(ScrollDistance::Step(ScrollAmount::Medium).ticks()),
            );
        }
    } else if let Some(ticks) = call.remove("ticks") {
        // Ticks without a view point are a page scroll of about that many wheel notches.
        if let Some(ticks) = count(&ticks) {
            if !call.contains_key("amount") {
                let pixels = ticks.saturating_mul(TICK_PIXELS as u64);
                set(call, reading, "amount", json!(format!("{pixels}px")));
            } else {
                reading.ignore("ticks");
            }
        } else {
            call.insert("ticks".into(), ticks);
        }
    }
}

fn read_keys(call: &mut Map<String, Value>, reading: &mut Reading) {
    if let Some(Value::String(written)) = call.get("key").cloned() {
        if let Some(named) = key(&written) {
            call.insert("key".into(), json!(named));
        } else if let Some((modifiers, named)) = chord(&written) {
            call.insert("key".into(), json!(named));
            let mut held: Vec<Value> = call
                .get("modifiers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for modifier in modifiers {
                if !held.contains(&json!(modifier)) {
                    held.push(json!(modifier));
                }
            }
            call.insert("modifiers".into(), Value::Array(held));
        } else if written.split_whitespace().count() > 1 && !call.contains_key("strokes") {
            let strokes: Option<Vec<String>> = written.split_whitespace().map(key).collect();
            if let Some(strokes) = strokes {
                call.remove("key");
                set(call, reading, "strokes", json!(strokes));
            }
        }
    }
    if let Some(Value::Array(strokes)) = call.get_mut("strokes") {
        for stroke in strokes.iter_mut() {
            if let Some(named) = stroke.as_str().and_then(key) {
                *stroke = json!(named);
            }
        }
    }
    // One key repeated is a sequence of one. A repeat of one is the default and changes nothing.
    let repeats = call
        .get("repeat")
        .and_then(count)
        .is_some_and(|times| times != 1);
    if repeats && !call.contains_key("strokes") {
        if let Some(key) = call.remove("key") {
            call.insert("strokes".into(), json!([key]));
        }
    }
}

/// The input fields each wait condition reads; any other is dropped.
fn wait_inputs(condition: WaitCondition) -> &'static [&'static str] {
    match condition {
        WaitCondition::LoadReady => &[],
        WaitCondition::UrlContains
        | WaitCondition::TextPresent
        | WaitCondition::TextAbsent
        | WaitCondition::Duration => &["value"],
        WaitCondition::TargetPresent
        | WaitCondition::TargetAbsent
        | WaitCondition::VisualSettle => &["target"],
        WaitCondition::SelectorPresent => &["selector"],
    }
}

/// The condition a wait's other fields imply when it names none.
fn implied_condition(call: &Map<String, Value>) -> WaitCondition {
    if call.contains_key("selector") {
        return WaitCondition::SelectorPresent;
    }
    match call.get("value") {
        Some(value) if duration_ms(value).is_some() => WaitCondition::Duration,
        Some(Value::String(text))
            if text.trim_start().starts_with("http://")
                || text.trim_start().starts_with("https://") =>
        {
            WaitCondition::UrlContains
        }
        Some(_) => WaitCondition::TextPresent,
        None if call.contains_key("target") => WaitCondition::TargetPresent,
        None => WaitCondition::LoadReady,
    }
}

fn read_wait(call: &mut Map<String, Value>, reading: &mut Reading) {
    let condition = match call.get("condition").and_then(Value::as_str) {
        Some(named) => match WaitCondition::named(named) {
            Ok(condition) => condition,
            // An unknown condition is refused in the catalog's words.
            Err(_) => return,
        },
        None => {
            let condition = implied_condition(call);
            call.insert("condition".into(), json!(condition.as_str()));
            condition
        }
    };
    for input in ["value", "target", "selector"] {
        if !wait_inputs(condition).contains(&input) {
            drop_field(call, reading, input);
        }
    }
    match call.get("value") {
        Some(value) if condition == WaitCondition::Duration => {
            if let Some(milliseconds) = duration_ms(value) {
                let bounded = milliseconds.clamp(DURATION_MS.min as u64, DURATION_MS.max as u64);
                if bounded != milliseconds {
                    reading.adjust("value", json!(bounded.to_string()));
                }
                call.insert("value".into(), json!(bounded.to_string()));
            }
        }
        Some(Value::Number(number)) => {
            let text = number.to_string();
            call.insert("value".into(), json!(text));
        }
        _ => {}
    }
}

fn read_expect(expect: &mut Map<String, Value>, reading: &mut Reading) {
    if let Some(written) = expect.get("condition").and_then(word) {
        expect.insert("condition".into(), json!(written));
    }
    if expect.get("condition").and_then(Value::as_str) == Some(WaitCondition::LoadReady.as_str())
        && expect.remove("value").is_some()
    {
        reading.ignore("expect.value");
    }
}

/// The field among `fields` closest to `unknown`, when one is close enough to be a slip.
pub(super) fn closest<'a>(unknown: &str, fields: &'a [String]) -> Option<&'a str> {
    let unknown = snake_case(unknown);
    fields
        .iter()
        .map(|field| (field, edit_distance(&unknown, field)))
        .filter(|(field, distance)| {
            *distance <= 2
                || (unknown.len() >= 3 && (field.contains(&unknown) || unknown.contains(*field)))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(field, _)| field.as_str())
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current.push(
                substitution
                    .min(previous[column + 1] + 1)
                    .min(current[column] + 1),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::READ_CHARS;
    use crate::language::{decode, decode_reading, LanguageError};

    /// ADR-0205's promise, from the 2026-10-29 session: a call written the way another tool or a
    /// person would write it means the canonical call, a field that does not apply is ignored and
    /// named, and what stays strict is still refused with its fix.
    #[test]
    fn a_familiar_call_means_its_canonical_call_and_only_authority_stays_strict() {
        let same: [(&str, Value, Value); 24] = [
            // The three calls the session's agent was refused.
            (
                "browser_press_key",
                json!({"key":"PageDown","repeat":2}),
                json!({"strokes":["PageDown"],"repeat":2}),
            ),
            (
                "browser_scroll",
                json!({"amount":600}),
                json!({"amount":"600px"}),
            ),
            ("browser_wait", json!({}), json!({"condition":"load_ready"})),
            // Values in their other forms.
            (
                "browser_scroll",
                json!({"amount":"600 PX"}),
                json!({"amount":"600px"}),
            ),
            (
                "browser_scroll",
                json!({"amount":-600}),
                json!({"direction":"up","amount":"600px"}),
            ),
            (
                "browser_scroll",
                json!({"amount":"bottom"}),
                json!({"direction":"down","amount":"end"}),
            ),
            (
                "browser_scroll",
                json!({"ticks":3}),
                json!({"amount":"300px"}),
            ),
            (
                "browser_scroll",
                json!({"target":"target_1","direction":"up"}),
                json!({"target":"target_1"}),
            ),
            (
                "browser_wait",
                json!({"value":"2s"}),
                json!({"condition":"duration","value":"2000"}),
            ),
            (
                "browser_wait",
                json!({"value":1500}),
                json!({"condition":"duration","value":"1500"}),
            ),
            (
                "browser_wait",
                json!({"value":"Ready"}),
                json!({"condition":"text_present","value":"Ready"}),
            ),
            (
                "browser_wait",
                json!({"value":"https://example.com/done"}),
                json!({"condition":"url_contains","value":"https://example.com/done"}),
            ),
            (
                "browser_wait",
                json!({"target":"target_1"}),
                json!({"condition":"target_present","target":"target_1"}),
            ),
            (
                "browser_press_key",
                json!({"key":"ctrl+a"}),
                json!({"key":"a","modifiers":["Control"]}),
            ),
            (
                "browser_press_key",
                json!({"key":"esc"}),
                json!({"key":"Escape"}),
            ),
            (
                "browser_press_key",
                json!({"key":"PageDown PageDown"}),
                json!({"strokes":["PageDown","PageDown"]}),
            ),
            (
                "browser_click",
                json!({"target":"target_1","button":"right","click_count":"2"}),
                json!({"target":"target_1","button":"secondary","click_count":2}),
            ),
            (
                "browser_navigate",
                json!({"url":"example.com/a"}),
                json!({"url":"https://example.com/a"}),
            ),
            (
                "browser_navigate",
                json!({"url":"localhost:3000"}),
                json!({"url":"http://localhost:3000"}),
            ),
            (
                "browser_read",
                json!({"timeoutMs":"5s","Mode":"Article"}),
                json!({"timeout_ms":5000,"mode":"article"}),
            ),
            ("browser_tabs", json!({}), json!({"action":"list"})),
            (
                "browser_window",
                json!({"percent":"125%"}),
                json!({"action":"zoom","percent":125}),
            ),
            ("browser_dialog", json!({}), json!({"action":"status"})),
            (
                "browser_history",
                json!({"action":"Back","bypass_cache":true}),
                json!({"action":"back"}),
            ),
        ];
        for (tool, familiar, canonical) in same {
            assert_eq!(
                decode(tool, familiar.clone()),
                decode(tool, canonical.clone()),
                "{tool} {familiar} should mean {canonical}"
            );
            assert!(decode(tool, canonical).is_ok(), "{tool} {familiar}");
        }

        // What Ghostlight left out or changed is named in the reading the result reports.
        let reported = |tool: &str, call: Value| {
            let (_, reading) = decode_reading(tool, call).expect("the call decodes");
            let mut facts = json!({});
            reading.report(&mut facts);
            facts
        };
        assert_eq!(
            reported(
                "browser_read",
                json!({"max_chars":999_999_999,"wait_for":"x"})
            ),
            json!({"ignored":["wait_for"],"adjusted":{"max_chars":READ_CHARS.max}})
        );
        assert_eq!(
            reported(
                "browser_wait",
                json!({"condition":"load_ready","value":"x"})
            ),
            json!({"ignored":["value"]})
        );
        assert_eq!(reported("browser_wait", json!({})), json!({}));

        // What stays strict: authority, two readings, an unknown field on a tool that acts, and
        // a value with no reading. Each refusal says what would work.
        let refusal = |tool: &str, call: Value| match decode(tool, call) {
            Err(LanguageError::Invalid(message)) => message,
            other => panic!("{tool}: expected a refusal, got {other:?}"),
        };
        assert!(decode(
            "browser_type_text",
            json!({"focused":true,"text":"x","user_authorized_credentials":"true"})
        )
        .is_err());
        assert!(decode(
            "browser_click",
            json!({"target":"target_1","selector":{"name":"Save"}})
        )
        .is_err());
        assert!(
            refusal("browser_click", json!({"target":"target_1","clik_count":2}))
                .contains("Did you mean `click_count`?")
        );
        let unknown = refusal("browser_click", json!({"target":"target_1","why":"x"}));
        assert!(unknown.contains("This tool takes") && unknown.contains("click_count"));
        assert!(refusal("browser_scroll", json!({"amount":"huge"})).contains("small, medium"));
        assert!(refusal("browser_history", json!({})).contains("back, forward"));
    }
}
