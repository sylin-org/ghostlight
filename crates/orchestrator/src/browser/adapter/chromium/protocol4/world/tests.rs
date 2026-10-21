use serde_json::json;

use super::*;
use crate::browser::adapter::TreeNodeKind;
use crate::language::field_problem::FieldProblemKind;

fn field(locator: &str, value: &str) -> PhysicalField {
    PhysicalField {
        locator: locator.into(),
        value: value.into(),
    }
}

fn file() -> PhysicalFile {
    PhysicalFile {
        name: "notes.txt".into(),
        media_type: "text/plain".into(),
        data: "aGVsbG8=".into(),
        size: 5,
    }
}

fn two_fields() -> [PhysicalField; 2] {
    [
        field("locator_a1b2c3d4_6", "Ada Lovelace"),
        field("locator_a1b2c3d4_7", "ada@example.com"),
    ]
}

#[test]
fn every_kind_has_one_constructor_and_the_classes_of_its_table_row() {
    let fields = two_fields();
    let files = [file()];
    let table = [
        (
            read_text(None, None, 10),
            WorldKind::ReadText,
            Content,
            Read,
            true,
        ),
        (
            inspect("controls", 5),
            WorldKind::Inspect,
            Content,
            Read,
            true,
        ),
        (
            inspect_tree(None, 6, 400),
            WorldKind::InspectTree,
            Content,
            Read,
            true,
        ),
        (
            find("Save", "control", 8),
            WorldKind::Find,
            Content,
            Read,
            true,
        ),
        (
            query_semantic("Email", None, false, true, 8),
            WorldKind::QuerySemantic,
            Content,
            Read,
            true,
        ),
        (describe(&[]), WorldKind::Describe, Content, Read, false),
        (
            describe_focused(),
            WorldKind::DescribeFocused,
            Content,
            Read,
            false,
        ),
        (
            verify_focused_text(false),
            WorldKind::VerifyFocusedText,
            Content,
            Read,
            false,
        ),
        (box_("l"), WorldKind::Box, Content, Read, false),
        (
            prepare_fill(&fields, false, None),
            WorldKind::PrepareFill,
            Content,
            Read,
            true,
        ),
        (
            verify_fill_retained(&fields, false, 3_000),
            WorldKind::VerifyFillRetained,
            Content,
            Read,
            false,
        ),
        (
            observe("load_ready", None, None, 5_000, false),
            WorldKind::Observe,
            Content,
            Read,
            false,
        ),
        (activate("l"), WorldKind::Activate, Content, Read, false),
        (hover("l"), WorldKind::Hover, Content, Read, false),
        (focus("l"), WorldKind::Focus, Content, Read, false),
        (
            scroll_point(1.0, 2.0),
            WorldKind::ScrollPoint,
            Content,
            Read,
            false,
        ),
        (
            viewport_point(1.0, 2.0),
            WorldKind::ViewportPoint,
            Metadata,
            Read,
            false,
        ),
        (
            scroll_offset(),
            WorldKind::ScrollOffset,
            Metadata,
            Read,
            false,
        ),
        (
            hit_point(1.0, 2.0, false),
            WorldKind::HitPoint,
            Metadata,
            Read,
            false,
        ),
        (owner_box(), WorldKind::OwnerBox, Metadata, Read, false),
        (
            owner_probe(1.0, 2.0),
            WorldKind::OwnerProbe,
            Metadata,
            Read,
            false,
        ),
        (focus_probe(), WorldKind::FocusProbe, Metadata, Read, false),
        (
            require_focused(),
            WorldKind::RequireFocused,
            Metadata,
            Read,
            false,
        ),
        (
            fill_local(&fields[0], false),
            WorldKind::FillLocal,
            Content,
            Effect,
            false,
        ),
        (
            focus_text_field(&fields[0], false),
            WorldKind::FocusTextField,
            Content,
            Effect,
            true,
        ),
        (
            submit_fill(&fields, "l", false),
            WorldKind::SubmitFill,
            Content,
            Effect,
            false,
        ),
        (
            type_text("l", "t", true, false),
            WorldKind::TypeText,
            Content,
            Effect,
            false,
        ),
        (
            clear_focused(false),
            WorldKind::ClearFocused,
            Content,
            Effect,
            false,
        ),
        (
            scroll(None, None, None),
            WorldKind::Scroll,
            Content,
            Effect,
            false,
        ),
        (
            upload_files("l", &files),
            WorldKind::UploadFiles,
            Content,
            Effect,
            false,
        ),
        (
            drop_files(1.0, 2.0, &files),
            WorldKind::DropFiles,
            Content,
            Effect,
            false,
        ),
        (
            attach_file("l", "demo.gif"),
            WorldKind::AttachFile,
            Content,
            Effect,
            false,
        ),
    ];
    assert_eq!(table.len(), WorldKind::ALL.len());
    for (message, name, scope, effect, settles) in table {
        assert_eq!(message.kind(), name);
        assert_eq!(message.payload()["kind"], name.as_str());
        assert_eq!(message.scope_class(), scope, "{name:?}");
        assert_eq!(message.effect_class(), effect, "{name:?}");
        assert_eq!(message.takes_settle(), settles, "{name:?}");
    }
}

