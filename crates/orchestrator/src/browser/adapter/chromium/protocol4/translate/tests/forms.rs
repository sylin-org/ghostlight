//! Form fill against a scripted protocol-4 extension.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use serde_json::{json, Value};

use super::super::super::keys;
use super::*;
use crate::browser::adapter::PhysicalField;
use crate::browser::BrowserDispatch;
use crate::governance::ReasonCode;
use crate::language::field_problem::FieldProblemKind;

/// A value the page sets itself; every other value is typed by the browser.
const LOCAL_VALUE: &str = "on";
/// A text value the browser types.
const TEXT_VALUE: &str = "Ada";

fn field(locator: &str, value: &str) -> PhysicalField {
    PhysicalField {
        locator: locator.into(),
        value: value.into(),
    }
}

fn fill(fields: Vec<PhysicalField>, submit: Option<&str>, timeout_ms: u64) -> BrowserCommand {
    BrowserCommand::Fill {
        tab_id: 12,
        fields,
        submit_locator: submit.map(Into::into),
        timeout_ms,
        allow_credentials: false,
    }
}

fn all_open() -> DocumentScope {
    scope(&["T.t1", "C.c1", "D.d1"], false)
}

/// The kind of every field a prepare message names, from its value.
fn plan(message: &Value) -> Value {
    let kinds: Vec<&str> = message["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| {
            if field["value"] == LOCAL_VALUE {
                "local"
            } else {
                "browser_text"
            }
        })
        .collect();
    returned(json!({"prepared": true, "field_kinds": kinds}))
}

/// A page that accepts every fill message.
fn answer(world: &wire::WorldCall) -> Value {
    match world.message["kind"].as_str().unwrap_or_default() {
        "prepare_fill" => plan(&world.message),
        "fill_local" => returned(json!({"filled": true})),
        "focus_text_field" => returned(json!({"focused": true,
            "subject": {"role": "textbox", "name": "Name"}})),
        "verify_fill_retained" => returned(json!({"retained": true})),
        "submit_fill" => returned(json!({"submitted": true})),
        _ => refused("unknown_kind"),
    }
}

/// A port whose page answers through `page`, and whose extension finishes every input.
fn rig(
    page: impl Fn(&wire::WorldCall) -> Value + Send + 'static,
) -> (RelayBrowserPort, FakeExtension) {
    port_with(Box::new(move |correlation, primitive| {
        Reply::Now(match primitive {
            wire::Primitive::Frames(_) => FakeExtension::ok(correlation, inventory()),
            wire::Primitive::ListTabs(_) => FakeExtension::ok(
                correlation,
                json!({"tabs": [{"tab_id": 12, "title": "Top", "url": TOP, "active": true,
                    "readiness": "complete"}]}),
            ),
            wire::Primitive::Call(call) => FakeExtension::ok(
                correlation,
                json!({"answers": call.calls.iter().map(&page).collect::<Vec<_>>()}),
            ),
            wire::Primitive::Input(input) => {
                let precondition = input.precondition.as_ref().map(&page);
                let refused = precondition
                    .as_ref()
                    .is_some_and(|answer| answer["envelope"]["ok"] == false);
                let completed = if refused { 0 } else { input.steps.len() };
                wire::Receipt {
                    dispatched: !refused,
                    ..FakeExtension::ok(
                        correlation,
                        json!({"completed_steps": completed, "precondition": precondition}),
                    )
                }
            }
            _ => FakeExtension::ok(correlation, Value::Null),
        })
    }))
}

