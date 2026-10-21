use serde_json::json;

use super::*;

/// The envelope of a stale locator whose detail is `detail`, or that has none.
fn stale_locator(detail: Option<Value>) -> Value {
    let mut error = json!({"code": "stale_locator", "message": "stale browser locator"});
    if let Some(detail) = detail {
        error["detail"] = detail;
    }
    json!({"ok": false, "error": error})
}

/// A page failure the service cannot read is a page exception, which may have followed an
/// effect, so the agent is never told nothing happened when the page did not say so.
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

/// A removed handle offers a trusted repeat only from a lookup the page made and this service
/// reads whole: a lookup it reads in part, one on a handle that went stale another way, and a
/// stale word it does not know offer none.
#[test]
fn a_removed_locator_offers_the_page_lookup_only_when_it_reads_whole() {
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
    let Some(AssetDetail::Stale {
        reason: StaleReason::Removed,
        lookup: Some(lookup),
    }) = decode_envelope(&stale_locator(Some(detail)))
        .unwrap_err()
        .detail
    else {
        panic!("a removed locator carries its lookup");
    };
    assert_eq!(
        (lookup.matched, lookup.same_count, lookup.truncated),
        (3, 1, false)
    );
    let candidates: Vec<_> = lookup
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.same,
                candidate.target.context.as_deref(),
                candidate.target.nth,
            )
        })
        .collect();
    assert_eq!(
        candidates,
        [(true, Some("Neon Rain"), Some(2)), (false, None, None)]
    );

    for (detail, reason) in [
        (json!({"stale": "removed"}), StaleReason::Removed),
        (
            json!({"stale": "removed", "matched": "many"}),
            StaleReason::Removed,
        ),
        (
            json!({"stale": "removed", "matched": 1,
                "candidates": [{"target": {"unheard_of": 1}, "same": true}]}),
            StaleReason::Removed,
        ),
        (
            json!({"stale": "unknown", "matched": 1, "same_count": 1}),
            StaleReason::Unknown,
        ),
        (json!({"stale": "replaced"}), StaleReason::Unknown),
    ] {
        assert_eq!(
            decode_envelope(&stale_locator(Some(detail.clone())))
                .unwrap_err()
                .detail,
            Some(AssetDetail::Stale {
                reason,
                lookup: None
            }),
            "{detail}"
        );
    }
}

/// A call that followed a handle says so as a fact, and the handle it followed never crosses the
/// seam: an envelope reports a non-empty `rebound` list as a flag, and a subject decodes with
/// `rebound` or without it and refuses any other member (commit 3466718c).
#[test]
fn an_envelope_reports_a_followed_handle_as_a_fact() {
    let followed = json!({"ok": true, "value": {}, "rebound": ["locator_a1b2c3d4_1"]});
    assert!(envelope_rebound(&followed));
    assert!(answer_rebound(&CallAnswer::Returned {
        envelope: followed.clone()
    }));
    assert_eq!(decode_envelope(&followed), Ok(json!({})));
    assert!(!envelope_rebound(&json!({"ok": true, "value": {}})));
    assert!(!envelope_rebound(
        &json!({"ok": true, "value": {}, "rebound": []})
    ));
    assert!(!answer_rebound(&CallAnswer::Skipped));

    let rectangle = json!({"left": 0.0, "top": 0.0, "width": 10.0, "height": 10.0});
    let subject = |subject: Value| -> Prepared {
        serde_json::from_value(json!({"subject": subject, "rectangle": rectangle})).unwrap()
    };
    assert!(
        !subject(json!({"role": "button", "name": "Save"}))
            .subject
            .rebound
    );
    assert!(
        subject(json!({"role": "button", "name": "Save", "rebound": true}))
            .subject
            .rebound
    );
    assert!(
        serde_json::from_value::<PhysicalActionSubject>(
            json!({"role": "button", "name": "Save", "locator": "locator_a1b2c3d4_1"})
        )
        .is_err(),
        "no other member crosses the seam"
    );
}

/// The page reports `interactive` readiness, which the model reads, though the tab wire has no
/// such word; a readiness the service does not know is unknown.
#[test]
fn an_observed_document_keeps_the_interactive_readiness_the_page_reports() {
    let readiness = |word: &str| -> BrowserReadiness {
        let observed: Observed =
            serde_json::from_value(json!({"satisfied": true, "elapsed_ms": 3, "readiness": word}))
                .unwrap();
        BrowserReadiness::from(observed.readiness)
    };
    assert_eq!(readiness("interactive"), BrowserReadiness::Interactive);
    assert_eq!(readiness("uninitialized"), BrowserReadiness::Unknown);
}
