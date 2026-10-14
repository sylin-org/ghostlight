//! Pointer, keyboard, capture, upload and script work against a scripted protocol-4 extension.

use base64::Engine as _;
use serde_json::{json, Value};

use super::super::super::geometry;
use super::super::super::keys;
use super::super::super::wire::{CaptureClip, InputStep, Point};
use super::*;
use crate::browser::adapter::{CaptureScope, PhysicalFile, PhysicalRectangle, ViewportGeometry};

const NESTED: &str = "C:c1:locator_a1_1";
const TOP_LOCATOR: &str = "T:t1:locator_a1_2";

/// What the scripted extension answers to a primitive other than `frames` and `list_tabs`.
enum Out {
    /// An outcome that changed nothing.
    Value(Value),
    /// An outcome of a primitive that may have changed the page.
    Effect(Value),
    /// A failure, with whether the primitive had begun to act.
    Fail(wire::Failure, bool),
}

/// A port whose extension answers `calls` for every world call and `other` for every other
/// primitive, over the three-document inventory.
fn rig(
    calls: Calls,
    other: impl Fn(&wire::Primitive) -> Out + Send + 'static,
) -> (RelayBrowserPort, FakeExtension) {
    port_with(Box::new(move |correlation, primitive| {
        let outcome = match primitive {
            wire::Primitive::Frames(_) => Out::Value(inventory()),
            wire::Primitive::ListTabs(_) => Out::Value(json!({"tabs": [{
                "tab_id": 12, "title": "Top", "url": TOP, "active": true,
                "readiness": "complete"}]})),
            wire::Primitive::Call(call) => Out::Value(json!({"answers": calls(call)})),
            other_primitive => other(other_primitive),
        };
        Reply::Now(match outcome {
            Out::Value(value) => FakeExtension::ok(correlation, value),
            Out::Effect(value) => wire::Receipt {
                dispatched: true,
                ..FakeExtension::ok(correlation, value)
            },
            Out::Fail(failure, dispatched) => wire::Receipt {
                dispatched,
                ..FakeExtension::failed(correlation, failure)
            },
        })
    }))
}

/// The extension finishing every step of an input, with its precondition answered by the page.
fn finished(primitive: &wire::Primitive) -> Out {
    let wire::Primitive::Input(input) = primitive else {
        return Out::Value(Value::Null);
    };
    Out::Effect(json!({
        "completed_steps": input.steps.len(),
        "precondition": input.precondition.as_ref().map(answer),
    }))
}

fn page_rectangle() -> Value {
    json!({"left": 10, "top": 20, "width": 40, "height": 10})
}

fn button() -> Value {
    json!({"role": "button", "name": "Save"})
}

/// The answer of the page's isolated world to one world call.
fn answer(world: &wire::WorldCall) -> Value {
    match world.message["kind"].as_str().unwrap_or_default() {
        "activate" => {
            returned(json!({"activated": true, "subject": button(), "rectangle": page_rectangle()}))
        }
        "hover" | "box" => returned(json!({"subject": button(), "rectangle": page_rectangle()})),
        "owner_box" => returned(json!({"left": 100, "top": 50, "width": 300, "height": 200})),
        "scroll" => returned(json!({"x": 0, "y": 40, "subject": null})),
        "scroll_offset" => returned(json!({"x": 5, "y": 900})),
        "observe" => observed(false, 1_000),
        "type_text" => returned(json!({"subject": {"role": "textbox", "name": "Email"}})),
        "upload_files" => returned(
            json!({"uploaded_count": 1, "uploaded_bytes": 9, "subject": {"role": "button",
                "name": "Choose file"}}),
        ),
        "focus" => returned(json!({"focused": true, "subject": {"role": "textbox",
            "name": "Email"}})),
        "require_focused" | "verify_focused_text" | "clear_focused" => {
            returned(json!({"focused": true}))
        }
        _ => refused("unknown_kind"),
    }
}

fn page() -> Calls {
    Box::new(|call| call.calls.iter().map(answer).collect())
}

