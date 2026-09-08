//! Actual executor dispatch binds read feedback and closes each composition child's visuals.

use super::*;
use ghostlight_bridge::browser::{PresentationActivity, PresentationKind, PresentationSignal};

#[derive(Default)]
struct PageSignals(Mutex<Vec<PresentationSignal>>);

impl PresentationPort for PageSignals {
    fn present(&self, _: &str, signal: PresentationSignal) -> Result<(), PresentationError> {
        self.0.lock().unwrap().push(signal);
        Ok(())
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
            reused: false,
            tab: tab(id, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/","new_tab":true}),
            None,
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
        let result = executor.execute(&workspace, tool, input, None, &CancellationToken::default());
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