#[test]
fn constructors_build_the_payloads_the_assets_read() {
    let fields = two_fields();
    let files = [file()];
    let cases = [
        (
            read_text(Some("locator_a1b2c3d4_1"), Some("article"), 20_000),
            json!({"kind":"read_text","locator":"locator_a1b2c3d4_1","mode":"article","max_chars":20000}),
        ),
        (
            read_text(None, None, 20_000),
            json!({"kind":"read_text","max_chars":20000}),
        ),
        (
            inspect("controls", 50),
            json!({"kind":"inspect","inspect_kind":"controls","max_items":50}),
        ),
        (
            inspect_tree(Some("locator_a1b2c3d4_2"), 6, 400),
            json!({"kind":"inspect_tree","locator":"locator_a1b2c3d4_2","max_depth":6,"max_nodes":400}),
        ),
        (
            find("Save changes", "control", 8),
            json!({"kind":"find","text":"Save changes","find_kind":"control","max_results":8}),
        ),
        (
            query_semantic("Email address", Some("textbox"), false, true, 8),
            json!({"kind":"query_semantic","name":"Email address","role":"textbox","exact":false,"form_scope":true,"max_results":8}),
        ),
        (
            describe(&["locator_a1b2c3d4_3".into(), "locator_a1b2c3d4_4".into()]),
            json!({"kind":"describe","locators":["locator_a1b2c3d4_3","locator_a1b2c3d4_4"]}),
        ),
        (describe_focused(), json!({"kind":"describe_focused"})),
        (
            verify_focused_text(false),
            json!({"kind":"verify_focused_text","allow_credentials":false}),
        ),
        (
            box_("locator_a1b2c3d4_5"),
            json!({"kind":"box","locator":"locator_a1b2c3d4_5"}),
        ),
        (
            prepare_fill(&fields, false, Some("locator_a1b2c3d4_8")),
            json!({
                "kind":"prepare_fill",
                "fields":[
                    {"locator":"locator_a1b2c3d4_6","value":"Ada Lovelace"},
                    {"locator":"locator_a1b2c3d4_7","value":"ada@example.com"}],
                "allow_credentials":false,
                "submit_locator":"locator_a1b2c3d4_8"}),
        ),
        (
            verify_fill_retained(&fields, false, 3_000),
            json!({
                "kind":"verify_fill_retained",
                "fields":[
                    {"locator":"locator_a1b2c3d4_6","value":"Ada Lovelace"},
                    {"locator":"locator_a1b2c3d4_7","value":"ada@example.com"}],
                "allow_credentials":false,
                "stable_ms":250,"poll_ms":25,"limit_ms":3000}),
        ),
        (
            observe(
                "text_present",
                Some("Saved"),
                Some("locator_a1b2c3d4_9"),
                5_000,
                true,
            ),
            json!({"kind":"observe","condition":"text_present","value":"Saved",
                "locator":"locator_a1b2c3d4_9","timeout_ms":5000,"until_satisfied":true}),
        ),
        (
            observe("load_ready", None, None, 5_000, false),
            json!({"kind":"observe","condition":"load_ready","timeout_ms":5000,"until_satisfied":false}),
        ),
        (
            activate("locator_a1b2c3d4_8"),
            json!({"kind":"activate","locator":"locator_a1b2c3d4_8"}),
        ),
        (
            hover("locator_a1b2c3d4_8"),
            json!({"kind":"hover","locator":"locator_a1b2c3d4_8"}),
        ),
        (
            focus("locator_a1b2c3d4_6"),
            json!({"kind":"focus","locator":"locator_a1b2c3d4_6"}),
        ),
        (
            scroll_point(640.0, 1800.0),
            json!({"kind":"scroll_point","x":640,"y":1800}),
        ),
        (
            viewport_point(640.0, 360.0),
            json!({"kind":"viewport_point","x":640,"y":360}),
        ),
        (scroll_offset(), json!({"kind":"scroll_offset"})),
        (
            hit_point(320.0, 240.0, false),
            json!({"kind":"hit_point","x":320,"y":240,"page":false}),
        ),
        (
            hit_point(320.5, 240.25, true),
            json!({"kind":"hit_point","x":320.5,"y":240.25,"page":true}),
        ),
        (owner_box(), json!({"kind":"owner_box"})),
        (
            owner_probe(320.0, 240.0),
            json!({"kind":"owner_probe","x":320,"y":240}),
        ),
        (focus_probe(), json!({"kind":"focus_probe"})),
        (require_focused(), json!({"kind":"require_focused"})),
        (
            fill_local(&field("locator_a1b2c3d4_10", "Canada"), false),
            json!({"kind":"fill_local",
                "field":{"locator":"locator_a1b2c3d4_10","value":"Canada"},
                "allow_credentials":false}),
        ),
        (
            focus_text_field(&field("locator_a1b2c3d4_6", "Ada Lovelace"), false),
            json!({"kind":"focus_text_field",
                "field":{"locator":"locator_a1b2c3d4_6","value":"Ada Lovelace"},
                "allow_credentials":false}),
        ),
        (
            submit_fill(&fields[..1], "locator_a1b2c3d4_8", false),
            json!({"kind":"submit_fill",
                "fields":[{"locator":"locator_a1b2c3d4_6","value":"Ada Lovelace"}],
                "submit_locator":"locator_a1b2c3d4_8","allow_credentials":false}),
        ),
        (
            type_text("locator_a1b2c3d4_6", "Ada Lovelace", true, false),
            json!({"kind":"type_text","locator":"locator_a1b2c3d4_6","text":"Ada Lovelace",
                "clear_first":true,"allow_credentials":false}),
        ),
        (
            clear_focused(false),
            json!({"kind":"clear_focused","allow_credentials":false}),
        ),
        (
            scroll(Some("locator_a1b2c3d4_11"), Some("down"), Some("page")),
            json!({"kind":"scroll","locator":"locator_a1b2c3d4_11","direction":"down","amount":"page"}),
        ),
        (scroll(None, None, None), json!({"kind":"scroll"})),
        (
            upload_files("locator_a1b2c3d4_12", &files),
            json!({"kind":"upload_files","locator":"locator_a1b2c3d4_12",
                "files":[{"name":"notes.txt","media_type":"text/plain","data":"aGVsbG8=","size":5}]}),
        ),
        (
            drop_files(400.0, 300.0, &files),
            json!({"kind":"drop_files","x":400,"y":300,
                "files":[{"name":"notes.txt","media_type":"text/plain","data":"aGVsbG8=","size":5}]}),
        ),
        (
            attach_file("locator_a1b2c3d4_12", "demo.gif"),
            json!({"kind":"attach_file","locator":"locator_a1b2c3d4_12","file_name":"demo.gif"}),
        ),
    ];
    for (message, expected) in cases {
        assert_eq!(message.to_json(), expected, "{:?}", message.kind());
    }
}

