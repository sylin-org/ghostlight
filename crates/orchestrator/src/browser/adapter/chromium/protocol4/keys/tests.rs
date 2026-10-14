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

fn descriptor(
    key: &str,
    code: &str,
    key_code: u16,
    modifiers: u8,
    text: &str,
    unmodified: &str,
) -> KeyDescriptor {
    KeyDescriptor {
        key: key.into(),
        code: Some(code.into()),
        key_code: Some(key_code),
        modifiers,
        text: Some(text.into()),
        unmodified_text: Some(unmodified.into()),
    }
}

#[test]
fn modifier_masks_match_the_cdp_vocabulary() {
    assert_eq!(modifier_mask(&["Alt", "Control", "Shift"]), 11);
    assert_eq!(modifier_mask::<&str>(&[]), 0);
    assert_eq!(modifier_mask(&["Meta"]), 4);
    assert_eq!(
        modifier_mask(&["Alt".to_owned(), "Control".to_owned(), "Meta".to_owned()]),
        7
    );
}

#[test]
fn an_unknown_modifier_name_adds_nothing() {
    assert_eq!(modifier_mask(&["Hyper", "Shift", "shift"]), 8);
}

#[test]
fn named_keys_receive_physical_codes() {
    assert_eq!(
        key_descriptor("Enter"),
        descriptor("Enter", "Enter", 13, 0, "\r", "\r")
    );
    assert_eq!(
        key_descriptor("x"),
        descriptor("x", "KeyX", 88, 0, "x", "x")
    );
    assert_eq!(
        key_descriptor("G"),
        descriptor("G", "KeyG", 71, 8, "G", "g")
    );
    assert_eq!(
        key_descriptor("7"),
        descriptor("7", "Digit7", 55, 0, "7", "7")
    );
}

#[test]
fn every_named_key_has_its_own_name_as_its_code() {
    for (name, key_code) in NAMED_KEYS {
        let described = key_descriptor(name);
        assert_eq!(described.code.as_deref(), Some(name));
        assert_eq!(described.key_code, Some(key_code));
        assert_eq!(described.modifiers, 0);
    }
}

#[test]
fn only_enter_among_named_keys_carries_text() {
    for (name, _) in NAMED_KEYS {
        let described = key_descriptor(name);
        assert_eq!(described.text.is_some(), name == "Enter", "{name}");
    }
}

#[test]
fn the_space_name_is_a_space_key() {
    let described = key_descriptor("Space");
    assert_eq!(described.key, " ");
    assert_eq!(described.code.as_deref(), Some("Space"));
    assert_eq!(described.key_code, Some(32));
}

#[test]
fn a_literal_space_types_with_text() {
    assert_eq!(
        key_descriptor(" "),
        descriptor(" ", "Space", 32, 0, " ", " ")
    );
}

#[test]
fn shifted_symbols_report_the_key_that_types_them() {
    assert_eq!(
        key_descriptor("!"),
        descriptor("!", "Digit1", 49, 8, "!", "1")
    );
    assert_eq!(
        key_descriptor("?"),
        descriptor("?", "Slash", 191, 8, "?", "/")
    );
    assert_eq!(
        key_descriptor("\""),
        descriptor("\"", "Quote", 222, 8, "\"", "'")
    );
    assert_eq!(
        key_descriptor("|"),
        descriptor("|", "Backslash", 220, 8, "|", "\\")
    );
}

#[test]
fn unshifted_punctuation_has_a_code() {
    assert_eq!(
        key_descriptor(";"),
        descriptor(";", "Semicolon", 186, 0, ";", ";")
    );
    assert_eq!(
        key_descriptor("["),
        descriptor("[", "BracketLeft", 219, 0, "[", "[")
    );
}

#[test]
fn a_character_with_no_physical_key_has_only_text() {
    let described = key_descriptor("\u{e9}");
    assert_eq!(described.code, None);
    assert_eq!(described.key_code, None);
    assert_eq!(described.modifiers, 0);
    assert_eq!(described.text.as_deref(), Some("\u{e9}"));
    assert_eq!(described.unmodified_text.as_deref(), Some("\u{e9}"));
}

