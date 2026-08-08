// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0101 R3 exhaustiveness guards for canonical operation -> physical mechanism planning.

use ghostlight_core::browser::mechanism::{
    auxiliary_plan, compile_operation, dynamic_operation_plan, operation_plan,
    BrowserAuxiliaryPurpose, BrowserControlId, MechanismId, OperationMechanismPlan,
};
use ghostlight_core::operation::registry::{descriptors, Handler};
use ghostlight_transport::operation::{IntentId, OperationId, OperationKey};
use serde_json::json;
use std::collections::HashSet;

#[test]
fn all_57_operations_have_one_explicit_physical_planning_class() {
    let mut direct = 0;
    let mut dynamic = 0;
    let mut composition = 0;
    let mut local = 0;

    assert_eq!(descriptors().len(), 57);
    for descriptor in descriptors() {
        let plan = operation_plan(descriptor.key).unwrap_or_else(|| {
            panic!(
                "missing physical plan for {} / {}",
                descriptor.key.id, descriptor.key.intent
            )
        });
        match (descriptor.handler, plan) {
            (Handler::Mechanism, OperationMechanismPlan::Direct) => {
                direct += 1;
                let request = compile_operation(descriptor.key, &json!({}))
                    .expect("direct compiler")
                    .expect("direct mechanism");
                assert!(MechanismId::ALL.contains(&request.id()));
            }
            (Handler::Local(_), OperationMechanismPlan::Dynamic(plan)) => {
                dynamic += 1;
                assert_eq!(dynamic_operation_plan(descriptor.key), Some(plan));
                assert_eq!(compile_operation(descriptor.key, &json!({})).unwrap(), None);
            }
            (Handler::Local(_), OperationMechanismPlan::Composition) => {
                composition += 1;
                assert_eq!(compile_operation(descriptor.key, &json!({})).unwrap(), None);
            }
            (Handler::Local(_), OperationMechanismPlan::Local) => {
                local += 1;
                assert_eq!(compile_operation(descriptor.key, &json!({})).unwrap(), None);
            }
            (Handler::Mechanism, other) => panic!(
                "direct handler {} / {} had {other:?}",
                descriptor.key.id, descriptor.key.intent
            ),
            (Handler::Local(_), other) => panic!(
                "local handler {} / {} had {other:?}",
                descriptor.key.id, descriptor.key.intent
            ),
        }
    }

    assert_eq!((direct, dynamic, composition, local), (38, 14, 2, 3));
}

#[test]
fn every_direct_operation_compiles_to_the_exact_physical_mechanism() {
    use IntentId::*;
    use MechanismId as M;
    use OperationId::*;

    let exact = [
        (BrowserTabs, TabsNew, M::WorkspaceTabCreate),
        (BrowserTabs, TabsFocus, M::TabFocus),
        (BrowserTabs, TabsClose, M::TabClose),
        (BrowserNavigate, NavigateUrl, M::NavigateUrl),
        (BrowserNavigate, NavigateBack, M::NavigateBack),
        (BrowserNavigate, NavigateForward, M::NavigateForward),
        (BrowserNavigate, NavigateReload, M::NavigateReload),
        (BrowserSnapshot, SnapshotCapture, M::PageSnapshot),
        (BrowserRead, ReadText, M::PageReadText),
        (BrowserFind, FindQuery, M::PageFind),
        (BrowserScreenshot, ScreenshotViewport, M::ScreenshotViewport),
        (BrowserScreenshot, ScreenshotRegion, M::ScreenshotRegion),
        (BrowserFill, FillField, M::FormSetValue),
        (BrowserWait, WaitDelay, M::WaitDelay),
        (BrowserWait, WaitUntil, M::WaitUntil),
        (BrowserDialog, DialogStatus, M::DialogInspect),
        (BrowserDialog, DialogAccept, M::DialogAccept),
        (BrowserDialog, DialogDismiss, M::DialogDismiss),
        (BrowserDialog, DialogRespond, M::DialogRespond),
        (BrowserInput, InputPointerClick, M::PointerClick),
        (BrowserInput, InputPointerRightClick, M::PointerClick),
        (BrowserInput, InputPointerDoubleClick, M::PointerClick),
        (BrowserInput, InputPointerTripleClick, M::PointerClick),
        (BrowserInput, InputPointerHover, M::PointerHover),
        (BrowserInput, InputPointerDrag, M::PointerDrag),
        (BrowserInput, InputTypeText, M::TextType),
        (BrowserInput, InputPressKey, M::KeyPress),
        (BrowserInput, InputWheel, M::WheelScroll),
        (BrowserInput, InputScrollToOffset, M::ScrollViewportToOffset),
        (BrowserViewport, ViewportResizeWindow, M::ViewportResize),
        (BrowserUpload, UploadClientFiles, M::UploadFiles),
        (BrowserConsole, ConsoleRead, M::ConsoleRead),
        (BrowserConsole, ConsoleReadAndClear, M::ConsoleRead),
        (BrowserNetwork, NetworkRead, M::NetworkRead),
        (BrowserNetwork, NetworkReadAndClear, M::NetworkRead),
        (BrowserEvaluate, EvaluateJavascript, M::PageEvaluate),
        (BrowserPresent, PresentNarrate, M::NarrationShow),
    ];

    assert_eq!(exact.len(), 37);
    for (id, intent, expected) in exact {
        let request = compile_operation(OperationKey::new(id, intent), &json!({}))
            .expect("direct compiler")
            .expect("direct mechanism");
        assert_eq!(request.id(), expected, "{id} / {intent}");
    }

    let list = OperationKey::new(BrowserTabs, TabsList);
    assert_eq!(
        compile_operation(list, &json!({})).unwrap().unwrap().id(),
        M::WorkspaceTabsInspect
    );
    assert_eq!(
        compile_operation(list, &json!({"create_if_empty":true}))
            .unwrap()
            .unwrap()
            .id(),
        M::WorkspaceTabsEnsure
    );
}

