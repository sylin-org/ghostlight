//! Core executor dispatch, invocation lifecycle, and error mapping tests.

use super::*;
use crate::browser::adapter::{
    PhysicalClick, PhysicalFill, PhysicalPointClick, PhysicalPointDrop, PhysicalSemantic,
    PhysicalTypeFocused, PhysicalViewPoint, PhysicalWait,
};
use crate::language::outcome::{Outcome, Refusal};
use crate::language::ReadMode;

/// ACCEPTANCE Executor 3-4, L-D6 and ADR-0203 Decision 5: a result says what happened, with its
/// true effect. A browser failure renders its typed refusal with detail, its own sentence and the
/// browser's text kept to detail; a capture over budget, a disconnection, a deadline and a
/// cancellation before dispatch each say so truthfully; a contract failure after dispatch never
/// claims nothing was sent; an unknown effect reads as unknown and guides recovery without replay;
/// history with no page that way says which way. A capture states its size, a wait how long it
/// waited, an empty object what else reads as empty; readiness keeps its vocabulary; a receipt names
/// its target without trusting its role, governance can remove target names and keep the role,
/// focused typing names the control it described, and the adapter budget keeps time for the
/// receipt.
#[test]
fn a_browser_failure_renders_its_typed_refusal_with_the_true_effect() {
    // primitive adapter errors route to an honest refusal with detail.
    {
        let (refusal, facts) = routing_refusal(&BrowserError::primitive("no such frame"), || {
            ExtensionAdvice::Plain
        })
        .expect("primitive errors route to a refusal");
        let crate::language::outcome::Refusal::BrowserPrimitive { detail, .. } = &refusal else {
            panic!("primitive errors must route to BrowserPrimitive");
        };
        assert_eq!(detail, "no such frame");
        assert_eq!(refusal.reason(), "browser_primitive_failed");
        assert_eq!(facts["detail"], "no such frame");
    }

    // a debugger refusal reads its own sentence and keeps the browser text in detail.
    {
        use crate::language::outcome::PrimitiveCause;
        const BROWSER_TEXT: &str = "Cannot attach to this page.";
        for (cause, code) in [
            (PrimitiveCause::DebuggerUnavailable, "debugger_unavailable"),
            (PrimitiveCause::SessionLost, "session_lost"),
        ] {
            let (executor, browser, _, workspace, _) = fixture();
            browser.push(Err(BrowserError::Primitive {
                detail: BROWSER_TEXT.into(),
                cause,
            }));
            let result = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com/"}),
                &CancellationToken::default(),
            );
            let refusal = Refusal::BrowserPrimitive {
                detail: BROWSER_TEXT.into(),
                cause,
            };
            assert_eq!(result.summary, refusal.summary(), "{code}");
            assert!(!result.summary.contains(BROWSER_TEXT), "{code}");
            assert_eq!(result.facts["reason"], code);
            assert_eq!(result.facts["detail"], BROWSER_TEXT);
        }
    }

    // a capture over the budget says capture too large with its step.
    {
        let (executor, browser, _, workspace, _) = fixture();
        super::documents::open(&executor, &browser, &workspace);
        browser.push(Err(BrowserError::CaptureTooLarge {
            detail: "No quality in the budget fits the size limit.".into(),
        }));
        let result = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({}),
            &CancellationToken::default(),
        );
        assert_eq!(result.summary, Refusal::CaptureTooLarge.summary());
        assert_eq!(result.facts["reason"], Refusal::CaptureTooLarge.reason());
        assert_eq!(result.facts["reason"], "screenshot_too_large");
        assert!(!result.next_steps.is_empty());
    }

    // error reasons stay truthful about disconnection and unknown effects.
    {
        use crate::browser::recovery::RecoveryFailure;
        let cases: Vec<(&str, BrowserError)> = vec![
            ("before_dispatch", BrowserError::DisconnectedBeforeDispatch),
            ("not_answering", BrowserError::NotAnsweringBeforeDispatch),
            ("busy", BrowserError::BusyBeforeDispatch),
            ("wrong_profile", BrowserError::WrongProfile),
            ("after_dispatch", BrowserError::DisconnectedAfterDispatch),
            ("cancelled_before", BrowserError::CancelledBeforeDispatch),
            ("cancelled_after", BrowserError::CancelledAfterDispatch),
            ("deadline_before", BrowserError::DeadlineBeforeDispatch),
            ("deadline_after", BrowserError::DeadlineAfterDispatch),
            ("primitive", BrowserError::primitive("x")),
            ("stale_view", BrowserError::StaleView),
            ("stale_locator", BrowserError::stale(StaleReason::Removed)),
            ("interlock", BrowserError::LocalInterlock("x".into())),
            ("effect_unknown", BrowserError::EffectUnknown("x".into())),
            (
                "script_threw",
                BrowserError::ScriptThrew("ReferenceError: missing is not defined".into()),
            ),
            ("protocol", BrowserError::Protocol("x".into())),
            ("authentication", BrowserError::Authentication),
            (
                "unknown_browser",
                BrowserError::UnknownBrowser("browser_x".into()),
            ),
            ("pinned", BrowserError::BrowserPinned),
            (
                "ambiguous",
                BrowserError::AmbiguousBrowser(vec!["a".into()]),
            ),
            (
                "manual",
                BrowserError::RecoveryManual {
                    browsers: Vec::new(),
                },
            ),
            (
                "recovery_failed",
                BrowserError::RecoveryFailed {
                    reason: RecoveryFailure::LaunchFailed,
                    details: vec![],
                },
            ),
            (
                "incompatible",
                BrowserError::Incompatible {
                    offered: 1,
                    required: 2,
                },
            ),
            (
                "capability_version",
                BrowserError::Unsupported {
                    primitive: "input".into(),
                },
            ),
        ];
        let (executor, _, _, workspace, _) = fixture();
        let publication = executor.governance.published();
        let cancellation = CancellationToken::default();
        let findings = crate::work::findings::FrameFindings::default();
        let context = crate::work::InvocationContext {
            settlement_pending: &std::sync::atomic::AtomicBool::new(false),
            settlement_requested: false,
            provenance: &crate::provenance::ConnectionEvidence::claimed("test", IntakeChannel::Mcp),
            invocation: &crate::work::InvocationId::mint(),
            workspace: &workspace,
            requested_browser: None,
            requested_tab: None,
            requested_target: None,
            repeat_tool: None,
            gesture: crate::language::page_refusal::PageGesture::Other,
            snapshot: publication.snapshot(),
            publication: &publication,
            requirements: crate::governance::CapabilitySet::READ,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(1),
            deadline_adjustable: false,
            cancellation: &cancellation,
            findings: &findings,
        };
        for (name, error) in &cases {
            let refused = executor.browser_failure(
                &context,
                crate::governance::Decision::permitted(),
                error.clone(),
                None,
            );
            let claims = refused.result.facts["reason"] == "browser_disconnected";
            let truth = matches!(error, BrowserError::DisconnectedBeforeDispatch);
            assert_eq!(
                claims, truth,
                "{name} must not claim disconnection dishonestly"
            );
            assert_eq!(
                error.effect_unknown(),
                matches!(
                    error,
                    BrowserError::DisconnectedAfterDispatch
                        | BrowserError::CancelledAfterDispatch
                        | BrowserError::DeadlineAfterDispatch
                        | BrowserError::EffectUnknown(_)
                        | BrowserError::ScriptThrew(_)
                ),
                "{name} effect-unknown classification drifted"
            );
            // A script the page threw on says what it threw, and its effect stays unknown: it may
            // have acted first. A page exception is no dialog, so no dialog step is offered (L-D6).
            if let BrowserError::ScriptThrew(exception) = error {
                let result = &refused.result;
                assert_eq!(result.facts["reason"], "script_threw");
                assert_eq!(
                    result.facts[crate::language::outcome::EXCEPTION_FACT],
                    exception.as_str()
                );
                assert_eq!(result.facts["phase"], "adapter_reported");
                assert_eq!(result.status, Status::Unknown);
                assert_eq!(result.effect, Effect::Unknown);
                assert_eq!(
                    result.summary,
                    "The script threw \"ReferenceError: missing is not defined\". It may have changed the page before it threw."
                );
                assert!(result
                    .next_steps
                    .iter()
                    .all(|step| !step.contains("dialog")));
            }
        }
    }

    // a contract failure after dispatch never claims nothing was sent.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/","new_tab":true}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "0:doc_top:locator_button".into(),
                role: "button".into(),
                name: "Save".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        browser.push(Err(BrowserError::Protocol(
            "expected document-bound receipt".into(),
        )));

        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"selector":{"name":"Save","exact":true},"visual_settle":false}),
            &CancellationToken::default(),
        );

        assert!(
            browser
                .calls()
                .iter()
                .any(|call| matches!(call, BrowserCommand::Activate(PhysicalClick { .. }))),
            "the click reached the browser before its receipt failed the contract"
        );
        let contract = crate::language::outcome::Refusal::BrowserContract;
        assert_eq!(clicked.status, Status::Failed);
        assert_eq!(clicked.facts["reason"], "browser_contract_failed");
        assert_eq!(clicked.summary, contract.summary());
        assert!(
            !clicked.summary.to_lowercase().contains("nothing was sent"),
            "{}",
            clicked.summary
        );
        assert_eq!(clicked.next_steps, contract.next_steps());
    }

    // deadlines speak for themselves instead of claiming disconnection.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);

        browser.push(Err(BrowserError::DeadlineBeforeDispatch));
        let before = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(before.status, Status::Failed);
        assert!(before
            .summary
            .contains("ran out of time before anything changed in the browser"));
        assert!(!before.summary.contains("disconnected"));
        assert_eq!(before.facts["reason"], "deadline");
        assert_eq!(before.facts["phase"], "before_dispatch");
        // Navigation takes timeout_ms, so more time is advice the decoder will accept.
        assert_eq!(
            before.next_steps,
            vec!["Repeat with a longer timeout_ms when the page genuinely needs more time."]
        );

        browser.push(Err(BrowserError::DeadlineAfterDispatch));
        let after = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(after.status, Status::Unknown);
        assert!(after
            .summary
            .contains("ran out of time before the browser confirmed"));
        assert!(!after.summary.contains("disconnected"));
        assert_eq!(after.facts["reason"], "deadline");
        assert_eq!(after.facts["phase"], "after_dispatch");
        assert_eq!(
            after.next_steps,
            vec!["Check the page with browser_read or browser_screenshot before repeating; the action may already have happened."]
        );
    }

    // cancellation before dispatch says cancelled not disconnected.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Err(BrowserError::CancelledBeforeDispatch));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Cancelled);
        assert_eq!(result.effect, Effect::None);
        assert!(result.repeat_safe);
        assert_eq!(
            result.summary,
            "Cancelled before anything reached the browser."
        );
        assert!(result.next_steps.is_empty());
        assert_eq!(result.facts["reason"], "cancelled");
        let records = audit.0.lock().unwrap();
        assert_eq!(
            records.last().unwrap().refusal_facts,
            Some(crate::language::audit::AuditRefusal::CancelledBeforeDispatch)
        );
    }

    // effect unknown receipts render as unknown with the browser reason.
    {
        let (executor, browser, _workspaces, workspace, _) = fixture();
        browser.connect(vec![summary("browser_chrome", true)]);
        browser.push(Ok(BrowserOutcome::EffectUnknown {
            reason: "the page stopped responding after dispatch".into(),
        }));

        let terminal = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );

        assert_eq!(terminal.status, Status::Unknown);
        assert_eq!(terminal.facts["reason"], "browser_effect_unknown");
        assert_eq!(
            terminal.facts["detail"],
            "the page stopped responding after dispatch"
        );
    }

    // uncertain effect guides recovery without replay.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Err(crate::browser::BrowserError::DisconnectedAfterDispatch));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Unknown);
        assert_eq!(result.effect, Effect::Unknown);
        assert!(!result.repeat_safe);
        assert_eq!(
                result.next_steps,
                vec![
                    "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly.".to_string(),
                    "Then observe the page with browser_read or browser_inspect to learn what happened.".to_string(),
                ]
            );
    }

    // history with no page that way is refused and says which way.
    {
        let (executor, browser, _workspaces, workspace, tab_handle, _target) =
            workspace_with_a_target();
        for (action, summary) in [
            (
                "back",
                "This tab has no earlier page in its history, so nothing was sent.",
            ),
            (
                "forward",
                "This tab has no later page in its history, so nothing was sent.",
            ),
        ] {
            browser.push(Err(BrowserError::NoHistoryEntry));
            let refused = executor.execute(
                &workspace,
                "browser_history",
                json!({"action":action,"tab":tab_handle}),
                &CancellationToken::default(),
            );
            assert_eq!(refused.status, Status::Failed, "{action}");
            assert_eq!(refused.effect, Effect::None, "{action}");
            assert!(refused.repeat_safe, "{action}");
            assert_eq!(refused.summary, summary);
            assert_eq!(refused.facts["reason"], "no_history_entry");
            assert_eq!(
                refused.next_steps,
                vec!["Call browser_navigate with url to open a page."]
            );
        }
    }

    // a captured image states its decoded size.
    {
        for (data, size) in [("QQ==", 1u64), ("QUI=", 2), ("QUJD", 3), ("QUJDRA==", 4)] {
            let (executor, browser, _, workspace, _) = fixture();
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
            browser.push(Ok(BrowserOutcome::Screenshot {
                tab_id: 7,
                mime_type: "image/png".into(),
                data: data.into(),
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
                uploaded_bytes: size,
                subject: None,
            }));
            let dropped = executor.execute(
                &workspace,
                "browser_upload",
                json!({
                    "tab": tab_handle,
                    "source_image": shot.facts["image"],
                    "view": shot.facts["view"],
                    "x": 10,
                    "y": 10
                }),
                &CancellationToken::default(),
            );
            assert_eq!(dropped.status, Status::Succeeded, "{data}: {dropped:?}");
            assert!(
                browser.calls().iter().any(|call| matches!(
                    call,
                    BrowserCommand::DropImageAt(PhysicalPointDrop { file, .. }) if file.size == size && file.data == data
                )),
                "{data} must state {size} bytes: {:?}",
                browser.calls()
            );
        }
    }

    // a capture reports its size and a wait reports how long it waited.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "image".into(),
            width: 1280,
            height: 720,
            viewport: ViewportGeometry {
                scope: CaptureScope::Viewport,
                page_x: 0.0,
                page_y: 0.0,
                css_width: 1280.0,
                css_height: 720.0,
                visual_page_x: 0.0,
                visual_page_y: 0.0,
                visual_css_width: 1280.0,
                visual_css_height: 720.0,
                device_scale: 1.0,
                zoom: 1.0,
                output_scale: 1.0,
            },
        }));
        let captured = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(captured.summary, "Captured the viewport at 1280x720.");

        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 1_830,
            readiness: BrowserReadiness::Complete,
        }));
        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":handle,"condition":"load_ready","timeout_ms":5_000}),
            &CancellationToken::default(),
        );
        assert_eq!(waited.summary, "example.com finished loading in 2 seconds.");

        let records = audit.0.lock().unwrap();
        let capture = &records[1].observed;
        assert_eq!((capture.width, capture.height), (Some(1280), Some(720)));
        // A capture is its own invocation. It reports the size it took and leaves the landing to
        // the invocation that navigated, because an observation never outlives its invocation.
        assert_eq!(capture.host.as_deref(), None);
        let wait = &records[2].observed;
        assert_eq!(wait.count, Some(2));
        assert_eq!(wait.readiness.as_deref(), Some("complete"));
    }

    // an empty object result says what else reads as empty.
    {
        use crate::browser::adapter::ScriptForm;

        let awaited = "DOM node, a Map or a class instance";
        let unawaited = "unawaited promise, a DOM node or a Map";
        for (value, truncated, form, step) in [
            ("{}", false, ScriptForm::Awaited, Some(awaited)),
            ("{}", false, ScriptForm::AsWritten, Some(unawaited)),
            ("{\"a\":1}", false, ScriptForm::Awaited, None),
            ("{\"a\":1}", false, ScriptForm::AsWritten, None),
            ("null", false, ScriptForm::AsWritten, None),
            ("{}", true, ScriptForm::Awaited, None),
            ("{}", true, ScriptForm::AsWritten, None),
        ] {
            let (executor, browser, _, workspace, audit) = fixture();
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(7, "https://example.com/"),
                committed_urls: vec!["https://example.com/".into()],
            }));
            executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com/"}),
                &CancellationToken::default(),
            );
            browser.push(Ok(BrowserOutcome::ScriptEvaluated {
                tab: tab(7, "https://example.com/"),
                value: value.into(),
                truncated,
                form,
                committed_urls: vec![],
            }));
            let result = executor.execute(
                &workspace,
                "browser_execute",
                json!({"script":"PRIVATE_SCRIPT"}),
                &CancellationToken::default(),
            );
            assert_eq!(result.status, Status::Succeeded, "{result:?}");
            assert_eq!(result.summary, "Executed JavaScript on example.com.");
            match step {
                Some(words) => {
                    assert_eq!(result.next_steps.len(), 1, "{value} {form:?}: {result:?}");
                    assert!(
                        result.next_steps[0].contains(words),
                        "{:?}",
                        result.next_steps
                    );
                }
                None => assert!(
                    result.next_steps.is_empty(),
                    "{value} {truncated} {form:?}: {:?}",
                    result.next_steps
                ),
            }
            assert_eq!(result.facts["truncated"], truncated);
            let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
            assert!(!encoded.contains("PRIVATE_SCRIPT") && !encoded.contains("reads as JSON"));
        }
    }

    // readiness names stay the vocabulary a result uses.
    {
        for value in [
            Readiness::NotApplicable,
            Readiness::Loading,
            Readiness::Interactive,
            Readiness::Complete,
            Readiness::Unknown,
        ] {
            let encoded = serde_json::to_value(value).unwrap();
            assert_eq!(
                encoded.as_str().unwrap(),
                readiness_name(value),
                "the observation and the result would disagree about readiness"
            );
        }
    }

    // physical action receipt names the target without trusting its role.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "hostile-role".into(),
                role: "Save my document".into(),
                name: "private patient action".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();

        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: Some(PhysicalActionSubject {
                role: "Save my document".into(),
                name: "Save patient record".into(),
                rebound: false,
            }),
        }));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );

        assert_eq!(
            clicked.summary,
            "Clicked the \"Save patient record\" control on example.com."
        );
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(!encoded.contains("Save my document"));
        assert!(encoded.contains("Save patient record"));
        assert!(!encoded.contains("private patient action"));
    }

    // governance can remove target names without losing the safe role.
    {
        let policy = temporary_policy("hide-target-names");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (executor, browser, _, workspace, audit) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "save".into(),
                role: "button".into(),
                name: "Save patient record".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: Some(PhysicalActionSubject {
                role: "button".into(),
                name: "Save patient record".into(),
                rebound: false,
            }),
        }));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );

        assert_eq!(clicked.summary, "Clicked a button on example.com.");
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(!encoded.contains("Save patient record"));
    }

    // focused typing names the control it described and settles.
    {
        let (executor, browser, _workspaces, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );

        // The describe step observes the focused control; the typed outcome itself carries
        // no subject. The receipt must name the described control instead of panicking
        // after the effect has already landed.
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            rebound: false,
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "0:locator_1".into(),
                role: "textbox".into(),
                name: "Ledger project".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        }));
        browser.push(Ok(BrowserOutcome::Typed {
            tab: tab(7, "https://example.com/"),
            character_count: 5,
            subject: None,
            committed_urls: vec![],
        }));

        let typed = executor.execute(
            &workspace,
            "browser_type_text",
            json!({"text":"Ember","focused":true,"clear_first":true}),
            &CancellationToken::default(),
        );

        assert_eq!(typed.status, Status::Succeeded);
        assert_eq!(typed.effect, Effect::Applied);
        assert!(
            typed.summary.contains("Ledger project"),
            "summary must name the described control: {}",
            typed.summary
        );
        let calls = browser.calls();
        assert!(matches!(calls[1], BrowserCommand::DescribeFocused { .. }));
        assert!(matches!(
            calls[2],
            BrowserCommand::TypeFocused(PhysicalTypeFocused { .. })
        ));
    }

    // adapter budget preserves time for the physical receipt.
    {
        assert_eq!(
            adapter_budget_ms(3_000, Duration::from_millis(3_000)),
            2_250
        );
        assert_eq!(adapter_budget_ms(100, Duration::from_millis(100)), 0);
        assert_eq!(adapter_budget_ms(500, Duration::from_millis(5_000)), 500);
    }
}