#[test]
fn a_plain_press_is_a_down_with_text_and_an_up_without() {
    let steps = press("x", 0);
    assert_eq!(steps.len(), 2);
    assert_eq!(key_step(&steps[0]), (KeyAction::Down, "x", Some("x"), 0));
    assert_eq!(key_step(&steps[1]), (KeyAction::Up, "x", None, 0));
}

#[test]
fn a_shortcut_modifier_drops_the_text() {
    for modifiers in [ALT, CONTROL, META, CONTROL | SHIFT] {
        let steps = press("a", modifiers);
        assert_eq!(
            key_step(&steps[0]),
            (KeyAction::Down, "a", None, modifiers),
            "{modifiers}"
        );
        assert_eq!(key_step(&steps[1]), (KeyAction::Up, "a", None, modifiers));
    }
}

#[test]
fn shift_alone_keeps_the_text() {
    let steps = press("a", SHIFT);
    assert_eq!(
        key_step(&steps[0]),
        (KeyAction::Down, "a", Some("a"), SHIFT)
    );
}

#[test]
fn a_capital_letter_adds_shift_to_the_held_modifiers() {
    let steps = press("G", CONTROL);
    assert_eq!(
        key_step(&steps[0]),
        (KeyAction::Down, "G", None, CONTROL | SHIFT)
    );
    assert_eq!(
        key_step(&steps[1]),
        (KeyAction::Up, "G", None, CONTROL | SHIFT)
    );
    let alone = press("G", 0);
    assert_eq!(
        key_step(&alone[0]),
        (KeyAction::Down, "G", Some("G"), SHIFT)
    );
}

#[test]
fn pressing_enter_carries_a_carriage_return() {
    let steps = press("Enter", 0);
    assert_eq!(
        key_step(&steps[0]),
        (KeyAction::Down, "Enter", Some("\r"), 0)
    );
    assert_eq!(key_step(&steps[1]), (KeyAction::Up, "Enter", None, 0));
}

#[test]
fn replace_text_selects_everything_before_it_types() {
    let steps = replace_text("Hi");
    let shape: Vec<_> = steps.iter().map(key_step).collect();
    assert_eq!(
        shape,
        [
            (KeyAction::Down, "Control", None, CONTROL),
            (KeyAction::Down, "a", None, CONTROL),
            (KeyAction::Up, "a", None, CONTROL),
            (KeyAction::Up, "Control", None, 0),
            (KeyAction::Down, "H", Some("H"), SHIFT),
            (KeyAction::Up, "H", None, SHIFT),
            (KeyAction::Down, "i", Some("i"), 0),
            (KeyAction::Up, "i", None, 0),
            (KeyAction::Down, "Tab", None, 0),
            (KeyAction::Up, "Tab", None, 0),
        ]
    );
}

#[test]
fn the_select_all_chord_uses_the_left_control_key() {
    let steps = replace_text("x");
    match &steps[0] {
        InputStep::Key { code, key_code, .. } => {
            assert_eq!(code.as_deref(), Some("ControlLeft"));
            assert_eq!(*key_code, Some(17));
        }
        other => panic!("not a key step: {other:?}"),
    }
    match &steps[1] {
        InputStep::Key { code, key_code, .. } => {
            assert_eq!(code.as_deref(), Some("KeyA"));
            assert_eq!(*key_code, Some(65));
        }
        other => panic!("not a key step: {other:?}"),
    }
}

#[test]
fn replace_text_with_nothing_deletes_the_selection() {
    let steps = replace_text("");
    let shape: Vec<_> = steps.iter().skip(4).map(key_step).collect();
    assert_eq!(
        shape,
        [
            (KeyAction::Down, "Backspace", None, 0),
            (KeyAction::Up, "Backspace", None, 0),
            (KeyAction::Down, "Tab", None, 0),
            (KeyAction::Up, "Tab", None, 0),
        ]
    );
}

#[test]
fn replace_text_types_every_line_break_as_enter() {
    let steps = replace_text("a\r\nb\rc\nd");
    let typed: Vec<_> = steps
        .iter()
        .skip(4)
        .map(key_step)
        .filter(|(action, ..)| *action == KeyAction::Down)
        .map(|(_, key, ..)| key)
        .collect();
    assert_eq!(
        typed,
        ["a", "Enter", "b", "Enter", "c", "Enter", "d", "Tab"]
    );
}

