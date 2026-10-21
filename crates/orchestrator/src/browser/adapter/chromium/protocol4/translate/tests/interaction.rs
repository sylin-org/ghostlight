//! Pointer, keyboard, capture, upload and script work against a scripted protocol-4 extension.

use serde_json::{json, Value};

use super::super::super::geometry;
use super::super::super::wire::{CaptureClip, InputStep};
use super::*;
use crate::browser::adapter::{CaptureScope, PhysicalRectangle, ScriptForm, ViewportGeometry};
use crate::browser::adapter::{PhysicalClick, PhysicalPressKey, PhysicalTypeFocused};
use crate::language::page_refusal::PageRefusal;
use crate::language::PointerButton;

const NESTED: &str = "C:c1:locator_a1_1";
/// The preparation budget the authority hands a capture in these tests, in milliseconds.
const SETTLE_MS: u64 = 300;

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
    BrowserCommand::Activate(PhysicalClick {
        tab_id: 12,
        locator: locator.into(),
        button: PointerButton::Primary,
        click_count: 1,
        modifiers: vec![],
    })
}

fn press(locator: Option<&str>, key: &str, modifiers: &[&str]) -> BrowserCommand {
    BrowserCommand::PressKey(PhysicalPressKey {
        tab_id: 12,
        locator: locator.map(Into::into),
        key: key.into(),
        modifiers: modifiers.iter().map(|name| (*name).into()).collect(),
    })
}

fn type_focused(clear_first: bool) -> BrowserCommand {
    BrowserCommand::TypeFocused(PhysicalTypeFocused {
        tab_id: 12,
        text: "hello".into(),
        clear_first,
        allow_credentials: false,
    })
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

fn a_dialog_during_a_click_is_success() {
    let (port, _fake) = rig(page(), |_| {
        Out::Effect(json!({"completed_steps": 2,
            "stopped": {"reason": "dialog", "dialog_type": "confirm"}}))
    });
    let result = unscoped(call(&port, scoped(all_open(), activate(NESTED))).unwrap());
    assert!(matches!(result, BrowserOutcome::Activated { .. }));
}

/// A refusal before any input names its own cause and sends nothing: a click whose preparation
/// failed, a click on a removed control (which offers the page's lookup bound to its document only
/// when the service reads it whole), and a key press or text whose control lost the focus, went
/// stale, moved or is not a text control; text typed into the focused control is typed only in the
/// first document that holds the focus, and a key press without a handle needs the bound subject
/// to hold it (ACCEPTANCE, executor and truth gates 6 and document boundary evidence 1).
#[test]
fn a_preparation_failure_is_its_own_refusal_and_sends_no_input() {
    for code in ["stale_locator", "not_visible", "disabled"] {
        let (port, fake) = rig(
            Box::new(move |call| call.calls.iter().map(|_| refused(code)).collect()),
            finished,
        );
        let error = call(&port, scoped(all_open(), activate(NESTED))).unwrap_err();
        let expected = match code {
            "stale_locator" => BrowserError::stale(StaleReason::Unknown),
            "not_visible" => BrowserError::PageRefused(PageRefusal::NotVisible),
            _ => BrowserError::PageRefused(PageRefusal::Disabled),
        };
        assert_eq!(error, expected, "{code}");
        assert!(inputs_seen(&fake).is_empty());
    }

    a_click_on_a_removed_control_returns_the_page_lookup_bound_to_its_document();
    a_click_on_a_removed_control_offers_no_lookup_it_cannot_read_whole();
    a_key_press_without_a_locator_requires_the_bound_subject_to_hold_the_focus();
    a_lost_focus_refuses_the_key_press_with_nothing_sent();
    a_stale_locator_refuses_the_key_press_with_nothing_sent();
    a_focus_that_moved_refuses_the_text_with_nothing_sent();
    typing_into_the_focused_control_clears_and_types_only_in_the_first_focused_document();
    typing_with_no_focused_text_control_fails_before_anything_is_sent();
}

/// The lookup a page makes in a failing click is bound to the document of the locator that
/// went stale, so its candidates are handles the service can act on.
fn a_click_on_a_removed_control_returns_the_page_lookup_bound_to_its_document() {
    let (port, fake) = rig(
        Box::new(|call| {
            call.calls
                .iter()
                .map(|_| {
                    json!({"status": "returned", "envelope": {"ok": false, "error": {
                        "code": "stale_locator",
                        "message": "stale browser locator",
                        "detail": {
                            "stale": "removed",
                            "matched": 2,
                            "same_count": 1,
                            "truncated": false,
                            "candidates": [{
                                "target": {"locator": "locator_a1_9", "role": "button",
                                    "name": "More options", "state": [], "credential_class": false,
                                    "context": "Neon Rain", "nth": 2},
                                "same": true
                            }]
                        }
                    }}})
                })
                .collect()
        }),
        finished,
    );

    let error = call(&port, scoped(all_open(), activate(NESTED))).unwrap_err();

    let BrowserError::StaleLocator {
        reason,
        lookup: Some(lookup),
    } = error
    else {
        panic!("the click was refused as stale, with a lookup: {error:?}");
    };
    assert_eq!(reason, StaleReason::Removed);
    assert_eq!((lookup.matched, lookup.same_count), (2, 1));
    assert_eq!(lookup.candidates[0].target.locator, "C:c1:locator_a1_9");
    assert!(lookup.candidates[0].same);
    assert!(inputs_seen(&fake).is_empty(), "nothing was sent");
}

fn a_click_on_a_removed_control_offers_no_lookup_it_cannot_read_whole() {
    for detail in [
        json!({"stale": "removed"}),
        json!({"stale": "removed", "matched": "many"}),
        json!({"stale": "removed", "matched": 1,
            "candidates": [{"target": {"unheard_of": 1}, "same": true}]}),
        json!({"stale": "unknown", "matched": 1, "same_count": 1}),
    ] {
        let answer = detail.clone();
        let (port, fake) = rig(
            Box::new(move |call| {
                call.calls
                    .iter()
                    .map(|_| {
                        json!({"status": "returned", "envelope": {"ok": false, "error": {
                            "code": "stale_locator", "message": "stale browser locator",
                            "detail": answer.clone()}}})
                    })
                    .collect()
            }),
            finished,
        );
        let error = call(&port, scoped(all_open(), activate(NESTED))).unwrap_err();
        assert!(
            matches!(error, BrowserError::StaleLocator { lookup: None, .. }),
            "{detail}: {error:?}"
        );
        assert!(inputs_seen(&fake).is_empty(), "{detail}: nothing was sent");
    }
}

/// Once input was dispatched, a failure leaves the effect unknown and never reads as no effect,
/// while a dialog the click itself opened is success (ACCEPTANCE, executor and truth gates 4).
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

    a_dialog_during_a_click_is_success();
    a_dialog_while_typing_leaves_the_effect_unknown();
}

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
}

