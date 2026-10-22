//! Starting the service: the gate a start passes, publication, the replacement's wait for the
//! relays it inherits, and a start that stalls.

use super::*;

/// Defect S-D3 (b0290246) and ACCEPTANCE C10: a service that becomes ready wakes every edge
/// that registered to wait for it, so a browser left open after a Quit comes back at once.
#[test]
fn a_published_service_wakes_the_edges_that_wait_for_it() {
    let home = TestHome::new("wakes-waiting-edges");
    let waiting = ghostlight_relay::wake::Waiting::register(home.path()).unwrap();
    let (woken, wakes) = mpsc::channel();
    thread::spawn(move || {
        if waiting.next().is_ok() {
            let _ = woken.send(());
        }
    });
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    host.publish_ready().unwrap();
    wakes
        .recv_timeout(PATIENCE)
        .expect("the waiting edge was woken");
}

/// The workbench origin is Ghostlight's own ceiling (ADR-0199's web apps amendment and
/// ADR-0203's amendment of 2026-10-19): publishing starts the workbench and bars agents from
/// its address before the page answers, and from nothing else.
#[test]
fn publishing_starts_the_workbench_and_bars_agents_from_its_address() {
    let home = TestHome::new("workbench-published");
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    assert!(host.workbench_address().is_none());
    host.publish_ready().unwrap();
    let address = host.workbench_address().expect("the workbench is served");
    assert!(address.url().starts_with("http://127.0.0.1:"));

    let snapshot = host.context.governance.snapshot();
    for url in [address.url().to_owned(), format!("{}app.js", address.url())] {
        let decision = snapshot.authorize_landing(crate::governance::CapabilitySet::READ, &url);
        assert!(!decision.allowed, "{url} must be barred from agents");
    }
    let elsewhere = snapshot.authorize_landing(
        crate::governance::CapabilitySet::READ,
        "https://example.com/",
    );
    assert!(elsewhere.allowed);

    // The page answers on that address, and the facade knows where it is.
    assert_eq!(host.workbench.web_address(), Some(address));
    host.publish_ready().unwrap();
    drop(host);
}

/// The terms of a start for `reason` that waits `lock_wait` for a held lock.
fn waiting(reason: StartReason, lock_wait: Duration) -> StartTerms {
    StartTerms {
        lock_wait,
        ..StartTerms::for_reason(reason)
    }
}

/// Every file and directory under `root`, as paths relative to it.
fn files_under(root: &std::path::Path) -> std::collections::BTreeSet<std::path::PathBuf> {
    let mut found = std::collections::BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path.clone());
            }
            found.insert(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
    found
}

