use serde_json::json;

use super::*;
use crate::browser::adapter::chromium::protocol4::wire::CallFailure;
use crate::browser::adapter::documents::PhysicalDocument;

fn physical(id: &str, parent: Option<&str>, url: &str) -> PhysicalDocument {
    PhysicalDocument {
        id: id.into(),
        url: url.into(),
        parent: parent.map(Into::into),
        supported: true,
    }
}

fn doc(frame: &str, loader: &str) -> FrameRef {
    FrameRef {
        frame_id: frame.into(),
        loader_id: loader.into(),
    }
}

/// A top document with one embedded document that policy excludes.
fn two_documents() -> Vec<PhysicalDocument> {
    vec![
        physical("T.t1", None, "https://allowed.test/"),
        physical("C.c1", Some("T.t1"), "https://excluded.test/"),
    ]
}

/// A top document with two embeds, one of which holds an embed of its own.
fn nested_documents() -> Vec<PhysicalDocument> {
    vec![
        physical("T.t1", None, "https://top.test/"),
        physical("A.a1", Some("T.t1"), "https://a.test/"),
        physical("A1.x1", Some("A.a1"), "https://a1.test/"),
        physical("B.b1", Some("T.t1"), "https://b.test/"),
    ]
}

fn scope_of(documents: Vec<PhysicalDocument>, allowed: &[&str]) -> DocumentScope {
    DocumentScope {
        documents,
        allowed: allowed.iter().map(|id| (*id).into()).collect(),
        subjects: vec![],
        mask: None,
        watch_changes: true,
        strict_tree: true,
        settle_ms: 0,
        settle_requested: true,
    }
}

fn answered(value: serde_json::Value) -> CallAnswer {
    CallAnswer::Returned {
        envelope: json!({"ok": true, "value": value}),
    }
}

fn refused(code: &str) -> CallAnswer {
    CallAnswer::Returned {
        envelope: json!({"ok": false, "error": {"code": code, "message": "m"}}),
    }
}

fn read(scope: &Scope, doc: &FrameRef) -> PlannedCall {
    scope.call(doc, world::read_text(None, None, 100)).unwrap()
}

fn outcome() -> BrowserOutcome {
    BrowserOutcome::Presented { rendered: false }
}

#[test]
fn a_content_call_outside_allowed_is_refused_before_anything_is_sent() {
    let scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    assert_eq!(
        scope.call(&doc("C", "c1"), world::read_text(None, None, 100)),
        Err(BrowserError::DocumentUnavailable)
    );
    assert_eq!(
        scope.call(&doc("C", "c1"), world::type_text("l", "t", false, false)),
        Err(BrowserError::DocumentUnavailable)
    );
    let planned = read(&scope, &doc("T", "t1"));
    assert_eq!(planned.call.frame, Some(doc("T", "t1")));
    assert_eq!(planned.call.owner_of, None);
    assert_eq!(planned.call.message["kind"], "read_text");
    assert_eq!(planned.scope, ScopeClass::Content);
    assert_eq!(planned.effect, EffectClass::Read);
}

#[test]
fn a_metadata_call_reaches_any_document_in_the_tree_and_no_other() {
    let scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let planned = scope
        .call(&doc("C", "c1"), world::viewport_point(1.0, 2.0))
        .unwrap();
    assert_eq!(planned.scope, ScopeClass::Metadata);
    assert_eq!(
        scope.call(&doc("C", "c2"), world::viewport_point(1.0, 2.0)),
        Err(BrowserError::DocumentUnavailable)
    );
    assert_eq!(
        scope.call(&doc("Z", "z1"), world::scroll_offset()),
        Err(BrowserError::DocumentUnavailable)
    );
}

#[test]
fn a_reused_frame_id_cannot_revive_a_handle_to_a_replaced_document() {
    let scope = Scope::admit(&scope_of(two_documents(), &["T.t1", "C.c1"])).unwrap();
    for message in [world::read_text(None, None, 1), world::focus_probe()] {
        assert_eq!(
            scope.call(&doc("C", "c-replaced"), message),
            Err(BrowserError::DocumentUnavailable)
        );
    }
}

