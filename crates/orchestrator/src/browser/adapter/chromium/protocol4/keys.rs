//! The input steps the service sends for keys, clicks, wheel turns and drags.
//!
//! The extension dispatches steps and reports facts; it never decides what a key press or a
//! click is. This module is that decision. A key becomes the descriptor Chromium expects, a
//! press becomes a down and an up, a click becomes the move, press and release sequence a real
//! pointer makes, and a drag becomes a path the extension walks while holding the button.
//!
//! All of it is pure: names in, wire steps out.

use super::wire::{InputStep, KeyAction, MouseAction, MouseButton, Point};
use crate::language::{PointerButton, ScrollDirection};

/// The modifier bit for Alt in the mask Chromium reads.
const ALT: u8 = 1;
/// The modifier bit for Control.
const CONTROL: u8 = 2;
/// The modifier bit for Meta.
const META: u8 = 4;
/// The modifier bit for Shift.
const SHIFT: u8 = 8;
/// The modifiers that turn a key press into a shortcut, so the key carries no text.
const SHORTCUT_MODIFIERS: u8 = ALT | CONTROL | META;

/// The modifier names the work vocabulary uses, with their bits.
const MODIFIER_BITS: [(&str, u8); 4] = [
    ("Alt", ALT),
    ("Control", CONTROL),
    ("Meta", META),
    ("Shift", SHIFT),
];

/// The wheel delta of one tick, in CSS pixels.
const WHEEL_TICK: f64 = 120.0;
/// The most intermediate moves a drag path may have.
const MAX_DRAG_STEPS: usize = 60;

/// The key that names a space.
const SPACE_NAME: &str = "Space";
/// The key a line break types as.
const ENTER_NAME: &str = "Enter";
/// The key that deletes, typed for an empty replacement.
const BACKSPACE_NAME: &str = "Backspace";
/// The key that leaves a field and lets the page commit its value.
const TAB_NAME: &str = "Tab";
/// The key value Chromium reports for the Control key.
const CONTROL_KEY: &str = "Control";
/// The physical code of the left Control key.
const CONTROL_CODE: &str = "ControlLeft";
/// The legacy virtual key code of Control.
const CONTROL_KEY_CODE: u16 = 17;
/// The key that selects the whole field together with Control.
const SELECT_ALL_KEY: &str = "a";
/// The physical code of the A key.
const SELECT_ALL_CODE: &str = "KeyA";
/// The legacy virtual key code of the A key.
const SELECT_ALL_KEY_CODE: u16 = 65;

/// The named keys, with their legacy virtual key codes. A named key's physical code is its name.
const NAMED_KEYS: [(&str, u16); 14] = [
    ("Enter", 13),
    ("Tab", 9),
    ("Escape", 27),
    ("Backspace", 8),
    ("Delete", 46),
    ("ArrowUp", 38),
    ("ArrowDown", 40),
    ("ArrowLeft", 37),
    ("ArrowRight", 39),
    ("Home", 36),
    ("End", 35),
    ("PageUp", 33),
    ("PageDown", 34),
    ("Space", 32),
];

/// The characters typed with Shift, with the unshifted key that types them.
const SHIFTED: [(char, char); 21] = [
    ('!', '1'),
    ('@', '2'),
    ('#', '3'),
    ('$', '4'),
    ('%', '5'),
    ('^', '6'),
    ('&', '7'),
    ('*', '8'),
    ('(', '9'),
    (')', '0'),
    ('_', '-'),
    ('+', '='),
    ('{', '['),
    ('}', ']'),
    ('|', '\\'),
    (':', ';'),
    ('"', '\''),
    ('<', ','),
    ('>', '.'),
    ('?', '/'),
    ('~', '`'),
];

/// The unshifted punctuation keys, with their physical codes and legacy virtual key codes.
const PUNCTUATION: [(char, &str, u16); 12] = [
    (';', "Semicolon", 186),
    ('=', "Equal", 187),
    (',', "Comma", 188),
    ('-', "Minus", 189),
    ('.', "Period", 190),
    ('/', "Slash", 191),
    ('`', "Backquote", 192),
    ('[', "BracketLeft", 219),
    ('\\', "Backslash", 220),
    (']', "BracketRight", 221),
    ('\'', "Quote", 222),
    (' ', "Space", 32),
];

/// What Chromium needs to know to type one key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyDescriptor {
    /// The key value.
    pub key: String,
    /// The physical key code, when the key has one.
    pub code: Option<String>,
    /// The legacy virtual key code, when the key has one.
    pub key_code: Option<u16>,
    /// The modifiers typing this key implies, such as Shift for a capital letter.
    pub modifiers: u8,
    /// The text the key produces.
    pub text: Option<String>,
    /// The text the key produces without its implied modifiers.
    pub unmodified_text: Option<String>,
}

