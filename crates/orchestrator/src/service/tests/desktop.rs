//! The shell on its `desktop` stream: the hand-over, the window a person asks for, the status,
//! the first close and the native acts.

use super::*;

/// The address, the bearer and the tray of a frame that hands over the workbench.
fn handed_over(handed: DesktopFrame) -> (String, String, TrayModel) {
    match handed {
        DesktopFrame::Workbench { url, bearer, tray } => (url, bearer, tray),
        other => panic!("not the workbench: {other:?}"),
    }
}

/// One request to the workbench on `port`; the status code and the whole answer come back.
fn workbench_call(
    port: u16,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
    use std::io::{Read, Write};

    let mut text = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    if let Some(bearer) = bearer {
        text.push_str(&format!("Authorization: Bearer {bearer}\r\n"));
    }
    if let Some(body) = body {
        text.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    text.push_str("\r\n");
    text.push_str(body.unwrap_or_default());
    let mut stream = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .expect("the workbench answers");
    stream.set_read_timeout(Some(PATIENCE)).unwrap();
    stream.write_all(text.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    let _ = stream.read_to_end(&mut bytes);
    let answer = String::from_utf8_lossy(&bytes).into_owned();
    let status = answer
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status in {answer}"));
    (status, answer)
}

/// The desktop-role contract (ADR-0199 Decision 7 and its shell milestone 2 amendment),
/// ACCEPTANCE D1 and D11, and defect 17ec7571: the shell that takes the one desktop place gets
/// the workbench and its bearer, and one that finds it taken is told so, with the holder shown
/// only for a shell that wanted a window.
#[test]
fn a_shell_that_attaches_gets_the_workbench_and_its_bearer_or_is_told_the_place_is_taken() {
    let home = TestHome::new("desktop-place");
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    host.publish_ready().unwrap();
    let address = host.workbench_address().unwrap();

    // The shell that takes the place is handed the workbench and the tray the service
    // describes.
    let (mut first, handed) = open_desktop();
    let (url, bearer, tray) = handed_over(handed);
    assert_eq!(url, address.url());
    assert_eq!(
        tray,
        host.workbench.tray_model(),
        "the workbench frame carries the tray the service describes"
    );
    eventually(|| host.quit.shells().attached() == 1);

    // A second shell is told the place is taken and leaves, and the first is shown.
    let (mut second, handed) = open_desktop();
    assert_eq!(handed, DesktopFrame::AlreadyOpen);
    assert_eq!(next_frame(&mut first), DesktopFrame::Show { address: None });
    assert!(
        matches!(
            frame::read_frame::<DesktopFrame>(&mut second),
            Ok(None) | Err(_)
        ),
        "the stream of the shell that was turned away ends"
    );
    assert_eq!(host.quit.shells().attached(), 1);

    // Presence's own tray shell that finds the place taken brings no window forward: the next
    // frame the holder hears is a later show.
    let mut tray_opening = desktop_opening();
    tray_opening.channel = Some(ShellView::Tray.as_str().to_owned());
    let (mut tray_shell, reply) = dial(&tray_opening);
    assert!(reply.accepted, "{reply:?}");
    assert_eq!(
        frame::read_frame::<DesktopFrame>(&mut tray_shell).unwrap(),
        Some(DesktopFrame::AlreadyOpen)
    );
    host.workbench.reveal(Some(PageAddress::About)).unwrap();
    assert_eq!(
        next_frame(&mut first),
        DesktopFrame::Show {
            address: Some(PageAddress::About)
        },
        "the first shell still serves, and the tray shell asked it to show nothing"
    );

    // The bearer makes the shell's page the window's; without it a screen cannot act as one.
    let hold = r#"{"intent":"hold"}"#;
    let (status, _) = workbench_call(
        address.port(),
        "POST",
        "/api/runtime",
        Some(&bearer),
        Some(hold),
    );
    assert_eq!(status, 200, "the bearer makes the request the window's");
    assert_eq!(
        host.workbench.look().status().service.runtime_state,
        RuntimeControlState::Held
    );
    let resume = r#"{"intent":"resume"}"#;
    let (status, _) = workbench_call(address.port(), "POST", "/api/runtime", None, Some(resume));
    assert_eq!(status, 401, "without the bearer a screen cannot resume");

    // The place is free again once the holder goes.
    drop(first);
    eventually(|| host.quit.shells().attached() == 0);
    let (_third, handed) = open_desktop();
    assert!(
        matches!(handed, DesktopFrame::Workbench { .. }),
        "the place is free again: {handed:?}"
    );
    drop(host);
}

/// The next frame the service wrote to the shell.
fn next_frame(stream: &mut Connection) -> DesktopFrame {
    frame::read_frame::<DesktopFrame>(stream)
        .unwrap()
        .expect("the service wrote a frame")
}

/// The id of the act the service asked the shell for, and the act.
fn next_act(stream: &mut Connection) -> (u64, NativeAct) {
    match next_frame(stream) {
        DesktopFrame::Act { id, act } => (id, act),
        other => panic!("expected an act, got {other:?}"),
    }
}

/// Run `request` on a thread of its own and give back what it answers.
fn in_background<T: Send + 'static>(
    request: impl FnOnce() -> T + Send + 'static,
) -> mpsc::Receiver<T> {
    let (answered, answers) = mpsc::channel();
    thread::spawn(move || {
        let _ = answered.send(request());
    });
    answers
}

/// ACCEPTANCE C16, ADR-0200 Decision 11 and defect c4c05e24 (JQ11): every way a person asks to
/// see Ghostlight shows the window at setup until setup is complete, and with no shell
/// attached starts one that shows the window at the address asked.
#[test]
fn a_persons_request_for_the_window_shows_it_at_setup_until_setup_is_complete() {
    use crate::browser::adapter::BrowserEvent;
    use crate::browser::BrowserEventSink;
    use crate::install::setup_state::{self, SetupRecord};
    use crate::workbench::setup::SetupProgress;

    {
        let home = TestHome::new("show-setup");
        let host = ServiceHost::start_for_test(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut shell = attached_shell(&host);
        let browser = crate::service::ServiceBrowserEvents::new(
            host.workbench.clone(),
            host.context.workspaces.clone(),
        );
        let click = |shell: &mut Connection, item| {
            frame::write_frame(shell, &DesktopFrame::Tray { item }).unwrap();
        };
        let at_setup = DesktopFrame::Show {
            address: Some(PageAddress::Setup),
        };

        // While setup is incomplete, the tray's Open, a control show (the install and
        // `ghostlight open`) and the browser's show request (the extension's Open Ghostlight)
        // each bring the window to setup.
        click(&mut shell, TrayItemId::Open);
        assert_eq!(next_frame(&mut shell), at_setup, "the tray's Open");
        assert!(cli_client::activate_workbench().unwrap());
        assert_eq!(next_frame(&mut shell), at_setup, "a control show");
        browser.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        assert_eq!(
            next_frame(&mut shell),
            at_setup,
            "the browser's show request"
        );
        eventually(|| !browser.showing.load(Ordering::SeqCst));

        // With setup complete the window opens where it last was, and the tray's Open asks for
        // nothing more than the shell already does: a Hold click is answered after it, by a
        // tray model, and the next show is a later one.
        let record = host.context.layout.setup_record();
        assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
        host.workbench.attach_setup(SetupProgress::load(&record));
        click(&mut shell, TrayItemId::Open);
        click(&mut shell, TrayItemId::Hold);
        while !matches!(next_frame(&mut shell), DesktopFrame::TrayModel { .. }) {}
        host.workbench.reveal(Some(PageAddress::About)).unwrap();
        assert_eq!(
            next_frame(&mut shell),
            DesktopFrame::Show {
                address: Some(PageAddress::About)
            },
            "no show was written for the open"
        );
        assert!(cli_client::activate_workbench().unwrap());
        assert_eq!(
            next_frame(&mut shell),
            DesktopFrame::Show { address: None },
            "a control show"
        );
        browser.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
        assert_eq!(
            next_frame(&mut shell),
            DesktopFrame::Show { address: None },
            "the browser's show request"
        );
        drop(host);
    }

    // With no shell attached, the request starts one without the tray argument and shows the
    // window at the address asked once it attaches.
    {
        let home = TestHome::new("show-launch");
        let fake = Arc::new(Fake::default());
        let host = ServiceHost::start(
            home.path(),
            agent(),
            World::test().with_presence({
                let fake = Arc::clone(&fake);
                move |_, _| fake.world()
            }),
        )
        .unwrap();
        host.publish_ready().unwrap();

        let asked = {
            let workbench = host.workbench.clone();
            in_background(move || workbench.reveal(Some(PageAddress::SettingsSharing)))
        };
        eventually(|| fake.launches() == [false]);
        let (mut stream, handed) = open_desktop();
        assert!(matches!(handed, DesktopFrame::Workbench { .. }));

        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Show {
                address: Some(PageAddress::SettingsSharing)
            },
            "the address asked for before the shell attached is shown when it does"
        );
        assert!(asked
            .recv_timeout(PATIENCE)
            .expect("the request is answered once the shell is attached")
            .is_ok());
        assert_eq!(
            fake.launches(),
            [false],
            "one launch, and not a tray one: the window is asked for"
        );
        drop(host);
    }
}

/// ACCEPTANCE D13 and defects c51808e7 and a034fc2d: status names the window by what the shell
/// last reported and a person last did, and a reason the shell gave for a window it cannot
/// build stands, also after the shell goes, until a person acts. A build that fails with the
/// runtime there reads `window_failed`, never a missing runtime, and the service words the
/// tooltip that says so until the window is asked for again (ADR-0203 Decision 12.7).
#[test]
fn the_status_names_the_window_by_what_the_shell_last_reported() {
    use crate::install::setup_state::{self, SetupRecord};
    use crate::intake::desktop::UnavailableReason;
    use crate::language::workbench::{self as words, WindowReason};
    use crate::workbench::setup::SetupProgress;

    let home = TestHome::new("status-window");
    let fake = Arc::new(Fake::default());
    let host = ServiceHost::start(
        home.path(),
        agent(),
        World::test().with_presence({
            let fake = Arc::clone(&fake);
            move |_, _| fake.world()
        }),
    )
    .unwrap();
    host.publish_ready().unwrap();
    // With setup complete a tray Open only says the person has the window open.
    let record = host.context.layout.setup_record();
    assert!(setup_state::record_if_absent(&record, &SetupRecord::now()).unwrap());
    host.workbench.attach_setup(SetupProgress::load(&record));
    let window = || cli_client::status_at(home.path()).unwrap().window;
    let send = |stream: &mut Connection, sent: DesktopFrame| {
        frame::write_frame(stream, &sent).unwrap();
    };
    let open = DesktopFrame::Tray {
        item: TrayItemId::Open,
    };
    let cannot_build = DesktopFrame::Unavailable {
        reason: UnavailableReason::WindowFailed,
    };
    let tooltip = |stream: &mut Connection| match next_frame(stream) {
        DesktopFrame::TrayModel { model } => model.tooltip,
        other => panic!("expected a tray model, got {other:?}"),
    };
    assert_eq!(window(), None, "presence has nothing to say yet");

    // Presence's own tray launch attaches with the icon only, and a tray Open is a person
    // opening the window.
    fake.advance(crate::service::presence::LAUNCH_DELAY);
    host.context.presence.tick();
    assert_eq!(fake.launches(), [true]);
    let (mut stream, _) = open_desktop();
    eventually(|| window() == Some(WindowReason::Tray));
    send(&mut stream, open.clone());
    eventually(|| window() == Some(WindowReason::Shown));

    // A show the shell cannot build reads as its reason while the shell stays attached, and a
    // later show the shell builds ends it.
    host.workbench.reveal(Some(PageAddress::About)).unwrap();
    assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
    send(&mut stream, cannot_build.clone());
    assert_eq!(tooltip(&mut stream), words::TRAY_WINDOW_FAILS);
    eventually(|| window() == Some(WindowReason::WindowFailed));
    assert_eq!(host.quit.shells().attached(), 1, "the shell is still there");
    host.workbench.reveal(None).unwrap();
    assert_ne!(tooltip(&mut stream), words::TRAY_WINDOW_FAILS);
    assert!(matches!(next_frame(&mut stream), DesktopFrame::Show { .. }));
    eventually(|| window() == Some(WindowReason::Shown));

    // A close that stays leaves the icon, and the tray's Open brings the window back.
    send(&mut stream, DesktopFrame::Closed { stays: true });
    eventually(|| window() == Some(WindowReason::Tray));
    send(&mut stream, open);
    eventually(|| window() == Some(WindowReason::Shown));

    // The reason a shell gave outlives it, until a person starts a shell again.
    send(&mut stream, cannot_build);
    eventually(|| window() == Some(WindowReason::WindowFailed));
    drop(stream);
    eventually(|| host.quit.shells().attached() == 0);
    assert_eq!(window(), Some(WindowReason::WindowFailed));
    let (_person, _) = open_desktop();
    eventually(|| window() == Some(WindowReason::Shown));
    drop(host);
}

/// Write `sent` to the shell's stream, then a click on the tray entry `item`, and return the
/// frames the shell hears before the tray model that answers the click. The service reads a
/// shell's frames in order and the click changes the tray, so whatever `sent` made the
/// service write comes first.
fn heard_before_a_click(
    stream: &mut Connection,
    sent: &DesktopFrame,
    item: TrayItemId,
) -> Vec<DesktopFrame> {
    frame::write_frame(stream, sent).unwrap();
    frame::write_frame(stream, &DesktopFrame::Tray { item }).unwrap();
    let mut heard = Vec::new();
    loop {
        match next_frame(stream) {
            DesktopFrame::TrayModel { .. } => return heard,
            other => heard.push(other),
        }
    }
}

/// ADR-0199's shell milestone 2 amendment and docs/trust/data-flows.md: the hint that
/// Ghostlight keeps running follows only a person's close that keeps the icon, once per home.
/// An update's goodbye, a shell that vanishes and a close that leaves no icon say nothing and
/// leave the hint to that first close, which writes `state/tray-hint` before it is said.
#[test]
fn only_the_first_close_that_keeps_the_icon_says_that_ghostlight_keeps_running() {
    use crate::language::workbench::{TRAY_HINT_BODY, TRAY_HINT_TITLE};

    let home = TestHome::new("tray-hint");
    let marker = Layout::new(home.path()).tray_hint();
    let hint = DesktopFrame::Notice {
        title: TRAY_HINT_TITLE.to_owned(),
        body: TRAY_HINT_BODY.to_owned(),
    };
    let stays = DesktopFrame::Closed { stays: true };
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    host.publish_ready().unwrap();

    // An update's goodbye and a shell that vanishes leave the hint unused.
    let mut updated = attached_shell(&host);
    host.quit.shells().say_goodbye(GoodbyeReason::Replace);
    assert_eq!(desktop_goodbye(&mut updated), GoodbyeReason::Replace);
    drop(updated);
    eventually(|| host.quit.shells().attached() == 0);
    drop(attached_shell(&host));
    eventually(|| host.quit.shells().attached() == 0);
    assert!(
        !marker.exists(),
        "neither an update nor a crash uses up the hint"
    );

    // A close that leaves no icon says nothing; the first close that keeps it says it, once.
    let mut shell = attached_shell(&host);
    let heard = heard_before_a_click(
        &mut shell,
        &DesktopFrame::Closed { stays: false },
        TrayItemId::Hold,
    );
    assert!(heard.is_empty(), "{heard:?}");
    assert!(!marker.exists());
    let heard = heard_before_a_click(&mut shell, &stays, TrayItemId::Resume);
    assert_eq!(heard, [hint]);
    assert!(
        marker.is_file(),
        "the marker is written before the hint is said"
    );
    let heard = heard_before_a_click(&mut shell, &stays, TrayItemId::Hold);
    assert!(heard.is_empty(), "{heard:?}");
    drop(shell);
    drop(host);

    // The next service of the home, which starts with its agents active, says it no more.
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    host.publish_ready().unwrap();
    let mut shell = attached_shell(&host);
    let heard = heard_before_a_click(&mut shell, &stays, TrayItemId::Hold);
    assert!(heard.is_empty(), "{heard:?}");
    drop(host);
}

/// ADR-0199 Decision 7 and its shell milestone 2 amendment (079727e9): every native act the
/// service asks of the shell gets one answer, the person's choice or the shell's failure, or
/// unavailable once the shell or the service goes, and no open dialog holds a quit.
#[test]
fn every_native_act_gets_one_answer_from_the_shell_or_unavailable_when_it_goes() {
    {
        let home = TestHome::new("shell-acts");
        let host = ServiceHost::start_for_test(home.path()).unwrap();
        host.publish_ready().unwrap();
        let mut stream = attached_shell(&host);

        // An answer for an act nobody asked is ignored, and the acts that follow are answered.
        answer(&mut stream, 99, ActOutcome::Done);

        // A pick returns what the person chose, nothing when they cancelled, and a failure.
        let route = host.route();
        let picked = in_background(move || route.pick_file("Choose a file"));
        let (id, act) = next_act(&mut stream);
        assert_eq!(
            act,
            NativeAct::PickFile {
                title: "Choose a file".to_owned()
            }
        );
        answer(
            &mut stream,
            id,
            ActOutcome::Picked {
                path: "chosen.json".to_owned(),
            },
        );
        assert_eq!(
            picked.recv_timeout(PATIENCE).unwrap().unwrap(),
            Some(PathBuf::from("chosen.json"))
        );

        let route = host.route();
        let closed = in_background(move || route.pick_file("Again"));
        let (id, _) = next_act(&mut stream);
        answer(&mut stream, id, ActOutcome::Cancelled);
        assert_eq!(closed.recv_timeout(PATIENCE).unwrap().unwrap(), None);

        let route = host.route();
        let refused = in_background(move || route.pick_file("Once more"));
        let (id, _) = next_act(&mut stream);
        answer(&mut stream, id, ActOutcome::Failed);
        assert!(matches!(
            refused.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::ActFailed(_))
        ));

        // A link and a folder are opened by the shell, and their failure is told.
        for (outcome, opened) in [(ActOutcome::Done, true), (ActOutcome::Failed, false)] {
            let route = host.route();
            let link = in_background(move || route.open_url("https://example.com/docs"));
            let (id, act) = next_act(&mut stream);
            assert_eq!(
                act,
                NativeAct::OpenUrl {
                    url: "https://example.com/docs".to_owned()
                }
            );
            answer(&mut stream, id, outcome.clone());
            assert_eq!(link.recv_timeout(PATIENCE).unwrap().is_ok(), opened);

            let route = host.route();
            let folder = in_background(move || route.open_folder(Path::new("logs")));
            let (id, act) = next_act(&mut stream);
            assert_eq!(
                act,
                NativeAct::OpenFolder {
                    path: "logs".to_owned()
                }
            );
            answer(&mut stream, id, outcome);
            assert_eq!(folder.recv_timeout(PATIENCE).unwrap().is_ok(), opened);
        }

        // A pick is released when the shell's stream ends.
        let route = host.route();
        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        drop(stream);
        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        eventually(|| host.quit.shells().attached() == 0);

        // A pick is released by a goodbye, which the shell hears.
        let mut stream = attached_shell(&host);
        let route = host.route();
        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        host.quit.shells().say_goodbye(GoodbyeReason::Replace);
        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        assert_eq!(
            next_frame(&mut stream),
            DesktopFrame::Goodbye {
                reason: GoodbyeReason::Replace
            }
        );
        drop(host);
    }

    // A quit releases a pick at once and does not wait for it, and no act starts after it
    // began. Its grace is far longer than this test waits, so a quit that waited for the
    // person's dialog would not end in time.
    {
        let (_home, host, quit_requested) = quittable_host("shell-acts-quit");
        let mut stream = attached_shell(&host);
        let route = host.route();
        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        host.quit_handle().begin(QuitReason::Quit, PATIENCE * 6);
        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        quit_requested
            .recv_timeout(PATIENCE)
            .expect("a person's open dialog does not hold the quit");
        assert!(
            matches!(
                host.route().pick_file("Too late"),
                Err(WorkbenchPresentationError::Unavailable)
            ),
            "no act starts once a quit has begun"
        );
        drop(host);
    }

    // The end of the session releases a pick.
    {
        let (_home, host, _quit_requested) = quittable_host("shell-acts-session-end");
        let mut stream = attached_shell(&host);
        let route = host.route();
        let picked = in_background(move || route.pick_file("Choose"));
        next_act(&mut stream);
        host.quit_handle().session_end();
        assert!(matches!(
            picked.recv_timeout(PATIENCE).unwrap(),
            Err(WorkbenchPresentationError::Unavailable)
        ));
        drop(host);
    }
}
