//! Byte goldens for the level-3 frames the service writes, so serving a second wire level beside
//! it can never change what a published extension reads.
//!
//! Correlations are minted per call and the service version moves with every release, so each is
//! replaced by a fixed placeholder before the comparison. The page-runtime script is pinned by its
//! own hash test and is replaced the same way.

use std::io::Read;

use serde_json::Value;

use super::*;
use crate::browser::adapter::{BrowserOutcome, BrowserReceipt, BrowserRequest};

const HELLO_ACCEPTED: &str = r#"{"kind":"hello_accepted","major":3,"service_version":"<version>","service_epoch":"service_bytes","control_state":"active"}"#;
const INSTALL_PAGE_RUNTIME: &str = r#"{"kind":"request","request":{"correlation":"<correlation>","workspace":"system","command":{"command":"install_page_runtime","revision":1,"sha256":"9eb521d0cab493adeda19cef6091c6513de27e06c59d509dcf2c3869ac79d8b4","script":"<script>"}}}"#;
const LIST_TABS_REQUEST: &str = r#"{"kind":"request","request":{"correlation":"<correlation>","workspace":"workspace_bytes","command":{"command":"list_tabs"}}}"#;
const ACKNOWLEDGE: &str = r#"{"kind":"acknowledge","correlation":"<correlation>"}"#;
const OPEN_TAB_REQUEST: &str = r#"{"kind":"request","request":{"correlation":"physical_bytes","workspace":"workspace_bytes","command":{"command":"open_tab","url":"https://example.com/","group_title":"Ghostlight - test","reuse":"never"}}}"#;
const DIALOG_RECEIPT: &str = r#"{"kind":"receipt","receipt":{"correlation":"physical_bytes","result":{"outcome":"dialog","tab_id":7,"present":true,"dialog_type":"alert"}}}"#;

const CORRELATION: &str = "<correlation>";
const VERSION: &str = "<version>";
const SCRIPT: &str = "<script>";

/// Read one native frame exactly as it crossed the socket.
fn raw_frame(stream: &mut TcpStream) -> String {
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let mut bytes = vec![0; u32::from_le_bytes(length) as usize];
    stream.read_exact(&mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

/// Replace one JSON string value, spelled exactly as serde writes it, with a placeholder.
fn mask(frame: &str, value: &str, placeholder: &str) -> String {
    let spelled = serde_json::to_string(value).unwrap();
    assert!(frame.contains(&spelled), "{frame} carries {value}");
    frame.replace(&spelled, &serde_json::to_string(placeholder).unwrap())
}

/// The correlation a request frame carries.
fn request_correlation(frame: &str) -> String {
    let value: Value = serde_json::from_str(frame).unwrap();
    value["request"]["correlation"].as_str().unwrap().to_owned()
}

fn tabs_receipt(correlation: String) -> BrowserFrame {
    BrowserFrame::Receipt {
        receipt: BrowserReceipt {
            correlation,
            result: BrowserOutcome::Tabs { tabs: vec![] },
        },
    }
}

#[test]
fn level_three_frames_keep_their_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, hold) = mpsc::channel::<()>();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        write_native(
            &mut stream,
            &BrowserFrame::Hello {
                major: ADAPTER_PROTOCOL_MAJOR,
                adapter_version: "1.3.8".into(),
                browser_id: TEST_BROWSER.into(),
                adapter_epoch: "adapter_bytes".into(),
                browser_name: Some("Chrome".into()),
                platform: Some(BrowserPlatform::Chromium),
                attended: false,
                capabilities: vec![
                    capability(adapter_capability::PAGE_RUNTIME),
                    capability(adapter_capability::TABS),
                ],
            },
        )
        .unwrap();
        let accepted = raw_frame(&mut stream);

        let install = raw_frame(&mut stream);
        let request: BrowserFrame = serde_json::from_str(&install).unwrap();
        let BrowserFrame::Request { request } = request else {
            panic!("page runtime installation request");
        };
        let BrowserCommand::InstallPageRuntime {
            revision,
            sha256,
            script,
        } = request.command
        else {
            panic!("closed page runtime mechanism");
        };
        write_native(
            &mut stream,
            &BrowserFrame::Receipt {
                receipt: BrowserReceipt {
                    correlation: request.correlation.clone(),
                    result: BrowserOutcome::PageRuntimeInstalled { revision, sha256 },
                },
            },
        )
        .unwrap();
        let install_acknowledge = raw_frame(&mut stream);
        let install = mask(
            &mask(&install, &request.correlation, CORRELATION),
            &script,
            SCRIPT,
        );
        let install_acknowledge = mask(&install_acknowledge, &request.correlation, CORRELATION);

        let list = raw_frame(&mut stream);
        let correlation = request_correlation(&list);
        write_native(&mut stream, &tabs_receipt(correlation.clone())).unwrap();
        let acknowledge = raw_frame(&mut stream);
        hold.recv().unwrap();
        (
            mask(&accepted, env!("CARGO_PKG_VERSION"), VERSION),
            install,
            install_acknowledge,
            mask(&list, &correlation, CORRELATION),
            mask(&acknowledge, &correlation, CORRELATION),
        )
    });

    let (stream, _) = listener.accept().unwrap();
    let port = RelayBrowserPort::new("service_bytes".into());
    port.attach(stream).unwrap();
    let cancelled = AtomicBool::new(false);
    port.call(
        TEST_BROWSER,
        "workspace_bytes",
        BrowserCommand::ListTabs,
        Instant::now() + Duration::from_secs(5),
        &cancelled,
    )
    .unwrap();
    release.send(()).unwrap();
    let (accepted, install, install_acknowledge, list, acknowledge) = client.join().unwrap();
    assert_eq!(accepted, HELLO_ACCEPTED);
    assert_eq!(install, INSTALL_PAGE_RUNTIME);
    assert_eq!(install_acknowledge, ACKNOWLEDGE);
    assert_eq!(list, LIST_TABS_REQUEST);
    assert_eq!(acknowledge, ACKNOWLEDGE);
}

#[test]
fn level_three_open_tab_and_dialog_frames_ignore_what_only_protocol_four_carries() {
    for focus_window in [false, true] {
        let frame = BrowserFrame::Request {
            request: BrowserRequest {
                correlation: "physical_bytes".into(),
                workspace: "workspace_bytes".into(),
                command: BrowserCommand::OpenTab {
                    url: "https://example.com/".into(),
                    group_title: "Ghostlight - test".into(),
                    reuse: "never".into(),
                    focus_window,
                },
            },
        };
        assert_eq!(serde_json::to_string(&frame).unwrap(), OPEN_TAB_REQUEST);
    }
    let dialog = BrowserFrame::Receipt {
        receipt: BrowserReceipt {
            correlation: "physical_bytes".into(),
            result: BrowserOutcome::Dialog {
                tab_id: 7,
                present: true,
                dialog_type: "alert".into(),
                message: None,
            },
        },
    };
    assert_eq!(
        serde_json::from_str::<BrowserFrame>(DIALOG_RECEIPT).unwrap(),
        dialog
    );
    assert_eq!(serde_json::to_string(&dialog).unwrap(), DIALOG_RECEIPT);
}