impl KeyDescriptor {
    /// One key step for this descriptor. `modifiers` is the whole mask the step carries.
    fn step(&self, action: KeyAction, modifiers: u8, with_text: bool) -> InputStep {
        InputStep::Key {
            action,
            key: self.key.clone(),
            code: self.code.clone(),
            key_code: self.key_code,
            text: self.text.clone().filter(|_| with_text),
            unmodified_text: self.unmodified_text.clone().filter(|_| with_text),
            modifiers,
        }
    }
}

/// The modifier mask for a list of modifier names. A name that is not a modifier adds nothing.
#[must_use]
pub fn modifier_mask<S: AsRef<str>>(modifiers: &[S]) -> u8 {
    modifiers
        .iter()
        .filter_map(|name| {
            MODIFIER_BITS
                .iter()
                .find(|(modifier, _)| *modifier == name.as_ref())
        })
        .fold(0, |mask, (_, bit)| mask | bit)
}

/// The descriptor of one key: a named key, or one character.
///
/// A capital letter and a shifted symbol carry Shift and report the key that types them
/// unshifted. Enter produces a carriage return, because Chromium needs that character to
/// perform its editing default, such as a line break in a text area.
#[must_use]
pub fn key_descriptor(key: &str) -> KeyDescriptor {
    if let Some((name, key_code)) = NAMED_KEYS.iter().find(|(name, _)| *name == key) {
        let text = (*name == ENTER_NAME).then(|| "\r".to_owned());
        return KeyDescriptor {
            key: if *name == SPACE_NAME {
                " ".to_owned()
            } else {
                (*name).to_owned()
            },
            code: Some((*name).to_owned()),
            key_code: Some(*key_code),
            modifiers: 0,
            unmodified_text: text.clone(),
            text,
        };
    }
    let mut base = key.to_owned();
    let mut modifiers = 0;
    let mut characters = key.chars();
    if let (Some(only), None) = (characters.next(), characters.next()) {
        if only.is_ascii_uppercase() {
            base = only.to_ascii_lowercase().to_string();
            modifiers = SHIFT;
        } else if let Some((_, unshifted)) = SHIFTED.iter().find(|(shifted, _)| *shifted == only) {
            base = unshifted.to_string();
            modifiers = SHIFT;
        }
    }
    let (code, key_code) = physical_key(&base).unzip();
    KeyDescriptor {
        key: key.to_owned(),
        code,
        key_code,
        modifiers,
        text: Some(key.to_owned()),
        unmodified_text: Some(base),
    }
}

/// The physical code and virtual key code of an unshifted one-character key.
fn physical_key(base: &str) -> Option<(String, u16)> {
    let mut characters = base.chars();
    let (Some(only), None) = (characters.next(), characters.next()) else {
        return None;
    };
    if only.is_ascii_alphabetic() {
        let upper = only.to_ascii_uppercase();
        return Some((format!("Key{upper}"), u16::from(upper as u8)));
    }
    if only.is_ascii_digit() {
        return Some((format!("Digit{only}"), u16::from(only as u8)));
    }
    PUNCTUATION
        .iter()
        .find(|(punctuation, _, _)| *punctuation == only)
        .map(|(_, code, key_code)| ((*code).to_owned(), *key_code))
}

/// The steps that press one key: a down and an up.
///
/// While Alt, Control or Meta is held the key carries no text, so a chord such as Control-A
/// reaches the browser as a command instead of typing a letter.
#[must_use]
pub fn press(key: &str, modifiers: u8) -> Vec<InputStep> {
    let descriptor = key_descriptor(key);
    let combined = modifiers | descriptor.modifiers;
    let with_text = modifiers & SHORTCUT_MODIFIERS == 0;
    vec![
        descriptor.step(KeyAction::Down, combined, with_text),
        descriptor.step(KeyAction::Up, combined, false),
    ]
}

/// The steps that replace the contents of the focused browser text field: select everything,
/// type the new value one key at a time, then leave the field so the page commits it.
///
/// Line breaks type as Enter, and an empty value deletes the selection. The sequence takes two
/// steps per character and a fixed handful more.
#[must_use]
pub fn replace_text(value: &str) -> Vec<InputStep> {
    let control = KeyDescriptor {
        key: CONTROL_KEY.to_owned(),
        code: Some(CONTROL_CODE.to_owned()),
        key_code: Some(CONTROL_KEY_CODE),
        modifiers: 0,
        text: None,
        unmodified_text: None,
    };
    let select_all = KeyDescriptor {
        key: SELECT_ALL_KEY.to_owned(),
        code: Some(SELECT_ALL_CODE.to_owned()),
        key_code: Some(SELECT_ALL_KEY_CODE),
        modifiers: 0,
        text: None,
        unmodified_text: None,
    };
    let mut steps = vec![
        control.step(KeyAction::Down, CONTROL, false),
        select_all.step(KeyAction::Down, CONTROL, false),
        select_all.step(KeyAction::Up, CONTROL, false),
        control.step(KeyAction::Up, 0, false),
    ];
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() {
        steps.extend(type_key(BACKSPACE_NAME));
    }
    for character in normalized.chars() {
        let key = if character == '\n' {
            ENTER_NAME.to_owned()
        } else {
            character.to_string()
        };
        steps.extend(type_key(&key));
    }
    steps.extend(type_key(TAB_NAME));
    steps
}