/// ACCEPTANCE Executor 6 (d0cea694): a stale handle refuses before dispatch with the word that
/// says why. The refusal offers fresh candidates from the live page; a page-side stale handle refuses
/// as a stale target with its word and a typed-into removed handle is tombstoned; an unknown word is
/// not remembered; a closed tab and a left document say so; the audit keeps only the word.
#[test]
fn a_stale_handle_refuses_before_dispatch_with_the_word_that_says_why() {
    // a stale target refusal offers fresh candidates from the live page.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);

        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "btn-1".into(),
                role: "button".into(),
                name: "Save".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle,"scope":"controls","max_items":10}),
            &CancellationToken::default(),
        );
        assert_eq!(
            inspected.status,
            Status::Succeeded,
            "inspect failed: {}",
            inspected.summary
        );

        let target_handle = inspected.facts["items"][0]["target"]
            .as_str()
            .expect("no target handle in inspect facts")
            .to_owned();

        // Commit a navigation: the document generation moves and the old handle goes stale.
        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.com/next"),
            committed_urls: vec![],
        }));
        let _ = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/next","tab":tab_handle}),
            &CancellationToken::default(),
        );

        // The page still has its Save button under a fresh locator. Clicking by the stale
        // handle fails -- with the live candidate riding in the facts.
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "btn-2".into(),
                role: "button".into(),
                name: "Save".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target_handle}),
            &CancellationToken::default(),
        );

        assert_eq!(clicked.status, Status::Failed);
        assert_eq!(clicked.facts["reason"], "stale_target");
        let candidates = clicked.facts["recovery_candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0]["name"], "Save");
        assert_eq!(candidates[0]["role"], "button");
    }

    // stale target fails before browser dispatch.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "old".into(),
                role: "button".into(),
                name: "Old".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.org/"),
            committed_urls: vec!["https://example.org/".into()],
        }));
        let navigated = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"tab":tab_handle,"url":"https://example.org"}),
            &CancellationToken::default(),
        );
        assert_eq!(navigated.status, Status::Succeeded);
        let before = browser.calls().len();
        // The live page still has a button answering to the old name.
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "new".into(),
                role: "button".into(),
                name: "Old".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let stale = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(stale.status, Status::Failed);
        assert_eq!(stale.facts["reason"], "stale_target");
        // Exactly one extra crossing happened: the candidate probe. The click itself never
        // dispatched.
        assert_eq!(browser.calls().len(), before + 1);
        assert!(matches!(
            browser.calls().last(),
            Some(BrowserCommand::QuerySemantic(PhysicalSemantic { name, .. }))
                if name == "Old"
        ));
        let candidates = stale.facts["recovery_candidates"].as_array().unwrap();
        assert_eq!(candidates[0]["name"], "Old");
    }

    // a page side stale handle refuses as stale target with its word.
    {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(BrowserError::stale(StaleReason::Removed)));
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "delete-2".into(),
                role: "button".into(),
                name: "Delete".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        assert_stale_removed(&clicked, PICK_STEP);
        let candidates = clicked.facts["recovery_candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0]["name"], "Delete");
    }

    // a tombstoned handle refuses before dispatch.
    {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(BrowserError::stale(StaleReason::Removed)));
        executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        let before = browser.calls().len();
        let again = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        assert_stale_removed(&again, REMOVED_STEP);
        assert!(browser.calls()[before..]
            .iter()
            .all(|call| matches!(call, BrowserCommand::QuerySemantic(PhysicalSemantic { .. }))));
    }

    // a typed into handle the page removed is tombstoned.
    {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(BrowserError::stale(StaleReason::Removed)));
        let typed = executor.execute(
            &workspace,
            "browser_type_text",
            json!({"tab":tab_handle,"target":target,"text":"hello"}),
            &CancellationToken::default(),
        );
        assert_stale_removed(&typed, REMOVED_STEP);
        let before = browser.calls().len();
        let again = executor.execute(
            &workspace,
            "browser_type_text",
            json!({"tab":tab_handle,"target":target,"text":"hello"}),
            &CancellationToken::default(),
        );
        assert_stale_removed(&again, REMOVED_STEP);
        assert!(browser.calls()[before..]
            .iter()
            .all(|call| matches!(call, BrowserCommand::QuerySemantic(PhysicalSemantic { .. }))));
    }

    // an unknown page side word is not remembered.
    {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        for _ in 0..2 {
            let before = browser.calls().len();
            browser.push(Err(BrowserError::stale(StaleReason::Unknown)));
            let clicked = executor.execute(
                &workspace,
                "browser_click",
                json!({"tab":tab_handle,"target":target}),
                &CancellationToken::default(),
            );
            assert_eq!(clicked.facts["reason"], "stale_target");
            assert_eq!(clicked.facts["stale"], "unknown");
            assert_eq!(
                clicked.summary,
                "Ghostlight does not know that target handle."
            );
            assert!(
                browser.calls().len() > before,
                "the click reached the browser"
            );
        }
    }

    // a closed tab and a left document say tab closed and navigated.
    {
        let (executor, browser, workspaces, workspace, tab_handle, target) =
            workspace_with_a_target();
        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.com/next"),
            committed_urls: vec![],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/next","tab":tab_handle}),
            &CancellationToken::default(),
        );
        let navigated = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(navigated.facts["stale"], "navigated");
        assert_eq!(
            navigated.summary,
            "That target belongs to an older version of the page."
        );
        assert_eq!(
            navigated.next_steps,
            vec!["Call browser_inspect to obtain current target handles."]
        );

        workspaces.apply_browser_close(FAKE_BROWSER, 7);
        let closed = executor.execute(
            &workspace,
            "browser_click",
            json!({"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.facts["reason"], "stale_target");
        assert_eq!(closed.facts["stale"], "tab_closed");
        assert_eq!(closed.summary, "That target's tab is closed.");
        assert_eq!(
            closed.next_steps,
            vec!["Call browser_tabs to choose an open tab."]
        );
    }

    // a stale refusal audits the word only.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "private".into(),
                role: "button".into(),
                name: "private patient action".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Err(BrowserError::stale(StaleReason::Removed)));
        executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(encoded.contains("removed"), "{encoded}");
        assert!(!encoded.contains(&target), "{encoded}");
        assert!(!encoded.contains("private patient action"), "{encoded}");
    }
}