#[test]
fn a_settle_budget_reaches_only_the_kinds_that_settle() {
    let settled = find("Save changes", "control", 8).with_settle(1_500, true);
    assert_eq!(
        settled.to_json(),
        json!({"kind":"find","text":"Save changes","find_kind":"control","max_results":8,
            "settle_ms":1500,"visual_settle":true})
    );
    let spent = read_text(None, None, 100).with_settle(0, true);
    assert_eq!(spent.payload()["settle_ms"], 0);
    assert_eq!(spent.payload()["visual_settle"], true);
    let declined = read_text(None, None, 100).with_settle(800, false);
    assert_eq!(declined.payload()["settle_ms"], 800);
    assert_eq!(declined.payload()["visual_settle"], false);
    let typed = focus_text_field(&field("locator_a1b2c3d4_6", "Ada"), false).with_settle(40, true);
    assert_eq!(typed.payload()["settle_ms"], 40);
    assert_eq!(typed.payload()["visual_settle"], true);
    let prepared = prepare_fill(&[], false, None).with_settle(0, true);
    assert_eq!(prepared.payload()["settle_ms"], 0);
    assert_eq!(prepared.payload()["visual_settle"], true);
    let read = describe(&[]);
    assert_eq!(read.clone().with_settle(500, true), read);
}