#[test]
fn replace_text_types_each_character_not_each_byte() {
    let steps = replace_text("\u{e9}\u{1f600}");
    assert_eq!(steps.len(), 4 + 2 * 2 + 2);
}

#[test]
fn a_click_moves_presses_and_releases() {
    let point = Point { x: 12.5, y: 40.0 };
    let steps = click(point, MouseButton::Left, 1, 0);
    assert_eq!(
        steps,
        [
            mouse(MouseAction::Moved, point, MouseButton::Left, 0, 0, 0.0),
            mouse(MouseAction::Pressed, point, MouseButton::Left, 1, 0, 0.0),
            mouse(MouseAction::Released, point, MouseButton::Left, 1, 0, 0.0),
        ]
    );
}

#[test]
fn a_hover_is_one_move_with_no_button() {
    let point = Point { x: 3.0, y: 9.5 };
    assert_eq!(
        hover(point),
        [mouse(
            MouseAction::Moved,
            point,
            MouseButton::None,
            0,
            0,
            0.0
        )]
    );
}

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

#[test]
fn no_clicks_send_nothing() {
    assert!(click(Point { x: 0.0, y: 0.0 }, MouseButton::Left, 0, 0).is_empty());
}

#[test]
fn button_names_map_to_wire_buttons() {
    assert_eq!(mouse_button("primary"), MouseButton::Left);
    assert_eq!(mouse_button("middle"), MouseButton::Middle);
    assert_eq!(mouse_button("secondary"), MouseButton::Right);
    assert_eq!(mouse_button("anything"), MouseButton::Left);
}

#[test]
fn the_wheel_turns_one_hundred_twenty_pixels_a_tick() {
    let point = Point { x: 30.0, y: 40.0 };
    let down = wheel(point, WheelDirection::Down, 3);
    assert_eq!(down.len(), 3);
    for step in &down {
        assert_eq!(
            *step,
            mouse(MouseAction::Wheel, point, MouseButton::None, 0, 0, 120.0)
        );
    }
    let up = wheel(point, WheelDirection::Up, 2);
    assert_eq!(up.len(), 2);
    for step in &up {
        assert_eq!(
            *step,
            mouse(MouseAction::Wheel, point, MouseButton::None, 0, 0, -120.0)
        );
    }
    assert!(wheel(point, WheelDirection::Down, 0).is_empty());
}

#[test]
fn wheel_directions_map_from_names() {
    assert_eq!(WheelDirection::from_name("up"), WheelDirection::Up);
    assert_eq!(WheelDirection::from_name("down"), WheelDirection::Down);
    assert_eq!(WheelDirection::from_name("sideways"), WheelDirection::Down);
}

#[test]
fn a_drag_path_walks_in_equal_strides_and_lands_on_the_end() {
    let path = drag_path(Point { x: 10.0, y: 20.0 }, Point { x: 30.0, y: 60.0 }, 2);
    assert_eq!(
        path,
        [
            Point { x: 10.0, y: 20.0 },
            Point { x: 20.0, y: 40.0 },
            Point { x: 30.0, y: 60.0 },
        ]
    );
}

#[test]
fn the_standard_drag_path_has_twelve_moves_after_the_start() {
    let start = Point { x: 3.0, y: 7.0 };
    let end = Point { x: 101.0, y: 50.0 };
    let path = drag_path(start, end, 12);
    assert_eq!(path.len(), 13);
    assert_eq!(path[0], start);
    assert_eq!(path[12], end);
    for pair in path.windows(2) {
        assert!(pair[1].x > pair[0].x && pair[1].y > pair[0].y);
    }
}

#[test]
fn a_drag_step_count_is_clamped_into_range() {
    let start = Point { x: 0.0, y: 0.0 };
    let end = Point { x: 1.0, y: 1.0 };
    assert_eq!(drag_path(start, end, 0), [start, end]);
    assert_eq!(drag_path(start, end, 1000).len(), MAX_DRAG_STEPS + 1);
}