#[test]
fn an_allowed_document_must_be_a_real_document_the_adapter_can_address() {
    let mut unsupported = two_documents();
    unsupported[1].supported = false;
    let placeholder = vec![
        physical("T.t1", None, "https://allowed.test/"),
        physical("unseen.0", Some("T.t1"), "https://hidden.test/"),
    ];
    for (documents, allowed) in [
        (two_documents(), "Missing.m1"),
        (two_documents(), "C.c2"),
        (two_documents(), "C"),
        (unsupported, "C.c1"),
        (placeholder, "unseen.0"),
    ] {
        assert_eq!(
            Scope::admit(&scope_of(documents, &[allowed])).unwrap_err(),
            BrowserError::DocumentUnavailable,
            "{allowed}"
        );
    }
    assert_eq!(
        Scope::admit(&scope_of(vec![], &[])).unwrap_err(),
        BrowserError::DocumentUnavailable
    );
}

#[test]
fn a_subject_that_is_not_a_real_document_is_dropped() {
    let mut scope = scope_of(two_documents(), &["T.t1"]);
    scope.strict_tree = false;
    scope.subjects = vec!["C.c9".into(), "unseen.0".into(), "T.t1".into()];
    let admitted = Scope::admit(&scope).unwrap();
    assert_eq!(
        admitted
            .guard()
            .documents
            .iter()
            .map(|d| d.frame_id.as_str())
            .collect::<Vec<_>>(),
        ["T"]
    );
}

#[test]
fn the_strict_guard_pins_the_whole_tree_with_its_urls() {
    let scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let guard = scope.guard();
    assert!(guard.exact);
    assert_eq!(guard.unseen, 0);
    assert_eq!(
        guard
            .documents
            .iter()
            .map(|d| (d.frame_id.as_str(), d.loader_id.as_str(), d.url.as_str()))
            .collect::<Vec<_>>(),
        [
            ("T", "t1", "https://allowed.test/"),
            ("C", "c1", "https://excluded.test/")
        ]
    );
}

#[test]
fn the_strict_guard_counts_unseen_frames_and_lists_no_placeholder() {
    let documents = vec![
        physical("T.t1", None, "https://allowed.test/"),
        physical("unseen.0", Some("T.t1"), "https://hidden.test/"),
        physical("unseen.1", Some("T.t1"), "https://other.test/"),
    ];
    let guard = Scope::admit(&scope_of(documents, &["T.t1"]))
        .unwrap()
        .guard();
    assert!(guard.exact);
    assert_eq!(guard.unseen, 2);
    assert_eq!(guard.documents.len(), 1);
}

#[test]
fn the_required_guard_names_subjects_the_top_and_input_subjects_with_their_ancestors() {
    let mut authority = scope_of(nested_documents(), &["T.t1", "A.a1", "A1.x1"]);
    authority.strict_tree = false;
    let mut scope = Scope::admit(&authority).unwrap();
    let names = |scope: &Scope| -> Vec<String> {
        let guard = scope.guard();
        assert!(!guard.exact);
        assert_eq!(guard.unseen, 0);
        guard.documents.iter().map(|d| d.frame_id.clone()).collect()
    };
    assert_eq!(names(&scope), ["T"]);
    scope.admit_input_subject(&doc("A1", "x1")).unwrap();
    assert_eq!(names(&scope), ["T", "A", "A1"]);
}

#[test]
fn the_required_guard_names_the_subjects_of_the_authority() {
    let mut authority = scope_of(nested_documents(), &["T.t1", "A.a1", "A1.x1"]);
    authority.strict_tree = false;
    authority.subjects = vec!["A1.x1".into()];
    let guard = Scope::admit(&authority).unwrap().guard();
    assert_eq!(
        guard
            .documents
            .iter()
            .map(|d| d.frame_id.as_str())
            .collect::<Vec<_>>(),
        ["T", "A", "A1"]
    );
}

#[test]
fn input_cannot_be_admitted_into_a_document_that_is_not_allowed() {
    let mut authority = scope_of(two_documents(), &["T.t1"]);
    authority.strict_tree = false;
    let mut scope = Scope::admit(&authority).unwrap();
    assert_eq!(
        scope.admit_input_subject(&doc("C", "c1")),
        Err(BrowserError::DocumentUnavailable)
    );
    assert_eq!(scope.guard().documents.len(), 1);
    assert_eq!(scope.admit_input_subject(&doc("T", "t1")), Ok(()));
}