#[test]
fn every_semantic_operation_has_the_exact_non_direct_planning_class() {
    use IntentId::*;
    use OperationId::*;
    use OperationMechanismPlan::*;

    let dynamic = [
        (BrowserAct, ActClick),
        (BrowserAct, ActRightClick),
        (BrowserAct, ActDoubleClick),
        (BrowserAct, ActTripleClick),
        (BrowserAct, ActHover),
        (BrowserAct, ActScrollIntoView),
        (BrowserAct, ActSetValue),
        (BrowserFill, FillFields),
        (BrowserFill, FillFieldsAndSubmit),
        (BrowserUpload, UploadCapturedArtifact),
        (BrowserRecord, RecordStart),
        (BrowserRecord, RecordStop),
        (BrowserRecord, RecordClear),
        (BrowserRecord, RecordExport),
    ];
    for (id, intent) in dynamic {
        assert!(matches!(
            operation_plan(OperationKey::new(id, intent)),
            Some(Dynamic(_))
        ));
    }

    for (id, intent) in [(BrowserFlow, FlowExecute), (BrowserFlow, FlowPreflight)] {
        assert_eq!(
            operation_plan(OperationKey::new(id, intent)),
            Some(Composition)
        );
    }
    for (id, intent) in [
        (WorkflowPlan, PlanUpdate),
        (BrowserContext, ContextDescribe),
        (BrowserRecord, RecordStatus),
    ] {
        assert_eq!(operation_plan(OperationKey::new(id, intent)), Some(Local));
    }
}

#[test]
fn every_dynamic_operation_has_one_exact_allowed_mechanism_and_control_set() {
    use BrowserControlId as C;
    use IntentId::*;
    use MechanismId as M;
    use OperationId::*;

    let exact: &[(OperationKey, &[M], &[C])] = &[
        (
            OperationKey::new(BrowserAct, ActClick),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerClick,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserAct, ActRightClick),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerClick,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserAct, ActDoubleClick),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerClick,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserAct, ActTripleClick),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerClick,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserAct, ActHover),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerHover,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserAct, ActScrollIntoView),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::ScrollTargetIntoView,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserAct, ActSetValue),
            &[
                M::ElementResolve,
                M::TargetCue,
                M::FormSetValue,
                M::WaitUntil,
            ],
            &[],
        ),
        (
            OperationKey::new(BrowserFill, FillFields),
            &[M::FormInspect, M::FormSetValue],
            &[],
        ),
        (
            OperationKey::new(BrowserFill, FillFieldsAndSubmit),
            &[M::FormInspect, M::FormSetValue, M::PointerClick],
            &[],
        ),
        (
            OperationKey::new(BrowserUpload, UploadCapturedArtifact),
            &[M::UploadImage],
            &[],
        ),
        (
            OperationKey::new(BrowserRecord, RecordStart),
            &[M::RecordingStart],
            &[C::RecordingCancel],
        ),
        (
            OperationKey::new(BrowserRecord, RecordStop),
            &[M::RecordingStop],
            &[C::RecordingCancel],
        ),
        (
            OperationKey::new(BrowserRecord, RecordClear),
            &[],
            &[C::RecordingCancel],
        ),
        (
            OperationKey::new(BrowserRecord, RecordExport),
            &[M::RecordingStop, M::UploadImage],
            &[C::RecordingCancel],
        ),
    ];

    assert_eq!(exact.len(), 14);
    for (key, mechanisms, controls) in exact {
        let plan = dynamic_operation_plan(*key).expect("dynamic plan");
        assert_eq!(plan.allowed_mechanisms, *mechanisms, "{key:?}");
        assert_eq!(plan.allowed_controls, *controls, "{key:?}");
        for mechanism in mechanisms.iter().copied() {
            assert!(
                ghostlight_core::browser::mechanism::MechanismRequest::for_operation(
                    *key,
                    mechanism,
                    json!({})
                )
                .is_ok()
            );
        }
        for control in controls.iter().copied() {
            assert!(
                ghostlight_core::browser::mechanism::BrowserControl::for_operation(
                    *key,
                    control,
                    json!({})
                )
                .is_ok()
            );
        }
    }

    let click = OperationKey::new(BrowserAct, ActClick);
    assert!(
        ghostlight_core::browser::mechanism::MechanismRequest::for_operation(
            click,
            M::PageEvaluate,
            json!({})
        )
        .is_err()
    );
    assert!(
        ghostlight_core::browser::mechanism::BrowserControl::for_operation(
            click,
            C::RecordingCancel,
            json!({})
        )
        .is_err()
    );
    assert_eq!(
        auxiliary_plan(BrowserAuxiliaryPurpose::RecordingInstrumentation).allowed_mechanisms,
        &[M::PointsRescale]
    );
}

