//! The idle exit, and a start that meets a service leaving on idle.

use super::*;

/// The idle limit of the tests of the idle exit. It is judged on a clock the test moves, so
/// real time passing never reaches it, however loaded the machine; in real time it only sets
/// how often the watch looks, a tenth of it.
const IDLE_LIMIT: Duration = Duration::from_secs(1);

/// Terms of an agent start whose service idles out after [`IDLE_LIMIT`].
fn idling() -> StartTerms {
    StartTerms {
        idle_exit: IDLE_LIMIT,
        ..agent()
    }
}

/// A world that also ends when a quit has begun, as the service's real one does, and
/// otherwise acts as `inner`.
struct EndsWithQuit(
    Box<dyn crate::service::presence::Outside>,
    crate::service::presence::Ending,
);

impl crate::service::presence::Outside for EndsWithQuit {
    fn now(&self) -> Instant {
        self.0.now()
    }

    fn display(&self) -> Result<(), crate::language::workbench::WindowReason> {
        self.0.display()
    }

    fn launch(&self, tray: bool) -> Result<(), crate::service::presence::LaunchRefusal> {
        self.0.launch(tray)
    }

    fn ending(&self) -> bool {
        self.0.ending() || (self.1)()
    }
}

/// A published service that idles out after [`IDLE_LIMIT`] on `clock`, and the channel its
/// quit signal sends on. The host is dropped before its home.
struct Idling {
    host: ServiceHost,
    home: TestHome,
    clock: HandClock,
    quit_requested: mpsc::Receiver<()>,
}

impl Idling {
    /// Wait until the idle watch has looked twice from now, and assert that the service
    /// asked for no exit, saying `why` it stays. A look is counted before it reads the clock,
    /// so the first sees everything done before this call; the second comes only because the
    /// first did not leave.
    fn looked_and_stayed(&self, why: &str) {
        let streams = &self.host.context.streams;
        let before = streams.looks();
        let until = Instant::now() + PATIENCE;
        while streams.looks() < before + 2 {
            assert!(Instant::now() < until, "the watch stopped looking: {why}");
            thread::sleep(Duration::from_millis(5));
        }
        assert!(self.quit_requested.try_recv().is_err(), "{why}");
    }

    /// Move the idle clock well past the limit and show that the service stays.
    fn stays_past_the_limit(&self, why: &str) {
        self.clock.advance(IDLE_LIMIT * 3);
        self.looked_and_stayed(why);
    }

    /// Wait until no stream is open, so the last close has armed the limit, then move the
    /// idle clock past the limit and expect the quit within [`PATIENCE`].
    fn leaves_past_the_limit(&self, why: &str) {
        eventually(|| self.host.context.streams.open() == 0);
        self.clock.advance(IDLE_LIMIT * 2);
        self.quit_requested.recv_timeout(PATIENCE).expect(why);
    }
}

/// A published host whose idle limit runs on a clock the test moves, whose quit signal is a
/// channel and whose presence acts on `fake`.
fn idling_host(name: &str, fake: &Arc<Fake>) -> Idling {
    let home = TestHome::new(name);
    let clock = HandClock::default();
    let fake = Arc::clone(fake);
    let host = ServiceHost::start(
        home.path(),
        idling(),
        World::test()
            .with_presence(move |_, ending| Box::new(EndsWithQuit(fake.world(), ending)))
            .with_idle_clock(Arc::new(clock.clone())),
    )
    .unwrap();
    let (quit, quit_requested) = mpsc::channel();
    host.install_quit_signal(Arc::new(move || {
        let _ = quit.send(());
    }));
    host.publish_ready().unwrap();
    Idling {
        host,
        home,
        clock,
        quit_requested,
    }
}