/// ACCEPTANCE Browser journey 5 and Bounded preparation (ADR-0195; 2513f862): a wait is decisive
/// within its deadline. A semantic selector polls the live page; an unsatisfied wait is decisive
/// before the deadline; a duration past or longer than the time left is refused safely without
/// sleeping; visual settlement observes the view and chains after a primary condition, which a
/// composite wait keeps when optional settlement expires. Optional preparation spends at most its
/// budget and opting out spends none.
#[test]
fn a_wait_is_decisive_within_its_deadline() {
    // waiting on a semantic selector polls the live page.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        // One miss, then the control appears.
        browser.push(Ok(BrowserOutcome::targets(7, vec![])));
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "b-1".into(),
                role: "button".into(),
                name: "Ready".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let waited = executor.execute(
                &workspace,
                "browser_wait",
                json!({"tab":tab_handle,"condition":"selector_present","selector":{"name":"Ready","role":"button"}}),
                &CancellationToken::default(),
            );
        assert_eq!(waited.status, Status::Succeeded);
        assert_eq!(waited.facts["condition"], "selector_present");
        assert_eq!(waited.facts["satisfied"], true);
        // Like every other wait, it says how ready the page was.
        assert_eq!(waited.facts["readiness"], json!(waited.readiness));
        assert!(waited.facts["readiness"].is_string());

        // A selector that never shows up inside its budget fails decisively.
        for _ in 0..5 {
            browser.push(Ok(BrowserOutcome::targets(7, vec![])));
        }
        let missed = executor.execute(
                &workspace,
                "browser_wait",
                json!({"tab":tab_handle,"condition":"selector_present","selector":{"name":"Never","role":"button"},"timeout_ms":250}),
                &CancellationToken::default(),
            );
        assert_eq!(missed.status, Status::Failed);
        assert_eq!(missed.facts["satisfied"], false);
        assert_eq!(missed.facts["readiness"], json!(missed.readiness));
    }

    // unsatisfied wait is decisive before the invocation deadline.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: false,
            elapsed_ms: 750,
            readiness: BrowserReadiness::Complete,
        }));
        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({
                "tab":tab_handle,
                "condition":"text_present",
                "value":"never present",
                "timeout_ms":1_000
            }),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Failed);
        assert_eq!(waited.effect, Effect::None);
        assert!(waited.repeat_safe);
        assert_eq!(waited.facts["satisfied"], false);
        let calls = browser.calls();
        let timeout_ms = calls
            .iter()
            .find_map(|call| match call {
                BrowserCommand::Observe(PhysicalWait { timeout_ms, .. }) => Some(*timeout_ms),
                _ => None,
            })
            .unwrap();
        assert!(timeout_ms <= 750);
        assert!(timeout_ms > 0);
    }

    // a duration wait past its deadline is repeat safe and says nothing was sent.
    {
        let (executor, browser, workspace, tab_handle) = one_open_tab();
        let sent = browser.calls().len();

        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":tab_handle,"condition":"duration","value":"10000"}),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Failed, "{}", waited.summary);
        assert_eq!(waited.effect, Effect::None);
        assert!(waited.repeat_safe);
        assert_eq!(
            waited.summary,
            "The job ran out of time before anything changed in the browser."
        );
        assert_eq!(waited.facts["reason"], "deadline");
        assert_eq!(waited.facts["phase"], "before_dispatch");
        assert_eq!(
            waited.next_steps,
            vec!["Repeat with a longer timeout_ms when the page genuinely needs more time."]
        );
        assert_eq!(browser.calls().len(), sent, "a sleep sends nothing");
    }

    // a duration longer than the time left is refused without sleeping.
    {
        let (executor, _browser, workspace, tab_handle) = one_open_tab();

        let started = std::time::Instant::now();
        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":tab_handle,"condition":"duration","value":"5000","timeout_ms":2_000}),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Failed, "{}", waited.summary);
        assert_eq!(waited.facts["phase"], "before_dispatch");
        assert!(
            started.elapsed() < Duration::from_millis(1_000),
            "the wait slept {:?}",
            started.elapsed()
        );

        let fits = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":tab_handle,"condition":"duration","value":"50","visual_settle":false,
                "timeout_ms":2_000}),
            &CancellationToken::default(),
        );
        assert_eq!(fits.status, Status::Succeeded, "{}", fits.summary);
        assert_eq!(fits.facts["satisfied"], true);
        assert!(fits.facts["elapsed_ms"].as_u64().unwrap() >= 50);
    }

    // wait condition visual settle observes view settlement.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 42,
            readiness: BrowserReadiness::Complete,
        }));
        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({
                "tab": tab_handle,
                "condition": "visual_settle",
                "timeout_ms": 1_000
            }),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Succeeded);
        assert_eq!(waited.facts["condition"], "visual_settle");
        assert_eq!(waited.facts["satisfied"], true);
        assert_eq!(waited.facts["elapsed_ms"], 42);
        assert!(waited
            .summary
            .contains("The view settled visually on example.com in under a second."));

        let calls = browser.calls();
        assert!(calls.iter().any(|call| matches!(
            call,
            BrowserCommand::Observe(PhysicalWait {
                condition: crate::language::PageCondition::VisualSettle,
                ..
            })
        )));
    }

    // wait composite visual settle chains after primary condition.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        // Primary observe (load_ready)
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 50,
            readiness: BrowserReadiness::Interactive,
        }));
        // Follow-up visual settle observe
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 30,
            readiness: BrowserReadiness::Complete,
        }));

        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({
                "tab": tab_handle,
                "condition": "load_ready",
                "visual_settle": true,
                "timeout_ms": 1_000
            }),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Succeeded);
        assert_eq!(waited.facts["condition"], "load_ready");
        assert_eq!(waited.facts["satisfied"], true);
        assert_eq!(waited.facts["visual_settle"], true);
        assert_eq!(waited.facts["elapsed_ms"], 80);

        let calls = browser.calls();
        let observe_conditions: Vec<&str> = calls
            .iter()
            .filter_map(|call| match call {
                BrowserCommand::Observe(PhysicalWait { condition, .. }) => Some(condition.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(observe_conditions, vec!["load_ready", "visual_settle"]);
    }

    // composite wait keeps its met condition when optional settlement expires.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        // Primary condition (target_present) satisfies in 20ms
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 20,
            readiness: BrowserReadiness::Interactive,
        }));
        // Follow-up visual settle fails (times out due to continuous animation) after 80ms
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: false,
            elapsed_ms: 80,
            readiness: BrowserReadiness::Interactive,
        }));

        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({
                "tab": tab_handle,
                "condition": "load_ready",
                "visual_settle": true,
                "timeout_ms": 100
            }),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Succeeded);
        assert_eq!(waited.facts["satisfied"], true);
        assert_eq!(waited.facts["visual_settled"], false);
        assert_eq!(waited.facts["visual_settle"], true);
        assert_eq!(waited.facts["elapsed_ms"], 100);
        assert!(waited.next_steps.is_empty());
    }

    // composite visual settle stress semantic selector chain.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        // Selector query misses once, then finds button
        browser.push(Ok(BrowserOutcome::targets(7, vec![])));
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "btn-submit".into(),
                role: "button".into(),
                name: "Submit".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));

        // Follow-up visual settle succeeds
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 35,
            readiness: BrowserReadiness::Complete,
        }));

        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({
                "tab": tab_handle,
                "condition": "selector_present",
                "selector": {"name": "Submit", "role": "button"},
                "visual_settle": true,
                "timeout_ms": 1_000
            }),
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Succeeded);
        assert_eq!(waited.facts["condition"], "selector_present");
        assert_eq!(waited.facts["satisfied"], true);
        assert_eq!(waited.facts["visual_settle"], true);
        assert!(waited.facts["elapsed_ms"].as_u64().unwrap() >= 35);

        let calls = browser.calls();
        assert!(calls
            .iter()
            .any(|c| matches!(c, BrowserCommand::QuerySemantic(PhysicalSemantic { .. }))));
        assert!(calls.iter().any(|c| matches!(
            c,
            BrowserCommand::Observe(PhysicalWait {
                condition: crate::language::PageCondition::VisualSettle,
                ..
            })
        )));
    }

    // optional preparation stays within its budget and opting out spends none.
    {
        for enabled in [true, false] {
            let (executor, browser, _, workspace, _) = fixture();
            super::documents::open(&executor, &browser, &workspace);
            browser.push(Ok(BrowserOutcome::targets(
                7,
                vec![ObservedTarget {
                    locator: "0:doc_top:locator_button".into(),
                    role: "button".into(),
                    name: "Save".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                }],
            )));
            browser.push(Ok(BrowserOutcome::Activated {
                tab: tab(7, "https://example.com/"),
                subject: None,
                committed_urls: vec![],
            }));
            let result = executor.execute(
                &workspace,
                "browser_click",
                json!({"selector":{"name":"Save","exact":true},"visual_settle":enabled}),
                &CancellationToken::default(),
            );
            assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
            let scopes = browser.scopes();
            assert_eq!(scopes.len(), 2);
            assert!(scopes[0].settle_ms <= 1000);
            assert_eq!(scopes[0].settle_ms > 0, enabled);
            assert_eq!(
                scopes[1].settle_ms, 0,
                "the action shares the lookup's budget"
            );
            assert!(scopes.iter().all(|scope| scope.settle_requested == enabled));
        }
        for (arguments, settles) in [
            (json!({"timeout_ms":600}), true),
            (json!({"timeout_ms":600,"visual_settle":false}), false),
            (json!({}), true),
        ] {
            let (executor, browser, _, workspace, _) = fixture();
            super::documents::open(&executor, &browser, &workspace);
            browser.push(Ok(super::documents::captured_image()));
            let result = executor.execute(
                &workspace,
                "browser_screenshot",
                arguments.clone(),
                &CancellationToken::default(),
            );
            assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
            let sent = browser
                .calls()
                .into_iter()
                .find_map(|call| match call {
                    BrowserCommand::Screenshot { settle_ms, .. } => Some(settle_ms),
                    _ => None,
                })
                .expect("a capture was sent");
            let ceiling = arguments["timeout_ms"].as_u64().map_or(1_000, |ms| ms / 2);
            assert!(sent <= ceiling, "{arguments}: {sent} ms of settle");
            assert_eq!(sent > 0, settles, "{arguments}");
        }
    }
}

/// ACCEPTANCE Executor 3: every tool gets deadline advice it can follow, and a call already at
/// the longest timeout is not told to raise it.
#[test]
fn every_tool_gets_deadline_advice_it_can_follow() {
    // every tool gets deadline advice it can follow.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );

        browser.push(Err(BrowserError::DeadlineBeforeDispatch));
        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({}),
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Failed);
        assert_eq!(read.facts["phase"], "before_dispatch");
        assert_eq!(
            read.next_steps,
            vec!["Repeat with a longer timeout_ms when the page genuinely needs more time."]
        );
        assert!(crate::language::decode("browser_read", json!({"timeout_ms":30_000})).is_ok());

        browser.push(Err(BrowserError::DeadlineBeforeDispatch));
        let longest = executor.execute(
            &workspace,
            "browser_read",
            json!({"timeout_ms":30_000}),
            &CancellationToken::default(),
        );
        assert_eq!(
            longest.next_steps,
            vec!["Repeat the call; if the page is very large, narrow it first (a target, a smaller max_chars, or a smaller inspect scope)."]
        );
        assert!(crate::language::decode("browser_read", json!({"timeout_ms":30_001})).is_err());
    }

    // calls at the longest timeout are not told to raise it.
    {
        let (executor, browser, _, workspace, _) = fixture();
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
                locator: "name-1".into(),
                role: "textbox".into(),
                name: "Name".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        let fields = json!([{"target":target,"value":"Ada"}]);

        browser.push(Err(BrowserError::DeadlineBeforeDispatch));
        let default_fill = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({"tab":tab_handle,"fields":fields}),
            &CancellationToken::default(),
        );
        assert_eq!(default_fill.status, Status::Failed, "{default_fill:?}");
        assert_eq!(default_fill.facts["reason"], "deadline");
        assert_eq!(default_fill.facts["phase"], "before_dispatch");
        assert!(!default_fill.next_steps.is_empty());
        assert!(
            !default_fill.next_steps.join(" ").contains("timeout_ms"),
            "{:?}",
            default_fill.next_steps
        );

        browser.push(Err(BrowserError::DeadlineBeforeDispatch));
        let longest_navigation = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/","timeout_ms":30_000}),
            &CancellationToken::default(),
        );
        assert_eq!(longest_navigation.facts["phase"], "before_dispatch");
        assert!(!longest_navigation
            .next_steps
            .join(" ")
            .contains("timeout_ms"));

        browser.push(Err(BrowserError::DeadlineBeforeDispatch));
        let shorter_fill = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({"tab":tab_handle,"fields":fields,"timeout_ms":20_000}),
            &CancellationToken::default(),
        );
        assert_eq!(shorter_fill.facts["phase"], "before_dispatch");
        assert_eq!(
            shorter_fill.next_steps,
            vec!["Repeat with a longer timeout_ms when the page genuinely needs more time."]
        );
    }
}

/// ACCEPTANCE Browser journey 1 and W-D4: a session opens, reads and closes, one truthful result
/// per call, in the browser it chose. An empty session says it has no tab yet and how to open one; a
/// dead tab navigated to is recreated under the same handle; a list reports live titles and drops
/// bindings that are gone; an inspection returns the tree it describes; closing a handle this
/// service does not hold claims nothing about the tab. Work follows the attended browser and stays
/// where it started; an ambiguous start names the choices and touches no browser; manual recovery
/// says what the model should do; a named browser opens there and a named stranger is refused.
#[test]
fn a_session_opens_reads_and_closes_in_the_browser_it_chose() {
    // open read close is one truthful result per call.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let handle = opened.facts["tab"].as_str().unwrap();
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "Example Domain".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/".into(),
        }));
        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Succeeded);
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Succeeded);
        assert_eq!(audit.0.lock().unwrap().len(), 3);
        let calls = browser.calls();
        assert_eq!(calls.len(), 3);
        assert!(matches!(
            &calls[0],
            BrowserCommand::OpenTab { url, group_title, .. }
                if url == "https://example.com" && group_title == "\u{1F47B} test"
        ));
        assert!(matches!(
            &calls[1],
            BrowserCommand::ReadDocument { mode, max_chars, .. }
                if *mode == ReadMode::Visible && *max_chars == 8_000
        ));
    }

    // an empty session says no tab yet and how to open one.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        let result = executor.execute(
            &workspace,
            "browser_diagnose",
            json!({}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Failed);
        assert_eq!(result.effect, Effect::None);
        assert_eq!(result.summary, "This session has no open tab yet.");
        assert_eq!(result.facts["reason"], "no_tab");
        assert_eq!(
            result.next_steps,
            vec!["Call browser_navigate with url to open a page."]
        );
        assert!(browser.calls().is_empty());
    }

    // navigating to a dead tab recreates it under the same handle.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(9, "https://example.com/again"),
            committed_urls: vec!["https://example.com/again".into()],
        }));

        let recovered = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/again","tab":"tab_deadbeefdeadbeefdeadbeefdeadbeef"}),
            &CancellationToken::default(),
        );

        assert_eq!(recovered.status, Status::Succeeded);
        assert_eq!(recovered.effect, Effect::Applied);
        assert!(!recovered.repeat_safe);
        assert_eq!(
            recovered.facts["tab"],
            json!("tab_deadbeefdeadbeefdeadbeefdeadbeef")
        );
        assert_eq!(recovered.facts["recovered"], "new_tab");
        assert!(recovered.summary.contains("That tab was gone"));
        assert!(matches!(browser.calls()[0], BrowserCommand::OpenTab { .. }));
    }

    // listing tabs reports live titles and drops bindings that are gone.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap();

        // The live browser says the bound tab moved and was retitled: the list shows the
        // current truth, not the remembered landing facts.
        browser.push(Ok(BrowserOutcome::Tabs {
            tabs: vec![PhysicalTab {
                tab_id: 7,
                title: "Example Domain moved".into(),
                url: "https://example.com/moved".into(),
                active: true,
                readiness: BrowserReadiness::Complete,
            }],
        }));
        let listed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(
            listed.facts["tabs"],
            json!([{
                "tab":handle,
                "title":"Example Domain moved",
                "url":"https://example.com/moved",
                "active":true,
                "readiness":"complete"
            }])
        );

        // The live browser says the tab is gone; remembered state would keep showing it.
        browser.push(Ok(BrowserOutcome::Tabs { tabs: vec![] }));
        let gone = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(gone.status, Status::Succeeded);
        assert_eq!(gone.facts["tabs"], json!([]));
    }

    // document inspection returns the tree it describes.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        let page = json!({"kind":"container","label":"","children":[
            {"kind":"heading","label":"Account settings","children":[]},
            {"kind":"container","label":"Profile","children":[
                {"kind":"control","label":"Save profile","children":[]}
            ]}
        ]});
        browser.push(Ok(BrowserOutcome::DocumentTree {
            tab_id: 7,
            tree: serde_json::from_value(page.clone()).unwrap(),
            truncated: false,
        }));
        let first = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"scope":"document"}),
            &CancellationToken::default(),
        );
        assert_eq!(first.status, Status::Succeeded, "{}", first.summary);
        assert_eq!(first.facts["tree"], page);
        assert_eq!(
            first.facts.pointer("/tree/children/1/children/0/label"),
            Some(&json!("Save profile")),
            "the label the page shows reaches the model"
        );
        assert_eq!(first.facts["nodes"], 4);
        assert!(first.facts["snapshot"]
            .as_str()
            .is_some_and(|handle| handle.starts_with("snapshot_")));
        assert!(first.facts.get("diff").is_none());
        assert_eq!(first.summary, "Returned the document outline: 4 nodes.");

        let changed = json!({"kind":"container","label":"","children":[
            {"kind":"heading","label":"Account settings","children":[]},
            {"kind":"container","label":"Profile","children":[
                {"kind":"control","label":"Saved","children":[]}
            ]},
            {"kind":"control","label":"Undo","children":[]}
        ]});
        browser.push(Ok(BrowserOutcome::DocumentTree {
            tab_id: 7,
            tree: serde_json::from_value(changed.clone()).unwrap(),
            truncated: false,
        }));
        let second = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"scope":"document"}),
            &CancellationToken::default(),
        );
        assert_eq!(second.status, Status::Succeeded, "{}", second.summary);
        assert_eq!(
            second.facts["diff"],
            json!({"added":1,"removed":0,"changed":1,"paths":["/1/0","/2"]})
        );
        for path in second.facts["diff"]["paths"].as_array().unwrap() {
            let position = path.as_str().unwrap().replace('/', "/children/");
            assert!(
                second.facts["tree"].pointer(&position).is_some(),
                "{path} is a position in the returned tree"
            );
        }
        assert_eq!(
            second.summary,
            "Returned the document outline: 5 nodes, 2 differences from the previous outline."
        );

        let shallow = json!({"kind":"container","label":"","children":[
            {"kind":"heading","label":"Account settings","children":[]}
        ]});
        browser.push(Ok(BrowserOutcome::DocumentTree {
            tab_id: 7,
            tree: serde_json::from_value(shallow.clone()).unwrap(),
            truncated: false,
        }));
        let third = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"scope":"document","max_depth":2}),
            &CancellationToken::default(),
        );
        assert_eq!(third.status, Status::Succeeded, "{}", third.summary);
        assert!(
            third.facts.get("diff").is_none(),
            "a tree of another depth is not compared with the last one"
        );
        assert_eq!(third.facts["tree"], shallow);
    }

    // closing a handle this service does not hold claims nothing about the tab.
    {
        let (executor, browser, _, workspace, _) = fixture();
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
        let handle = opened.facts["tab"].clone();

        // A new service: the same browser, still showing the tab, and a store that never held it.
        let restarted = WorkspaceStore::default();
        let (next, next_workspace, _) =
            executor_on(GovernanceFacade::new(None), &restarted, browser.clone());
        let calls_before = browser.calls().len();
        let closed = next.execute(
            &next_workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            &CancellationToken::default(),
        );

        assert_eq!(closed.status, Status::Succeeded);
        assert_eq!(closed.effect, Effect::None);
        assert_eq!(closed.summary, Outcome::TabNotHeld.summary());
        assert_eq!(closed.next_steps, Outcome::TabNotHeld.next_steps());
        assert_eq!(closed.facts["closed"], false);
        assert_eq!(browser.calls().len(), calls_before, "nothing is sent");
    }

    // work follows the attended browser and then stays where it started.
    {
        let (executor, browser, workspaces, workspace, _) = fixture();
        browser.connect(vec![
            summary("browser_chrome", false),
            summary("browser_edge", true),
        ]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));

        // Nothing named a browser, so the work goes where the person last was.
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        assert_eq!(browser.routed(), vec!["browser_edge"]);
        assert_eq!(
            workspaces.browser_of(workspace.as_str()).as_deref(),
            Some("browser_edge")
        );

        // The person turns to Chrome. Established work does not follow them there.
        browser.connect(vec![
            summary("browser_chrome", true),
            summary("browser_edge", false),
        ]);
        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.com/next"),
            committed_urls: vec!["https://example.com/next".into()],
        }));
        let followed = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/next"}),
            &CancellationToken::default(),
        );
        assert_eq!(followed.status, Status::Succeeded);
        assert_eq!(browser.routed(), vec!["browser_edge", "browser_edge"]);
    }

    // an ambiguous bootstrap names the choices and touches no browser.
    {
        let (executor, browser, workspaces, workspace, _) = fixture();
        browser.connect(vec![
            summary("browser_chrome", false),
            summary("browser_edge", false),
        ]);

        let refused = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );

        assert_eq!(refused.status, Status::Failed);
        assert_eq!(refused.effect, Effect::None);
        assert_eq!(refused.facts["reason"], json!("browser_ambiguous"));
        assert_eq!(
            refused.facts["browsers"],
            json!(["browser_chrome", "browser_edge"])
        );
        assert!(refused.repeat_safe);
        // Nothing was dispatched and nothing was bound, so naming a browser next still works.
        assert!(browser.routed().is_empty());
        assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    }

    // manual recovery maps to model directed summary and stable facts.
    {
        let (refusal, facts) = routing_refusal(
            &BrowserError::RecoveryManual {
                browsers: vec!["Chromium".into()],
            },
            || ExtensionAdvice::Plain,
        )
        .expect("manual recovery is a model-facing refusal");

        assert_eq!(refusal.reason(), "browser_startup_manual");
        assert_eq!(
            facts,
            json!({
                "browser":"Chromium",
                "browsers":["Chromium"]
            })
        );
        assert_eq!(
                refusal.summary(),
                "No browser is connected. Ask the user to open a Chromium browser window with the Ghostlight extension installed, then repeat the call."
            );
        assert!(refusal.next_steps().is_empty());

        let (plural, plural_facts) = routing_refusal(
            &BrowserError::RecoveryManual {
                browsers: vec!["Google Chrome".into(), "Microsoft Edge".into()],
            },
            || ExtensionAdvice::Plain,
        )
        .expect("plural manual recovery is model-facing");
        assert_eq!(
                plural.summary(),
                "No browser is connected. Ask the user to open a Google Chrome or Microsoft Edge browser window with the Ghostlight extension installed, then repeat the call."
            );
        assert_eq!(
            plural_facts,
            json!({
                "browsers":["Google Chrome", "Microsoft Edge"]
            })
        );
        assert_eq!(plural.reason(), "browser_startup_manual");
    }

    // a named browser opens there and a named stranger is refused.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![
            summary("browser_chrome", false),
            summary("browser_edge", true),
        ]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));

        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true,"browser":"browser_chrome"}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        assert_eq!(browser.routed(), vec!["browser_chrome"]);

        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary("browser_edge", true)]);
        let refused = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true,"browser":"browser_absent"}),
            &CancellationToken::default(),
        );
        assert_eq!(refused.status, Status::Failed);
        assert_eq!(refused.facts["reason"], json!("browser_unknown"));
        assert!(browser.routed().is_empty());
    }
}