#[test]
fn only_cancelled_not_retained_and_page_exception_can_follow_an_effect() {
    let every = [
        "stale_locator",
        "not_visible",
        "disabled",
        "read_only",
        "not_editable",
        "not_fillable",
        "option_missing",
        "credential_authorization_required",
        "submit_outside_form",
        "not_file_input",
        "no_focused_control",
        "focus_lost",
        "outside_viewport",
        "no_subject",
        "unknown_kind",
        "invalid_request",
        "runtime_missing",
        "cancelled",
        "unsatisfied",
        "not_retained",
        "payload_mismatch",
        "page_exception",
    ];
    for name in every {
        let code: AssetErrorCode = serde_json::from_value(json!(name)).expect(name);
        assert_eq!(serde_json::to_value(code).expect(name), json!(name));
        let after_effect = matches!(name, "cancelled" | "not_retained" | "page_exception");
        assert_eq!(code.is_pre_effect(), !after_effect, "{name}");
    }
}

#[test]
fn only_the_codes_that_refuse_a_control_are_page_refusals() {
    for refusal in PageRefusal::ALL {
        let code: AssetErrorCode = serde_json::from_value(json!(refusal.as_str())).unwrap();
        assert_eq!(code.page_refusal(), Some(refusal));
        assert!(code.is_pre_effect(), "{}", refusal.as_str());
    }
    for name in [
        "stale_locator",
        "not_fillable",
        "no_focused_control",
        "page_exception",
    ] {
        let code: AssetErrorCode = serde_json::from_value(json!(name)).unwrap();
        assert_eq!(code.page_refusal(), None, "{name}");
    }
}

#[test]
fn an_unknown_code_is_a_page_exception() {
    let code: AssetErrorCode = serde_json::from_value(json!("quota_exceeded")).unwrap();
    assert_eq!(code, AssetErrorCode::PageException);
    assert!(!code.is_pre_effect());
}

#[test]
fn an_envelope_carries_a_value_or_a_coded_failure() {
    assert_eq!(
        decode_envelope(&json!({"ok": true, "value": {"submitted": true}})),
        Ok(json!({"submitted": true}))
    );
    assert_eq!(decode_envelope(&json!({"ok": true})), Ok(Value::Null));
    assert_eq!(
        decode_envelope(&json!({"ok": false,
            "error": {"code": "stale_locator", "message": "stale browser locator"}})),
        Err(AssetError {
            code: AssetErrorCode::StaleLocator,
            message: "stale browser locator".into(),
            detail: Some(AssetDetail::Stale {
                reason: StaleReason::Unknown,
                lookup: None,
            }),
        })
    );
}

/// The envelope of a stale locator whose detail is `detail`, or that has none.
fn stale_locator(detail: Option<Value>) -> Value {
    let mut error = json!({"code": "stale_locator", "message": "stale browser locator"});
    if let Some(detail) = detail {
        error["detail"] = detail;
    }
    json!({"ok": false, "error": error})
}