/// One key typed with no held modifiers: the descriptor's own, and its text on the down only.
fn type_key(key: &str) -> [InputStep; 2] {
    let descriptor = key_descriptor(key);
    [
        descriptor.step(KeyAction::Down, descriptor.modifiers, true),
        descriptor.step(KeyAction::Up, descriptor.modifiers, false),
    ]
}

/// The wire button a pointer button presses.
#[must_use]
pub const fn mouse_button(button: PointerButton) -> MouseButton {
    match button {
        PointerButton::Primary => MouseButton::Left,
        PointerButton::Middle => MouseButton::Middle,
        PointerButton::Secondary => MouseButton::Right,
    }
}

/// One mouse step with the fields that vary. Everything else is the wire default.
fn mouse(
    action: MouseAction,
    point: Point,
    button: MouseButton,
    click_count: u8,
    modifiers: u8,
    delta_y: f64,
) -> InputStep {
    InputStep::Mouse {
        action,
        x: point.x,
        y: point.y,
        button,
        buttons: 0,
        click_count,
        modifiers,
        delta_x: 0.0,
        delta_y,
        force: 0.0,
    }
}

/// The steps for `clicks` clicks at a point: for each, the pointer moves there, presses and
/// releases, and the click count rises with each click so a double click is one.
#[must_use]
pub fn click(point: Point, button: MouseButton, clicks: u8, modifiers: u8) -> Vec<InputStep> {
    (1..=clicks)
        .flat_map(|count| {
            [
                mouse(MouseAction::Moved, point, button, 0, modifiers, 0.0),
                mouse(MouseAction::Pressed, point, button, count, modifiers, 0.0),
                mouse(MouseAction::Released, point, button, count, modifiers, 0.0),
            ]
        })
        .collect()
}

/// The step that moves the pointer to a point, with no button down and no modifier held.
#[must_use]
pub fn hover(point: Point) -> Vec<InputStep> {
    vec![mouse(
        MouseAction::Moved,
        point,
        MouseButton::None,
        0,
        0,
        0.0,
    )]
}

/// Which way the wheel turns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WheelDirection {
    /// Toward the top of the page.
    Up,
    /// Toward the bottom of the page.
    Down,
}

impl WheelDirection {
    /// The way the wheel turns for a scroll direction, or `None` for a sideways one, which a
    /// wheel scroll does not take.
    #[must_use]
    pub const fn of(direction: ScrollDirection) -> Option<Self> {
        match direction {
            ScrollDirection::Up => Some(Self::Up),
            ScrollDirection::Down => Some(Self::Down),
            ScrollDirection::Left | ScrollDirection::Right => None,
        }
    }

    /// The vertical delta of one tick.
    fn delta_y(self) -> f64 {
        match self {
            Self::Up => -WHEEL_TICK,
            Self::Down => WHEEL_TICK,
        }
    }
}

/// The steps that turn the wheel `ticks` times at a point, 120 pixels a tick.
#[must_use]
pub fn wheel(point: Point, direction: WheelDirection, ticks: u8) -> Vec<InputStep> {
    (0..ticks)
        .map(|_| {
            mouse(
                MouseAction::Wheel,
                point,
                MouseButton::None,
                0,
                0,
                direction.delta_y(),
            )
        })
        .collect()
}

/// The path of a drag: the start, then `steps` moves in equal strides, the last landing exactly
/// on the end. A count outside 1 to 60 is clamped into it.
#[must_use]
pub fn drag_path(start: Point, end: Point, steps: usize) -> Vec<Point> {
    let steps = steps.clamp(1, MAX_DRAG_STEPS);
    let mut path = Vec::with_capacity(steps + 1);
    path.push(start);
    for step in 1..steps {
        let ratio = step as f64 / steps as f64;
        path.push(Point {
            x: start.x + (end.x - start.x) * ratio,
            y: start.y + (end.y - start.y) * ratio,
        });
    }
    path.push(end);
    path
}

#[cfg(test)]
mod tests;
