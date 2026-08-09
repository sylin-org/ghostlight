// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Exact operation-to-mechanism architecture guards for Ghostlight's sole operation model.

use ghostlight::browser::mechanism::{
    compile_operation, dynamic_operation_plan, operation_plan, MechanismId, OperationMechanismPlan,
};
use ghostlight::operation::registry::{descriptors, Handler};
use ghostlight_transport::operation::OperationKind;
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn every_operation_has_one_registry_row_and_one_physical_plan() {
    assert_eq!(descriptors().len(), OperationKind::ALL.len());

    let mut seen = BTreeSet::new();
    for descriptor in descriptors() {
        assert!(seen.insert(descriptor.operation));
        match (descriptor.handler, operation_plan(descriptor.operation)) {
            (Handler::Mechanism, OperationMechanismPlan::Direct)
            | (Handler::Composition, OperationMechanismPlan::Composition)
            | (Handler::Local(_), OperationMechanismPlan::Local)
            | (Handler::Local(_), OperationMechanismPlan::Dynamic(_)) => {}
            (handler, plan) => panic!(
                "{} has inconsistent handler {handler:?} and plan {plan:?}",
                descriptor.operation
            ),
        }
    }
}

#[test]
fn direct_operations_compile_to_the_expected_private_mechanism() {
    use MechanismId as M;
    use OperationKind as O;

    let cases = [
        (O::BrowserListTabs, json!({}), M::WorkspaceTabsInspect),
        (O::BrowserFocusTab, json!({"tab": 7}), M::TabFocus),
        (O::BrowserCloseTab, json!({"tab": 7}), M::TabClose),
        (O::BrowserGoBack, json!({"tab": 7}), M::NavigateBack),
        (O::BrowserGoForward, json!({"tab": 7}), M::NavigateForward),
        (O::BrowserReloadPage, json!({"tab": 7}), M::NavigateReload),
        (O::BrowserInspectPage, json!({"tab": 7}), M::PageSnapshot),
        (
            O::BrowserInspectPage,
            json!({"tab": 7, "query": "Save"}),
            M::PageFind,
        ),
        (
            O::BrowserScrollPage,
            json!({"tab": 7, "direction": "down", "amount": "page"}),
            M::WheelScroll,
        ),
        (O::BrowserPressEscape, json!({"tab": 7}), M::KeyPress),
        (O::BrowserGetDialog, json!({"tab": 7}), M::DialogInspect),
        (
            O::BrowserHandleDialog,
            json!({"tab": 7, "action": "accept"}),
            M::DialogAccept,
        ),
        (
            O::BrowserHandleDialog,
            json!({"tab": 7, "action": "dismiss"}),
            M::DialogDismiss,
        ),
        (
            O::BrowserHandleDialog,
            json!({"tab": 7, "action": "respond", "response": "yes"}),
            M::DialogRespond,
        ),
    ];

    for (operation, input, expected) in cases {
        let request = compile_operation(operation, &input)
            .expect("direct operation compiles")
            .expect("direct operation emits one mechanism");
        assert_eq!(request.id(), expected, "{operation}");
    }
}

#[test]
fn every_dynamic_mechanism_is_owned_by_a_ghostlight_operation() {
    let mut allowed = BTreeSet::new();
    for operation in OperationKind::ALL {
        if let Some(plan) = dynamic_operation_plan(*operation) {
            allowed.extend(plan.allowed_mechanisms.iter().copied());
            assert!(plan.allowed_controls.is_empty());
        }
    }

    for mechanism in allowed {
        assert!(
            MechanismId::ALL.contains(&mechanism),
            "operation plan references an unknown mechanism: {mechanism}"
        );
    }
}