#[test]
fn a_stale_locator_carries_the_word_the_page_gave() {
    for (detail, expected) in [
        (json!({"stale": "removed"}), StaleReason::Removed),
        (json!({"stale": "unknown"}), StaleReason::Unknown),
        (json!({"stale": "replaced"}), StaleReason::Unknown),
        (json!({"stale": 7}), StaleReason::Unknown),
        (json!({"problems": []}), StaleReason::Unknown),
        (json!("removed"), StaleReason::Unknown),
    ] {
        let error = decode_envelope(&stale_locator(Some(detail.clone()))).unwrap_err();
        assert_eq!(error.code, AssetErrorCode::StaleLocator, "{detail}");
        assert_eq!(
            error.detail,
            Some(AssetDetail::Stale {
                reason: expected,
                lookup: None
            }),
            "{detail}"
        );
    }
    let bare = decode_envelope(&stale_locator(None)).unwrap_err();
    assert_eq!(
        bare.detail,
        Some(AssetDetail::Stale {
            reason: StaleReason::Unknown,
            lookup: None
        })
    );
}

#[test]
fn a_removed_locator_carries_the_lookup_the_page_made() {
    let detail = json!({
        "stale": "removed",
        "matched": 3,
        "same_count": 1,
        "truncated": false,
        "candidates": [
            {"target": {"locator": "locator_a1b2c3d4_8", "role": "button", "name": "More options",
                "state": ["collapsed"], "credential_class": false, "context": "Neon Rain", "nth": 2},
             "same": true},
            {"target": {"locator": "locator_a1b2c3d4_9", "role": "button", "name": "More options",
                "state": [], "credential_class": false}, "same": false}
        ]
    });

    let error = decode_envelope(&stale_locator(Some(detail))).unwrap_err();

    let Some(AssetDetail::Stale {
        reason,
        lookup: Some(lookup),
    }) = error.detail
    else {
        panic!("the removed locator carries its lookup");
    };
    assert_eq!(reason, StaleReason::Removed);
    assert_eq!(
        (lookup.matched, lookup.same_count, lookup.truncated),
        (3, 1, false)
    );
    assert_eq!(lookup.candidates.len(), 2);
    assert!(lookup.candidates[0].same && !lookup.candidates[1].same);
    assert_eq!(
        lookup.candidates[0].target.context.as_deref(),
        Some("Neon Rain")
    );
    assert_eq!(lookup.candidates[0].target.nth, Some(2));
}

#[test]
fn a_lookup_this_build_cannot_read_whole_is_no_lookup() {
    for detail in [
        json!({"stale": "removed"}),
        json!({"stale": "removed", "matched": "many"}),
        json!({"stale": "removed", "matched": 1, "candidates": [{"target": {"unheard_of": 1}, "same": true}]}),
        json!({"stale": "unknown", "matched": 1, "same_count": 1}),
    ] {
        let error = decode_envelope(&stale_locator(Some(detail.clone()))).unwrap_err();
        let Some(AssetDetail::Stale { lookup, .. }) = error.detail else {
            panic!("a stale locator always has a reason");
        };
        assert_eq!(lookup, None, "{detail}");
    }
}

#[test]
fn a_stale_word_on_another_code_is_not_a_reason() {
    let envelope = json!({"ok": false, "error": {
        "code": "not_visible", "message": "x", "detail": {"stale": "removed"}}});
    assert_eq!(decode_envelope(&envelope).unwrap_err().detail, None);
}

/// The envelope of a refused fill that names `problem` for its first field.
fn refused_fill(problem: &str) -> Value {
    json!({"ok": false, "error": {
        "code": "option_missing",
        "message": "select option not found",
        "detail": {"problems": [
            {"field": 0, "problem": problem, "options": ["Developer", "Other"]},
            {"field": 2, "problem": "option_disabled"}
        ]}
    }})
}

#[test]
fn a_refused_fill_carries_the_fields_it_could_not_take() {
    let error = decode_envelope(&refused_fill("option_missing")).unwrap_err();
    assert_eq!(error.code, AssetErrorCode::OptionMissing);
    assert_eq!(error.message, "select option not found");
    let Some(AssetDetail::Fields(detail)) = error.detail else {
        panic!("the detail decodes as fields");
    };
    assert_eq!(detail.problems.len(), 2);
    assert_eq!(detail.problems[0].field, 0);
    assert_eq!(detail.problems[0].kind, FieldProblemKind::OptionMissing);
    assert_eq!(detail.problems[0].options.len(), 2);
    assert_eq!(detail.problems[1].field, 2);
    assert_eq!(detail.problems[1].kind, FieldProblemKind::OptionDisabled);
}

