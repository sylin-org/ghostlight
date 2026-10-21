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
fn a_tab_close_the_policy_refuses_shows_the_kept_tab_notice_in_a_flow_too() {
    for composed in [false, true] {
        let policy = temporary_policy("tab-close-notice");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (mut executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
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
        let close = json!({"action":"close","tab":opened.facts["tab"]});
        let (tool, input) = if composed {
            (
                "browser_flow",
                json!({"steps":[{"tool":"browser_tabs","arguments":close}]}),
            )
        } else {
            ("browser_tabs", close)
        };
        let result = executor.execute(&workspace, tool, input, &CancellationToken::default());
        assert_eq!(result.status, Status::Blocked, "composed: {composed}");
        let signals = signals.0.lock().unwrap();
        let denial = signals
            .iter()
            .find(|signal| signal.signal == PresentationKind::Denial)
            .unwrap_or_else(|| panic!("no denial notice, composed: {composed}"));
        assert_eq!(
            denial.phase, "Ghostlight kept this tab open",
            "composed: {composed}"
        );
        assert_eq!(denial.tab_id, Some(7), "composed: {composed}");
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

/// What the page is told and the browser commands already sent when it is told, in one order.
struct OrderedSignals {
    browser: Arc<FakeBrowser>,
    log: Mutex<Vec<(usize, PresentationSignal)>>,
}

impl PresentationPort for OrderedSignals {
    fn present(&self, _: &str, signal: PresentationSignal) -> Result<(), PresentationError> {
        let sent = self.browser.calls().len();
        self.log.lock().unwrap().push((sent, signal));
        Ok(())
    }
}

impl OrderedSignals {
    /// The targets named for `invocation`: how many browser commands had been sent when the page
    /// was told, and each target's activity, tab and locator.
    fn targets(&self, invocation: &str) -> Vec<(usize, PresentationActivity, Option<u64>, String)> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, signal)| {
                signal.invocation == invocation && signal.signal == PresentationKind::Target
            })
            .map(|(sent, signal)| {
                (
                    *sent,
                    signal.activity,
                    signal.tab_id,
                    signal.locator.clone().unwrap_or_default(),
                )
            })
            .collect()
    }

    /// The starts `invocation` drew on tab `tab`, with how many commands had been sent by then.
    fn starts_on(&self, invocation: &str, tab: u64) -> Vec<(usize, PresentationActivity)> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, signal)| {
                signal.invocation == invocation
                    && signal.signal == PresentationKind::Start
                    && signal.tab_id == Some(tab)
            })
            .map(|(sent, signal)| (*sent, signal.activity))
            .collect()
    }
}