#[test]
fn reserved_operation_pairs_do_not_acquire_a_mechanism_by_name_similarity() {
    for key in [
        OperationKey::new(OperationId::BrowserAct, IntentId::ActFocus),
        OperationKey::new(OperationId::BrowserAct, IntentId::ActPressKey),
        OperationKey::new(OperationId::BrowserDownload, IntentId::ReadText),
        OperationKey::new(OperationId::BrowserVisibility, IntentId::PresentNarrate),
    ] {
        assert!(operation_plan(key).is_none());
        assert!(compile_operation(key, &json!({})).is_err());
    }
}

#[test]
fn physical_ids_are_unique_and_never_reuse_surface_or_operation_names() {
    let mut seen = HashSet::new();
    for id in MechanismId::ALL {
        assert!(seen.insert(id.as_str()), "duplicate mechanism id: {id}");
        assert!(OperationId::parse(id.as_str()).is_none());
        assert!(![
            "computer",
            "navigate",
            "find",
            "form_fill",
            "act_on",
            "browser_batch",
            "script",
            "dialog",
            "gif_creator",
        ]
        .contains(&id.as_str()));
    }
}

#[test]
fn every_physical_mechanism_is_owned_by_direct_dynamic_auxiliary_or_instrumentation() {
    let mut owned = HashSet::new();
    for descriptor in descriptors() {
        match operation_plan(descriptor.key).expect("operation plan") {
            OperationMechanismPlan::Direct => {
                let mechanism = compile_operation(descriptor.key, &json!({}))
                    .expect("direct compile")
                    .expect("direct request");
                owned.insert(mechanism.id());
            }
            OperationMechanismPlan::Dynamic(plan) => {
                owned.extend(plan.allowed_mechanisms.iter().copied());
            }
            OperationMechanismPlan::Composition | OperationMechanismPlan::Local => {}
        }
    }
    owned.insert(
        compile_operation(
            OperationKey::new(OperationId::BrowserTabs, IntentId::TabsList),
            &json!({"create_if_empty":true}),
        )
        .unwrap()
        .unwrap()
        .id(),
    );
    for purpose in BrowserAuxiliaryPurpose::ALL {
        owned.extend(auxiliary_plan(*purpose).allowed_mechanisms.iter().copied());
    }

    assert_eq!(
        owned,
        MechanismId::ALL.iter().copied().collect::<HashSet<_>>()
    );
}

#[test]
fn every_one_way_control_is_owned_by_a_dynamic_operation_or_auxiliary_purpose() {
    let mut owned = HashSet::new();
    for descriptor in descriptors() {
        if let Some(plan) = dynamic_operation_plan(descriptor.key) {
            owned.extend(plan.allowed_controls.iter().copied());
        }
    }
    for purpose in BrowserAuxiliaryPurpose::ALL {
        owned.extend(auxiliary_plan(*purpose).allowed_controls.iter().copied());
    }

    assert_eq!(
        owned,
        BrowserControlId::ALL
            .iter()
            .copied()
            .collect::<HashSet<_>>()
    );
}

#[test]
fn physical_discriminants_replace_legacy_action_strings() {
    let click = compile_operation(
        OperationKey::new(OperationId::BrowserInput, IntentId::InputPointerDoubleClick),
        &json!({"tab":4,"point":[10,20]}),
    )
    .unwrap()
    .unwrap();
    assert_eq!(click.id(), MechanismId::PointerClick);
    assert_eq!(click.input()["button"], "left");
    assert_eq!(click.input()["count"], 2);
    assert!(click.input().get("action").is_none());

    let ensured = compile_operation(
        OperationKey::new(OperationId::BrowserTabs, IntentId::TabsList),
        &json!({"create_if_empty":true}),
    )
    .unwrap()
    .unwrap();
    assert_eq!(ensured.id(), MechanismId::WorkspaceTabsEnsure);
    assert!(ensured.input().get("create_if_empty").is_none());
}
