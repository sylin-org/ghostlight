//! Actual executor dispatch binds read feedback and closes each composition child's visuals.

use super::*;
use crate::browser::adapter::{PresentationActivity, PresentationKind, PresentationSignal};

#[derive(Default)]
struct PageSignals(Mutex<Vec<PresentationSignal>>);

impl PresentationPort for PageSignals {
    fn present(&self, _: &str, signal: PresentationSignal) -> Result<(), PresentationError> {
        self.0.lock().unwrap().push(signal);
        Ok(())
    }
}

#[test]
fn credential_guidance_finishes_visuals_without_a_denial_notice() {
    for composed in [false, true] {
        let (mut executor, browser, _, workspace, _) = fixture();
        let signals = Arc::new(PageSignals::default());
        executor.presentation = PresentationReactor::new(signals.clone());
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
        assert_eq!(opened.status, Status::Succeeded);
        signals.0.lock().unwrap().clear();
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            rebound: false,
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "password".into(),
                role: "textbox".into(),
                name: "Password".into(),
                state: vec![],
                credential_class: true,
                ..Default::default()
            }],
        }));
        let input = json!({"tab":opened.facts["tab"],"focused":true,"text":"PRIVATE_SECRET"});
        let (tool, input) = if composed {
            (
                "browser_flow",
                json!({"steps":[{"tool":"browser_type_text","arguments":input}]}),
            )
        } else {
            ("browser_type_text", input)
        };
        let result = executor.execute(&workspace, tool, input, &CancellationToken::default());
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::None);
        let signals = signals.0.lock().unwrap();
        assert!(signals
            .iter()
            .all(|signal| signal.signal != PresentationKind::Denial));
        assert!(signals
            .iter()
            .any(|signal| signal.signal == PresentationKind::Completion));
        assert!(!serde_json::to_string(&*signals)
            .unwrap()
            .contains("PRIVATE_SECRET"));
    }
}

#[test]
fn direct_and_composed_reads_render_only_their_dispatched_tabs() {
    let (mut executor, browser, _, workspace, audit) = fixture();
    let signals = Arc::new(PageSignals::default());
    executor.presentation = PresentationReactor::new(signals.clone());
    let mut handles = Vec::new();
    for id in [7, 11] {
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(id, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/","new_tab":true}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Succeeded);
        handles.push(result.facts["tab"].clone());
    }
    for composed in [false, true] {
        signals.0.lock().unwrap().clear();
        let before = browser.calls().len();
        let tabs = if composed { vec![7, 11] } else { vec![7] };
        for id in &tabs {
            browser.push(Ok(BrowserOutcome::Text {
                tab_id: *id,
                text: "PRIVATE_READ_CONTENT".into(),
                truncated: false,
                title: "Page".into(),
                url: "https://example.com/".into(),
            }));
        }
        let (tool, input) = if composed {
            (
                "browser_flow",
                json!({"steps":[
                    {"id":"first","tool":"browser_read","arguments":{"tab":handles[0]}},
                    {"id":"second","tool":"browser_read","arguments":{"tab":handles[1]}}
                ]}),
            )
        } else {
            ("browser_read", json!({"tab":handles[0]}))
        };
        let result = executor.execute(&workspace, tool, input, &CancellationToken::default());
        assert_eq!(result.status, Status::Succeeded, "{result:?}");
        assert_eq!(
            browser.calls()[before..]
                .iter()
                .map(|call| match call {
                    BrowserCommand::ReadDocument { tab_id, .. } => *tab_id,
                    _ => panic!("unexpected read dispatch"),
                })
                .collect::<Vec<_>>(),
            tabs
        );
        let signals = signals.0.lock().unwrap();
        let rendered: Vec<_> = signals
            .iter()
            .filter(|signal| {
                signal.activity == PresentationActivity::Read && signal.tab_id.is_some()
            })
            .collect();
        assert_eq!(rendered.len(), tabs.len() * 2);
        for (phase, tab) in rendered.chunks_exact(2).zip(&tabs) {
            assert_eq!(phase[0].signal, PresentationKind::Start);
            assert_eq!(phase[1].signal, PresentationKind::Completion);
            assert_eq!(phase[0].tab_id, Some(*tab));
            assert_eq!(phase[1].tab_id, Some(*tab));
            assert_eq!(phase[0].invocation, result.invocation);
            assert_eq!(phase[1].invocation, result.invocation);
        }
        assert!(!serde_json::to_string(&*signals)
            .unwrap()
            .contains("PRIVATE_READ_CONTENT"));
        let audit = audit.0.lock().unwrap();
        let records: Vec<_> = audit
            .iter()
            .filter(|record| record.invocation == result.invocation)
            .collect();
        assert_eq!(
            records
                .iter()
                .filter(|record| record.step.is_none())
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|record| record.step.is_some())
                .count(),
            if composed { 2 } else { 0 }
        );
    }
}