/// A fill names each field once the page has set it, an upload its input once the files are
/// attached, and a scroll its target once the page has scrolled to it, so the feedback lands
/// where the control now is and only on work that happened. A targeted scroll draws no cue at
/// the viewport centre before its own; a scroll with no target cues the page itself. A fill the
/// browser does not complete and an image dropped at a point name nothing, and typing still
/// starts its signature on the field's tab before it names the field.
#[test]
fn fill_upload_and_scroll_name_their_targets_once_the_browser_has_acted() {
    let (mut executor, browser, _, workspace, _) = fixture();
    let signals = Arc::new(OrderedSignals {
        browser: browser.clone(),
        log: Mutex::default(),
    });
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
    let fields = json!([
        {"target":handle(0),"value":"Ada"},
        {"target":handle(1),"value":"ada@example.com"}
    ]);
    let sent = |command: fn(&BrowserCommand) -> bool| {
        browser
            .calls()
            .iter()
            .rposition(command)
            .expect("the command was sent")
            + 1
    };

    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: controls[..2].to_vec(),
    }));
    browser.push(Err(BrowserError::DeadlineBeforeDispatch));
    let unfilled = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"tab":tab_handle,"fields":fields}),
        &CancellationToken::default(),
    );
    assert_eq!(unfilled.status, Status::Failed, "{unfilled:?}");
    assert!(signals.targets(unfilled.invocation.as_str()).is_empty());

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
        json!({"tab":tab_handle,"fields":fields}),
        &CancellationToken::default(),
    );
    assert_eq!(filled.status, Status::Succeeded, "{filled:?}");
    let after_fill = sent(|call| matches!(call, BrowserCommand::Fill { .. }));
    assert_eq!(
        signals.targets(filled.invocation.as_str()),
        [
            (
                after_fill,
                PresentationActivity::Fill,
                Some(7),
                "name-1".into()
            ),
            (
                after_fill,
                PresentationActivity::Fill,
                Some(7),
                "email-1".into()
            ),
        ]
    );

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
    let after_upload = sent(|call| matches!(call, BrowserCommand::UploadFiles { .. }));
    assert_eq!(
        signals.targets(uploaded.invocation.as_str()),
        [(
            after_upload,
            PresentationActivity::Upload,
            Some(7),
            "file-1".into()
        )]
    );

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
    let after_scroll = sent(|call| matches!(call, BrowserCommand::Scroll { .. }));
    assert_eq!(
        signals.targets(revealed.invocation.as_str()),
        [(
            after_scroll,
            PresentationActivity::Scroll,
            Some(7),
            "file-1".into()
        )]
    );
    assert!(
        signals
            .starts_on(revealed.invocation.as_str(), 7)
            .is_empty(),
        "a targeted scroll cues only its target"
    );

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
    assert!(signals.targets(scrolled.invocation.as_str()).is_empty());
    assert_eq!(
        signals
            .starts_on(scrolled.invocation.as_str(), 7)
            .iter()
            .map(|(_, activity)| *activity)
            .collect::<Vec<_>>(),
        [PresentationActivity::Scroll]
    );

    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: controls[..1].to_vec(),
    }));
    browser.push(Ok(BrowserOutcome::Typed {
        tab: tab(7, "https://example.com/"),
        character_count: 3,
        subject: None,
        committed_urls: Vec::new(),
    }));
    let typed = executor.execute(
        &workspace,
        "browser_type_text",
        json!({"tab":tab_handle,"target":handle(0),"text":"Ada"}),
        &CancellationToken::default(),
    );
    assert_eq!(typed.status, Status::Succeeded, "{typed:?}");
    let started = signals.starts_on(typed.invocation.as_str(), 7);
    let named = signals.targets(typed.invocation.as_str());
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].1, PresentationActivity::Type);
    assert_eq!(named.len(), 1);
    assert!(started[0].0 <= named[0].0);

    browser.push(Ok(BrowserOutcome::Screenshot {
        tab_id: 7,
        mime_type: "image/png".into(),
        data: "QQ==".into(),
        width: 800,
        height: 600,
        viewport: ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 0.0,
            page_y: 0.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 0.0,
            visual_page_y: 0.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 1.0,
        },
    }));
    let shot = executor.execute(
        &workspace,
        "browser_screenshot",
        json!({"tab":tab_handle}),
        &CancellationToken::default(),
    );
    assert_eq!(shot.status, Status::Succeeded, "{shot:?}");
    browser.push(Ok(BrowserOutcome::FilesUploaded {
        tab_id: 7,
        uploaded_count: 1,
        uploaded_bytes: 1,
        subject: None,
    }));
    let dropped = executor.execute(
        &workspace,
        "browser_upload",
        json!({"tab":tab_handle,"source_image":shot.facts["image"],"view":shot.facts["view"],"x":10,"y":10}),
        &CancellationToken::default(),
    );
    assert_eq!(dropped.status, Status::Succeeded, "{dropped:?}");
    assert!(signals.targets(dropped.invocation.as_str()).is_empty());
}

/// A read that finds its tab on a refused page holds the tab, and the hold shows on the page the
/// way a navigation's hold does.
#[test]
fn a_read_that_holds_a_tab_announces_the_hold() {
    let reference = Arc::new(PageSignals::default());
    PresentationReactor::new(reference.clone()).react(&crate::events::DomainEvent::HoldEntered {
        invocation: "reference".into(),
        workspace: "reference".into(),
        physical_id: 7,
    });
    let hold = reference.0.lock().unwrap()[0].clone();

    let policy = example_com_only();
    let (mut executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
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

    // The page moved itself to a refused host without a navigation Ghostlight saw.
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Elsewhere".into(),
        truncated: false,
        title: "Elsewhere".into(),
        url: "https://elsewhere.test/".into(),
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":opened.facts["tab"]}),
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Blocked, "{}", read.summary);
    assert_eq!(read.facts["held"], true);
    let signals = signals.0.lock().unwrap();
    assert!(
        signals.iter().any(|signal| signal.signal == hold.signal
            && signal.phase == hold.phase
            && signal.detail == hold.detail
            && signal.tab_id == Some(7)),
        "{signals:?}"
    );
}