/// ADR-0199 Decisions 9 and 10 and ACCEPTANCE C13: a start applies the quit latch, then takes
/// the lock, and touches nothing before it holds it, so Quit holds a browser or recover start
/// back, also one that waited for the lock, and any other start clears the latch.
#[test]
fn a_start_applies_the_quit_latch_and_takes_the_lock_before_it_touches_any_state() {
    const HELD_BACK: [StartReason; 2] = [StartReason::Browser, StartReason::Recover];
    const CLEARING: [StartReason; 4] = [
        StartReason::Agent,
        StartReason::Command,
        StartReason::Person,
        StartReason::Update,
    ];

    // With no latch, a browser start serves.
    let directory = temp_home("latch-clear");
    assert!(start_for(&directory, StartTerms::for_reason(StartReason::Browser)).is_ok());

    // A set latch holds a browser or recover start back before it takes the lock.
    let directory = temp_home("latch-refuses");
    let layout = Layout::new(directory.path());
    crate::service::latch::set(&layout.quit_latch()).unwrap();
    for reason in HELD_BACK {
        let error = start_for(&directory, StartTerms::for_reason(reason))
            .err()
            .expect("Quit holds this start back");
        assert!(error.is::<QuitLatched>(), "{reason:?}");
        assert!(
            crate::service::latch::is_set(&layout.quit_latch()),
            "{reason:?}: the latch stays"
        );
        assert!(
            !layout.run().exists(),
            "{reason:?}: the lock was never taken"
        );
    }

    // Any other start clears it once it holds the lock, and serves.
    for reason in CLEARING {
        let directory = temp_home("latch-cleared");
        let layout = Layout::new(directory.path());
        crate::service::latch::set(&layout.quit_latch()).unwrap();
        let started = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
        assert!(
            !crate::service::latch::is_set(&layout.quit_latch()),
            "{reason:?}"
        );
        drop(started);
    }

    // A start that finds the lock held gives way and touches nothing, the latch included.
    let directory = temp_home("lock-held-gives-way");
    let layout = Layout::new(directory.path());
    let holder = start_for(&directory, agent()).unwrap();
    crate::service::latch::set(&layout.quit_latch()).unwrap();
    let before = files_under(&directory);
    for reason in CLEARING {
        let error = start_for(&directory, StartTerms::asking_once(reason))
            .err()
            .expect("the lock is held");
        assert!(error.is::<ServiceAlreadyRunning>(), "{reason:?}");
        assert!(
            crate::service::latch::is_set(&layout.quit_latch()),
            "{reason:?}: a start that gives way leaves the latch alone"
        );
    }
    assert_eq!(files_under(&directory), before, "no state was opened");
    drop(holder);

    // A browser start that began before a Quit, and waited for the lock the quitting service
    // held, is refused once it holds the lock.
    let directory = temp_home("latch-set-while-waiting");
    let layout = Layout::new(directory.path());
    let holder = start_for(&directory, agent()).unwrap();
    let (began, beginning) = mpsc::channel();
    let starter = {
        let directory = directory.path().to_path_buf();
        thread::spawn(move || {
            // The token check comes before the start's first look at the latch.
            ServiceHost::start(
                &directory,
                waiting(StartReason::Browser, PATIENCE),
                World::test().with_token(move || {
                    let _ = began.send(());
                    Ok(false)
                }),
            )
            .map(drop)
        })
    };
    beginning.recv_timeout(PATIENCE).expect("the start began");
    crate::service::latch::set(&layout.quit_latch()).unwrap();
    drop(holder);
    let error = starter
        .join()
        .unwrap()
        .expect_err("Quit holds this start back");
    assert!(error.is::<QuitLatched>());
    assert!(
        crate::service::latch::is_set(&layout.quit_latch()),
        "the latch stays"
    );
    assert!(
        crate::service::ServiceLock::is_free(&layout.service_lock()),
        "the refused start released the lock"
    );
}

/// How many of a browser relay's looks for a returning service a replacement may take to
/// publish and still have that relay waited for. The fourth look comes 7.5 s after the old
/// service ended; the measured updates published within about a second of that end.
const REPLACEMENT_LOOKS: usize = 4;

/// How long a relay's look that finds the service takes to put its browser back: the relay
/// exits, the extension opens a new native port at once and the new relay's hello is
/// answered. Well under a second where it was measured
/// (`docs/testing/alpine-2.0.1-bundle-2026-10-20.md`); two seconds leaves room for a slow
/// native-host start.
const RELAY_HANDOVER: Duration = Duration::from_secs(2);

/// A home that remembers a browser, as every home a browser has connected to does.
fn home_that_remembers_a_browser(name: &str) -> Scratch {
    let directory = temp_home(name);
    let layout = Layout::new(directory.path());
    crate::install::last_browser::record(&layout.last_browser_record(), "chrome").unwrap();
    directory
}

/// Defect S-D2 (9c257daa) and ACCEPTANCE C19: after an update or a recovery a call with no
/// browser waits for the replaced service's relays, on their own backoff, up to the return
/// grace, while any other start, or one in a home no browser has used, waits only the short window.
#[test]
fn a_replacement_waits_for_the_relays_of_the_service_it_replaced() {
    use crate::browser::{BrowserPort, RELAY_RETURN_WINDOW, RETURN_GRACE};
    use ghostlight_relay::spawn::Backoff;

    // Each relay of the ended service looks again on its own doubling backoff, counted from
    // the moment it lost the old service, so a replacement that publishes just after one look
    // waits nearly the whole next wait for the look that finds it.
    let mut backoff = Backoff::new();
    let longest_wait = (0..REPLACEMENT_LOOKS)
        .map(|_| backoff.next_wait())
        .max()
        .expect("the relay looks at least once");
    let slowest = longest_wait + RELAY_HANDOVER;
    assert!(
        slowest < RETURN_GRACE,
        "a relay can be back {slowest:?} after the publication, past the return grace"
    );

    for reason in [StartReason::Update, StartReason::Recover] {
        let directory = home_that_remembers_a_browser("replacement-relays");
        let host = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
        host.publish_ready().unwrap();

        host.browser.backdate(slowest);
        assert!(
            host.browser.browser_returning(),
            "{reason:?}: a relay back {slowest:?} after the publication is still waited for"
        );
        host.browser.backdate(RETURN_GRACE - slowest);
        assert!(
            !host.browser.browser_returning(),
            "{reason:?}: the wait for the replaced service's relays ends with the return grace"
        );
        drop(host);
    }

    // A start that replaced no service, and an update in a home no browser has connected to,
    // as on a first install, keep nearly the whole budget of a call for starting a browser.
    for (reason, directory) in [
        (StartReason::Update, temp_home("first-install")),
        (
            StartReason::Agent,
            home_that_remembers_a_browser("agent-start"),
        ),
        (
            StartReason::Browser,
            home_that_remembers_a_browser("browser-start"),
        ),
    ] {
        let host = start_for(&directory, StartTerms::for_reason(reason)).unwrap();
        host.publish_ready().unwrap();
        assert!(
            host.browser.browser_returning(),
            "{reason:?}: relays may be reconnecting"
        );
        host.browser.backdate(RELAY_RETURN_WINDOW);
        assert!(
            !host.browser.browser_returning(),
            "{reason:?}: nothing replaced holds the wait past the relay window"
        );
        drop(host);
    }
}

