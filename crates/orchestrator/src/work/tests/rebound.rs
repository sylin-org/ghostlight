//! A handle that followed its control through a re-render (ADR-0198 Decision 17): the action
//! says so in one structural fact and one audit bool, with its usual sentence, and no handle,
//! row or link goes into the facts' history or the audit.

use super::*;

/// The click of a workspace's one target, answered with a subject that is or is not rebound.
fn click_with(
    rebound: bool,
) -> (
    crate::work::result::InvocationResult,
    Arc<MemoryAudit>,
    String,
) {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "more-options".into(),
            role: "button".into(),
            name: "More options".into(),
            context: Some("Neon Rain".into()),
            nth: Some(2),
            ..Default::default()
        }],
    )));
    let found = executor.execute(
        &workspace,
        "browser_find",
        json!({"tab":tab_handle,"text":"More options"}),
        None,
        &CancellationToken::default(),
    );
    let target = found.facts["matches"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: Some(PhysicalActionSubject {
            role: "button".into(),
            name: "More options".into(),
            rebound,
        }),
    }));
    let clicked = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        None,
        &CancellationToken::default(),
    );
    (clicked, audit, target)
}

#[test]
fn a_followed_handle_adds_one_fact_and_keeps_the_usual_sentence() {
    let (followed, _, _) = click_with(true);
    let (plain, _, _) = click_with(false);

    assert_eq!(followed.status, Status::Succeeded);
    assert_eq!(followed.facts["rebound"], true);
    assert_eq!(
        followed.summary, plain.summary,
        "the sentence is the action's usual one"
    );
    assert_eq!(followed.next_steps, plain.next_steps);
    assert!(
        plain.facts.get("rebound").is_none(),
        "an ordinary click says nothing"
    );
}

#[test]
fn a_followed_handle_audits_a_bool_and_no_handle_context_or_link() {
    let (followed, audit, target) = click_with(true);
    let (_, plain_audit, _) = click_with(false);

    assert_eq!(followed.facts["rebound"], true);
    let records = audit.0.lock().unwrap();
    let record = records.last().unwrap();
    assert!(record.observed.rebound);
    let encoded = serde_json::to_string(&*records).unwrap();
    assert!(encoded.contains("\"rebound\":true"));
    for private in [target.as_str(), "more-options", "Neon Rain"] {
        assert!(
            !encoded.contains(private),
            "{private} stays out of the audit"
        );
    }
    let plain = plain_audit.0.lock().unwrap();
    assert!(!plain.last().unwrap().observed.rebound);
    assert!(
        !serde_json::to_string(&*plain).unwrap().contains("rebound"),
        "an ordinary action leaves the audit line as it was"
    );
}

#[test]
fn the_history_sentence_holds_no_handle_or_context() {
    let (followed, audit, target) = click_with(true);

    let records = audit.0.lock().unwrap();
    let summary = &records.last().unwrap().summary;
    assert_eq!(summary, &followed.summary);
    assert!(!summary.contains(&target));
    assert!(!summary.contains("Neon Rain"));
}