/// ACCEPTANCE Governance 10 and 15-17: the audit holds no page, caller or browser text. A failed
/// flow's record excludes earlier reads and error payloads, exception text and unknown tool names,
/// script results and invalid input; refusal facts appear for failures only; a refused navigation
/// audits only the attempted host.
#[test]
fn the_audit_holds_no_page_caller_or_browser_text() {
    // failed flow audit excludes prior read and error payloads.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(
                7,
                "https://example.com/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT",
            ),
            committed_urls: vec![
                "https://example.com/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT".into(),
            ],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "PRIVATE_PAGE_SENTINEL".into(),
            truncated: false,
            title: "PRIVATE_TITLE_SENTINEL".into(),
            url: "https://example.com/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT".into(),
        }));
        browser.push(Err(BrowserError::EffectUnknown(
            "PRIVATE_EXCEPTION_SENTINEL".into(),
        )));
        let result = executor.execute(&workspace, "browser_flow", json!({"steps":[
                {"id":"PRIVATE_STEP_ID", "tool":"browser_read", "arguments":{}},
                {"id":"execute", "tool":"browser_execute", "arguments":{"script":"throw 'PRIVATE_SCRIPT_SENTINEL'"}}
            ]}), &CancellationToken::default());
        assert_ne!(result.status, Status::Succeeded);
        let client = serde_json::to_string(&result).unwrap();
        assert!(client.contains("PRIVATE_PAGE_SENTINEL"));
        assert!(client.contains("PRIVATE_EXCEPTION_SENTINEL"));
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(
            !encoded.contains("PRIVATE_"),
            "audit copied a flow payload: {encoded}"
        );
    }

    // audit excludes primitive exception text and unknown tool names.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Err(BrowserError::primitive("PRIVATE_EXCEPTION_SENTINEL")));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        assert!(serde_json::to_string(&result)
            .unwrap()
            .contains("PRIVATE_EXCEPTION_SENTINEL"));
        executor.execute(
            &workspace,
            "PRIVATE_TOOL_SENTINEL",
            json!({"PRIVATE_KEY":"PRIVATE_VALUE"}),
            &CancellationToken::default(),
        );
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(
            !encoded.contains("PRIVATE_"),
            "audit copied caller/browser text: {encoded}"
        );
    }

    // audit excludes script results and invalid input context.
    {
        let (executor, browser, _, workspace, audit) = fixture();
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
        let tab_handle = opened.facts["tab"].as_str().unwrap();
        browser.push(Ok(BrowserOutcome::ScriptEvaluated {
            tab: tab(7, "https://example.com/"),
            value: json!({"PRIVATE_RESULT_KEY":"PRIVATE_RESULT_VALUE"}).to_string(),
            truncated: false,
            form: crate::browser::adapter::ScriptForm::AsWritten,
            committed_urls: vec![],
        }));
        let result = executor.execute(
            &workspace,
            "browser_execute",
            json!({"script":"'PRIVATE_SCRIPT'"}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Succeeded, "{result:?}");
        assert_eq!(
            result.facts["value"]["PRIVATE_RESULT_KEY"],
            "PRIVATE_RESULT_VALUE"
        );

        for (tool, arguments) in [
            (
                "browser_fill_form",
                json!({"fields":[{"target":"PRIVATE_TARGET_HANDLE","value":"PRIVATE_TYPED_VALUE"}]}),
            ),
            (
                "browser_upload",
                json!({"target":"PRIVATE_TARGET_HANDLE","paths":["C:/PRIVATE_FILE_PATH"]}),
            ),
            (
                "browser_click",
                json!({"selector":{"name":"PRIVATE_SELECTOR","PRIVATE_KEY":"PRIVATE_VALUE"}}),
            ),
            (
                "browser_wait",
                json!({"condition":"PRIVATE_CONDITION","value":"PRIVATE_WAIT_VALUE"}),
            ),
        ] {
            let result =
                executor.execute(&workspace, tool, arguments, &CancellationToken::default());
            assert_ne!(result.status, Status::Succeeded, "{result:?}");
        }
        let records = audit.0.lock().unwrap();
        let encoded = serde_json::to_string(&*records).unwrap();
        assert!(
            !encoded.contains("PRIVATE_"),
            "audit copied input/result context: {encoded}"
        );
        assert!(!encoded.contains(tab_handle));
        assert_eq!(records[1].summary, "Executed JavaScript on example.com.");
    }

    // audit records carry refusal facts for failures and omit them for successes.
    {
        let (executor, browser, _workspaces, workspace, audit) = fixture();
        browser.connect(vec![summary("browser_chrome", true)]);

        browser.push(Err(BrowserError::primitive(
            "target is not visible for focus",
        )));
        let _ = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );

        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let _ = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            &CancellationToken::default(),
        );

        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 2);
        let failure = &records[0];
        assert_eq!(failure.status, "failed");
        let facts = failure
            .refusal_facts
            .as_ref()
            .expect("failure carries facts");
        assert_eq!(
            facts,
            &crate::language::audit::AuditRefusal::BrowserPrimitiveFailed
        );
        assert_eq!(
            failure.summary,
            "The browser could not complete this operation."
        );
        let success = &records[1];
        assert_eq!(success.status, "succeeded");
        assert!(success.refusal_facts.is_none());
    }

    // refused navigation audits only the attempted host.
    {
        let policy = TestPolicy::new();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
        document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
        fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
        let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"http://127.0.0.1/private/record-42?token=secret#detail"}),
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Blocked);
        assert!(result.summary.contains("127.0.0.1"));
        assert!(browser.calls().is_empty());

        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].observed.host.as_deref(), Some("127.0.0.1"));
        let encoded = serde_json::to_string(&records[0]).unwrap();
        assert!(!encoded.contains("record-42"));
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("secret"));
    }
}

/// ACCEPTANCE Governance 9 and ADR-0203: a refused call or landing changes nothing it may not.
/// Retired tools and inputs never dispatch or touch the person's controls; a close the policy or the
/// local setting refuses is blocked before dispatch; Ghostlight's own control page is refused by
/// name and other loopback ports are not; a tab whose landing is unknown is refused rather than
/// checked by capability alone; a credential target returns guidance without holding the session;
/// a denied redirect is compensated without replay risk, or stays visibly open when close policy or
/// the local setting refuses the compensation.
#[test]
fn a_refused_call_or_landing_changes_nothing_it_may_not() {
    // retired tools and inputs never dispatch or change human controls.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        let mut cases = vec![
            (
                "browser_sequence".to_string(),
                json!({"steps":[{"action":"click","target":"target_one"}]}),
            ),
            ("browser_workspace".to_string(), json!({"action":"list"})),
            (
                "browser_workspace".to_string(),
                json!({"action":"switch","workspace":"workspace_other"}),
            ),
            (
                "browser_flow".to_string(),
                json!({"dry_run":true,"steps":[{"tool":"browser_tabs"}]}),
            ),
            (
                "browser_flow".to_string(),
                json!({"dry_run":false,"steps":[{"tool":"browser_tabs"}]}),
            ),
        ];
        for tool in crate::language::catalog() {
            for field in ["restrict_hosts", "restrict_capabilities"] {
                let mut input = tool
                    .input_schema
                    .get("examples")
                    .and_then(|examples| examples.get(0))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                input[field] = json!(["read"]);
                cases.push((tool.name.clone(), input));
            }
        }
        for field in ["restrict_hosts", "restrict_capabilities"] {
            let mut arguments = json!({"script":"MUST_NOT_RUN"});
            arguments[field] = json!(["read"]);
            cases.push(("browser_flow".to_string(), json!({"steps":[{"tool":"browser_tabs","arguments":{"action":"list"}},{"tool":"browser_execute","arguments":arguments}]})));
        }
        for (tool, input) in cases {
            let result = executor.execute(&workspace, &tool, input, &CancellationToken::default());
            assert_eq!(result.status, Status::Failed, "{tool}: {result:?}");
            assert_eq!(result.effect, Effect::None);
            assert!(
                result
                    .next_steps
                    .iter()
                    .any(|step| step.contains("removed")),
                "{tool}: {result:?}"
            );
        }
        assert!(browser.calls().is_empty());
        assert_eq!(
            executor.governance.runtime_state(),
            RuntimeControlState::Active
        );
        assert!(audit
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|record| record.permissions.checks.is_empty()));
    }

    // tab close policy blocks before browser dispatch.
    {
        let policy = temporary_policy("tab-close");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap();
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Blocked);
        assert_eq!(closed.effect, Effect::None);
        assert_eq!(closed.facts["reason"], "tab_close_denied");
        assert_eq!(browser.calls().len(), 1);
    }

    // local preserve tabs refusal is blocked without an effect.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap();
        browser.push(Err(crate::browser::BrowserError::LocalInterlock(
            "preserved".into(),
        )));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Blocked);
        assert_eq!(closed.effect, Effect::None);
        assert!(closed.repeat_safe);
        assert_eq!(closed.facts["reason"], "browser_local_interlock");
        assert_eq!(closed.next_steps.len(), 1);
    }

    // the workbench origin is refused by name and other loopback ports are not.
    {
        let governance = GovernanceFacade::new(None);
        governance.protect_workbench_port(41259);
        let (executor, browser, _, workspace, audit) = fixture_with_governance(governance);
        for (url, origin) in [
            ("http://127.0.0.1:41259/", "http://127.0.0.1:41259"),
            (
                "http://localhost:41259/app?token=secret",
                "http://localhost:41259",
            ),
            ("http://[::1]:41259/", "http://[::1]:41259"),
        ] {
            let result = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":url}),
                &CancellationToken::default(),
            );
            assert_eq!(result.status, Status::Blocked, "{url}");
            assert_eq!(result.facts["reason"], "own_workbench");
            assert_eq!(
                result.summary,
                format!(
                    "Blocked: {origin} is Ghostlight's own control page and is never automated. Other ports on this computer are not affected."
                )
            );
            assert!(
                result
                    .next_steps
                    .iter()
                    .all(|step| !step.contains("policy")),
                "{:?}",
                result.next_steps
            );
        }
        assert!(browser.calls().is_empty());

        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].observed.host.as_deref(), Some("127.0.0.1"));
        let encoded = serde_json::to_string(&*records).unwrap();
        assert!(encoded.contains("own_workbench"));
        assert!(!encoded.contains("/app"));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains(":41259"));
        assert!(!encoded.contains("http://"));
        drop(records);

        let other = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"http://127.0.0.1:41258/"}),
            &CancellationToken::default(),
        );
        assert_ne!(other.facts["reason"], "own_workbench", "{other:?}");
        assert!(!other.summary.contains("control page"), "{other:?}");
    }

    // a tab whose landing is not yet known is refused rather than checked by capability alone.
    {
        // Acquiring a tab does not grant destination authority. Until a known landing has
        // been checked, an empty URL must fail closed rather than union host-specific grants.
        let policy = temporary_policy("unknown-landing");
        fs::write(
                &policy,
                r#"{"schema":4,"name":"work test","version":"1","grants":[{"id":"approved","hosts":{"allow":["approved.example"]},"allowed":["read"]}]}"#,
            )
            .unwrap();
        let (executor, browser, workspaces, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));

        // An ordinary, fully governed opener tab, admitted under the narrow policy.
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://approved.example/"),
            committed_urls: vec!["https://approved.example/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://approved.example","new_tab":true}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded, "{opened:?}");
        // Even an explicitly acquired tab without a known destination must fail closed.
        let handle = workspaces
            .acquire(&workspace)
            .unwrap()
            .add_tab(FAKE_BROWSER, &tab(8, ""))
            .unwrap()
            .handle;

        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab": handle.as_str()}),
            &CancellationToken::default(),
        );

        assert_eq!(
            read.status,
            Status::Blocked,
            "a tab with no known landing must be refused, not checked by capability alone: {read:?}"
        );
        assert!(
            browser
                .calls()
                .iter()
                .all(|call| !matches!(call, BrowserCommand::Observe(PhysicalWait { .. }))),
            "the browser must never be asked to read a tab whose destination was never checked"
        );
    }

    // credential target returns guidance without holding the session.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        let credential = ObservedTarget {
            locator: "password-1".into(),
            role: "textbox".into(),
            name: "Password".into(),
            state: vec![],
            credential_class: true,
            ..Default::default()
        };
        browser.push(Ok(BrowserOutcome::targets(7, vec![credential.clone()])));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            rebound: false,
            tab_id: 7,
            targets: vec![credential],
        }));
        let result = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({"tab":tab_handle,"fields":[{"target":target,"value":"not-sent"}]}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.facts["reason"], "credential_authorization_required");
        assert_eq!(result.facts["user_authorization_required"], true);
        assert_eq!(result.facts["values_sent"], false);
        assert!(
            browser.control_states().is_empty(),
            "an authorization publishes no control state"
        );
        assert!(!browser
            .calls()
            .iter()
            .any(|call| matches!(call, BrowserCommand::Fill(PhysicalFill { .. }))));
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "ordinary page".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/".into(),
        }));
        let next = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(
            next.status,
            Status::Succeeded,
            "credential guidance does not block later work: {next:?}"
        );
    }

    // denied redirect is compensated without replay risk.
    {
        let policy = TestPolicy::new();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
        document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
        fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "http://127.0.0.1/private"),
            committed_urls: vec![
                "https://example.com/".into(),
                "http://127.0.0.1/private".into(),
            ],
        }));
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::None);
        assert_eq!(result.facts["compensated"], true);
        assert!(result.repeat_safe);
    }

    // denied redirect remains visibly open when close policy refuses compensation.
    {
        let policy = temporary_policy("retained-denied-landing");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&policy).unwrap()).unwrap();
        document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
        fs::write(&policy, serde_json::to_vec(&document).unwrap()).unwrap();
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "http://127.0.0.1/private"),
            committed_urls: vec![
                "https://example.com/".into(),
                "http://127.0.0.1/private".into(),
            ],
        }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::Applied);
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["compensated"], false);
        assert_eq!(result.facts["retained"], true);
        assert_eq!(browser.calls().len(), 1);
    }

    // denied redirect remains visibly open when local preservation refuses compensation.
    {
        let policy = TestPolicy::new();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
        document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
        fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "http://127.0.0.1/private"),
            committed_urls: vec![
                "https://example.com/".into(),
                "http://127.0.0.1/private".into(),
            ],
        }));
        browser.push(Err(crate::browser::BrowserError::LocalInterlock(
            "preserved".into(),
        )));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::Applied);
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["compensated"], false);
        assert_eq!(result.facts["retained"], true);
        assert_eq!(browser.calls().len(), 2);
    }
}