/// ACCEPTANCE C22 (ADR-0199 Decision 12): a stream of any role, or work in flight, keeps an
/// idle service, and its idle exit says nothing, latches nothing, starts no shell and adds
/// nothing to another end, so the next browser start is served.
#[test]
fn the_idle_exit_waits_for_every_stream_and_work_and_leaves_quietly() {
    use crate::browser::adapter::chromium::protocol4::fake::FakeExtension;
    use crate::browser::adapter::chromium::protocol4::fake::{Reply as FakeReply, Script};

    // A browser that answers every request with an empty tab list.
    let listing = || -> Script {
        Box::new(|correlation, _| {
            FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
        })
    };

    // An open MCP stream holds the service, and the last close arms the limit again.
    {
        let idling = idling_host("idle-stream", &Arc::default());
        let socket = open_mcp(None);
        idling.stays_past_the_limit("an open stream holds the service");
        drop(socket);
        idling.leaves_past_the_limit("the last close arms the limit and the service leaves");
    }

    // A control call is an opening, so each one restarts the span.
    {
        let idling = idling_host("idle-control", &Arc::default());
        for _ in 0..4 {
            idling.clock.advance(IDLE_LIMIT * 9 / 10);
            idling.looked_and_stayed("the last call came less than a limit ago");
            cli_client::status().expect("the service answers a status call");
            eventually(|| idling.host.context.streams.open() == 0);
        }
        idling.leaves_past_the_limit("the service leaves a limit after the last call");
    }

    // An attached shell is a stream.
    {
        let idling = idling_host("idle-shell", &Arc::default());
        let (shell, _handed) = open_desktop();
        idling.stays_past_the_limit("an attached shell holds the service");
        drop(shell);
        idling.leaves_past_the_limit("the service leaves once the shell has gone");
    }

    // Work the executor still holds re-arms the limit instead of leaving.
    {
        let idling = idling_host("idle-work", &Arc::default());
        let running = idling.host.quit.drain.admit().unwrap();
        idling.stays_past_the_limit("held work keeps the service");
        drop(running);
        idling.leaves_past_the_limit("the service leaves a limit after the work finished");
    }

    // A browser stream opened through the intake holds the service until it ends, and a
    // connection that replaces it for the same browser is still that one stream (the browser
    // port reports the replacement as an attach and never the retired connection's end).
    {
        let idling = idling_host("idle-browser", &Arc::default());
        let host = &idling.host;
        // Stand in for the browser edge: carry the extension's bytes over a browser stream.
        let (native, extension) = pair().unwrap();
        let carrier = thread::spawn(move || {
            let (service, reply) = dial(&opening(Role::Browser));
            assert!(reply.accepted, "{reply:?}");
            service.set_read_timeout(None).unwrap();
            let mut to_service = (native.try_clone().unwrap(), service.try_clone().unwrap());
            let mut to_extension = (service, native);
            thread::spawn(move || {
                let _ = std::io::copy(&mut to_extension.0, &mut to_extension.1);
            });
            let _ = std::io::copy(&mut to_service.0, &mut to_service.1);
            // The extension is gone: end the stream the service holds as the edge would.
            to_service.1.shutdown();
        });
        let fake =
            FakeExtension::connect(extension, &FakeExtension::hello("browser_idle"), listing());
        drop(fake.wait(|seen| !seen.frames.is_empty()));
        eventually(|| {
            host.browser
                .connected_browsers()
                .iter()
                .any(|browser| browser.id == "browser_idle")
        });
        // The opening is counted out once the browser is attached: from here only the browser
        // holds the service.
        eventually(|| host.context.streams.open() == 1);
        idling.stays_past_the_limit("an attached browser holds the service");

        let (service, extension) = pair().unwrap();
        let again =
            FakeExtension::connect(extension, &FakeExtension::hello("browser_idle"), listing());
        host.browser.attach(service).unwrap();
        assert_eq!(
            host.context.streams.open(),
            1,
            "a connection that replaces another for the same browser is the same stream"
        );
        idling.stays_past_the_limit("the browser holds the service on its new connection");
        again.close();
        drop(again);
        idling.leaves_past_the_limit("the service leaves once the browser has gone");
        fake.close();
        drop(fake);
        let _ = carrier.join();
    }

    // The idle exit sets no latch, admits nothing more, starts no shell when one falls due,
    // and the next browser start is served.
    {
        let fake = Arc::new(Fake::default());
        let idling = idling_host("idle-exit", &fake);
        idling.leaves_past_the_limit("the idle service asks the process to exit");
        fake.advance(crate::service::presence::LAUNCH_DELAY);
        idling.host.context.presence.tick();
        assert!(
            fake.launches().is_empty(),
            "a service that is leaving starts no shell"
        );
        let Idling { host, home, .. } = idling;
        assert!(
            !crate::service::latch::is_set(&Layout::new(home.path()).quit_latch()),
            "an idle exit is not a person's Quit"
        );
        assert!(
            host.context.quit.drain.admit().is_err(),
            "no new work is admitted once the service is leaving"
        );
        assert!(
            host.context.streams.admit().is_none(),
            "no stream is admitted once the service is leaving"
        );
        drop(host);
        let browser = start_for(home.path(), StartTerms::for_reason(StartReason::Browser));
        assert!(browser.is_ok(), "the next browser start serves");
    }

    // The control: on a service that stays, presence starts the tray shell when it falls due.
    {
        let live = Arc::new(Fake::default());
        let idling = idling_host("idle-exit-control", &live);
        live.advance(crate::service::presence::LAUNCH_DELAY);
        idling.host.context.presence.tick();
        assert_eq!(live.launches(), [true]);
    }

    // Once the end of the session is under way, the idle limit passing adds nothing.
    {
        let idling = idling_host("idle-session-end", &Arc::default());
        idling.host.quit_handle().session_end();
        idling
            .quit_requested
            .recv_timeout(PATIENCE)
            .expect("the session end asks the process to exit");
        idling.clock.advance(IDLE_LIMIT * 3);
        let streams = Arc::clone(&idling.host.context.streams);
        let before = streams.looks();
        eventually(|| streams.looks() > before);
        let Idling {
            host,
            quit_requested,
            ..
        } = idling;
        // Dropping the host joins the idle watch, so what that look decided has been done, and
        // the quit signal, with its sender, is gone.
        drop(host);
        assert_eq!(
            quit_requested.recv_timeout(PATIENCE),
            Err(mpsc::RecvTimeoutError::Disconnected),
            "the idle limit passing during the exit makes no second request"
        );
    }
}

