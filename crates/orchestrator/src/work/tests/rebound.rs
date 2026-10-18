//! A handle that followed its control through a re-render (ADR-0198 Decision 17): the action
//! says so in one structural fact and one audit bool, with its usual sentence, and no handle,
//! row or link goes into the facts' history or the audit.

use super::*;

/// A workspace with one tab open and one found target, ready for the next browser reply.
fn found_target() -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    crate::workspace::WorkspaceId,
    Arc<MemoryAudit>,
    String,
    String,
) {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
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
        &CancellationToken::default(),
    );
    let target = found.facts["matches"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    (executor, browser, workspace, audit, tab_handle, target)
}

/// The click of a workspace's one target, answered with a subject that is or is not rebound.
fn click_with(
    rebound: bool,
) -> (
    crate::work::result::InvocationResult,
    Arc<MemoryAudit>,
    String,
) {
    let (executor, browser, workspace, audit, tab_handle, target) = found_target();
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
        &CancellationToken::default(),
    );
    (clicked, audit, target)
}

/// What describing the target answers before an action that describes first.
fn described(rebound: bool) -> BrowserOutcome {
    BrowserOutcome::TargetsDescribed {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "more-options".into(),
            role: "button".into(),
            name: "More options".into(),
            ..Default::default()
        }],
        rebound,
    }
}

/// One action that describes its target first, run twice: once when describing followed the
/// handle and once when it did not. The action's own call finds the control live both times.
/// Returns the followed run and the plain run.
fn after_describing(
    tool: &str,
    arguments: impl Fn(&str, &str) -> serde_json::Value,
    answers: impl Fn() -> Vec<BrowserOutcome>,
) -> (
    crate::work::result::InvocationResult,
    crate::work::result::InvocationResult,
    Arc<MemoryAudit>,
) {
    let run = |rebound: bool| {
        let (executor, browser, workspace, audit, tab_handle, target) = found_target();
        browser.push(Ok(described(rebound)));
        for answer in answers() {
            browser.push(Ok(answer));
        }
        let result = executor.execute(
            &workspace,
            tool,
            arguments(&tab_handle, &target),
            &CancellationToken::default(),
        );
        (result, audit)
    };
    let (followed, audit) = run(true);
    let (plain, _) = run(false);
    (followed, plain, audit)
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

fn typed() -> Vec<BrowserOutcome> {
    vec![BrowserOutcome::Typed {
        tab: tab(7, "https://example.com/"),
        character_count: 2,
        subject: Some(PhysicalActionSubject {
            role: "button".into(),
            name: "More options".into(),
            rebound: false,
        }),
        committed_urls: vec![],
    }]
}

#[test]
fn typing_after_a_followed_describe_reports_rebound_with_its_usual_sentence() {
    let (followed, plain, audit) = after_describing(
        "browser_type_text",
        |tab, target| json!({"tab":tab,"target":target,"text":"hi"}),
        typed,
    );

    assert_eq!(followed.status, Status::Succeeded, "{followed:?}");
    assert_eq!(followed.facts["rebound"], true);
    assert_eq!(followed.summary, plain.summary);
    assert!(plain.facts.get("rebound").is_none());
    let records = audit.0.lock().unwrap();
    assert!(records.last().unwrap().observed.rebound);
}

#[test]
fn uploading_after_a_followed_describe_reports_rebound_with_its_usual_sentence() {
    let (followed, plain, audit) = after_describing(
        "browser_upload",
        |tab, target| json!({"tab":tab,"target":target,"files":[{"name":"a.txt","data_base64":"eA=="}]}),
        || {
            vec![BrowserOutcome::FilesUploaded {
                tab_id: 7,
                uploaded_count: 1,
                uploaded_bytes: 1,
                subject: Some(PhysicalActionSubject {
                    role: "button".into(),
                    name: "More options".into(),
                    rebound: false,
                }),
            }]
        },
    );

    assert_eq!(followed.status, Status::Succeeded, "{followed:?}");
    assert_eq!(followed.facts["rebound"], true);
    assert_eq!(followed.summary, plain.summary);
    assert!(plain.facts.get("rebound").is_none());
    assert!(audit.0.lock().unwrap().last().unwrap().observed.rebound);
}

fn filled(rebound: bool) -> Vec<BrowserOutcome> {
    vec![BrowserOutcome::Filled {
        tab: tab(7, "https://example.com/"),
        filled_count: 1,
        held: Vec::new(),
        submitted: false,
        rebound,
        committed_urls: vec![],
    }]
}

#[test]
fn filling_after_a_followed_describe_reports_rebound_with_its_usual_sentence() {
    let (followed, plain, audit) = after_describing(
        "browser_fill_form",
        |tab, target| json!({"tab":tab,"fields":[{"target":target,"value":"x"}]}),
        || filled(false),
    );

    assert_eq!(followed.status, Status::Succeeded, "{followed:?}");
    assert_eq!(followed.facts["rebound"], true);
    assert_eq!(followed.summary, plain.summary);
    assert!(plain.facts.get("rebound").is_none());
    assert!(audit.0.lock().unwrap().last().unwrap().observed.rebound);
}

#[test]
fn a_fill_that_followed_a_handle_while_preparing_reports_rebound() {
    let (executor, browser, workspace, audit, tab_handle, target) = found_target();
    browser.push(Ok(described(false)));
    for answer in filled(true) {
        browser.push(Ok(answer));
    }

    let result = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"tab":tab_handle,"fields":[{"target":target,"value":"x"}]}),
        &CancellationToken::default(),
    );

    assert_eq!(result.facts["rebound"], true);
    assert!(audit.0.lock().unwrap().last().unwrap().observed.rebound);
}