/// ACCEPTANCE Executor 5: a flow follows on_error and the person's directives. It stops after a
/// refused child without reverting prior effects; a step not started carries its policy sequence;
/// on_error governs runtime argument failures; continue reports a denial after a later success and
/// honors human attention and invocation limits; the result budget keeps step metadata and
/// reference values; a short flow uses its default tab among several; a read-only composition is
/// repeat-safe only when fully successful. (It absorbs composition.rs's and reading.rs's inline
/// tests of mixed progress, unknown causes, between-step limits and coverage.)
#[test]
fn a_flow_follows_on_error_and_human_directives() {
    // flow stops after a refused child without reverting prior effects.
    {
        let policy = TestPolicy::new();
        policy.set(json!(["read"]));
        let (executor, browser, _workspaces, workspace, _) =
            fixture_with_governance(policy.facade());
        for id in [7, 8] {
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(id, "https://example.com/"),
                committed_urls: vec!["https://example.com/".into()],
            }));
        }

        let result = executor.execute(
                &workspace,
                "browser_flow",
                json!({

                    "steps":[
                        {"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                        {"id":"denied","tool":"browser_execute","arguments":{"script":"42"}},
                        {"id":"after","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
                    ]
                }),
                &CancellationToken::default(),
            );

        let calls = browser.calls();
        assert_eq!(calls.len(), 1, "neither refused nor later work dispatches");
        assert!(matches!(calls[0], BrowserCommand::OpenTab { .. }));
        assert_eq!(result.facts["stopped"], true);
        assert_eq!(result.facts["completed"], 1);
        assert_eq!(result.facts["total"], 3);
        let steps = result.facts["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(steps[2]["status"], "not_run");
        assert_eq!(steps[0]["result"]["status"], "succeeded");
        assert_eq!(steps[0]["result"]["effect"], "applied");
        assert_eq!(steps[1]["result"]["status"], "blocked");
        assert_eq!(steps[1]["result"]["effect"], "none");
        assert_eq!(result.effect, Effect::Partial);
        assert!(!result.repeat_safe);
    }

    // a not started step carries policy seq under managed policy.
    {
        use crate::governance::managed::ManagedPaths;
        use crate::work::result::UnexecutedStatus;

        let root = Scratch::named("not-started-managed");
        let governance = GovernanceFacade::with_managed_paths(
            None,
            ManagedPaths::signed(
                &root,
                r#"{"schema":4,"name":"org","version":"1","grants":[{"id":"org","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
            ),
        );
        let sequence = governance.snapshot().managed_sequence();
        assert!(sequence.is_some(), "the signed bundle is in force");
        let (executor, browser, _workspaces, workspace, audit) =
            fixture_with_governance(governance);

        let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({"steps":[{"id":"bad","tool":"browser_read","arguments":{"max_chars":"lots"}}]}),
            &CancellationToken::default(),
        );

        assert!(browser.calls().is_empty(), "{}", result.summary);
        assert_eq!(result.facts["progress"]["counts"]["not_started"], 1);
        let records = audit.0.lock().unwrap();
        assert_eq!(
            records.len(),
            2,
            "one line for the step and one for the flow"
        );
        let step = &records[0];
        assert!(step.step.is_some_and(|step| step.preparation_failed));
        assert_eq!(step.status, UnexecutedStatus::NotStarted.stored_name());
        assert_eq!(step.effect, Effect::None.stored_name());
        assert_eq!(step.reason, crate::governance::ReasonCode::InvalidRequest);
        assert_eq!(step.policy_seq, sequence);
        assert_eq!(records[1].tool, "browser_flow");
        assert_eq!(records[1].policy_seq, sequence);
    }

    // flow error policy controls runtime argument failures.
    {
        // A tab handle resolves but is not a numeric read limit. A missing field fails
        // reference resolution instead. Both failures must honor the same error policy.
        for pointer in ["/facts/tab", "/facts/missing"] {
            for on_error in ["stop", "continue"] {
                let (executor, browser, _workspaces, workspace, _) = fixture();
                for id in [7, 8] {
                    browser.push(Ok(BrowserOutcome::TabOpened {
                        tab: tab(id, "https://example.com/"),
                        committed_urls: vec!["https://example.com/".into()],
                    }));
                }

                let result = executor.execute(
                        &workspace,
                        "browser_flow",
                        json!({
                            "on_error":on_error,
                            "steps":[
                                {"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                                {"id":"invalid","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"open","pointer":pointer}}}},
                                {"id":"after","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
                            ]
                        }),
                        &CancellationToken::default(),
                    );

                let should_stop = on_error == "stop";
                let calls = browser.calls();
                assert_eq!(
                    calls.len(),
                    if should_stop { 1 } else { 2 },
                    "{on_error} after {pointer}: {}",
                    result.summary
                );
                assert!(calls
                    .iter()
                    .all(|call| matches!(call, BrowserCommand::OpenTab { .. })));
                assert_eq!(result.facts["stopped"], should_stop);
                let steps = result.facts["steps"].as_array().unwrap();
                assert_eq!(steps.len(), 3);
                assert_eq!(result.status, Status::Failed);
                assert_eq!(result.facts["completed"], if should_stop { 1 } else { 2 });
                assert_eq!(result.effect, Effect::Partial);
                assert_eq!(result.facts["progress"]["counts"]["not_started"], 1);
                assert_eq!(steps[0]["result"]["effect"], "applied");
                assert!(steps[1]["error"].is_string());
                assert!(!result.repeat_safe);
                if should_stop {
                    assert_eq!(result.effect, Effect::Partial);
                } else {
                    assert_eq!(steps[2]["id"], "after");
                    assert_eq!(steps[2]["result"]["status"], "succeeded");
                }
            }
        }
    }

    // continue reports policy denial after later independent success.
    {
        let policy = TestPolicy::new();
        policy.set(json!(["read"]));
        let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
        for id in [7, 8] {
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(id, "https://example.com/"),
                committed_urls: vec!["https://example.com/".into()],
            }));
        }
        let result = executor.execute(&workspace, "browser_flow", json!({
                "on_error":"continue",  "steps":[
                    {"id":"first","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                    {"id":"denied","tool":"browser_execute","arguments":{"script":"PRIVATE_SCRIPT"}},
                    {"id":"last","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
                ]
            }), &CancellationToken::default());
        assert_eq!(browser.calls().len(), 2);
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::Partial);
        assert_eq!(result.summary, "Completed 2 of 3 steps. 1 blocked.");
        assert_eq!(result.facts["completed"], 2);
        assert_eq!(result.facts["stopped"], false);
        assert_eq!(result.facts["progress"]["counts"]["blocked"], 1);
        assert!(!result.repeat_safe);
        let records = audit.0.lock().unwrap();
        let record = records.last().unwrap();
        assert!(
            !record.allowed,
            "later admission cannot erase the child's policy denial"
        );
        assert_eq!(
            serde_json::to_value(record.composition).unwrap(),
            result.facts["progress"]
        );
        assert!(!serde_json::to_string(record).unwrap().contains("PRIVATE_"));
    }

    // flow result budget preserves step metadata and reference values.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        browser.push(Ok(BrowserOutcome::ScriptEvaluated {
            tab: tab(7, "https://example.com/"),
            value: json!({"PRIVATE_PAYLOAD":"x".repeat(110_000),"limit":500}).to_string(),
            truncated: false,
            form: crate::browser::adapter::ScriptForm::AsWritten,
            committed_urls: vec![],
        }));
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "visible".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/".into(),
        }));
        let result = executor.execute(&workspace, "browser_flow", json!({"steps":[
                {"id":"large","tool":"browser_execute","arguments":{"script":"({limit:500})"}},
                {"id":"read","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"large","pointer":"/facts/value/limit"}}}}
            ]}), &CancellationToken::default());
        assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
        assert_eq!(result.facts["completed"], 2);
        for row in result.facts["steps"].as_array().unwrap() {
            assert_eq!(row["omitted"], true);
            assert_eq!(row["status"], "succeeded");
            assert!(row["effect"].is_string());
            assert!(row["repeat_safe"].is_boolean());
            assert!(row.get("result").is_none());
        }
        assert_eq!(result.facts["steps"][0]["effect"], "applied");
        assert_eq!(result.facts["steps"][1]["effect"], "none");
        assert!(!serde_json::to_string(&*audit.0.lock().unwrap())
            .unwrap()
            .contains("PRIVATE_"));
    }

    // short flow uses its default tab with multiple controlled pages.
    {
        let (executor, browser, _, workspace, _) = fixture();
        let mut handles = Vec::new();
        for id in [7, 8] {
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(id, "https://example.com/"),
                committed_urls: vec![],
            }));
            let opened = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com/","new_tab":true}),
                &CancellationToken::default(),
            );
            assert_eq!(opened.status, Status::Succeeded);
            handles.push(opened.facts["tab"].clone());
        }
        for id in [7, 8, 7] {
            browser.push(Ok(BrowserOutcome::Text {
                tab_id: id,
                text: "read".into(),
                title: "Example".into(),
                url: "https://example.com/".into(),
                truncated: false,
            }));
        }
        let before = browser.calls().len();
        let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({"tab":handles[0],"steps":[
                {"tool":"browser_read"},
                {"tool":"browser_read","arguments":{"tab":handles[1]}},
                {"tool":"browser_read"}
            ]}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Succeeded, "{result:?}");
        let actual: Vec<_> = browser.calls()[before..]
            .iter()
            .filter_map(|command| {
                if let BrowserCommand::ReadDocument { tab_id, .. } = command.primitive() {
                    Some(*tab_id)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(actual, [7, 8, 7]);
        assert_eq!(result.facts["steps"][2]["id"], "step_3");
    }

    // read only compositions are repeat safe only when fully successful.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        for fail_last in [false, true] {
            for index in 0..2 {
                browser.push(if fail_last && index == 1 {
                    Err(BrowserError::primitive("PRIVATE_ERROR"))
                } else {
                    Ok(BrowserOutcome::Text {
                        tab_id: 7,
                        text: "read".into(),
                        truncated: false,
                        title: "Example".into(),
                        url: "https://example.com/".into(),
                    })
                });
            }
            let result = executor.execute(
                &workspace,
                "browser_flow",
                json!({"steps":[
                    {"id":"one","tool":"browser_read","arguments":{}},
                    {"id":"two","tool":"browser_read","arguments":{}}
                ]}),
                &CancellationToken::default(),
            );
            assert_eq!(
                result.status,
                if fail_last {
                    Status::Failed
                } else {
                    Status::Succeeded
                }
            );
            assert_eq!(result.effect, Effect::None);
            assert_eq!(result.repeat_safe, !fail_last);
            assert_eq!(result.facts["completed"], if fail_last { 1 } else { 2 });
        }
    }

    // continue honors human attention and invocation limits.
    {
        for (error, cause, status, effect) in [
            (
                BrowserError::LocalInterlock("PRIVATE_INTERLOCK".into()),
                "attention_required",
                Status::Blocked,
                Effect::None,
            ),
            (
                BrowserError::CancelledBeforeDispatch,
                "cancelled",
                Status::Cancelled,
                Effect::None,
            ),
            (
                BrowserError::CancelledAfterDispatch,
                "cancelled",
                Status::Unknown,
                Effect::Unknown,
            ),
            (
                BrowserError::DeadlineBeforeDispatch,
                "deadline",
                Status::Failed,
                Effect::None,
            ),
            (
                BrowserError::DeadlineAfterDispatch,
                "deadline",
                Status::Unknown,
                Effect::Unknown,
            ),
        ] {
            let (executor, browser, _, workspace, _) = fixture();
            browser.push(Err(error));
            let result = executor.execute(&workspace, "browser_flow", json!({"on_error":"continue","steps":[
                    {"id":"first","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                    {"id":"later","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
                ]}), &CancellationToken::default());
            assert_eq!(browser.calls().len(), 1, "{cause}: {}", result.summary);
            assert_eq!(result.status, status);
            assert_eq!(result.effect, effect);
            assert_eq!(result.facts["progress"]["issue"]["cause"], cause);
            assert_eq!(result.facts["steps"][1]["status"], "not_run");
        }
    }
}