#[test]
fn settling_calls_share_one_remaining_budget_and_zero_leaves_no_wait() {
    for (budget, waits) in [(0, false), (1_000, true)] {
        let mut authority = scope_of(two_documents(), &["T.t1"]);
        authority.settle_ms = budget;
        let scope = Scope::admit(&authority).unwrap();
        let planned = scope
            .call(&doc("T", "t1"), world::read_text(None, None, 10))
            .unwrap();
        assert_eq!(planned.call.message["visual_settle"], true);
        assert!(planned.call.message["settle_ms"].as_u64().unwrap() <= budget);
        assert_eq!(
            planned.call.message["settle_ms"].as_u64().unwrap() > 0,
            waits
        );
    }
}

#[test]
fn a_spent_budget_leaves_nothing_for_the_next_call() {
    let mut authority = scope_of(two_documents(), &["T.t1"]);
    authority.settle_ms = 1_000;
    let mut scope = Scope::admit(&authority).unwrap();
    scope.settle_until = Instant::now() + Duration::from_millis(40);
    assert!(scope.remaining_settle_ms() <= 40);
    scope.settle_until = Instant::now()
        .checked_sub(Duration::from_millis(1))
        .unwrap_or_else(Instant::now);
    let planned = read(&scope, &doc("T", "t1"));
    assert_eq!(planned.call.message["settle_ms"], 0);
    assert_eq!(planned.call.message["visual_settle"], true);
}

#[test]
fn a_declined_settle_stays_declined_whatever_the_budget() {
    let mut authority = scope_of(two_documents(), &["T.t1"]);
    authority.settle_ms = 1_000;
    authority.settle_requested = false;
    let scope = Scope::admit(&authority).unwrap();
    let planned = read(&scope, &doc("T", "t1"));
    assert!(planned.call.message["settle_ms"].as_u64().unwrap() > 0);
    assert_eq!(planned.call.message["visual_settle"], false);
}

#[test]
fn the_budget_is_capped_and_other_kinds_get_none() {
    let mut authority = scope_of(two_documents(), &["T.t1"]);
    authority.settle_ms = 60_000;
    let scope = Scope::admit(&authority).unwrap();
    assert!(scope.remaining_settle_ms() <= 1_000);
    let describe = scope
        .call(&doc("T", "t1"), world::describe_focused())
        .unwrap();
    assert!(describe.call.message.get("settle_ms").is_none());
    assert!(describe.call.message.get("visual_settle").is_none());
}

#[test]
fn an_answer_marks_its_document_visited_and_a_failure_marks_it_unavailable() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1", "C.c1"])).unwrap();
    let top = read(&scope, &doc("T", "t1"));
    let child = read(&scope, &doc("C", "c1"));
    scope.record(&top, &answered(json!({"text": "permitted"})));
    scope.record(
        &child,
        &CallAnswer::Failed {
            code: CallFailure::DocumentChanged,
            message: String::new(),
            started: false,
        },
    );
    let observed = scope.observation(&outcome());
    assert_eq!(observed.visited, ["T.t1"]);
    assert_eq!(observed.unavailable, ["C.c1"]);
    assert!(!observed.limited_by_size);
}

#[test]
fn an_asset_failure_marks_its_document_unavailable() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let top = read(&scope, &doc("T", "t1"));
    scope.record(&top, &refused("page_exception"));
    let observed = scope.observation(&outcome());
    assert!(observed.visited.is_empty());
    assert_eq!(observed.unavailable, ["T.t1"]);
}

#[test]
fn an_unmet_condition_still_counts_as_visited() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let top = read(&scope, &doc("T", "t1"));
    scope.record(&top, &refused("unsatisfied"));
    let observed = scope.observation(&outcome());
    assert_eq!(observed.visited, ["T.t1"]);
    assert!(observed.unavailable.is_empty());
}

#[test]
fn a_call_that_never_ran_records_nothing() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let top = read(&scope, &doc("T", "t1"));
    scope.record(&top, &CallAnswer::Skipped);
    let observed = scope.observation(&outcome());
    assert!(observed.visited.is_empty());
    assert!(observed.unavailable.is_empty());
}

#[test]
fn an_unknown_answer_marks_its_document_unavailable() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let top = read(&scope, &doc("T", "t1"));
    scope.record(&top, &CallAnswer::Unknown);
    assert_eq!(scope.observation(&outcome()).unavailable, ["T.t1"]);
}