#[test]
fn saving_a_recording_to_a_target_after_a_followed_describe_reports_rebound() {
    let (followed, plain, audit) = after_saving_to_a_target();

    assert_eq!(followed.status, Status::Succeeded, "{followed:?}");
    assert_eq!(followed.facts["rebound"], true);
    assert_eq!(followed.summary, plain.summary);
    assert_eq!(
        followed.summary,
        "Attached a replay of 1 second of page changes to the page."
    );
    assert!(plain.facts.get("rebound").is_none());
    assert!(audit.0.lock().unwrap().last().unwrap().observed.rebound);
}

#[test]
fn a_save_the_page_did_not_take_keeps_the_recording_and_names_a_file_input() {
    let (executor, browser, workspace, _audit, _, target) = found_target();
    browser.push(Ok(BrowserOutcome::RecordingStopped {
        summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        changed: false,
    }));
    browser.push(Ok(described(false)));
    browser.push(Ok(BrowserOutcome::RecordingExportFailed {
        reason: "the page did not accept the replay".into(),
        page_refused: true,
    }));

    let result = executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"save","target":target}),
        &CancellationToken::default(),
    );

    assert_eq!(result.status, Status::Failed, "{result:?}");
    assert_eq!(
        result.summary,
        "The page did not take the replay at that target, so nothing was attached; the recording is kept."
    );
    assert!(result.next_steps[0].contains("file input"), "{result:?}");
}

/// A page-target save run twice: once when describing the target followed its handle and once
/// when it did not. Returns the followed run, the plain run and the followed run's audit.
fn after_saving_to_a_target() -> (
    crate::work::result::InvocationResult,
    crate::work::result::InvocationResult,
    Arc<MemoryAudit>,
) {
    let run = |rebound: bool| {
        let (executor, browser, workspace, audit, _, target) = found_target();
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: false,
        }));
        browser.push(Ok(described(rebound)));
        browser.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            encoded: EncodedRecording {
                frame_count: 2,
                captured_frame_count: 2,
                duration_ms: 1_000,
                width: 800,
                height: 600,
                byte_count: 6,
            },
            delivery: RecordingDelivery::Attached { tab_id: 7 },
        }));
        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","target":target}),
            &CancellationToken::default(),
        );
        (result, audit)
    };
    let (followed, audit) = run(true);
    let (plain, _) = run(false);
    (followed, plain, audit)
}
