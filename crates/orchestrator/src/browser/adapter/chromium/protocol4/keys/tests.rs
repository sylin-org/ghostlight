use super::*;

fn key_step(step: &InputStep) -> (KeyAction, &str, Option<&str>, u8) {
    match step {
        InputStep::Key {
            action,
            key,
            text,
            modifiers,
            ..
        } => (*action, key.as_str(), text.as_deref(), *modifiers),
        other => panic!("not a key step: {other:?}"),
    }
}

/// `browser_press_key` types a key's text unless a shortcut modifier is held, so Control-A reaches
/// Chromium as a command and not as a typed "a", and Enter types the carriage return a form
/// submits on. The release never carries text.
#[test]
fn a_press_types_its_text_unless_a_shortcut_modifier_is_held() {
    let cases: [(&str, u8, Option<&str>, u8); 8] = [
        ("x", 0, Some("x"), 0),
        ("a", SHIFT, Some("a"), SHIFT),
        ("a", ALT, None, ALT),
        ("a", CONTROL, None, CONTROL),
        ("a", META, None, META),
        ("G", 0, Some("G"), SHIFT),
        ("G", CONTROL, None, CONTROL | SHIFT),
        (ENTER_NAME, 0, Some("\r"), 0),
    ];
    for (key, held, text, modifiers) in cases {
        let steps = press(key, held);
        assert_eq!(
            steps.iter().map(key_step).collect::<Vec<_>>(),
            [
                (KeyAction::Down, key, text, modifiers),
                (KeyAction::Up, key, None, modifiers),
            ],
            "{key} with {held}"
        );
    }
}

/// Filling a text field selects what it holds with Control-A, types each character (a line break
/// as Enter, an empty value as Backspace) and ends with Tab so the page commits the value.
/// Chromium runs the select-all command only when the chord names its physical keys.
#[test]
fn replace_text_selects_everything_types_each_character_and_commits() {
    let chord = [
        (KeyAction::Down, CONTROL_KEY, None, CONTROL),
        (KeyAction::Down, SELECT_ALL_KEY, None, CONTROL),
        (KeyAction::Up, SELECT_ALL_KEY, None, CONTROL),
        (KeyAction::Up, CONTROL_KEY, None, 0),
    ];
    let steps = replace_text("x");
    let physical: Vec<_> = steps[..2]
        .iter()
        .map(|step| match step {
            InputStep::Key { code, key_code, .. } => (code.as_deref(), *key_code),
            other => panic!("not a key step: {other:?}"),
        })
        .collect();
    assert_eq!(
        physical,
        [
            (Some(CONTROL_CODE), Some(CONTROL_KEY_CODE)),
            (Some(SELECT_ALL_CODE), Some(SELECT_ALL_KEY_CODE)),
        ]
    );

    let typed = |value: &str| -> Vec<String> {
        let steps = replace_text(value);
        let shape: Vec<_> = steps.iter().map(key_step).collect();
        assert_eq!(shape[..chord.len()], chord, "{value:?} selects first");
        shape[chord.len()..]
            .iter()
            .filter(|(action, ..)| *action == KeyAction::Down)
            .map(|(_, key, ..)| (*key).to_owned())
            .collect()
    };
    assert_eq!(typed("Hi"), ["H", "i", TAB_NAME]);
    assert_eq!(typed(""), [BACKSPACE_NAME, TAB_NAME]);
    assert_eq!(
        typed("a\r\nb\rc\nd"),
        ["a", ENTER_NAME, "b", ENTER_NAME, "c", ENTER_NAME, "d", TAB_NAME]
    );
    assert_eq!(typed("\u{e9}\u{1f600}"), ["\u{e9}", "\u{1f600}", TAB_NAME]);
}

/// A double click is two clicks whose second press says `clickCount` 2, which is what makes
/// Chromium fire `dblclick`; the button and modifiers hold for every step.
#[test]
fn a_double_click_raises_the_count_with_each_click() {
    let point = Point { x: 1.0, y: 2.0 };
    let counts: Vec<_> = click(point, MouseButton::Right, 2, SHIFT)
        .into_iter()
        .map(|step| match step {
            InputStep::Mouse {
                action,
                button,
                click_count,
                modifiers,
                ..
            } => (action, button, click_count, modifiers),
            other => panic!("not a mouse step: {other:?}"),
        })
        .collect();
    assert_eq!(
        counts,
        [
            (MouseAction::Moved, MouseButton::Right, 0, SHIFT),
            (MouseAction::Pressed, MouseButton::Right, 1, SHIFT),
            (MouseAction::Released, MouseButton::Right, 1, SHIFT),
            (MouseAction::Moved, MouseButton::Right, 0, SHIFT),
            (MouseAction::Pressed, MouseButton::Right, 2, SHIFT),
            (MouseAction::Released, MouseButton::Right, 2, SHIFT),
        ]
    );
}