fn kinds(call: &wire::Call) -> Vec<String> {
    call.calls
        .iter()
        .map(|world| world.message["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn inputs_seen(fake: &FakeExtension) -> Vec<wire::Input> {
    fake.wait(|_| true)
        .requests
        .iter()
        .filter_map(|(_, primitive)| match primitive {
            wire::Primitive::Input(input) => Some(input.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_fill_across_documents_groups_fields_by_document_and_submits_last() {
    let (port, fake) = rig(answer);
    let fields = vec![
        field("T:t1:locator_a1_1", TEXT_VALUE),
        field("C:c1:locator_a1_2", LOCAL_VALUE),
        field("T:t1:locator_a1_3", LOCAL_VALUE),
    ];
    let (observation, result) = unwrap_scoped(
        call(
            &port,
            scoped(all_open(), fill(fields, Some("T:t1:locator_a1_9"), 10_000)),
        )
        .unwrap(),
    );
    let BrowserOutcome::Filled {
        tab,
        filled_count,
        submitted,
        ..
    } = result
    else {
        panic!("a fill");
    };
    assert_eq!(tab.tab_id, 12);
    assert_eq!(filled_count, 3);
    assert!(submitted);
    assert_eq!(observation.visited, ["T.t1", "C.c1"]);

    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0].mode, wire::CallMode::Parallel);
    assert_eq!(kinds(&calls[0]), ["prepare_fill", "prepare_fill"]);
    assert_eq!(frames_of(&calls[0]), ["T", "C"], "first appearance order");
    assert!(calls[0].guard.is_some());
    assert_eq!(
        calls[0].calls[0].message["fields"],
        json!([{"locator": "locator_a1_1", "value": TEXT_VALUE},
               {"locator": "locator_a1_3", "value": LOCAL_VALUE}]),
        "the page reads document-local locators"
    );
    assert_eq!(calls[0].calls[0].message["submit_locator"], "locator_a1_9");
    assert!(calls[0].calls[1].message.get("submit_locator").is_none());
    assert_eq!(kinds(&calls[1]), ["fill_local"]);
    assert_eq!(frames_of(&calls[1]), ["T"]);
    assert_eq!(kinds(&calls[2]), ["fill_local"]);
    assert_eq!(frames_of(&calls[2]), ["C"]);
    assert_eq!(calls[3].mode, wire::CallMode::Parallel);
    assert_eq!(
        kinds(&calls[3]),
        ["verify_fill_retained", "verify_fill_retained"]
    );
    assert_eq!(
        calls[3].settle_ms, 0,
        "a submit settles after its click instead"
    );
    assert!(calls[3].calls[0].message["limit_ms"].as_u64().unwrap() <= 2_000);
    assert_eq!(kinds(&calls[4]), ["submit_fill"]);
    assert_eq!(frames_of(&calls[4]), ["T"]);
    assert_eq!(calls[4].settle_ms, 250);

    let inputs = inputs_seen(&fake);
    assert_eq!(inputs.len(), 1);
    assert_eq!(
        primitives_order(&fake),
        ["call", "input", "call", "call", "call", "call", "list_tabs"]
    );
}

/// The wire name of every primitive the extension received, in order.
fn primitives_order(fake: &FakeExtension) -> Vec<String> {
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

#[test]
fn every_document_prepares_before_any_field_is_edited() {
    let (port, fake) = rig(|world| {
        if world
            .frame
            .as_ref()
            .is_some_and(|frame| frame.frame_id == "C")
        {
            refused("read_only")
        } else {
            answer(world)
        }
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(
                vec![
                    field("T:t1:locator_a1_1", LOCAL_VALUE),
                    field("C:c1:locator_a1_2", LOCAL_VALUE),
                ],
                None,
                10_000,
            ),
        ),
    )
    .unwrap_err();
    assert_eq!(error, BrowserError::Primitive("read_only".into()));
    assert_eq!(primitives_order(&fake), ["call"], "no field was edited");
}

#[test]
fn documents_prepare_together_inside_one_settle_budget() {
    const BUDGET_MS: u64 = 400;
    let (port, fake) = rig(answer);
    let mut open = all_open();
    open.settle_ms = BUDGET_MS;
    call(
        &port,
        scoped(
            open,
            fill(
                vec![
                    field("T:t1:locator_a1_1", LOCAL_VALUE),
                    field("C:c1:locator_a1_2", LOCAL_VALUE),
                    field("D:d1:locator_a1_3", LOCAL_VALUE),
                ],
                None,
                10_000,
            ),
        ),
    )
    .unwrap();
    let prepare = &calls_seen(&fake)[0];
    assert_eq!(kinds(prepare), ["prepare_fill"; 3]);
    assert_eq!(
        prepare.mode,
        wire::CallMode::Parallel,
        "waits that run one after another would add up past the budget"
    );
    for world in &prepare.calls {
        let settle = world.message["settle_ms"].as_u64().unwrap();
        assert!(
            settle > 0 && settle <= BUDGET_MS,
            "each document waits at most the one shared budget"
        );
    }
}

#[test]
fn consecutive_local_fields_share_one_sequence() {
    let (port, fake) = rig(answer);
    call(
        &port,
        scoped(
            all_open(),
            fill(
                vec![
                    field("T:t1:locator_a1_1", LOCAL_VALUE),
                    field("T:t1:locator_a1_2", LOCAL_VALUE),
                    field("T:t1:locator_a1_3", LOCAL_VALUE),
                ],
                None,
                10_000,
            ),
        ),
    )
    .unwrap();
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1].mode, wire::CallMode::Sequence);
    assert_eq!(kinds(&calls[1]), ["fill_local", "fill_local", "fill_local"]);
    assert!(calls[1].guard.is_some());
    assert_eq!(
        calls[2].settle_ms, 25,
        "a fill without a submit settles briefly"
    );
    assert!(inputs_seen(&fake).is_empty());
}

#[test]
fn a_text_field_is_one_input_behind_its_focus() {
    let (port, fake) = rig(answer);
    call(
        &port,
        scoped(
            all_open(),
            fill(vec![field("C:c1:locator_a1_1", TEXT_VALUE)], None, 10_000),
        ),
    )
    .unwrap();
    let inputs = inputs_seen(&fake);
    assert_eq!(inputs.len(), 1);
    let precondition = inputs[0].precondition.as_ref().unwrap();
    assert_eq!(precondition.message["kind"], "focus_text_field");
    assert_eq!(precondition.message["field"]["locator"], "locator_a1_1");
    assert_eq!(precondition.frame.as_ref().unwrap().frame_id, "C");
    assert_eq!(inputs[0].steps, keys::replace_text(TEXT_VALUE));
    assert!(inputs[0].guard.is_some());
    assert!(calls_seen(&fake)
        .iter()
        .all(|call| !kinds(call).contains(&"focus_text_field".to_owned())));
}

#[test]
fn a_refused_focus_is_a_plain_failure_with_no_key_sent() {
    let (port, _fake) = rig(|world| match world.message["kind"].as_str() {
        Some("focus_text_field") => refused("not_editable"),
        _ => answer(world),
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(vec![field("T:t1:locator_a1_1", TEXT_VALUE)], None, 10_000),
        ),
    )
    .unwrap_err();
    assert_eq!(error, BrowserError::Primitive("not_editable".into()));
}

#[test]
fn values_that_did_not_hold_after_edits_leave_the_effect_unknown() {
    let (port, _fake) = rig(|world| match world.message["kind"].as_str() {
        Some("verify_fill_retained") => refused("not_retained"),
        _ => answer(world),
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(vec![field("T:t1:locator_a1_1", LOCAL_VALUE)], None, 10_000),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        BrowserError::EffectUnknown("target did not retain filled value".into())
    );
}

#[test]
fn a_pause_between_fields_after_an_edit_leaves_the_effect_unknown() {
    let (port, fake) = rig(answer);
    let admitted = AtomicUsize::new(0);
    let admit = || {
        if admitted.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(())
        } else {
            Err(BrowserError::RuntimeControl(ReasonCode::RuntimeHold))
        }
    };
    let cancelled = AtomicBool::new(false);
    let error = port
        .call_guarded(
            FOUR,
            WORKSPACE,
            scoped(
                all_open(),
                fill(
                    vec![
                        field("T:t1:locator_a1_1", LOCAL_VALUE),
                        field("T:t1:locator_a1_2", TEXT_VALUE),
                    ],
                    None,
                    10_000,
                ),
            ),
            BrowserDispatch {
                deadline: Instant::now() + PATIENCE,
                cancelled: &cancelled,
                admit: &admit,
            },
        )
        .unwrap_err();
    assert_eq!(
        error,
        BrowserError::EffectUnknown("held during a multi-step action".into())
    );
    assert!(
        inputs_seen(&fake).is_empty(),
        "the held field was not typed"
    );
}

#[test]
fn a_spent_budget_after_an_edit_leaves_the_effect_unknown() {
    let (port, fake) = rig(|world| {
        if world.message["kind"] == "fill_local" {
            thread::sleep(Duration::from_millis(400));
        }
        answer(world)
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(
                vec![
                    field("T:t1:locator_a1_1", LOCAL_VALUE),
                    field("C:c1:locator_a1_2", LOCAL_VALUE),
                ],
                None,
                300,
            ),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        BrowserError::EffectUnknown("form fill exhausted its physical execution budget".into())
    );
    let calls = calls_seen(&fake);
    assert_eq!(calls.len(), 2, "the second document was never edited");
    assert_eq!(frames_of(&calls[1]), ["T"]);
}

#[test]
fn a_submit_outside_the_fields_documents_is_refused_before_anything_is_sent() {
    for scoped_fill in [true, false] {
        let (port, fake) = rig(answer);
        let command = fill(
            vec![field("T:t1:locator_a1_1", LOCAL_VALUE)],
            Some("D:d1:locator_a1_9"),
            10_000,
        );
        let command = if scoped_fill {
            scoped(all_open(), command)
        } else {
            command
        };
        assert_eq!(
            call(&port, command).unwrap_err(),
            BrowserError::Primitive("submit control is not contained in the resolved form".into())
        );
        assert!(fake.wait(|_| true).requests.is_empty());
    }
}

#[test]
fn malformed_locators_are_plain_failures_before_anything_is_sent() {
    let (port, fake) = rig(answer);
    assert_eq!(
        call(
            &port,
            fill(vec![field("locator_a1_1", LOCAL_VALUE)], None, 10_000)
        )
        .unwrap_err(),
        BrowserError::Primitive("field target is not frame-scoped".into())
    );
    assert_eq!(
        call(
            &port,
            fill(
                vec![field("T:t1:locator_a1_1", LOCAL_VALUE)],
                Some("locator_a1_9"),
                10_000
            )
        )
        .unwrap_err(),
        BrowserError::Primitive("stale browser locator".into())
    );
    assert!(fake.wait(|_| true).requests.is_empty());
}

#[test]
fn a_plan_without_one_kind_per_field_is_refused_before_any_edit() {
    let (port, fake) = rig(|world| match world.message["kind"].as_str() {
        Some("prepare_fill") => returned(json!({"prepared": true, "field_kinds": ["local"]})),
        _ => answer(world),
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(
                vec![
                    field("T:t1:locator_a1_1", LOCAL_VALUE),
                    field("T:t1:locator_a1_2", LOCAL_VALUE),
                ],
                None,
                10_000,
            ),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        BrowserError::Primitive("content primitive returned an invalid fill plan".into())
    );
    assert_eq!(primitives_order(&fake), ["call"]);
}

/// A page's refusal of a fill, naming the fields (by place in the document) it cannot take.
fn rejected(problems: Value) -> Value {
    json!({"status": "returned", "envelope": {"ok": false, "error": {
        "code": "option_missing",
        "message": "select option not found",
        "detail": {"problems": problems}
    }}})
}

#[test]
fn fields_every_document_refused_come_back_together_named_by_their_place_in_the_call() {
    let (port, fake) = rig(|world| {
        let frame = world.frame.as_ref().map(|frame| frame.frame_id.as_str());
        match (world.message["kind"].as_str(), frame) {
            (Some("prepare_fill"), Some("C")) => rejected(json!([
                {"field": 0, "problem": "option_missing", "options": ["A"]}
            ])),
            (Some("prepare_fill"), Some("T")) => rejected(json!([
                {"field": 1, "problem": "option_disabled"}
            ])),
            _ => answer(world),
        }
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(
                vec![
                    field("T:t1:locator_a1_1", LOCAL_VALUE),
                    field("C:c1:locator_a1_2", LOCAL_VALUE),
                    field("T:t1:locator_a1_3", LOCAL_VALUE),
                ],
                None,
                10_000,
            ),
        ),
    )
    .unwrap_err();
    let BrowserError::FieldsRejected { problems } = error else {
        panic!("expected the refused fields, got {error:?}");
    };
    let places: Vec<(usize, FieldProblemKind)> = problems
        .iter()
        .map(|problem| (problem.field, problem.kind))
        .collect();
    assert_eq!(
        places,
        [
            (1, FieldProblemKind::OptionMissing),
            (2, FieldProblemKind::OptionDisabled)
        ]
    );
    assert_eq!(primitives_order(&fake), ["call"], "no field was edited");
}

#[test]
fn a_problem_for_a_place_the_document_does_not_have_is_an_invalid_plan() {
    let (port, _fake) = rig(|world| match world.message["kind"].as_str() {
        Some("prepare_fill") => rejected(json!([{"field": 4, "problem": "option_missing"}])),
        _ => answer(world),
    });
    let error = call(
        &port,
        scoped(
            all_open(),
            fill(vec![field("T:t1:locator_a1_1", LOCAL_VALUE)], None, 10_000),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        BrowserError::Primitive("content primitive returned an invalid fill plan".into())
    );
}