#[test]
fn a_problem_word_this_build_does_not_know_leaves_no_detail() {
    let error = decode_envelope(&refused_fill("option_vanished")).unwrap_err();
    assert_eq!(error.code, AssetErrorCode::OptionMissing);
    assert_eq!(error.message, "select option not found");
    assert_eq!(error.detail, None);
}

#[test]
fn a_detail_that_is_not_a_list_of_problems_is_ignored() {
    for detail in [
        json!("text"),
        json!({"problems": []}),
        json!({"problems": [{"field": "0"}]}),
        json!(null),
    ] {
        let envelope = json!({"ok": false, "error": {
            "code": "not_fillable", "message": "target is not fillable", "detail": detail}});
        assert_eq!(
            decode_envelope(&envelope).unwrap_err().detail,
            None,
            "{envelope}"
        );
    }
}

#[test]
fn a_failure_the_service_cannot_read_assumes_the_page_changed() {
    for envelope in [
        json!({"ok": false}),
        json!({"ok": false, "error": {"code": 7}}),
        json!({"ok": false, "error": {"code": "invented", "message": "x"}}),
        json!({"value": 1}),
        json!("text"),
        json!(null),
    ] {
        let error = decode_envelope(&envelope).unwrap_err();
        assert_eq!(error.code, AssetErrorCode::PageException, "{envelope}");
        assert!(!error.code.is_pre_effect());
    }
}

#[test]
fn a_failure_description_is_bounded() {
    let error = decode_envelope(&json!({"ok": false,
        "error": {"code": "disabled", "message": "x".repeat(2_000)}}))
    .unwrap_err();
    assert_eq!(error.message.chars().count(), 500);
}

#[test]
fn only_a_returned_answer_has_an_envelope() {
    let returned = CallAnswer::Returned {
        envelope: json!({"ok": true, "value": 3}),
    };
    assert_eq!(decode_answer(&returned), Some(Ok(json!(3))));
    assert_eq!(decode_answer(&CallAnswer::Skipped), None);
    assert_eq!(decode_answer(&CallAnswer::Unknown), None);
    let failed = CallAnswer::Failed {
        code: super::super::wire::CallFailure::Timeout,
        message: String::new(),
        started: true,
    };
    assert_eq!(decode_answer(&failed), None);
}

/// An envelope says a call followed a handle only by a non-empty `rebound` list, and the list
/// itself never leaves the decoder.
#[test]
fn an_envelope_reports_a_followed_handle_as_a_fact() {
    let followed = json!({"ok": true, "value": {}, "rebound": ["locator_a1b2c3d4_1"]});
    assert!(envelope_rebound(&followed));
    assert!(answer_rebound(&CallAnswer::Returned {
        envelope: followed.clone()
    }));
    assert_eq!(
        decode_envelope(&followed),
        Ok(json!({})),
        "the value is unchanged"
    );
    assert!(!envelope_rebound(&json!({"ok": true, "value": {}})));
    assert!(!envelope_rebound(
        &json!({"ok": true, "value": {}, "rebound": []})
    ));
    assert!(!answer_rebound(&CallAnswer::Skipped));
    assert!(!answer_rebound(&CallAnswer::Unknown));
}

/// A subject says it was rebound only when the page followed the handle, and a bare subject from
/// a page that never followed anything still decodes.
#[test]
fn a_subject_decodes_with_and_without_rebound() {
    let rectangle = json!({"left": 0.0, "top": 0.0, "width": 10.0, "height": 10.0});
    let plain: Prepared = serde_json::from_value(json!({
        "subject": {"role": "button", "name": "Save"}, "rectangle": rectangle}))
    .unwrap();
    assert!(!plain.subject.rebound);
    let followed: Prepared = serde_json::from_value(json!({
        "subject": {"role": "button", "name": "Save", "rebound": true}, "rectangle": rectangle}))
    .unwrap();
    assert!(followed.subject.rebound);
    assert_eq!(
        serde_json::to_value(&plain.subject).unwrap(),
        json!({"role": "button", "name": "Save"}),
        "an ordinary subject keeps its two members"
    );
    assert_eq!(
        serde_json::to_value(&followed.subject).unwrap()["rebound"],
        json!(true)
    );
    let unknown = serde_json::from_value::<PhysicalActionSubject>(
        json!({"role": "button", "name": "Save", "locator": "locator_a1b2c3d4_1"}),
    );
    assert!(unknown.is_err(), "no other member crosses the seam");
}