pub(super) fn inputs_seen(fake: &FakeExtension) -> Vec<wire::Input> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Input(input) => Some(input.clone()),
            _ => None,
        })
        .collect()
}

fn captures_seen(fake: &FakeExtension) -> Vec<wire::Capture> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Capture(capture) => Some(capture.clone()),
            _ => None,
        })
        .collect()
}

/// The wire name of every primitive the extension received, in order.
pub(super) fn primitives_seen(fake: &FakeExtension) -> Vec<String> {
    fake.wait(|_| true)
        .requests
        .iter()
        .map(|(_, primitive)| {
            serde_json::to_value(primitive).unwrap()["name"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// The message kind of every world call of one `call`.
pub(super) fn kinds(call: &wire::Call) -> Vec<String> {
    call.calls
        .iter()
        .map(|world| world.message["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn all_open() -> DocumentScope {
    scope(&["T.t1", "C.c1", "D.d1"], false)
}

fn activate(locator: &str) -> BrowserCommand {
    BrowserCommand::Activate {
        tab_id: 12,
        locator: locator.into(),
        button: "left".into(),
        click_count: 1,
    }
}

fn press(locator: Option<&str>, key: &str, modifiers: &[&str]) -> BrowserCommand {
    BrowserCommand::PressKey {
        tab_id: 12,
        locator: locator.map(Into::into),
        key: key.into(),
        modifiers: modifiers.iter().map(|name| (*name).into()).collect(),
    }
}

fn type_focused(clear_first: bool) -> BrowserCommand {
    BrowserCommand::TypeFocused {
        tab_id: 12,
        text: "hello".into(),
        clear_first,
        allow_credentials: false,
    }
}

pub(super) fn unscoped(outcome: BrowserOutcome) -> BrowserOutcome {
    unwrap_scoped(outcome).1
}

fn done(steps: u32) -> Value {
    json!({"completed_steps": steps, "stopped": null})
}

pub(super) fn view() -> Value {
    json!({"visual_page_x": 0, "visual_page_y": 0, "visual_css_width": 800,
           "visual_css_height": 600, "content_css_width": 800, "content_css_height": 2000,
           "device_scale": 1, "zoom": 1})
}

fn captured(view: Value) -> Value {
    json!({"mime_type": "image/jpeg", "data": "/9j/", "width": 800, "height": 600,
           "output_scale": 1, "clip": {"x": 0, "y": 0, "width": 800, "height": 600},
           "view": view})
}

pub(super) fn expected_view() -> ViewportGeometry {
    ViewportGeometry {
        scope: CaptureScope::Viewport,
        page_x: 0.0,
        page_y: 0.0,
        css_width: 800.0,
        css_height: 600.0,
        visual_page_x: 0.0,
        visual_page_y: 0.0,
        visual_css_width: 800.0,
        visual_css_height: 600.0,
        device_scale: 1.0,
        zoom: 1.0,
        output_scale: 1.0,
    }
}

#[test]
fn a_click_goes_to_the_targets_centre_in_the_top_viewport() {
    let (port, fake) = rig(page(), finished);
    let (observation, result) =
        unwrap_scoped(call(&port, scoped(all_open(), activate(NESTED))).unwrap());
    let BrowserOutcome::Activated {
        tab,
        subject,
        committed_urls,
    } = result
    else {
        panic!("an activation");
    };
    assert_eq!(tab.tab_id, 12);
    assert_eq!(subject.unwrap().name, "Save");
    assert!(committed_urls.is_empty());
    assert_eq!(observation.visited, ["C.c1"]);
    let calls = calls_seen(&fake);
    assert_eq!(calls[0].mode, wire::CallMode::Sequence);
    assert_eq!(frames_of(&calls[0]), ["C", "T"]);
    assert_eq!(kinds(&calls[0]), ["activate", "owner_box"]);
    assert_eq!(calls[0].calls[1].owner_of.as_deref(), Some("C"));
    assert!(calls[0].guard.is_some());
    let inputs = inputs_seen(&fake);
    assert_eq!(inputs.len(), 1);
    assert_eq!(
        inputs[0].steps,
        keys::click(
            Point { x: 130.0, y: 75.0 },
            keys::mouse_button("left"),
            1,
            0
        ),
        "the middle of the rectangle, shifted by the owner's content box"
    );
    assert_eq!(inputs[0].settle_ms, 250);
    assert!(inputs[0].precondition.is_none());
    assert!(inputs[0].guard.is_some());
    assert_eq!(
        primitives_seen(&fake),
        ["call", "input", "list_tabs"],
        "a scope names its tree, so no inventory is read"
    );
}

#[test]
fn a_click_in_the_top_document_needs_no_owner_box() {
    let (port, fake) = rig(page(), finished);
    call(&port, scoped(all_open(), activate(TOP_LOCATOR))).unwrap();
    let calls = calls_seen(&fake);
    assert_eq!(kinds(&calls[0]), ["activate"]);
    assert_eq!(
        inputs_seen(&fake)[0].steps,
        keys::click(Point { x: 30.0, y: 25.0 }, keys::mouse_button("left"), 1, 0)
    );
}

#[test]
fn a_modified_click_holds_its_modifiers() {
    let (port, fake) = rig(page(), finished);
    call(
        &port,
        scoped(
            all_open(),
            BrowserCommand::ActivateModified {
                tab_id: 12,
                locator: TOP_LOCATOR.into(),
                button: "left".into(),
                click_count: 2,
                modifiers: vec!["Shift".into()],
            },
        ),
    )
    .unwrap();
    assert_eq!(
        inputs_seen(&fake)[0].steps,
        keys::click(
            Point { x: 30.0, y: 25.0 },
            keys::mouse_button("left"),
            2,
            keys::modifier_mask(&["Shift"])
        )
    );
}

#[test]
fn a_dialog_during_a_click_is_success() {
    let (port, _fake) = rig(page(), |_| {
        Out::Effect(json!({"completed_steps": 2,
            "stopped": {"reason": "dialog", "dialog_type": "confirm"}}))
    });
    let result = unscoped(call(&port, scoped(all_open(), activate(NESTED))).unwrap());
    assert!(matches!(result, BrowserOutcome::Activated { .. }));
}

#[test]
fn a_preparation_failure_is_a_plain_failure_and_sends_no_input() {
    for code in ["stale_locator", "not_visible"] {
        let (port, fake) = rig(
            Box::new(move |call| call.calls.iter().map(|_| refused(code)).collect()),
            finished,
        );
        let error = call(&port, scoped(all_open(), activate(NESTED))).unwrap_err();
        assert!(
            matches!(error, BrowserError::Primitive(_)),
            "{code} is a plain failure, not {error:?}"
        );
        assert!(inputs_seen(&fake).is_empty());
    }
}

#[test]
fn a_failure_after_the_click_has_an_unknown_effect() {
    for dispatched in [true, false] {
        let (port, _fake) = rig(page(), move |_| {
            Out::Fail(wire::Failure::BrowserRefused, dispatched)
        });
        let error = call(&port, scoped(all_open(), activate(NESTED))).unwrap_err();
        assert_eq!(
            matches!(error, BrowserError::EffectUnknown(_)),
            dispatched,
            "{error:?}"
        );
    }
    let (port, _fake) = rig(page(), |_| Out::Effect(done(1)));
    assert_eq!(
        call(&port, scoped(all_open(), activate(NESTED))),
        Err(BrowserError::EffectUnknown(
            "input stopped before its last step".into()
        )),
        "input that stopped short of its last step may have landed in part"
    );
}

#[test]
fn a_hover_moves_the_pointer_to_the_targets_centre() {
    let (port, fake) = rig(page(), finished);
    let result = unscoped(
        call(
            &port,
            scoped(
                all_open(),
                BrowserCommand::Hover {
                    tab_id: 12,
                    locator: NESTED.into(),
                },
            ),
        )
        .unwrap(),
    );
    assert!(matches!(
        result,
        BrowserOutcome::Hovered {
            tab_id: 12,
            subject: Some(_)
        }
    ));
    let calls = calls_seen(&fake);
    assert_eq!(kinds(&calls[0]), ["hover", "owner_box"]);
    let inputs = inputs_seen(&fake);
    assert_eq!(inputs[0].steps, keys::hover(Point { x: 130.0, y: 75.0 }));
    assert_eq!(inputs[0].settle_ms, 0);
}

#[test]
fn a_scroll_in_a_nested_document_reports_the_top_documents_offset() {
    let scroll = |locator: Option<&str>| BrowserCommand::Scroll {
        tab_id: 12,
        locator: locator.map(Into::into),
        direction: Some("down".into()),
        amount: Some("page".into()),
    };
    let (port, fake) = rig(page(), |_| Out::Value(Value::Null));
    let nested = unscoped(call(&port, scoped(all_open(), scroll(Some(NESTED)))).unwrap());
    assert_eq!(
        nested,
        BrowserOutcome::Scrolled {
            tab_id: 12,
            x: 5.0,
            y: 900.0,
            subject: None
        }
    );
    let page_scroll = unscoped(call(&port, scoped(all_open(), scroll(None))).unwrap());
    assert_eq!(
        page_scroll,
        BrowserOutcome::Scrolled {
            tab_id: 12,
            x: 0.0,
            y: 40.0,
            subject: None
        }
    );
    let calls = calls_seen(&fake);
    assert_eq!(kinds(&calls[0]), ["scroll", "scroll_offset"]);
    assert_eq!(frames_of(&calls[0]), ["C", "T"]);
    assert_eq!(kinds(&calls[1]), ["scroll"]);
    assert_eq!(frames_of(&calls[1]), ["T"]);
}

#[test]
fn typing_into_a_control_is_one_guarded_call_to_its_document() {
    let (port, fake) = rig(page(), |_| Out::Value(Value::Null));
    let result = unscoped(
        call(
            &port,
            scoped(
                all_open(),
                BrowserCommand::TypeText {
                    tab_id: 12,
                    locator: NESTED.into(),
                    text: "h\u{e9}llo".into(),
                    clear_first: true,
                    allow_credentials: false,
                },
            ),
        )
        .unwrap(),
    );
    let BrowserOutcome::Typed {
        character_count,
        subject,
        ..
    } = result
    else {
        panic!("typed text");
    };
    assert_eq!(character_count, 5);
    assert_eq!(subject.unwrap().role, "textbox");
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 1);
    assert_eq!(frames_of(&calls[0]), ["C"]);
    assert_eq!(calls[0].calls[0].message["clear_first"], true);
    assert!(calls[0].guard.is_some());
    assert!(inputs_seen(&fake).is_empty());
}

#[test]
fn a_key_press_with_a_locator_focuses_the_control_first() {
    let (port, fake) = rig(page(), |_| {
        Out::Effect(
            json!({"completed_steps": 2, "precondition": returned(json!({
            "focused": true, "subject": {"role": "textbox", "name": "Email"}}))}),
        )
    });
    let result =
        unscoped(call(&port, scoped(all_open(), press(Some(NESTED), "Enter", &[]))).unwrap());
    let BrowserOutcome::KeyPressed { key, subject, .. } = result else {
        panic!("a key press");
    };
    assert_eq!(key, "Enter");
    assert_eq!(subject.unwrap().name, "Email");
    let input = &inputs_seen(&fake)[0];
    let precondition = input.precondition.as_ref().unwrap();
    assert_eq!(precondition.message["kind"], "focus");
    assert_eq!(precondition.frame.as_ref().unwrap().frame_id, "C");
    assert_eq!(input.steps, keys::press("Enter", 0));
    assert!(input.guard.is_some());
}

#[test]
fn a_key_press_without_a_locator_requires_the_bound_subject_to_hold_the_focus() {
    let (port, fake) = rig(page(), finished);
    let mut bound = all_open();
    bound.subjects = vec!["C.c1".into()];
    call(&port, scoped(bound, press(None, "Tab", &[]))).unwrap();
    let precondition = inputs_seen(&fake)[0].precondition.clone().unwrap();
    assert_eq!(precondition.message["kind"], "require_focused");
    assert_eq!(precondition.frame.unwrap().frame_id, "C");

    let (port, fake) = rig(page(), finished);
    call(&port, scoped(all_open(), press(None, "Tab", &[]))).unwrap();
    assert!(
        inputs_seen(&fake)[0].precondition.is_none(),
        "a scope that bound no subject presses into whatever holds the focus"
    );
    let (port, fake) = rig(page(), finished);
    call(&port, press(None, "Tab", &[])).unwrap();
    assert!(inputs_seen(&fake)[0].precondition.is_none());
}

#[test]
fn a_modified_key_drops_its_text() {
    let (port, fake) = rig(page(), finished);
    for modifiers in [
        &[][..],
        &["Control"][..],
        &["Alt"][..],
        &["Meta"][..],
        &["Shift"][..],
    ] {
        call(&port, scoped(all_open(), press(None, "a", modifiers))).unwrap();
    }
    let texts: Vec<Option<String>> = inputs_seen(&fake)
        .iter()
        .map(|input| match &input.steps[0] {
            InputStep::Key { text, .. } => text.clone(),
            other => panic!("a key event, not {other:?}"),
        })
        .collect();
    assert_eq!(
        texts,
        [Some("a".into()), None, None, None, Some("a".into()),]
    );
}

/// An input the extension reports as the real one does for a refused precondition: the receipt
/// says it dispatched, because the precondition answered, but no step was sent.
fn precondition_refused(code: &'static str) -> impl Fn(&wire::Primitive) -> Out + Send + 'static {
    move |_| Out::Effect(json!({"completed_steps": 0, "precondition": refused(code)}))
}

#[test]
fn a_lost_focus_refuses_the_key_press_with_nothing_sent() {
    let (port, _fake) = rig(page(), precondition_refused("focus_lost"));
    let mut bound = all_open();
    bound.subjects = vec!["C.c1".into()];
    let error = call(&port, scoped(bound, press(None, "Tab", &[]))).unwrap_err();
    assert_eq!(error, BrowserError::Primitive("focus_lost".into()));
}

#[test]
fn a_stale_locator_refuses_the_key_press_with_nothing_sent() {
    let (port, _fake) = rig(page(), precondition_refused("stale_locator"));
    let error = call(&port, scoped(all_open(), press(Some(NESTED), "Enter", &[]))).unwrap_err();
    assert_eq!(error, BrowserError::Primitive("stale_locator".into()));
}

#[test]
fn a_focus_that_moved_refuses_the_text_with_nothing_sent() {
    let (port, _fake) = rig(
        focus_held_by(&["C"]),
        precondition_refused("no_focused_control"),
    );
    let error = call(&port, scoped(all_open(), type_focused(false))).unwrap_err();
    assert_eq!(error, BrowserError::Primitive("no_focused_control".into()));
}

/// Typing into the focused control with the focus held by `holders`, each answering its
/// `verify_focused_text`.
fn focus_held_by(holders: &'static [&'static str]) -> Calls {
    Box::new(move |call| {
        call.calls
            .iter()
            .zip(frames_of(call))
            .map(
                |(world, frame)| match world.message["kind"].as_str().unwrap() {
                    "verify_focused_text" if !holders.contains(&frame.as_str()) => {
                        refused("no_focused_control")
                    }
                    _ => answer(world),
                },
            )
            .collect()
    })
}

#[test]
fn typing_into_the_focused_control_clears_and_types_only_in_the_first_focused_document() {
    let (port, fake) = rig(focus_held_by(&["C", "D"]), finished);
    let result = unscoped(call(&port, scoped(all_open(), type_focused(true))).unwrap());
    assert!(matches!(
        result,
        BrowserOutcome::Typed {
            character_count: 5,
            subject: None,
            ..
        }
    ));
    let calls = calls_seen(&fake);
    assert_eq!(calls[0].mode, wire::CallMode::Parallel);
    assert_eq!(frames_of(&calls[0]), ["T", "C", "D"]);
    assert!(calls[0].guard.is_some());
    assert_eq!(calls[1].mode, wire::CallMode::Sequence);
    assert_eq!(kinds(&calls[1]), ["clear_focused", "verify_focused_text"]);
    assert_eq!(
        frames_of(&calls[1]),
        ["C", "C"],
        "the first focused document in tree order, and no other"
    );
    assert_eq!(calls.len(), 2);
    let input = &inputs_seen(&fake)[0];
    let precondition = input.precondition.as_ref().unwrap();
    assert_eq!(precondition.message["kind"], "verify_focused_text");
    assert_eq!(precondition.frame.as_ref().unwrap().frame_id, "C");
    assert_eq!(
        input.steps,
        [InputStep::InsertText {
            text: "hello".into()
        }]
    );
    assert!(input.guard.is_some());
}

#[test]
fn typing_into_the_focused_control_without_clearing_sends_only_the_text() {
    let (port, fake) = rig(focus_held_by(&["T"]), finished);
    call(&port, scoped(all_open(), type_focused(false))).unwrap();
    assert_eq!(calls_seen(&fake).len(), 1);
    let input = &inputs_seen(&fake)[0];
    assert_eq!(
        input
            .precondition
            .as_ref()
            .unwrap()
            .frame
            .as_ref()
            .unwrap()
            .frame_id,
        "T"
    );
}

#[test]
fn typing_with_no_focused_text_control_fails_before_anything_is_sent() {
    let (port, fake) = rig(focus_held_by(&[]), finished);
    let error = call(&port, scoped(all_open(), type_focused(true))).unwrap_err();
    assert_eq!(error, BrowserError::Primitive("no_focused_control".into()));
    assert!(inputs_seen(&fake).is_empty());
    assert_eq!(calls_seen(&fake).len(), 1, "nothing was cleared");
}

#[test]
fn a_dialog_while_typing_leaves_the_effect_unknown() {
    let (port, _fake) = rig(focus_held_by(&["T"]), |_| {
        Out::Effect(
            json!({"completed_steps": 0, "precondition": returned(json!({})),
            "stopped": {"reason": "dialog", "dialog_type": "alert"}}),
        )
    });
    assert!(matches!(
        call(&port, scoped(all_open(), type_focused(false))),
        Err(BrowserError::EffectUnknown(_))
    ));
}

#[test]
fn a_viewport_capture_is_one_primitive_after_a_best_effort_settle() {
    let (port, fake) = rig(page(), |_| Out::Value(captured(view())));
    let result = call(
        &port,
        BrowserCommand::Screenshot {
            tab_id: 12,
            locator: None,
            full_page: false,
            visual_settle: None,
        },
    )
    .unwrap();
    let BrowserOutcome::Screenshot {
        tab_id,
        viewport,
        width,
        height,
        ..
    } = result
    else {
        panic!("a screenshot");
    };
    assert_eq!((tab_id, width, height), (12, 800, 600));
    assert_eq!(viewport, expected_view());
    assert_eq!(
        primitives_seen(&fake),
        ["call", "capture"],
        "a settle, then the capture; the service sends no hide or restore calls"
    );
    let calls = calls_seen(&fake);
    assert_eq!(calls[0].calls[0].message["condition"], "visual_settle");
    assert!(calls[0].calls[0].frame.is_none());
    let captures = captures_seen(&fake);
    assert_eq!(captures[0].clip, CaptureClip::Viewport);
    assert_eq!(captures[0].budget, geometry::ordinary_budget());
}

#[test]
fn a_capture_goes_on_when_the_settle_fails_and_skips_it_when_asked() {
    let (port, fake) = rig(
        Box::new(|call| {
            call.calls
                .iter()
                .map(|_| refused("page_exception"))
                .collect()
        }),
        |_| Out::Value(captured(view())),
    );
    let shot = |full_page, visual_settle| BrowserCommand::Screenshot {
        tab_id: 12,
        locator: None,
        full_page,
        visual_settle,
    };
    let full = call(&port, shot(true, None)).unwrap();
    let BrowserOutcome::Screenshot { viewport, .. } = full else {
        panic!("a screenshot");
    };
    assert_eq!(viewport.scope, CaptureScope::FullPage);
    call(&port, shot(false, Some(false))).unwrap();
    assert_eq!(
        primitives_seen(&fake),
        ["call", "capture", "capture"],
        "visual_settle false sends no settle"
    );
    assert_eq!(captures_seen(&fake)[0].clip, CaptureClip::FullPage);
}

#[test]
fn a_target_capture_clips_to_the_target_placed_in_the_top_viewport() {
    let (port, fake) = rig(page(), |_| Out::Value(captured(view())));
    let result = call(
        &port,
        BrowserCommand::Screenshot {
            tab_id: 12,
            locator: Some(NESTED.into()),
            full_page: false,
            visual_settle: None,
        },
    )
    .unwrap();
    let BrowserOutcome::Screenshot { viewport, .. } = result else {
        panic!("a screenshot");
    };
    assert_eq!(viewport.scope, CaptureScope::Target);
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 1);
    assert_eq!(kinds(&calls[0]), ["observe", "box", "owner_box"]);
    assert_eq!(frames_of(&calls[0]), ["top", "C", "T"]);
    assert!(calls[0].guard.is_none());
    assert_eq!(
        captures_seen(&fake)[0].clip,
        CaptureClip::ViewportRect {
            left: 110.0,
            top: 70.0,
            width: 40.0,
            height: 10.0
        },
        "the target's rectangle moved by the embedding document's content box"
    );
}

fn region() -> PhysicalRectangle {
    PhysicalRectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    }
}

#[test]
fn a_region_capture_magnifies_the_page_rectangle_of_the_view_it_was_chosen_on() {
    let (port, fake) = rig(page(), |_| Out::Value(captured(view())));
    let result = call(
        &port,
        BrowserCommand::ScreenshotRegion {
            tab_id: 12,
            region: region(),
            expected_viewport: expected_view(),
            visual_settle: Some(false),
        },
    )
    .unwrap();
    let BrowserOutcome::Screenshot { viewport, .. } = result else {
        panic!("a screenshot");
    };
    assert_eq!(viewport.scope, CaptureScope::Region);
    let captures = captures_seen(&fake);
    assert_eq!(
        captures[0].clip,
        CaptureClip::PageRect {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0
        }
    );
    assert_eq!(captures[0].budget, geometry::region_budget());
    assert_eq!(primitives_seen(&fake), ["capture"]);
}

#[test]
fn a_region_capture_of_a_moved_view_is_stale_and_drops_the_image() {
    for moved in [
        json!({"visual_page_y": 5}),
        json!({"zoom": 1.5}),
        json!({"device_scale": 2}),
        json!({"visual_css_width": 790}),
    ] {
        let mut facts = view();
        facts
            .as_object_mut()
            .unwrap()
            .extend(moved.as_object().unwrap().clone());
        let (port, _fake) = rig(page(), move |_| Out::Value(captured(facts.clone())));
        assert_eq!(
            call(
                &port,
                BrowserCommand::ScreenshotRegion {
                    tab_id: 12,
                    region: region(),
                    expected_viewport: expected_view(),
                    visual_settle: None,
                },
            ),
            Err(BrowserError::Primitive("stale screenshot view".into())),
            "{moved}"
        );
    }
    let mut nearly = view();
    nearly["visual_page_y"] = json!(0.5);
    let (port, _fake) = rig(page(), move |_| Out::Value(captured(nearly.clone())));
    assert!(call(
        &port,
        BrowserCommand::ScreenshotRegion {
            tab_id: 12,
            region: region(),
            expected_viewport: expected_view(),
            visual_settle: None,
        },
    )
    .is_ok());
}

fn file(bytes: usize) -> PhysicalFile {
    PhysicalFile {
        name: "report.bin".into(),
        media_type: "application/octet-stream".into(),
        data: base64::engine::general_purpose::STANDARD.encode(vec![7_u8; bytes]),
        size: bytes as u64,
    }
}

fn upload(files: Vec<PhysicalFile>) -> BrowserCommand {
    BrowserCommand::UploadFiles {
        tab_id: 12,
        locator: NESTED.into(),
        files,
    }
}

#[test]
fn an_upload_is_one_guarded_call_to_the_inputs_document() {
    let (port, fake) = rig(page(), |_| Out::Value(Value::Null));
    let result = unscoped(call(&port, scoped(all_open(), upload(vec![file(9)]))).unwrap());
    assert_eq!(
        result,
        BrowserOutcome::FilesUploaded {
            tab_id: 12,
            uploaded_count: 1,
            uploaded_bytes: 9,
            subject: Some(crate::browser::adapter::PhysicalActionSubject {
                role: "button".into(),
                name: "Choose file".into()
            }),
        }
    );
    let calls = calls_seen(&fake);
    assert_eq!(frames_of(&calls[0]), ["C"]);
    assert!(calls[0].guard.is_some());
    assert!(fake.wait(|_| true).of_kind("command_chunk").is_empty());
}

#[test]
fn an_upload_over_the_direct_frame_size_travels_in_chunks() {
    let (port, fake) = rig(page(), |_| Out::Value(Value::Null));
    let bytes = wire::DIRECT_FRAME_BYTES;
    let result = unscoped(call(&port, scoped(all_open(), upload(vec![file(bytes)]))).unwrap());
    assert!(matches!(result, BrowserOutcome::FilesUploaded { .. }));
    let seen = fake.wait(|seen| !seen.of_kind("command_chunk").is_empty());
    assert!(seen.of_kind("command_chunk").len() > 1);
    drop(seen);
    let calls = calls_seen(&fake);
    assert_eq!(
        calls[0].calls[0].message["files"][0]["size"],
        u64::try_from(bytes).unwrap(),
        "the extension reassembled the whole request"
    );
}

fn evaluate(script: &str) -> BrowserCommand {
    BrowserCommand::EvaluateScript {
        tab_id: 12,
        script: script.into(),
        max_result_chars: 100,
    }
}

fn evaluated_seen(fake: &FakeExtension) -> Vec<wire::EvaluateMain> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::EvaluateMain(evaluate) => Some(evaluate.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_script_runs_in_the_main_world_with_the_scope_guard() {
    let (port, fake) = rig(page(), |_| {
        Out::Effect(json!({"value": "42", "truncated": false, "exception": null}))
    });
    let result = unscoped(call(&port, scoped(all_open(), evaluate("6 * 7"))).unwrap());
    let BrowserOutcome::ScriptEvaluated {
        tab,
        value,
        truncated,
        ..
    } = result
    else {
        panic!("a script result");
    };
    assert_eq!((tab.tab_id, value.as_str(), truncated), (12, "42", false));
    let evaluated = evaluated_seen(&fake);
    assert_eq!(evaluated[0].script, "6 * 7");
    assert_eq!(evaluated[0].max_result_chars, 100);
    assert_eq!(evaluated[0].settle_ms, 100);
    assert!(evaluated[0].guard.is_some());

    call(&port, evaluate("6 * 7")).unwrap();
    assert!(
        evaluated_seen(&fake)[1].guard.is_none(),
        "work sent without a scope carries no guard"
    );
}

#[test]
fn a_script_that_threw_may_already_have_acted() {
    let (port, _fake) = rig(page(), |_| {
        Out::Effect(json!({"value": "", "truncated": false,
            "exception": "ReferenceError: missing is not defined"}))
    });
    assert_eq!(
        call(&port, scoped(all_open(), evaluate("missing()"))),
        Err(BrowserError::EffectUnknown(
            "ReferenceError: missing is not defined".into()
        ))
    );
}