/// Every action with a control under it tells the page which one, so its feedback lands there: a
/// fill names each field, an upload its input and a targeted scroll its target. The target binds
/// the feedback to its tab, so the scroll that follows draws no second cue in the middle of the
/// page. A scroll with no target names nothing and cues the page itself.
#[test]
fn fill_upload_and_a_targeted_scroll_indicate_their_targets() {
    let (mut executor, browser, _, workspace, _) = fixture();
    let signals = Arc::new(PageSignals::default());
    executor.presentation = PresentationReactor::new(signals.clone());
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
    let control = |locator: &str, role: &str, name: &str| ObservedTarget {
        locator: locator.into(),
        role: role.into(),
        name: name.into(),
        state: vec![],
        credential_class: false,
        ..Default::default()
    };
    let controls = vec![
        control("name-1", "textbox", "Name"),
        control("email-1", "textbox", "Email"),
        control("file-1", "button", "Attachment"),
    ];
    browser.push(Ok(BrowserOutcome::targets(7, controls.clone())));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({"tab":tab_handle}),
        &CancellationToken::default(),
    );
    let handle = |index: usize| inspected.facts["items"][index]["target"].clone();
    let targets = |signals: &[PresentationSignal],
                   invocation: &str|
     -> Vec<(PresentationActivity, Option<u64>, Option<String>)> {
        signals
            .iter()
            .filter(|signal| {
                signal.invocation == invocation && signal.signal == PresentationKind::Target
            })
            .map(|signal| (signal.activity, signal.tab_id, signal.locator.clone()))
            .collect()
    };

    signals.0.lock().unwrap().clear();
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: controls[..2].to_vec(),
    }));
    browser.push(Ok(BrowserOutcome::Filled {
        rebound: false,
        tab: tab(7, "https://example.com/"),
        filled_count: 2,
        held: Vec::new(),
        submitted: false,
        committed_urls: Vec::new(),
    }));
    let filled = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"tab":tab_handle,"fields":[{"target":handle(0),"value":"Ada"},{"target":handle(1),"value":"ada@example.com"}]}),
        &CancellationToken::default(),
    );
    assert_eq!(filled.status, Status::Succeeded, "{filled:?}");
    let fill_signals = signals.0.lock().unwrap().clone();
    assert_eq!(
        targets(&fill_signals, filled.invocation.as_str()),
        [
            (PresentationActivity::Fill, Some(7), Some("name-1".into())),
            (PresentationActivity::Fill, Some(7), Some("email-1".into())),
        ]
    );
    assert!(fill_signals.iter().all(|signal| signal.click.is_none()));
    assert!(!serde_json::to_string(&fill_signals)
        .unwrap()
        .contains("ada@example.com"));

    signals.0.lock().unwrap().clear();
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: controls[2..].to_vec(),
    }));
    browser.push(Ok(BrowserOutcome::FilesUploaded {
        tab_id: 7,
        uploaded_count: 1,
        uploaded_bytes: 1,
        subject: None,
    }));
    let uploaded = executor.execute(
        &workspace,
        "browser_upload",
        json!({"tab":tab_handle,"target":handle(2),"files":[{"name":"note.txt","data_base64":"eA=="}]}),
        &CancellationToken::default(),
    );
    assert_eq!(uploaded.status, Status::Succeeded, "{uploaded:?}");
    assert_eq!(
        targets(&signals.0.lock().unwrap(), uploaded.invocation.as_str()),
        [(PresentationActivity::Upload, Some(7), Some("file-1".into()))]
    );

    signals.0.lock().unwrap().clear();
    browser.push(Ok(BrowserOutcome::Scrolled {
        tab_id: 7,
        x: 0.0,
        y: 400.0,
        subject: None,
    }));
    let revealed = executor.execute(
        &workspace,
        "browser_scroll",
        json!({"tab":tab_handle,"target":handle(2)}),
        &CancellationToken::default(),
    );
    assert_eq!(revealed.status, Status::Succeeded, "{revealed:?}");
    let scroll_signals = signals.0.lock().unwrap().clone();
    assert_eq!(
        targets(&scroll_signals, revealed.invocation.as_str()),
        [(PresentationActivity::Scroll, Some(7), Some("file-1".into()))]
    );
    assert!(
        !scroll_signals
            .iter()
            .any(|signal| signal.signal == PresentationKind::Start && signal.tab_id == Some(7)),
        "a targeted scroll cues only its target: {scroll_signals:?}"
    );

    signals.0.lock().unwrap().clear();
    browser.push(Ok(BrowserOutcome::Scrolled {
        tab_id: 7,
        x: 0.0,
        y: 800.0,
        subject: None,
    }));
    let scrolled = executor.execute(
        &workspace,
        "browser_scroll",
        json!({"tab":tab_handle}),
        &CancellationToken::default(),
    );
    assert_eq!(scrolled.status, Status::Succeeded, "{scrolled:?}");
    let page_signals = signals.0.lock().unwrap().clone();
    assert!(targets(&page_signals, scrolled.invocation.as_str()).is_empty());
    assert!(page_signals
        .iter()
        .any(|signal| signal.signal == PresentationKind::Start
            && signal.activity == PresentationActivity::Scroll
            && signal.tab_id == Some(7)));
}