/// ACCEPTANCE Executor 1 and ADR-0202: direct calls and flow steps cross one executor and one
/// receipt seam. Direct and composed reads keep equivalent safe receipts, child receipts share
/// authority without holding the session, direct and flow actions use the same physical path, the
/// seam observes landing facts but no outcome measurement or page detail, recording actions cross
/// only the extension-owned receipt seam, and repeated policy denials stay independent requests.
#[test]
fn one_executor_and_one_receipt_serve_every_call_and_step() {
    // direct and composed reads keep equivalent safe receipts.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        for composed in [false, true] {
            browser.push(Ok(BrowserOutcome::Text {
                tab_id: 7,
                text: "PRIVATE_PAGE_CONTENT".into(),
                truncated: false,
                title: "PRIVATE_TITLE".into(),
                url: "https://example.com/PRIVATE_PATH".into(),
            }));
            let (tool, arguments) = if composed {
                (
                    "browser_flow",
                    json!({"steps":[{"id":"PRIVATE_LABEL","tool":"browser_read","arguments":{}}]}),
                )
            } else {
                ("browser_read", json!({}))
            };
            let result =
                executor.execute(&workspace, tool, arguments, &CancellationToken::default());
            assert_eq!(result.status, Status::Succeeded);
            assert!(serde_json::to_string(&result)
                .unwrap()
                .contains("PRIVATE_PAGE_CONTENT"));
        }
        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 4);
        assert!(records[1].step.is_none());
        assert_eq!(records[2].step.unwrap().position, 1);
        let safe = |record: &AuditRecord| {
            json!({
                "tool":record.tool,"requirements":record.capabilities,"status":record.status,
                "effect":record.effect,"summary":record.summary,"observed":record.observed,
                "permissions":record.permissions,"reason":record.reason
            })
        };
        assert_eq!(safe(&records[1]), safe(&records[2]));
        assert!(!serde_json::to_string(&*records)
            .unwrap()
            .contains("PRIVATE_"));
    }

    // child receipts share authority without holding the session.
    {
        let path = temporary_policy("h4-denials");
        fs::write(
            &path,
            r#"{"schema":4,"name":"deny","version":"1","grants":[]}"#,
        )
        .unwrap();
        let governance = GovernanceFacade::new(Some(path.to_path_buf()));
        let (executor, browser, _, workspace, audit) = fixture_with_governance(governance.clone());
        let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({"on_error":"continue","steps":[
                {"id":"PRIVATE_LABEL_ONE","tool":"browser_tabs","arguments":{"action":"list"}},
                {"id":"PRIVATE_LABEL_TWO","tool":"browser_tabs","arguments":{"action":"list"}}
            ]}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert!(browser.calls().is_empty());
        assert_eq!(
            governance.runtime_state(),
            RuntimeControlState::Active,
            "denied children leave human controls unchanged"
        );
        {
            let records = audit.0.lock().unwrap();
            assert_eq!(records.len(), 3);
            assert_eq!(records[0].step.unwrap().position, 1);
            assert_eq!(records[1].step.unwrap().position, 2);
            assert!(records[2].step.is_none());
            assert!(records
                .iter()
                .all(|record| record.authority == records[0].authority));
            assert!(records[..2]
                .iter()
                .all(|record| record.capabilities == crate::governance::CapabilitySet::READ));
            assert!(!serde_json::to_string(&*records)
                .unwrap()
                .contains("PRIVATE_"));
        }
        executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(
            governance.runtime_state(),
            RuntimeControlState::Active,
            "a later direct denial leaves human controls unchanged"
        );
    }

    // direct and flow actions use the same physical executor path.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![ObservedTarget {
                locator: "button-1".into(),
                role: "button".into(),
                name: "Go".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            }],
        )));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();

        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: None,
        }));
        let direct = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(direct.status, Status::Succeeded);

        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: None,
        }));
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 5,
            readiness: BrowserReadiness::Complete,
        }));
        let sequence = executor.execute(&workspace, "browser_flow", json!({"tab":tab_handle,"steps":[{"tool":"browser_click","arguments":{"target":target}},{"tool":"browser_wait","arguments":{"condition":"load_ready","visual_settle":false}}]}), &CancellationToken::default());
        assert_eq!(sequence.status, Status::Succeeded);
        let calls = browser.calls();
        assert_eq!(
            calls
                .iter()
                .filter(|call| matches!(call, BrowserCommand::Activate(PhysicalClick { .. })))
                .count(),
            2
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| matches!(call, BrowserCommand::Observe(PhysicalWait { .. })))
                .count(),
            1
        );
        for unknown in [false, true] {
            for named in [false, true] {
                let tool = "browser_flow";
                browser.push(Ok(BrowserOutcome::Activated {
                    tab: tab(7, "https://example.com/"),
                    committed_urls: vec![],
                    subject: None,
                }));
                browser.push(Err(if unknown {
                    BrowserError::DisconnectedAfterDispatch
                } else {
                    BrowserError::primitive("PRIVATE_WAIT_FAILURE")
                }));
                let before = browser.calls().len();
                let mut arguments = json!({"steps":[
                    {"id":"click","tool":"browser_click","arguments":{"tab":tab_handle,"target":target}},
                    {"id":"wait","tool":"browser_wait","arguments":{"tab":tab_handle,"condition":"load_ready"}},
                    {"id":"later","tool":"browser_click","arguments":{"tab":tab_handle,"target":target}}
                ]});
                if !named {
                    for step in arguments["steps"].as_array_mut().unwrap() {
                        step.as_object_mut().unwrap().remove("id");
                    }
                }
                let result =
                    executor.execute(&workspace, tool, arguments, &CancellationToken::default());
                assert_eq!(browser.calls().len() - before, 2, "{tool}");
                assert_eq!(
                    result.status,
                    if unknown {
                        Status::Unknown
                    } else {
                        Status::Failed
                    }
                );
                assert_eq!(
                    result.effect,
                    if unknown {
                        Effect::Unknown
                    } else {
                        Effect::Partial
                    }
                );
                assert_eq!(result.facts["progress"]["counts"]["succeeded"], 1);
                assert_eq!(result.facts["progress"]["effects"]["applied"], 1);
                assert_eq!(result.facts["steps"][2]["status"], "not_run");
                assert!(!result.repeat_safe);
                assert!(result.summary.contains(if unknown {
                    "Connection lost during step 2."
                } else {
                    "Step 2 failed."
                }));
            }
        }
    }

    // browser seam observes landing facts but not outcome measurements.
    {
        let tabs = observed_from(&BrowserOutcome::Tabs {
            tabs: vec![tab(7, "https://example.com/")],
        });
        assert_eq!(tabs, Observed::default());

        let text = observed_from(&BrowserOutcome::Text {
            tab_id: 7,
            text: "three private words".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/private?id=3".into(),
        });
        assert_eq!(text.host.as_deref(), Some("example.com"));
        assert_eq!(text.count, None);

        let wait = observed_from(&BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 1_830,
            readiness: BrowserReadiness::Complete,
        });
        assert_eq!(wait.readiness.as_deref(), Some("complete"));
        assert_eq!(wait.count, None);
    }

    // outcome language and the seam observe without carrying page detail.
    {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://Example.com/patients/48219?ssn=1#note"),
            committed_urls: vec!["https://example.com/patients/48219?ssn=1#note".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/patients/48219?ssn=1#note"}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "Patient 48219 has an appointment".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/patients/48219?ssn=1#note".into(),
        }));
        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":handle}),
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Succeeded);
        assert_eq!(read.summary, "Read 5 words from example.com.");

        let records = audit.0.lock().unwrap();
        let landing = &records[0].observed;
        assert_eq!(landing.host.as_deref(), Some("example.com"));
        assert_eq!(landing.readiness.as_deref(), Some("complete"));
        let text = &records[1].observed;
        assert_eq!(text.host.as_deref(), Some("example.com"));
        assert_eq!(text.count, Some(5));

        // The model-facing facts legitimately carry the URL and the text. The audit carries the
        // same action, and none of it.
        assert!(read.facts["url"].as_str().unwrap().contains("48219"));
        let encoded = serde_json::to_string(&*records).unwrap();
        for detail in ["patients", "48219", "ssn", "note", "appointment"] {
            assert!(
                !encoded.contains(detail),
                "the audit leaked {detail} from a page"
            );
        }
    }

    // record actions cross only the extension owned request receipt seam.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::RecordingStarted {
            recording_id: "recording_1".into(),
            summary: recording_summary(RecordingState::Recording, "https://example.com/"),
            existing: false,
        }));
        assert_eq!(
            executor
                .execute(
                    &workspace,
                    "browser_record",
                    json!({"action":"start","tab":handle}),
                    &CancellationToken::default(),
                )
                .status,
            Status::Succeeded
        );

        browser.push(Ok(BrowserOutcome::RecordingStatus {
            summary: recording_summary(RecordingState::Recording, "https://example.com/"),
        }));
        executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"status"}),
            &CancellationToken::default(),
        );

        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: true,
        }));
        executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"stop"}),
            &CancellationToken::default(),
        );

        browser.push(Ok(BrowserOutcome::RecordingDiscarded { released_bytes: 1 }));
        executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"discard"}),
            &CancellationToken::default(),
        );

        let calls = browser.calls();
        assert!(matches!(
            calls[1],
            BrowserCommand::StartRecording { tab_id: 7 }
        ));
        assert!(matches!(calls[2], BrowserCommand::StatusRecording { .. }));
        assert!(matches!(calls[3], BrowserCommand::StopRecording { .. }));
        assert!(matches!(calls[4], BrowserCommand::DiscardRecording { .. }));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, BrowserCommand::ExportRecording { .. })));
    }

    // repeated policy denials remain independent requests.
    {
        let policy = TestPolicy::new();
        policy.set(json!([]));
        let governance = policy.facade();
        let (executor, browser, _, workspace, _) = fixture_with_governance(governance.clone());
        for _ in 0..6 {
            let denied = executor.execute(
                &workspace,
                "browser_tabs",
                json!({"action":"list"}),
                &CancellationToken::default(),
            );
            assert_eq!(denied.status, Status::Blocked);
            assert_eq!(denied.effect, Effect::None);
        }
        assert!(browser.calls().is_empty());
        assert_eq!(governance.runtime_state(), RuntimeControlState::Active);
        policy.set(json!(["read"]));
        let allowed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            &CancellationToken::default(),
        );
        assert_eq!(allowed.status, Status::Succeeded);
        // The session holds no tab, so its list is answered without a browser.
        assert!(browser.calls().is_empty());
    }
}

/// ACCEPTANCE Browser journeys 17-18 (79d460c9, C16, be54ad44): a session holds one recording and
/// never names it, starts a new one after saving, and keeps it saveable across a relay restart. A
/// save asks for one finished replay, a download never returns its bytes, and a client save
/// authorizes its sources before bytes cross; a pause refuses status, stop and discard; a stopped
/// recording's status says it stopped and a running one's that it runs.
#[test]
fn a_session_holds_one_recording_and_saves_it_as_authorized() {
    // a session holds one recording and never names it.
    {
        let (executor, browser, _, workspace, tab_handle, _) = workspace_with_a_target();

        // A stopped recording no save delivered holds the slot: the start asks its status and stops.
        holding_recording(&executor, &workspace);
        browser.push(Ok(BrowserOutcome::RecordingStatus {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        }));
        let calls_before = browser.calls().len();
        let held = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"start","tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(held.status, Status::Failed);
        assert_eq!(held.summary, "This session already has a recording.");
        assert_eq!(held.facts["reason"], "held");
        assert!(
            browser.calls()[calls_before..]
                .iter()
                .all(|call| !matches!(call, BrowserCommand::StartRecording { .. })),
            "a held recording starts nothing"
        );
        assert_eq!(
            held.next_steps,
            vec!["Call browser_record with action save or discard to finish the current recording, then start again."]
        );

        browser.push(Ok(BrowserOutcome::RecordingNotFound));
        let missing = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"status"}),
            &CancellationToken::default(),
        );
        assert_eq!(missing.summary, "This session has no recording.");
        assert_eq!(missing.facts["reason"], "not_found");

        holding_recording(&executor, &workspace);
        browser.push(Ok(BrowserOutcome::RecordingStatus {
            summary: recording_summary(RecordingState::Recording, "https://example.com/"),
        }));
        let status = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"status"}),
            &CancellationToken::default(),
        );
        assert_eq!(status.status, Status::Succeeded);
        assert!(status.facts.get("recording").is_none());

        let named = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"stop","recording":"recording_one"}),
            &CancellationToken::default(),
        );
        assert_eq!(named.status, Status::Failed);
        assert!(
            named
                .facts
                .to_string()
                .contains("unknown field `recording`"),
            "{}",
            named.facts
        );
    }

    // a session starts a new recording after saving one.
    {
        let (executor, browser, _, workspace, tab_handle, _) = workspace_with_a_target();
        let url = "https://example.com/";
        let started = |executor: &ApplicationExecutor| {
            browser.push(Ok(BrowserOutcome::RecordingStarted {
                recording_id: "recording_1".into(),
                summary: recording_summary(RecordingState::Recording, url),
                existing: false,
            }));
            executor.execute(
                &workspace,
                "browser_record",
                json!({"action":"start","tab":tab_handle}),
                &CancellationToken::default(),
            )
        };

        assert_eq!(started(&executor).status, Status::Succeeded);

        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, url),
            changed: true,
        }));
        browser.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, url),
            encoded: EncodedRecording {
                frame_count: 3,
                captured_frame_count: 3,
                duration_ms: 900,
                width: 640,
                height: 400,
                byte_count: 2_048,
            },
            delivery: RecordingDelivery::Downloaded,
        }));
        let saved = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","download":true}),
            &CancellationToken::default(),
        );
        assert_eq!(saved.status, Status::Succeeded);

        // The saved recording is discarded so the next start can replace it.
        browser.push(Ok(BrowserOutcome::RecordingStatus {
            summary: recording_summary(RecordingState::Frozen, url),
        }));
        browser.push(Ok(BrowserOutcome::RecordingDiscarded {
            released_bytes: 10,
        }));
        let again = started(&executor);
        assert_eq!(again.status, Status::Succeeded);
        assert_eq!(again.effect, Effect::Applied);
        assert!(browser.calls().iter().any(|call| matches!(
            call,
            BrowserCommand::DiscardRecording { recording_id } if recording_id == "recording_1"
        )));
    }

    // a relay restart keeps the workspaces recording saveable.
    {
        let (mut executor, browser, _, workspace, tab_handle, _) = workspace_with_a_target();
        let url = "https://example.com/";
        browser.push(Ok(BrowserOutcome::RecordingStarted {
            recording_id: "recording_7".into(),
            summary: recording_summary(RecordingState::Recording, url),
            existing: false,
        }));
        let started = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"start","tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(started.status, Status::Succeeded);

        let reconnected = Arc::new(FakeBrowser::default());
        reconnected.connect(vec![summary(FAKE_BROWSER, true)]);
        executor.browser = reconnected.clone();
        reconnected.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, url),
            changed: true,
        }));
        reconnected.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, url),
            encoded: EncodedRecording {
                frame_count: 3,
                captured_frame_count: 3,
                duration_ms: 900,
                width: 640,
                height: 400,
                byte_count: 2_048,
            },
            delivery: RecordingDelivery::Downloaded,
        }));
        let saved = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","download":true}),
            &CancellationToken::default(),
        );
        assert_eq!(saved.status, Status::Succeeded, "{saved:?}");
        assert!(reconnected.calls().iter().any(|call| matches!(
            call,
            BrowserCommand::ExportRecording { recording_id, .. } if recording_id == "recording_7"
        )));
    }

    // a save asks the browser for one finished replay.
    {
        let (executor, browser, _, workspace, _) = fixture();
        holding_recording(&executor, &workspace);
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: true,
        }));
        browser.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            encoded: EncodedRecording {
                frame_count: 17,
                captured_frame_count: 65,
                duration_ms: 30_400,
                width: 1_280,
                height: 800,
                byte_count: 3_804_453,
            },
            delivery: RecordingDelivery::Returned {
                mime_type: "image/gif".into(),
                data: "R0lGODlh".into(),
            },
        }));

        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save"}),
            &CancellationToken::default(),
        );

        // Stop, then export. Nothing in between: the frames never come here to be encoded.
        let calls = browser.calls();
        assert!(matches!(calls[0], BrowserCommand::StopRecording { .. }));
        assert!(matches!(
            calls[1],
            BrowserCommand::ExportRecording {
                destination: RecordingDestination::Client,
                max_output_bytes: RECORDING_TRANSFER_MAX_BYTES,
                ..
            }
        ));
        assert_eq!(calls.len(), 2);
        // The sentence is what a person would say about a replay. The mechanism it was made from
        // is real, and belongs in the facts.
        assert_eq!(
            result.summary,
            "Saved a replay of 30 seconds of page changes."
        );
        assert_eq!(result.facts["frame_count"], json!(17));
        assert_eq!(result.facts["captured_frame_count"], json!(65));
        assert_eq!(result.facts["gif_bytes"], json!(3_804_453));
        assert_eq!(result.facts["delivery"], json!("returned_to_client"));
    }

    // a download save never returns the replay bytes.
    {
        let (executor, browser, _, workspace, _) = fixture();
        holding_recording(&executor, &workspace);
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: true,
        }));
        browser.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            encoded: EncodedRecording {
                frame_count: 40,
                captured_frame_count: 40,
                duration_ms: 1_500,
                width: 1_280,
                height: 800,
                byte_count: 9_000_000,
            },
            delivery: RecordingDelivery::Downloaded,
        }));

        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","download":true}),
            &CancellationToken::default(),
        );

        assert!(matches!(
            browser.calls()[1],
            BrowserCommand::ExportRecording {
                destination: RecordingDestination::Download { .. },
                // A replay that stays in the browser is not bounded by what can cross out of it.
                max_output_bytes: RECORDING_LOCAL_MAX_BYTES,
                ..
            }
        ));
        assert_eq!(
            result.summary,
            "Downloaded a replay of 2 seconds of page changes."
        );
        assert!(
            result.content.is_empty(),
            "a browser-local save must return no bytes: {:?}",
            result.content
        );
    }

    // client save authorizes source before recording bytes cross.
    {
        let policy = TestPolicy::new();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
        document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
        fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        holding_recording(&executor, &workspace);
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "http://127.0.0.1/private"),
            changed: true,
        }));

        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save"}),
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Blocked);
        assert!(matches!(
            browser.calls().as_slice(),
            [BrowserCommand::StopRecording { .. }]
        ));
    }

    // a paused runtime refuses recording status stop and discard.
    {
        // Every operation in this executor -- even ones needing no capability at all, like
        // activating a tab -- crosses the runtime gate before it can reach the browser. Recording
        // status, stop and discard cross it like every other operation, so a pause holds them too.
        let governance = GovernanceFacade::new(None);
        assert_eq!(
            governance.apply_runtime_intent(RuntimeControlIntent::Hold),
            RuntimeControlState::Held
        );
        let (executor, browser, _, workspace, _) = fixture_with_governance(governance);

        for action in ["status", "stop", "discard"] {
            let result = executor.execute(
                &workspace,
                "browser_record",
                json!({"action":action}),
                &CancellationToken::default(),
            );
            assert_ne!(
                result.status,
                Status::Succeeded,
                "recording {action} must not succeed while paused: {result:?}"
            );
        }
        assert!(
            browser.calls().is_empty(),
            "a paused runtime must never reach the browser at all: {:?}",
            browser.calls()
        );
    }

    // a stopped recordings status says it stopped and a running one that it runs.
    {
        let (executor, browser, _, workspace, _) = fixture();
        holding_recording(&executor, &workspace);
        for (state, active) in [
            (RecordingState::Frozen, false),
            (RecordingState::Recording, true),
        ] {
            let summary = recording_summary(state, "https://example.com/");
            let expected = Outcome::RecordingObserved {
                frames: summary.frame_count,
                duration_ms: summary.duration_ms,
                active,
            };
            browser.push(Ok(BrowserOutcome::RecordingStatus { summary }));
            let status = executor.execute(
                &workspace,
                "browser_record",
                json!({"action":"status"}),
                &CancellationToken::default(),
            );
            assert_eq!(status.status, Status::Succeeded, "{status:?}");
            assert_eq!(status.summary, expected.summary(), "{state:?}");
        }
    }
}