#[test]
fn typed_values_read_what_the_asset_returns() {
    let prepared: Prepared = serde_json::from_value(json!({
        "activated": true,
        "subject": {"role": "button", "name": "Save"},
        "rectangle": {"left": 10.0, "top": 20.0, "width": 100.0, "height": 40.0}}))
    .unwrap();
    assert_eq!(prepared.subject.name, "Save");
    assert_eq!(prepared.rectangle.center(), Point { x: 60.0, y: 40.0 });

    let text: Text = serde_json::from_value(json!({"text": "hi", "truncated": true,
        "title": "T", "url": "https://example.com/"}))
    .unwrap();
    assert!(text.truncated);

    let article: Article = serde_json::from_value(json!({"text": "body", "truncated": false,
        "article_found": true, "title": "T", "url": "https://example.com/"}))
    .unwrap();
    assert!(article.article_found);
    assert_eq!(
        article.into_text(7),
        BrowserOutcome::Text {
            tab_id: 7,
            text: "body".into(),
            truncated: false,
            title: "T".into(),
            url: "https://example.com/".into()
        }
    );

    let targets: Targets = serde_json::from_value(json!({"targets": [{
        "locator": "locator_a1b2c3d4_1", "role": "button", "name": "Save",
        "state": ["focused"], "credential_class": false}], "truncated": true}))
    .unwrap();
    assert_eq!(targets.targets.len(), 1);
    assert!(targets.truncated);

    let tree: TreeValue = serde_json::from_value(json!({"tree": {"kind": "container",
        "label": "", "children": [{"kind": "heading", "label": "Title", "children": []},
        {"kind": "invented", "label": "Panel"}]}, "nodes": 3, "truncated": false}))
    .unwrap();
    assert_eq!(tree.tree.children[0].label, "Title");
    assert_eq!(tree.tree.children[0].kind, TreeNodeKind::Heading);
    assert_eq!(
        tree.tree.children[1].kind,
        TreeNodeKind::Container,
        "a kind the service does not know is a container"
    );
    assert!(tree.tree.children[1].children.is_empty());
    assert_eq!(tree.tree.nodes(), 3);

    let submitted: Submitted = serde_json::from_value(json!({"submitted": true})).unwrap();
    assert!(submitted.submitted);

    let hit: Hit = serde_json::from_value(json!({"x": 3.0, "y": 4.0, "embed": true,
        "subject": null}))
    .unwrap();
    assert!(hit.embed && hit.subject.is_none());

    let owner: OwnerBox = serde_json::from_value(json!({"left": 8.0, "top": 9.0,
        "width": 300.0, "height": 150.0}))
    .unwrap();
    assert_eq!(owner.left, 8.0);

    let probe: OwnerProbe =
        serde_json::from_value(json!({"hit": true, "left": 8.0, "top": 9.0})).unwrap();
    assert!(probe.hit);

    let focus: FocusProbe = serde_json::from_value(json!({"focused": false})).unwrap();
    assert!(!focus.focused);

    let observed: Observed = serde_json::from_value(json!({"satisfied": true,
        "elapsed_ms": 120, "readiness": "complete"}))
    .unwrap();
    assert_eq!(observed.elapsed_ms, 120);
}

#[test]
fn an_observed_document_keeps_the_interactive_readiness_the_page_reports() {
    let observed: Observed = serde_json::from_value(
        json!({"satisfied": true, "elapsed_ms": 3, "readiness": "interactive"}),
    )
    .unwrap();
    assert_eq!(
        BrowserReadiness::from(observed.readiness),
        BrowserReadiness::Interactive
    );
    let unknown: Observed = serde_json::from_value(
        json!({"satisfied": false, "elapsed_ms": 0, "readiness": "uninitialized"}),
    )
    .unwrap();
    assert_eq!(
        BrowserReadiness::from(unknown.readiness),
        BrowserReadiness::Unknown
    );
}