/// An input the extension reports as the real one does for a refused precondition: the receipt
/// says it dispatched, because the precondition answered, but no step was sent.
fn precondition_refused(code: &'static str) -> impl Fn(&wire::Primitive) -> Out + Send + 'static {
    move |_| Out::Effect(json!({"completed_steps": 0, "precondition": refused(code)}))
}

fn a_lost_focus_refuses_the_key_press_with_nothing_sent() {
    let (port, _fake) = rig(page(), precondition_refused("focus_lost"));
    let mut bound = all_open();
    bound.subjects = vec!["C.c1".into()];
    let error = call(&port, scoped(bound, press(None, "Tab", &[]))).unwrap_err();
    assert_eq!(error, BrowserError::primitive("focus_lost"));
}

fn a_stale_locator_refuses_the_key_press_with_nothing_sent() {
    let (port, _fake) = rig(page(), move |_| {
        Out::Effect(json!({"completed_steps": 0, "precondition": {
            "status": "returned",
            "envelope": {"ok": false, "error": {
                "code": "stale_locator",
                "message": "stale browser locator",
                "detail": {"stale": "removed"}
            }}
        }}))
    });
    let error = call(&port, scoped(all_open(), press(Some(NESTED), "Enter", &[]))).unwrap_err();
    assert_eq!(error, BrowserError::stale(StaleReason::Removed));
}

fn a_focus_that_moved_refuses_the_text_with_nothing_sent() {
    let (port, _fake) = rig(
        focus_held_by(&["C"]),
        precondition_refused("no_focused_control"),
    );
    let error = call(&port, scoped(all_open(), type_focused(false))).unwrap_err();
    assert_eq!(error, BrowserError::primitive("no_focused_control"));
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

fn typing_with_no_focused_text_control_fails_before_anything_is_sent() {
    let (port, fake) = rig(focus_held_by(&[]), finished);
    let error = call(&port, scoped(all_open(), type_focused(true))).unwrap_err();
    assert_eq!(error, BrowserError::primitive("no_focused_control"));
    assert!(inputs_seen(&fake).is_empty());
    assert_eq!(calls_seen(&fake).len(), 1, "nothing was cleared");
}

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

fn a_viewport_capture_is_one_primitive_after_a_best_effort_settle() {
    let (port, fake) = rig(page(), |_| Out::Value(captured(view())));
    let result = call(
        &port,
        BrowserCommand::Screenshot {
            tab_id: 12,
            locator: None,
            full_page: false,
            settle_ms: SETTLE_MS,
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
        ["list_tabs", "call", "capture"],
        "the tab is showing, a settle, then the capture; the service sends no hide or restore calls"
    );
    let calls = calls_seen(&fake);
    assert_eq!(calls[0].calls[0].message["condition"], "visual_settle");
    assert_eq!(
        calls[0].calls[0].message["timeout_ms"], SETTLE_MS,
        "the page settles for the authority's budget"
    );
    assert!(calls[0].calls[0].frame.is_none());
    let captures = captures_seen(&fake);
    assert_eq!(captures[0].clip, CaptureClip::Viewport);
    assert_eq!(captures[0].budget, geometry::ordinary_budget());
}

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
    let shot = |full_page, settle_ms| BrowserCommand::Screenshot {
        tab_id: 12,
        locator: None,
        full_page,
        settle_ms,
    };
    let full = call(&port, shot(true, SETTLE_MS)).unwrap();
    let BrowserOutcome::Screenshot { viewport, .. } = full else {
        panic!("a screenshot");
    };
    assert_eq!(viewport.scope, CaptureScope::FullPage);
    call(&port, shot(false, 0)).unwrap();
    assert_eq!(
        primitives_seen(&fake),
        ["list_tabs", "call", "capture", "list_tabs", "capture"],
        "a zero budget sends no settle"
    );
    assert_eq!(captures_seen(&fake)[0].clip, CaptureClip::FullPage);
}

fn a_target_screenshot_under_one_second_still_measures() {
    let (port, fake) = rig(page(), |_| Out::Value(captured(view())));
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_millis(800);
    let result = port.call(
        FOUR,
        BrowserCommand::Screenshot {
            tab_id: 12,
            locator: Some(NESTED.into()),
            full_page: false,
            settle_ms: 400,
        },
        deadline,
        &cancelled,
    );
    let Ok(BrowserOutcome::Screenshot { viewport, .. }) = result else {
        panic!("a target capture inside the deadline, not {result:?}");
    };
    assert_eq!(viewport.scope, CaptureScope::Target);
    let calls = calls_seen(&fake);
    assert_eq!(kinds(&calls[0]), ["observe", "box", "owner_box"]);
    assert_eq!(
        calls[0].calls[0].message["timeout_ms"], 400,
        "the settle spends the authority's budget, not a second of its own"
    );
    assert!(calls[0].timeout_ms < 800, "the call fits in the deadline");
}

fn region() -> PhysicalRectangle {
    PhysicalRectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    }
}

/// A capture refuses a tab its window does not show before capturing anything (commit
/// efe54caf); otherwise it settles at best within the job's budget, goes on when the settle fails,
/// skips it when asked, and measures a target within its deadline (ADR-0195).
#[test]
fn a_tab_its_window_does_not_show_is_refused_before_any_capture() {
    let (port, fake) = port_with(Box::new(|correlation, primitive| {
        Reply::Now(match primitive {
            wire::Primitive::ListTabs(_) => FakeExtension::ok(
                correlation,
                json!({"tabs": [{"tab_id": 12, "title": "Top", "url": TOP, "active": false,
                                 "readiness": "complete"}]}),
            ),
            _ => FakeExtension::ok(correlation, captured(view())),
        })
    }));
    let viewport = BrowserCommand::Screenshot {
        tab_id: 12,
        locator: None,
        full_page: false,
        settle_ms: SETTLE_MS,
    };
    let region = BrowserCommand::ScreenshotRegion {
        tab_id: 12,
        region: region(),
        expected_viewport: expected_view(),
        settle_ms: SETTLE_MS,
    };
    assert_eq!(call(&port, viewport), Err(BrowserError::TabNotShowing));
    assert_eq!(call(&port, region), Err(BrowserError::TabNotShowing));
    assert_eq!(
        primitives_seen(&fake),
        ["list_tabs", "list_tabs"],
        "no settle and no capture waits for a frame the browser will not draw"
    );

    a_viewport_capture_is_one_primitive_after_a_best_effort_settle();
    a_capture_goes_on_when_the_settle_fails_and_skips_it_when_asked();
    a_target_screenshot_under_one_second_still_measures();
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

/// The scripts the extension was asked to evaluate, in order.
fn scripts_seen(fake: &FakeExtension) -> Vec<String> {
    evaluated_seen(fake)
        .into_iter()
        .map(|evaluated| evaluated.script)
        .collect()
}

fn script_value(value: &str) -> Value {
    json!({"value": value, "truncated": false, "exception": null})
}

fn script_form(outcome: BrowserOutcome) -> (String, ScriptForm) {
    match unscoped(outcome) {
        BrowserOutcome::ScriptEvaluated { value, form, .. } => (value, form),
        other => panic!("a script result, not {other:?}"),
    }
}

fn an_awaited_expression_is_sent_once_and_its_value_returned() {
    let (port, fake) = rig(page(), |_| Out::Effect(script_value("42")));
    let outcome = call(
        &port,
        scoped(
            all_open(),
            evaluate("new Promise(r => setTimeout(() => r(42), 50))"),
        ),
    )
    .unwrap();
    assert_eq!(script_form(outcome), ("42".into(), ScriptForm::Awaited));
    assert_eq!(
        scripts_seen(&fake),
        ["await (
new Promise(r => setTimeout(() => r(42), 50))
)"]
    );
}

fn a_refusal_before_dispatch_sends_the_original_once_and_returns_its_answer() {
    let (port, fake) = rig(page(), |primitive| match primitive {
        wire::Primitive::EvaluateMain(evaluate) if evaluate.script.starts_with("await (") => {
            Out::Fail(wire::Failure::InvalidRequest, false)
        }
        _ => Out::Effect(script_value("7")),
    });
    let outcome = call(&port, scoped(all_open(), evaluate("a, b"))).unwrap();
    assert_eq!(script_form(outcome), ("7".into(), ScriptForm::AsWritten));
    assert_eq!(
        scripts_seen(&fake),
        [
            "await (
a, b
)",
            "a, b"
        ]
    );
}

fn the_original_scripts_own_refusal_is_what_the_agent_sees() {
    let (port, fake) = rig(page(), |_| Out::Fail(wire::Failure::InvalidRequest, false));
    let error = call(&port, scoped(all_open(), evaluate("x y"))).unwrap_err();
    assert!(matches!(error, BrowserError::Protocol(_)), "{error:?}");
    assert_eq!(
        scripts_seen(&fake),
        [
            "await (
x y
)",
            "x y"
        ]
    );
}