/// A session holding one open tab on example.com, and that tab's handle.
fn one_open_tab() -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    crate::workspace::WorkspaceId,
    String,
) {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
    (executor, browser, workspace, tab_handle)
}

/// ACCEPTANCE Executor 8 (8f2241c0): screenshot coordinates and regions resolve once, chain and
/// expire; a moved view is a stale view with a screenshot step; a point outside the image gives the
/// image's size.
#[test]
fn a_view_handle_resolves_once_and_expires() {
    // screenshot coordinates and regions resolve once chain and expire.
    {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        let viewport = ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 10.0,
            page_y: 20.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 10.0,
            visual_page_y: 20.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 0.5,
        };
        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "image".into(),
            width: 400,
            height: 300,
            viewport,
        }));
        let screenshot = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert!(screenshot.facts.get("data").is_none());
        assert!(matches!(
            screenshot.content.as_slice(),
            [ServiceContent::Image { mime_type, data }]
                if mime_type == "image/jpeg" && data == "image"
        ));
        let view = screenshot.facts["view"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: None,
        }));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"view":view,"x":100,"y":50}),
            &CancellationToken::default(),
        );
        assert_eq!(clicked.status, Status::Succeeded);
        assert!(browser.calls().iter().any(|call| matches!(
            call,
            BrowserCommand::ActivatePoint(PhysicalPointClick { at: PhysicalViewPoint { point, expected_viewport, .. }, .. })
                if point.x == 210.0 && point.y == 120.0 && *expected_viewport == viewport
        )));

        let first_region_viewport = ViewportGeometry {
            scope: CaptureScope::Region,
            page_x: 210.0,
            page_y: 120.0,
            css_width: 400.0,
            css_height: 200.0,
            output_scale: 4.0,
            ..viewport
        };
        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "magnified".into(),
            width: 1600,
            height: 800,
            viewport: first_region_viewport,
        }));
        let first_region = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"tab":tab_handle,"view":view,"x":100,"y":50,"width":200,"height":100}),
            &CancellationToken::default(),
        );
        assert_eq!(
            first_region.summary,
            "Captured the magnified region at 1600x800."
        );
        let first_region_view = first_region.facts["view"].as_str().unwrap().to_owned();
        assert!(browser.calls().iter().any(|call| matches!(
            call,
            BrowserCommand::ScreenshotRegion { region, expected_viewport, .. }
                if region.x == 210.0
                    && region.y == 120.0
                    && region.width == 400.0
                    && region.height == 200.0
                    && *expected_viewport == viewport
        )));

        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "magnified-again".into(),
            width: 1600,
            height: 800,
            viewport: ViewportGeometry {
                scope: CaptureScope::Region,
                page_x: 310.0,
                page_y: 170.0,
                css_width: 100.0,
                css_height: 50.0,
                output_scale: 16.0,
                ..viewport
            },
        }));
        let second_region = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"view":first_region_view,"x":400,"y":200,"width":400,"height":200}),
            &CancellationToken::default(),
        );
        assert_eq!(second_region.status, Status::Succeeded);
        let current_view = second_region.facts["view"].as_str().unwrap().to_owned();
        assert!(matches!(
            browser.calls().last(),
            Some(BrowserCommand::ScreenshotRegion { region, expected_viewport, .. })
                if region.x == 310.0
                    && region.y == 170.0
                    && region.width == 100.0
                    && region.height == 50.0
                    && *expected_viewport == first_region_viewport
        ));

        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.org/"),
            committed_urls: vec!["https://example.org/".into()],
        }));
        let _ = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"tab":tab_handle,"url":"https://example.org"}),
            &CancellationToken::default(),
        );
        let before = browser.calls().len();
        let stale = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"view":current_view,"x":1,"y":1}),
            &CancellationToken::default(),
        );
        assert_eq!(stale.facts["reason"], "stale_view");
        assert_eq!(browser.calls().len(), before);
    }

    // a moved view is a stale view with a screenshot next step.
    {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(BrowserError::StaleView));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            &CancellationToken::default(),
        );
        assert_eq!(clicked.status, Status::Failed);
        assert_eq!(clicked.effect, Effect::None);
        assert_eq!(
            clicked.summary,
            "The page has moved since that screenshot was taken."
        );
        assert_eq!(clicked.facts["reason"], "stale_view");
        assert_eq!(
            clicked.next_steps,
            vec!["Call browser_screenshot to obtain a current view handle."]
        );
    }

    // a point outside the image gives the image size.
    {
        let (executor, browser, _, workspace, tab_handle, _) = workspace_with_a_target();
        let viewport = ViewportGeometry {
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
            output_scale: 0.5,
        };
        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "image".into(),
            width: 400,
            height: 300,
            viewport,
        }));
        let screenshot = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let view = screenshot.facts["view"].as_str().unwrap().to_owned();
        let dispatched = browser.calls().len();

        for (tool, input) in [
            (
                "browser_click",
                json!({"tab":tab_handle,"view":view,"x":400,"y":10}),
            ),
            (
                "browser_screenshot",
                json!({"tab":tab_handle,"view":view,"x":300,"y":0,"width":101,"height":10}),
            ),
        ] {
            let refused = executor.execute(&workspace, tool, input, &CancellationToken::default());
            assert_eq!(
                refused.status,
                Status::Failed,
                "{tool}: {}",
                refused.summary
            );
            assert_eq!(refused.effect, Effect::None, "{tool}");
            assert_eq!(refused.facts["reason"], "outside_image", "{tool}");
            assert_eq!(
                refused.facts["image"],
                json!({"width":400,"height":300}),
                "{tool}"
            );
            assert_eq!(
                refused.summary,
                "Those coordinates are outside the view's image, which is 400 by 300 pixels."
            );
            assert_eq!(
                refused.next_steps,
                vec!["Repeat with coordinates inside the image, or call browser_screenshot for a new view."]
            );
        }
        assert_eq!(
            browser.calls().len(),
            dispatched,
            "nothing reaches the browser"
        );
    }
}

/// ACCEPTANCE Browser journey 24: a session never reaches another session's tabs and is never
/// told to switch; the refusal names the owning session by its label, never an unlabeled one by its
/// id, across every permutation.
#[test]
fn a_session_never_reaches_another_sessions_tabs() {
    // a session never reaches another sessions tabs and is never told to switch.
    {
        let (executor, browser, _, workspace_a, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        let workspace_b = executor
            .workspaces
            .admit("Workspace B".into(), IntakeChannel::Mcp);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(42, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace_b,
            "browser_navigate",
            json!({"url": "https://example.com", "new_tab": true}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let tab_b = opened.facts["tab"].as_str().unwrap().to_owned();
        let dispatched = browser.calls().len();

        // Session A holds B's handle. It is refused before any browser work, the refusal points
        // at A's own tabs, and nothing in it names B's workspace or a way into it.
        let refused = executor.execute(
            &workspace_a,
            "browser_read",
            json!({"tab": tab_b}),
            &CancellationToken::default(),
        );
        assert_eq!(refused.status, Status::Failed);
        assert_eq!(refused.effect, Effect::None);
        assert_eq!(refused.facts["reason"], "ownership_mismatch");
        assert_eq!(
            refused.next_steps,
            vec!["Call browser_tabs with action list to see this session's tabs, then continue with those handles."]
        );
        let rendered = serde_json::to_string(&refused).unwrap();
        assert!(!rendered.contains(workspace_b.as_str()), "{rendered}");
        assert!(!rendered.contains("browser_workspace"), "{rendered}");

        // The removed switch, called directly or as a flow step, is refused and dispatches nothing.
        for (tool, input) in [
            (
                "browser_workspace",
                json!({"action": "switch", "workspace": workspace_b.as_str()}),
            ),
            (
                "browser_flow",
                json!({"steps": [{"tool": "browser_workspace", "arguments": {"action": "switch", "workspace": workspace_b.as_str()}}]}),
            ),
        ] {
            let result = executor.execute(&workspace_a, tool, input, &CancellationToken::default());
            assert_eq!(result.status, Status::Failed, "{tool}: {result:?}");
            assert_eq!(result.effect, Effect::None, "{tool}: {result:?}");
        }
        assert_eq!(browser.calls().len(), dispatched);

        // A's own listing never shows B's tab. A holds no tab, so it asks no browser at all.
        let listed = executor.execute(
            &workspace_a,
            "browser_tabs",
            json!({"action": "list"}),
            &CancellationToken::default(),
        );
        assert_eq!(listed.status, Status::Succeeded);
        assert_eq!(listed.facts["tabs"], json!([]));
        assert_eq!(browser.calls().len(), dispatched);

        // B still owns and reads its tab.
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 42,
            text: "Workspace B content".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/".into(),
        }));
        let read = executor.execute(
            &workspace_b,
            "browser_read",
            json!({"tab": tab_b}),
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Succeeded);
        assert_eq!(read.facts["text"], "Workspace B content");
    }

    // cross workspace tab discovery names owning workspace and guidance.
    {
        let (executor, browser, _, workspace_a, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        let workspace_b = executor
            .workspaces
            .admit("Workspace B".into(), IntakeChannel::Mcp);

        // Open a tab in Workspace B
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(42, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace_b,
            "browser_navigate",
            json!({"url": "https://example.com", "new_tab": true}),
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let tab_b = opened.facts["tab"].as_str().unwrap();

        // Attempt to use Workspace B's tab from Workspace A
        let result = executor.execute(
            &workspace_a,
            "browser_read",
            json!({"tab": tab_b}),
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Failed);
        assert_eq!(result.facts["reason"], "ownership_mismatch");
        assert_eq!(result.facts["owner_workspace"], "Workspace B");
        assert_eq!(
            result.summary,
            "That handle belongs to another Ghostlight session (Workspace B)."
        );
        assert_eq!(
            result.next_steps,
            vec!["Call browser_tabs with action list to see this session's tabs, then continue with those handles."]
        );
    }

    // a refusal never names an unlabeled session by its workspace id.
    {
        for label in ["", "\u{202e}\u{200b}\u{0007}"] {
            let (executor, browser, _, workspace_a, _) = fixture();
            browser.connect(vec![summary(FAKE_BROWSER, true)]);
            let workspace_b = executor.workspaces.admit(label.into(), IntakeChannel::Mcp);
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(42, "https://example.com/"),
                committed_urls: vec!["https://example.com/".into()],
            }));
            let opened = executor.execute(
                &workspace_b,
                "browser_navigate",
                json!({"url": "https://example.com", "new_tab": true}),
                &CancellationToken::default(),
            );
            assert_eq!(opened.status, Status::Succeeded);
            let tab_b = opened.facts["tab"].as_str().unwrap().to_owned();
            browser.push(Ok(BrowserOutcome::targets(
                42,
                vec![ObservedTarget {
                    locator: "btn-1".into(),
                    role: "button".into(),
                    name: "Save".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                }],
            )));
            let inspected = executor.execute(
                &workspace_b,
                "browser_inspect",
                json!({"tab": tab_b, "scope": "controls", "max_items": 10}),
                &CancellationToken::default(),
            );
            assert_eq!(inspected.status, Status::Succeeded, "{inspected:?}");
            let target_b = inspected.facts["items"][0]["target"]
                .as_str()
                .unwrap()
                .to_owned();
            let dispatched = browser.calls().len();

            for (tool, input) in [
                ("browser_read", json!({"tab": tab_b})),
                ("browser_click", json!({"target": target_b})),
            ] {
                let refused =
                    executor.execute(&workspace_a, tool, input, &CancellationToken::default());
                assert_eq!(refused.status, Status::Failed, "{tool}: {refused:?}");
                assert_eq!(refused.effect, Effect::None, "{tool}");
                assert_eq!(refused.facts["reason"], "ownership_mismatch", "{tool}");
                assert!(
                    refused.facts.get("owner_workspace").is_none(),
                    "{tool}: {refused:?}"
                );
                assert_eq!(
                    refused.summary,
                    "That handle belongs to a different Ghostlight session."
                );
                assert_eq!(
                    refused.next_steps,
                    vec!["Collect fresh handles from this session, then continue with those."]
                );
                let rendered = serde_json::to_string(&refused).unwrap();
                assert!(!rendered.contains(workspace_b.as_str()), "{rendered}");
            }
            assert_eq!(browser.calls().len(), dispatched);
        }
    }

    // cross workspace tab discovery stress permutations.
    {
        let (executor, browser, _, _, _) = fixture();
        browser.connect(vec![summary(FAKE_BROWSER, true)]);

        let names = ["Finance", "Legal", "Engineering", "Marketing", "Security"];
        let mut workspaces = Vec::new();
        let mut tab_handles = Vec::new();

        for (index, name) in names.iter().enumerate() {
            let ws = executor
                .workspaces
                .admit((*name).into(), IntakeChannel::Mcp);
            let pid = u64::try_from(100 + index).unwrap();
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(pid, &format!("https://{}.internal/", name.to_lowercase())),
                committed_urls: vec![format!("https://{}.internal/", name.to_lowercase())],
            }));
            let opened = executor.execute(
                &ws,
                "browser_navigate",
                json!({"url": format!("https://{}.internal/", name.to_lowercase()), "new_tab": true}),
                &CancellationToken::default(),
            );
            assert_eq!(opened.status, Status::Succeeded);
            let handle = opened.facts["tab"].as_str().unwrap().to_owned();
            tab_handles.push(handle);
            workspaces.push(ws);
        }

        // Stress: every workspace attempts to access every tab (25 total accesses)
        for (caller_idx, caller_ws) in workspaces.iter().enumerate() {
            for (target_idx, target_handle) in tab_handles.iter().enumerate() {
                if caller_idx == target_idx {
                    // Own workspace access
                    browser.push(Ok(BrowserOutcome::Text {
                        tab_id: u64::try_from(100 + target_idx).unwrap(),
                        text: "Authorized workspace content".into(),
                        truncated: false,
                        title: format!("{} Dashboard", names[target_idx]),
                        url: format!("https://{}.internal/", names[target_idx].to_lowercase()),
                    }));
                    let result = executor.execute(
                        caller_ws,
                        "browser_read",
                        json!({"tab": target_handle}),
                        &CancellationToken::default(),
                    );
                    assert_eq!(result.status, Status::Succeeded);
                } else {
                    // Cross-workspace access attempt
                    let result = executor.execute(
                        caller_ws,
                        "browser_read",
                        json!({"tab": target_handle}),
                        &CancellationToken::default(),
                    );
                    let expected_owner = names[target_idx];
                    assert_eq!(result.status, Status::Failed);
                    assert_eq!(result.facts["reason"], "ownership_mismatch");
                    assert_eq!(result.facts["owner_workspace"], expected_owner);
                    assert_eq!(
                        result.summary,
                        format!(
                            "That handle belongs to another Ghostlight session ({expected_owner})."
                        )
                    );
                    assert_eq!(
                        result.next_steps,
                        vec!["Call browser_tabs with action list to see this session's tabs, then continue with those handles."]
                    );
                }
            }
        }
    }
}

