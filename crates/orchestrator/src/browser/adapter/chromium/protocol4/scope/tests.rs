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

fn refused(code: AssetErrorCode) -> CallAnswer {
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

/// A content call or input reaches only a real document the scope allows, a metadata call only a
/// document in the tree, and a reused frame id never revives a handle to the document it
/// replaced: everything else is refused before anything is sent (ACCEPTANCE, document boundary
/// evidence 1).
#[test]
fn a_call_reaches_only_the_documents_its_scope_admits() {
    let scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let unavailable = Err(BrowserError::DocumentUnavailable);
    assert_eq!(
        scope.call(&doc("C", "c1"), world::read_text(None, None, 100)),
        unavailable
    );
    assert_eq!(
        scope.call(&doc("C", "c1"), world::type_text("l", "t", false, false)),
        unavailable
    );
    let planned = read(&scope, &doc("T", "t1"));
    assert_eq!(planned.call.frame, Some(doc("T", "t1")));
    assert_eq!(planned.call.owner_of, None);

    let metadata = scope
        .call(&doc("C", "c1"), world::viewport_point(1.0, 2.0))
        .unwrap();
    assert_eq!(metadata.scope, ScopeClass::Metadata);
    for frame in [doc("C", "c2"), doc("Z", "z1")] {
        assert_eq!(scope.call(&frame, world::scroll_offset()), unavailable);
    }

    let both = Scope::admit(&scope_of(two_documents(), &["T.t1", "C.c1"])).unwrap();
    for message in [world::read_text(None, None, 1), world::focus_probe()] {
        assert_eq!(both.call(&doc("C", "c-replaced"), message), unavailable);
    }

    let mut unsupported = two_documents();
    unsupported[1].supported = false;
    let placeholder = vec![
        physical("T.t1", None, "https://allowed.test/"),
        physical("unseen.0", Some("T.t1"), "https://hidden.test/"),
    ];
    for (documents, allowed) in [
        (two_documents(), &["Missing.m1"][..]),
        (two_documents(), &["C.c2"]),
        (two_documents(), &["C"]),
        (unsupported, &["C.c1"]),
        (placeholder, &["unseen.0"]),
        (vec![], &[]),
    ] {
        assert_eq!(
            Scope::admit(&scope_of(documents, allowed)).map(|_| ()),
            Err(BrowserError::DocumentUnavailable),
            "{allowed:?}"
        );
    }

    let mut authority = scope_of(two_documents(), &["T.t1"]);
    authority.strict_tree = false;
    let mut inputs = Scope::admit(&authority).unwrap();
    assert_eq!(
        inputs.admit_input_subject(&doc("C", "c1")),
        Err(BrowserError::DocumentUnavailable)
    );
    assert_eq!(inputs.admit_input_subject(&doc("T", "t1")), Ok(()));
}

/// The guard names every document whose change stops the work before its effect: a strict guard
/// pins the whole tree with its urls and counts the frames it cannot see, and a required guard
/// names the top, the subjects and each input subject with its ancestors, never a document that
/// is not real (ACCEPTANCE, browser job journeys 15).
#[test]
fn the_guard_names_every_document_whose_change_stops_the_work() {
    let strict = Scope::admit(&scope_of(two_documents(), &["T.t1"]))
        .unwrap()
        .guard();
    assert!(strict.exact);
    assert_eq!(strict.unseen, 0);
    assert_eq!(
        strict
            .documents
            .iter()
            .map(|d| (d.frame_id.as_str(), d.loader_id.as_str(), d.url.as_str()))
            .collect::<Vec<_>>(),
        [
            ("T", "t1", "https://allowed.test/"),
            ("C", "c1", "https://excluded.test/")
        ]
    );
    let hidden = vec![
        physical("T.t1", None, "https://allowed.test/"),
        physical("unseen.0", Some("T.t1"), "https://hidden.test/"),
        physical("unseen.1", Some("T.t1"), "https://other.test/"),
    ];
    let unseen = Scope::admit(&scope_of(hidden, &["T.t1"])).unwrap().guard();
    assert_eq!((unseen.unseen, unseen.documents.len()), (2, 1));

    let names = |scope: &Scope| -> Vec<String> {
        let guard = scope.guard();
        assert!(!guard.exact);
        guard.documents.iter().map(|d| d.frame_id.clone()).collect()
    };
    let mut authority = scope_of(nested_documents(), &["T.t1", "A.a1", "A1.x1"]);
    authority.strict_tree = false;
    let mut scope = Scope::admit(&authority).unwrap();
    assert_eq!(names(&scope), ["T"]);
    scope.admit_input_subject(&doc("A1", "x1")).unwrap();
    assert_eq!(names(&scope), ["T", "A", "A1"]);

    authority.subjects = vec!["A1.x1".into()];
    assert_eq!(names(&Scope::admit(&authority).unwrap()), ["T", "A", "A1"]);

    let mut dropped = scope_of(two_documents(), &["T.t1"]);
    dropped.strict_tree = false;
    dropped.subjects = vec!["C.c9".into(), "unseen.0".into(), "T.t1".into()];
    assert_eq!(names(&Scope::admit(&dropped).unwrap()), ["T"]);
}

/// The calls of one job share the authority's one settle budget, and the agent's choice not to
/// settle stays its own whatever the budget, so a spent budget waits no more and a declined
/// settle never waits (commit 3e80fb91, ADR-0198 Decision 35). A call of a kind that does not
/// settle carries no budget.
#[test]
fn one_settle_budget_is_shared_and_the_agents_choice_is_kept() {
    let settle = |planned: &PlannedCall| {
        (
            planned.call.message["settle_ms"].as_u64(),
            planned.call.message["visual_settle"].as_bool(),
        )
    };
    for (budget, waits) in [(0, false), (1_000, true)] {
        let mut authority = scope_of(two_documents(), &["T.t1"]);
        authority.settle_ms = budget;
        let scope = Scope::admit(&authority).unwrap();
        let (left, requested) = settle(&read(&scope, &doc("T", "t1")));
        let left = left.unwrap();
        assert!(left <= budget);
        assert_eq!((left > 0, requested), (waits, Some(true)), "{budget}");
        let describe = scope
            .call(&doc("T", "t1"), world::describe_focused())
            .unwrap();
        assert_eq!(settle(&describe), (None, None));
    }

    let mut authority = scope_of(two_documents(), &["T.t1"]);
    authority.settle_ms = 1_000;
    let mut spent = Scope::admit(&authority).unwrap();
    spent.settle_until = Instant::now()
        .checked_sub(Duration::from_millis(1))
        .unwrap_or_else(Instant::now);
    assert_eq!(
        settle(&read(&spent, &doc("T", "t1"))),
        (Some(0), Some(true))
    );

    authority.settle_requested = false;
    let declined = Scope::admit(&authority).unwrap();
    let (left, requested) = settle(&read(&declined, &doc("T", "t1")));
    assert!(left.unwrap() > 0);
    assert_eq!(requested, Some(false));
}

/// What a job reports it reached tells visited, unavailable and size-limited documents apart,
/// and never names a document the scope did not admit: an answer is a visit, an unmet condition
/// is a visit, a failure or an unknown answer is unavailable, a skipped or metadata call is
/// nothing, and a truncated answer or result is limited (ACCEPTANCE, document boundary evidence
/// 3).
#[test]
fn what_a_request_reached_is_within_what_was_admitted() {
    let mut scope = Scope::admit(&scope_of(nested_documents(), &["T.t1", "A.a1"])).unwrap();
    for document in nested_documents() {
        let (frame, loader) = document.id.split_once('.').unwrap();
        let call = PlannedCall::unscoped(&doc(frame, loader), world::read_text(None, None, 1));
        scope.record(&call, &answered(json!({"text": "x"})));
    }
    assert_eq!(scope.observation(&outcome()).visited, ["T.t1", "A.a1"]);

    let changed = CallAnswer::Failed {
        code: CallFailure::DocumentChanged,
        message: String::new(),
        started: false,
    };
    for (answer, visited, unavailable) in [
        (answered(json!({"text": "permitted"})), true, false),
        (refused(AssetErrorCode::Unsatisfied), true, false),
        (refused(AssetErrorCode::PageException), false, true),
        (changed, false, true),
        (CallAnswer::Unknown, false, true),
        (CallAnswer::Skipped, false, false),
    ] {
        let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
        let top = read(&scope, &doc("T", "t1"));
        scope.record(&top, &answer);
        let observed = scope.observation(&outcome());
        assert_eq!(
            (
                !observed.visited.is_empty(),
                !observed.unavailable.is_empty()
            ),
            (visited, unavailable),
            "{answer:?}"
        );
    }

    let mut scope = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    let metadata = scope
        .call(&doc("C", "c1"), world::viewport_point(1.0, 2.0))
        .unwrap();
    scope.record(&metadata, &answered(json!({"x": 1})));
    let top = read(&scope, &doc("T", "t1"));
    scope.record(&top, &answered(json!({"text": "t", "truncated": false})));
    let observed = scope.observation(&outcome());
    assert_eq!(observed.visited, ["T.t1"]);
    assert!(!observed.limited_by_size);
    scope.record(&top, &answered(json!({"text": "t", "truncated": true})));
    assert!(scope.observation(&outcome()).limited_by_size);

    let cut = BrowserOutcome::Text {
        tab_id: 1,
        text: String::new(),
        truncated: true,
        title: String::new(),
        url: String::new(),
    };
    let fresh = Scope::admit(&scope_of(two_documents(), &["T.t1"])).unwrap();
    assert!(fresh.observation(&cut).limited_by_size);
}