/// `browser_execute` runs a script at most once: an awaited expression is sent once and its
/// value returned, only a refusal before dispatch sends the original again, the original's own
/// refusal is what the agent sees, and a script that threw may already have acted (L-D6, commit
/// 28ea74b3; ACCEPTANCE, browser job journeys 14).
#[test]
fn nothing_but_a_refusal_before_dispatch_sends_a_script_again() {
    let failures = [
        (wire::Failure::InvalidRequest, true),
        (wire::Failure::Timeout, true),
        (wire::Failure::Timeout, false),
        (wire::Failure::SessionLost, true),
        (wire::Failure::BrowserRefused, false),
        (wire::Failure::DocumentChanged, false),
        (wire::Failure::DebuggerUnavailable, false),
    ];
    for (failure, dispatched) in failures {
        let (port, fake) = rig(page(), move |_| Out::Fail(failure, dispatched));
        assert!(call(&port, scoped(all_open(), evaluate("f()"))).is_err());
        assert_eq!(
            scripts_seen(&fake),
            ["await (
f()
)"],
            "{failure:?} dispatched {dispatched}"
        );
    }

    let (port, fake) = rig(page(), |_| {
        Out::Effect(
            json!({"value": "", "truncated": false, "exception": "TypeError: f is not a function"}),
        )
    });
    assert_eq!(
        call(&port, scoped(all_open(), evaluate("f()"))),
        Err(BrowserError::ScriptThrew(
            "TypeError: f is not a function".into()
        ))
    );
    assert_eq!(scripts_seen(&fake).len(), 1);

    an_awaited_expression_is_sent_once_and_its_value_returned();
    a_refusal_before_dispatch_sends_the_original_once_and_returns_its_answer();
    the_original_scripts_own_refusal_is_what_the_agent_sees();
    a_script_that_threw_may_already_have_acted();
}

fn a_script_that_threw_may_already_have_acted() {
    let (port, _fake) = rig(page(), |_| {
        Out::Effect(json!({"value": "", "truncated": false,
            "exception": "ReferenceError: missing is not defined"}))
    });
    assert_eq!(
        call(&port, scoped(all_open(), evaluate("missing()"))),
        Err(BrowserError::ScriptThrew(
            "ReferenceError: missing is not defined".into()
        ))
    );
}