/// One focused typing call with an expectation, whose check meets `check`.
fn typed_with_expectation(
    check: Result<BrowserOutcome, BrowserError>,
) -> (crate::work::result::InvocationResult, Arc<MemoryAudit>) {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/","new_tab":true}),
        &CancellationToken::default(),
    );
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "textbox".into(),
            role: "textbox".into(),
            name: "Message".into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        }],
    }));
    browser.push(Ok(BrowserOutcome::Typed {
        tab: tab(7, "https://example.com/"),
        character_count: 4,
        subject: None,
        committed_urls: vec![],
    }));
    browser.push(check);
    let result = executor.execute(
        &workspace,
        "browser_type_text",
        json!({"text":"sent","focused":true,"expect":{"condition":"text_present","value":"done"}}),
        &CancellationToken::default(),
    );
    (result, audit)
}

/// An expectation the page did not confirm keeps the applied action in the result, the audit and
/// the history.
#[test]
fn unconfirmed_expectations_keep_the_applied_action_in_every_account() {
    // unconfirmed expectations keep the applied action in every account.
    {
        const TYPED: &str = "Typed 4 characters into the \"Message\" text field on example.com.";
        const LOOK_FIRST: &str = "Check the page with browser_read or browser_inspect before repeating; the action already happened.";
        let cases = [
            (
                Ok(BrowserOutcome::Observed {
                    tab_id: 7,
                    satisfied: false,
                    elapsed_ms: 2_000,
                    readiness: BrowserReadiness::Complete,
                }),
                Status::Failed,
                "expectation_unmet",
                format!("The expected condition did not hold after the action. {TYPED}"),
                json!({"reason":"action_unconfirmed","cause":"unmet"}),
            ),
            (
                Err(BrowserError::CancelledBeforeDispatch),
                Status::Cancelled,
                "cancelled",
                format!(
                    "The expected condition was not checked because the call was cancelled. {TYPED}"
                ),
                json!({"reason":"action_unconfirmed","cause":"cancelled"}),
            ),
            (
                Err(BrowserError::DisconnectedBeforeDispatch),
                Status::Failed,
                "browser_disconnected",
                format!(
                    "The expected condition was not checked because the browser disconnected. {TYPED}"
                ),
                json!({"reason":"action_unconfirmed","cause":"disconnected"}),
            ),
            (
                Err(BrowserError::RecoveryManual {
                    browsers: vec!["Chromium".into()],
                }),
                Status::Failed,
                "browser_unavailable",
                format!(
                    "The expected condition was not checked because Ghostlight could not reach the browser. {TYPED}"
                ),
                json!({"reason":"action_unconfirmed","cause":"browser_unavailable"}),
            ),
        ];
        for (check, status, reason, summary, refusal) in cases {
            let (result, audit) = typed_with_expectation(check);
            assert_eq!(result.status, status, "{}", result.summary);
            assert_eq!(result.effect, Effect::Applied, "{}", result.summary);
            assert!(!result.repeat_safe, "{}", result.summary);
            assert_eq!(result.summary, summary);
            assert_eq!(result.next_steps, vec![LOOK_FIRST]);
            assert_eq!(result.facts["reason"], reason);
            assert_eq!(result.facts["typed"], true, "the action's facts stay");
            assert_eq!(result.facts["character_count"], 4);
            let records = audit.0.lock().unwrap();
            let record = records.last().unwrap();
            assert_eq!(record.effect, "applied");
            assert_eq!(record.summary, summary, "the audit keeps the same account");
            assert_eq!(
                serde_json::to_value(&record.refusal_facts).unwrap(),
                refusal
            );
        }
        let (confirmed, _) = typed_with_expectation(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 10,
            readiness: BrowserReadiness::Complete,
        }));
        assert_eq!(confirmed.status, Status::Succeeded);
        assert_eq!(confirmed.summary, TYPED);
    }
}

/// A workspace with one open tab and one current target, ready for an action.
pub(super) fn workspace_with_a_target() -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
    String,
    String,
) {
    let (executor, browser, workspaces, workspace, _) = fixture();
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
            locator: "delete".into(),
            role: "button".into(),
            name: "Delete".into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        }],
    )));
    let found = executor.execute(
        &workspace,
        "browser_find",
        json!({"tab":tab_handle,"text":"Delete"}),
        &CancellationToken::default(),
    );
    let target = found.facts["matches"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    (executor, browser, workspaces, workspace, tab_handle, target)
}

fn delete_click(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    tab_handle: &str,
    target: &str,
) -> crate::work::result::InvocationResult {
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: Some(PhysicalActionSubject {
            role: "button".into(),
            name: "Delete".into(),
            rebound: false,
        }),
    }));
    executor.execute(
        workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        &CancellationToken::default(),
    )
}

/// ADR-0158 item 5 and c2df20e2: a click that opens a dialog says so, a fill whose submit raises
/// a confirm writes no field value, dialog calls keep the tab's record current, a prompt's status
/// names its field and what accept answers, and an answer with none open names no dialog.
#[test]
fn a_dialog_is_named_where_it_blocks_and_answered_as_it_asks() {
    // a click that opens a dialog says so.
    {
        use crate::language::outcome::DialogKind;
        let (executor, browser, workspaces, workspace, tab_handle, target) =
            workspace_with_a_target();

        let plain = delete_click(&executor, &browser, &workspace, &tab_handle, &target);
        assert_eq!(
            plain.summary,
            "Clicked the \"Delete\" button on example.com."
        );
        assert!(plain.facts.get("dialog_opened").is_none());

        workspaces.apply_browser_dialog(FAKE_BROWSER, 7, Some(DialogKind::Confirm));
        let opened = delete_click(&executor, &browser, &workspace, &tab_handle, &target);
        assert_eq!(opened.status, Status::Succeeded);
        assert_eq!(
            opened.summary,
            "Clicked the \"Delete\" button on example.com. A confirm dialog is now open."
        );
        assert_eq!(opened.facts["dialog_opened"], "confirm");
        assert_eq!(
            opened.next_steps,
            vec!["Call browser_dialog with action status to read it, then accept, dismiss, or respond with text for a prompt."]
        );

        workspaces.apply_browser_dialog(FAKE_BROWSER, 7, None);
        let closed = delete_click(&executor, &browser, &workspace, &tab_handle, &target);
        assert!(closed.facts.get("dialog_opened").is_none());
        assert!(closed.next_steps.is_empty());
    }

    // a fill whose submit raises a confirm writes no field value.
    {
        use crate::browser::adapter::FieldNow;
        use crate::language::outcome::DialogKind;
        let (executor, browser, workspaces, workspace, audit) = fixture();
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
        let control = |locator: &str, role: &str, name: &str| ObservedTarget {
            locator: locator.into(),
            role: role.into(),
            name: name.into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        };
        let controls = vec![
            control("plan", "combobox", "Plan"),
            control("phone", "textbox", "Phone"),
            control("send", "button", "Send"),
        ];
        browser.push(Ok(BrowserOutcome::targets(7, controls.clone())));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            &CancellationToken::default(),
        );
        let handle = |index: usize| inspected.facts["items"][index]["target"].clone();
        let reading = |field: usize, now: &str, tolerated: bool| FieldNow {
            field,
            now: now.into(),
            tolerated,
            matched: None,
            picked: None,
            group: None,
        };
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            rebound: false,
            tab_id: 7,
            targets: controls,
        }));
        browser.push(Ok(BrowserOutcome::Filled {
            rebound: false,
            tab: tab(7, "https://example.com/"),
            filled_count: 2,
            held: vec![
                reading(0, "PRIVATE_CHOICE", false),
                reading(1, "PRIVATE_READING", true),
            ],
            submitted: true,
            committed_urls: Vec::new(),
        }));
        workspaces.apply_browser_dialog(FAKE_BROWSER, 7, Some(DialogKind::Confirm));

        let filled = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({
                "tab": tab_handle,
                "fields": [
                    {"target": handle(0), "value": "pro"},
                    {"target": handle(1), "value": "5551234567"}
                ],
                "submit_target": handle(2)
            }),
            &CancellationToken::default(),
        );

        assert_eq!(filled.status, Status::Succeeded, "{filled:?}");
        assert_eq!(filled.facts["dialog_opened"], "confirm");
        for sentinel in ["PRIVATE_CHOICE", "PRIVATE_READING"] {
            assert!(filled.summary.contains(sentinel), "{}", filled.summary);
        }
        let records = audit.0.lock().unwrap();
        let record = records.last().unwrap();
        assert_eq!(record.tool, "browser_fill_form");
        assert_eq!(
            record.summary,
            "Filled 2 fields on example.com and submitted the form. A confirm dialog is now open."
        );
        let line = serde_json::to_string(record).unwrap();
        for sentinel in ["PRIVATE_CHOICE", "PRIVATE_READING", "5551234567"] {
            assert!(!line.contains(sentinel), "{line}");
        }
    }

    // dialog calls keep the record of the tab current.
    {
        use crate::language::outcome::DialogKind;
        let (executor, browser, workspaces, workspace, tab_handle, target) =
            workspace_with_a_target();
        let dialog_open = || workspaces.dialog_open(&workspace, 7);

        browser.push(Ok(BrowserOutcome::Dialog {
            tab_id: 7,
            present: true,
            dialog_type: "prompt".into(),
            message: Some("Name?".into()),
            url: Some("https://example.com/".into()),
            default_prompt: None,
        }));
        let status = executor.execute(
            &workspace,
            "browser_dialog",
            json!({"action":"status","tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(
            status.summary,
            "A prompt dialog is currently visible: \"Name?\"."
        );
        assert_eq!(dialog_open(), Some(DialogKind::Prompt));

        browser.push(Ok(BrowserOutcome::Dialog {
            tab_id: 7,
            present: true,
            dialog_type: "prompt".into(),
            message: None,
            url: Some("https://example.com/".into()),
            default_prompt: None,
        }));
        browser.push(Ok(BrowserOutcome::DialogHandled {
            tab_id: 7,
            dialog_type: "prompt".into(),
            accepted: true,
            url: Some("https://example.com/".into()),
        }));
        executor.execute(
            &workspace,
            "browser_dialog",
            json!({"action":"respond","text":"Ada","tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(dialog_open(), None);
        let clicked = delete_click(&executor, &browser, &workspace, &tab_handle, &target);
        assert!(clicked.facts.get("dialog_opened").is_none());

        workspaces.apply_browser_dialog(FAKE_BROWSER, 7, Some(DialogKind::Alert));
        browser.push(Ok(BrowserOutcome::Dialog {
            tab_id: 7,
            present: false,
            dialog_type: String::new(),
            message: None,
            url: None,
            default_prompt: None,
        }));
        executor.execute(
            &workspace,
            "browser_dialog",
            json!({"action":"status","tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(dialog_open(), None);
    }

    // a prompts status names its field and what accept answers.
    {
        let (executor, browser, _workspaces, workspace, tab_handle, _target) =
            workspace_with_a_target();
        browser.push(Ok(BrowserOutcome::Dialog {
            tab_id: 7,
            present: true,
            dialog_type: "prompt".into(),
            message: Some("What should it say?".into()),
            default_prompt: Some("hello from the desk".into()),
            url: Some("https://example.com/".into()),
        }));
        let status = executor.execute(
            &workspace,
            "browser_dialog",
            json!({"action":"status","tab":tab_handle}),
            &CancellationToken::default(),
        );
        assert_eq!(status.status, Status::Succeeded);
        assert_eq!(
            status.summary,
            "A prompt dialog is currently visible: \"What should it say?\". Its field holds \"hello from the desk\"."
        );
        assert_eq!(
            status.next_steps,
            vec!["Call browser_dialog with action accept to answer with the text in its field, as OK does, respond with your own text, or dismiss, before continuing."]
        );
        assert_eq!(status.facts["dialog_type"], "prompt");
    }

    // a dialog answer with none open names no dialog visible.
    {
        let (executor, browser, _, workspace, _) = fixture();
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
        browser.push(Ok(BrowserOutcome::Dialog {
            tab_id: 7,
            present: false,
            dialog_type: String::new(),
            message: None,
            url: None,
            default_prompt: None,
        }));
        let result = executor.execute(
            &workspace,
            "browser_dialog",
            json!({"action":"accept","tab":opened.facts["tab"]}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Failed, "{result:?}");
        assert_eq!(result.facts["reason"], "no_dialog_visible");
        assert_eq!(result.facts["handled"], false);
    }
}

/// The step of a removed handle's refusal that has no candidates to pick from.
pub(super) const REMOVED_STEP: &str =
    "Call browser_find or browser_inspect to obtain current target handles.";
/// The step of a removed handle's refusal that lists candidates.
pub(super) const PICK_STEP: &str = "Pick a target from recovery_candidates by its context, or call browser_find for current target handles.";

/// The refusal of a handle whose control the page found gone, as the model reads it, with the
/// step it gives.
fn assert_stale_removed(result: &crate::work::result::InvocationResult, step: &str) {
    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.effect, Effect::None);
    assert!(result.repeat_safe);
    assert_eq!(result.summary, "That target is no longer on the page.");
    assert_eq!(result.facts["reason"], "stale_target");
    assert_eq!(result.facts["stale"], "removed");
    assert_eq!(result.next_steps, vec![step]);
}