/// ADR-0199 Decision 9 step 2 and its amendment of 2026-10-16: an elevated token, or one that
/// cannot be read, is refused before the start takes any state, and the debug-build waiver
/// lets only an elevated answer through.
#[test]
fn an_elevated_or_unreadable_token_is_refused_before_any_state() {
    use crate::service::decide_elevation;

    let directory = temp_home("elevated");
    let refused = ServiceHost::start(&directory, agent(), World::test().with_token(|| Ok(true)))
        .err()
        .expect("an elevated start is refused");
    assert_eq!(format!("{refused:#}"), crate::service::ELEVATED_SERVICE);
    assert!(
        !directory.join("run").exists(),
        "no lock, run directory or socket was made"
    );
    let unreadable = ServiceHost::start(
        &directory,
        agent(),
        World::test().with_token(|| Err(std::io::Error::other("no token"))),
    )
    .err()
    .expect("a token that cannot be read is refused");
    assert!(format!("{unreadable:#}").contains("no token"));
    assert!(!directory.join("run").exists());
    assert!(ServiceHost::start(&directory, agent(), World::test()).is_ok());

    // The waiver: only an elevated answer can be waived.
    let refused = decide_elevation(Ok(true), false).unwrap_err();
    assert_eq!(format!("{refused:#}"), crate::service::ELEVATED_SERVICE);
    assert!(decide_elevation(Ok(true), true).is_ok());
    assert!(decide_elevation(Ok(false), false).is_ok());
    assert!(decide_elevation(Ok(false), true).is_ok());
    for waived in [false, true] {
        let unreadable =
            decide_elevation(Err(std::io::Error::other("no token")), waived).unwrap_err();
        assert!(format!("{unreadable:#}").contains("no token"), "{waived}");
    }
}

/// ADR-0199 Decision 9 step 6: a start that does not accept openings in time stops by itself,
/// and becoming ready settles that limit, so a ready service never stops for it.
#[test]
fn a_stalled_start_stops_by_itself_and_becoming_ready_cancels_the_stop() {
    // A start whose limit passes before it accepts openings stops.
    let stalled_home = temp_home("ready-stalled");
    let stalled = ServiceHost::start_for_test(&stalled_home).unwrap();
    let (stopped, stop) = mpsc::channel::<()>();
    stalled
        .expect_ready_by(Instant::now(), move || {
            let _ = stopped.send(());
        })
        .unwrap();
    stop.recv_timeout(PATIENCE)
        .expect("a start that is not ready by its limit stops");
    drop(stalled);

    let directory = temp_home("ready-published");
    let host = ServiceHost::start_for_test(&directory).unwrap();
    let (expired, fired) = mpsc::channel::<()>();
    // A limit far past what this test waits, armed before the endpoint is bound, as serve does.
    host.expect_ready_by(Instant::now() + PATIENCE * 6, move || {
        let _ = expired.send(());
    })
    .unwrap();
    host.publish_ready().unwrap();
    // A settled limit ends its watcher without a stop, which drops the stop unmade.
    assert_eq!(
        fired.recv_timeout(PATIENCE),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
    drop(host);
}