/// Defect m2-06 (987fe943, ADR-0199 amendment of 2026-10-16): a start that arrives while the
/// service is leaving on idle waits for the lock the leaving service still holds and serves,
/// instead of giving way to a service that is about to exit.
#[test]
fn a_start_that_arrives_while_the_service_is_leaving_on_idle_waits_for_it_and_serves() {
    let idling = idling_host("idle-exit-race", &Arc::default());
    idling.leaves_past_the_limit("the idle service asks the process to exit");
    let Idling { host, home, .. } = idling;
    // The old service still holds the lock and the endpoint while it tears down.
    let directory = home.path().to_path_buf();
    let (began, beginning) = mpsc::channel();
    let starter = thread::spawn(move || {
        let started = Instant::now();
        // The token check comes before the start first asks for the lock.
        let served = ServiceHost::start(
            &directory,
            StartTerms::for_reason(StartReason::Browser),
            World::test().with_token(move || {
                let _ = began.send(());
                Ok(false)
            }),
        )
        .map(drop);
        (served, started.elapsed())
    });
    beginning.recv_timeout(PATIENCE).expect("the start began");
    drop(host);
    let (served, waited) = starter.join().unwrap();
    served.expect("the start takes the lock the old service released");
    assert!(waited < crate::service::lifetime::LEAVING_LOCK_WAIT);
}