#[test]
fn metadata_calls_and_unadmitted_documents_record_nothing() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let metadata = scope
        .call(&doc("C", "c1"), world::viewport_point(1.0, 2.0))
        .unwrap();
    scope.record(&metadata, &answered(json!({"x": 1})));
    let outside = PlannedCall::unscoped(&doc("C", "c1"), world::read_text(None, None, 1));
    scope.record(&outside, &answered(json!({"text": "leaked"})));
    let top = PlannedCall::top(world::read_text(None, None, 1));
    scope.record(&top, &answered(json!({"text": "unnamed"})));
    let observed = scope.observation(&outcome());
    assert!(observed.visited.is_empty());
    assert!(observed.unavailable.is_empty());
}

#[test]
fn what_a_request_reached_is_a_subset_of_what_was_admitted() {
    let mut scope = Scope::admit(&scope_of(nested_documents(), &["T.t1", "A.a1"])).unwrap();
    let planned: Vec<_> = nested_documents()
        .iter()
        .map(|document| {
            let (frame, loader) = document.id.split_once('.').unwrap();
            PlannedCall::unscoped(&doc(frame, loader), world::read_text(None, None, 1))
        })
        .collect();
    for call in &planned {
        scope.record(call, &answered(json!({"text": "x"})));
    }
    let observed = scope.observation(&outcome());
    assert_eq!(observed.visited, ["T.t1", "A.a1"]);
}

#[test]
fn a_truncated_answer_or_result_sets_limited() {
    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let top = read(&scope, &doc("T", "t1"));
    scope.record(&top, &answered(json!({"text": "t", "truncated": false})));
    assert!(!scope.observation(&outcome()).limited_by_size);
    scope.record(&top, &answered(json!({"text": "t", "truncated": true})));
    assert!(scope.observation(&outcome()).limited_by_size);

    let fresh = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let cut = BrowserOutcome::Text {
        tab_id: 1,
        text: String::new(),
        truncated: true,
        title: String::new(),
        url: String::new(),
    };
    assert!(fresh.observation(&cut).limited_by_size);

    let mut limited = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    limited.limit();
    assert!(limited.observation(&outcome()).limited_by_size);
}

#[test]
fn a_governed_page_needs_point_routing_and_an_open_one_does_not() {
    let open = Scope::admit(&scope_of(two_documents(), &["T.t1", "C.c1"])).unwrap();
    assert!(!open.needs_point_check());
    let governed = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    assert!(governed.needs_point_check());

    let mut unsupported = two_documents();
    unsupported[1].supported = false;
    let closed = Scope::admit(&scope_of(unsupported, &["T.t1"])).unwrap();
    assert!(closed.needs_point_check());

    let hidden = vec![
        physical("T.t1", None, "https://allowed.test/"),
        physical("unseen.0", Some("T.t1"), "https://hidden.test/"),
    ];
    assert!(Scope::admit(&scope_of(hidden, &["T.t1"]))
        .unwrap()
        .needs_point_check());
}

#[test]
fn content_documents_are_the_allowed_ones_in_tree_order() {
    let scope = Scope::admit(&scope_of(nested_documents(), &["B.b1", "T.t1", "A1.x1"])).unwrap();
    assert_eq!(
        scope.content_docs().cloned().collect::<Vec<_>>(),
        [doc("T", "t1"), doc("A1", "x1"), doc("B", "b1")]
    );
    assert_eq!(scope.top(), doc("T", "t1"));
}

#[test]
fn a_locator_names_its_document_and_without_one_the_top_is_used() {
    let scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    assert_eq!(
        scope.doc_of(Some("C:c1:locator_a1b2c3d4_2")),
        Ok(doc("C", "c1"))
    );
    assert_eq!(
        scope.doc_or_top(Some("C:c1:locator_a1b2c3d4_2")),
        Ok(doc("C", "c1"))
    );
    assert_eq!(scope.doc_or_top(None), Ok(doc("T", "t1")));
    let stale = Err(BrowserError::Primitive("stale browser locator".into()));
    assert_eq!(scope.doc_of(None), stale);
    assert_eq!(scope.doc_or_top(Some("locator_3")), stale);
}

#[test]
fn an_owner_call_names_the_frame_it_owns_and_is_not_admitted_by_the_scope() {
    let planned = PlannedCall::owner(&doc("T", "t1"), "C", world::owner_box());
    assert_eq!(planned.call.frame, Some(doc("T", "t1")));
    assert_eq!(planned.call.owner_of.as_deref(), Some("C"));
    assert_eq!(planned.scope, ScopeClass::Metadata);
    assert_eq!(planned.effect, EffectClass::Read);
}
