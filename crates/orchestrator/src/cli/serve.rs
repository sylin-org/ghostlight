//! `serve`: the service in this process until a quit.
//!
//! It has no window of its own: it binds its endpoint as soon as it has started (ADR-0199
//! Decision 5), and the shell that shows the window and the tray is started by presence
//! (`service::presence`). A start first relaunches itself once, so the process that serves is
//! never a living child of its starter (ADR-0199 amendment of 2026-10-21).

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};

use ghostlight_relay::os::session;
use ghostlight_relay::protocol::StartReason;

use crate::cli::parse::{ServeStart, RELAUNCHED_OPTION};
use crate::install::layout::Layout;
use crate::install::record;
use crate::language::environment::SESSION_LISTENER_FAILED;
use crate::service::{Quit, ServiceHost, StartTerms, World};

/// The status a `serve` start exits with when it never bound its endpoint in time.
pub const READY_STALLED_EXIT: i32 = 3;

/// Run the service for `start`, until a quit.
///
/// A start a starter made relaunches itself once (ADR-0199 amendment of 2026-10-21). It refuses
/// an elevated token, so that refusal still reaches whoever ran `serve`, then starts this image
/// again with [`RELAUNCHED_OPTION`] and exits 0. The relaunched copy serves. The service is
/// therefore never a living child of its starter: an agent host that ends its MCP server by
/// killing the server's process tree, as the Copilot CLI does, cannot reach it, and on Linux it
/// belongs to init.
///
/// The service shows nothing itself. It exits 0 without a word when another service already
/// serves or when Quit holds a browser or recover start back, and it stops by itself, with a
/// nonzero status and a diagnostics event, when it is not accepting openings in time.
pub fn serve(start: ServeStart) -> anyhow::Result<()> {
    let home = ghostlight_relay::home::home()?;
    let ended = if start.relaunched {
        serve_in(
            &home,
            start.reason,
            listen_for_session_end,
            |home, reason| {
                ServiceHost::start(home, StartTerms::for_reason(reason), World::system())
            },
        )
    } else {
        relaunch(
            &home,
            start.reason,
            ghostlight_win_peer::is_elevated,
            spawn_relaunched,
        )
    };
    record_start_failure(&home, &ended);
    settle_serve(ended)
}

/// Refuse an elevated token as a serving start does, then have `spawn` start the copy that
/// serves for `reason` in `home`, and return without waiting for it.
fn relaunch(
    home: &Path,
    reason: StartReason,
    elevated: impl FnOnce() -> io::Result<bool>,
    spawn: impl FnOnce(&Path, StartReason) -> io::Result<()>,
) -> anyhow::Result<()> {
    crate::service::refuse_elevated_token(elevated)?;
    spawn(home, reason).map_err(|error| anyhow::anyhow!("relaunch the service: {error}"))
}

/// Start this image as `serve --reason <reason>` with [`RELAUNCHED_OPTION`], detached, in
/// `home`.
fn spawn_relaunched(home: &Path, reason: StartReason) -> io::Result<()> {
    let image = std::env::current_exe()?;
    let [serve, option, word] = ghostlight_relay::spawn::serve_arguments(reason);
    let arguments = [serve, option, word, OsStr::new(RELAUNCHED_OPTION)];
    ghostlight_relay::os::spawn_detached(&image, &arguments, home).map(drop)
}

/// Leave the reason a start failed in the home's start-failure record, where its starter reads
/// it: a started service's output goes nowhere. Meeting the running service and being held back
/// by Quit are not failures and leave nothing.
fn record_start_failure(home: &Path, ended: &anyhow::Result<()>) {
    let Err(error) = ended else {
        return;
    };
    if error.is::<crate::service::ServiceAlreadyRunning>()
        || error.is::<crate::service::QuitLatched>()
    {
        return;
    }
    let reason = format!("{error:#}\n");
    let _ = record::atomic_replace(&Layout::new(home).start_failure(), reason.as_bytes());
}

/// Listen for the end of the session from this process's start.
///
/// On the final signal the fallback exit is armed first. A service that exists then says goodbye
/// with `session_end` on every stream and leaves through its quit; a start that has no service
/// yet holds nothing and leaves at once.
fn listen_for_session_end(ending: Arc<OnceLock<Quit>>) -> io::Result<session::SessionListener> {
    session::listen(move |event| session::stop_on_final(event, || end_session(ending.get())))
}

/// End this process because the session is ending: through the quit path when the service
/// exists, at once when it does not.
fn end_session(quit: Option<&Quit>) {
    match quit {
        Some(quit) => quit.session_end(),
        None => std::process::exit(0),
    }
}

/// Serve from the Ghostlight home `home` until a quit, with `listen` hearing the end of the
/// session and `start` making the service.
///
/// In order: the session listener is installed, so a start still waiting for the service lock
/// leaves at once; the service starts under its lock; the quit signal is installed; the ready
/// limit is armed; and only then the endpoint is bound, so a quit that arrives the moment the
/// first peer can connect is heard. The process then waits for the quit signal.
fn serve_in<L>(
    home: &Path,
    reason: StartReason,
    listen: impl FnOnce(Arc<OnceLock<Quit>>) -> io::Result<L>,
    start: impl FnOnce(&Path, StartReason) -> anyhow::Result<ServiceHost>,
) -> anyhow::Result<()> {
    let ending = Arc::new(OnceLock::<Quit>::new());
    let _session_listener = listen(Arc::clone(&ending))
        .map_err(|error| anyhow::anyhow!("{SESSION_LISTENER_FAILED} {error}"))?;
    let host = start(home, reason)?;
    let _ = ending.set(host.quit_handle());
    let (quit_sender, quit_received) = mpsc::channel::<()>();
    host.install_quit_signal(Arc::new(move || {
        let _ = quit_sender.send(());
    }));
    host.expect_ready_by(host.ready_deadline(), || {
        std::process::exit(READY_STALLED_EXIT)
    })?;
    host.publish_ready()?;
    // This start answers now, so a reason an earlier one recorded no longer applies.
    let _ = fs::remove_file(Layout::new(home).start_failure());
    eprintln!("Ghostlight {} ready", env!("CARGO_PKG_VERSION"));
    // A quit signal that is never made, because the service was dropped, also ends the wait.
    let _ = quit_received.recv();
    Ok(())
}

/// Turn how a `serve` start ended into its exit: finding the service already running, or being
/// held back by Quit, is success.
pub fn settle_serve(ended: anyhow::Result<()>) -> anyhow::Result<()> {
    match ended {
        Err(error)
            if error.is::<crate::service::ServiceAlreadyRunning>()
                || error.is::<crate::service::QuitLatched>() =>
        {
            Ok(())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::scratch::Scratch;

    /// Protects the service's start lifecycle and defect bd29366c: a start relaunches itself
    /// for its reason and the copy serves, an elevated start is refused before it relaunches, the
    /// session listener is installed before the service starts and the endpoint binds after, a refusal
    /// to start settles as success and any other failure as failure, and a failed start leaves its
    /// reason for the installer while a service that publishes removes the one an earlier start left.
    #[test]
    fn a_service_start_relaunches_for_its_reason_and_leaves_why_it_ended() {
        {
            let home = Scratch::named("relaunch");
            let mut relaunched = None;
            relaunch(
                home.path(),
                StartReason::Agent,
                || Ok(false),
                |directory, reason| {
                    relaunched = Some((directory.to_path_buf(), reason));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                relaunched,
                Some((home.path().to_path_buf(), StartReason::Agent))
            );
        }
        {
            let home = Scratch::named("relaunch-elevated");
            let mut relaunched = false;
            let refused = relaunch(
                home.path(),
                StartReason::Agent,
                || Ok(true),
                |_, _| {
                    relaunched = true;
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(
                format!("{refused:#}"),
                crate::language::environment::ELEVATED_SERVICE
            );
            assert!(!relaunched, "nothing was started");
        }
        {
            use crate::cli::client::control_at;
            use ghostlight_relay::protocol::{Control, QuitReason};

            let home = crate::service::test_home::TestHome::new("serve-order");
            let order = std::sync::Mutex::new(Vec::<&str>::new());
            let path = home.path().to_path_buf();
            let probe = path.clone();
            let quitter = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(20);
                while Instant::now() < deadline {
                    if control_at(
                        &probe,
                        Control::Quit {
                            reason: QuitReason::Replace,
                        },
                    )
                    .is_ok()
                    {
                        return true;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                false
            });
            let served = serve_in(
                &path,
                StartReason::Agent,
                |_| {
                    order.lock().unwrap().push("listener");
                    assert!(
                        control_at(&path, Control::Readiness).is_err(),
                        "nothing is bound before the listener is installed"
                    );
                    Ok(())
                },
                |home, _| {
                    order.lock().unwrap().push("start");
                    ServiceHost::start_for_test(home)
                },
            );
            assert!(quitter.join().unwrap(), "the endpoint was bound and quit");
            served.unwrap();
            assert_eq!(*order.lock().unwrap(), ["listener", "start"]);
        }
        {
            assert!(settle_serve(Err(crate::service::ServiceAlreadyRunning.into())).is_ok());
            assert!(settle_serve(Err(crate::service::QuitLatched.into())).is_ok());
            assert!(settle_serve(Err(anyhow::anyhow!("the endpoint could not be bound"))).is_err());
            assert!(settle_serve(Ok(())).is_ok());
        }
        {
            let home = crate::service::test_home::TestHome::new("start-failure");
            let record = Layout::new(home.path()).start_failure();
            record_start_failure(
                home.path(),
                &Err(crate::service::ServiceAlreadyRunning.into()),
            );
            record_start_failure(home.path(), &Err(crate::service::QuitLatched.into()));
            record_start_failure(home.path(), &Ok(()));
            assert!(!record.exists());
            let failed = Err(anyhow::anyhow!("path must be shorter than SUN_LEN")
                .context("bind the relay endpoint"));
            record_start_failure(home.path(), &failed);
            assert_eq!(
                fs::read_to_string(&record).unwrap(),
                "bind the relay endpoint: path must be shorter than SUN_LEN\n"
            );
        }
        {
            let home = crate::service::test_home::TestHome::new("start-failure-cleared");
            let path = home.path().to_path_buf();
            let record = Layout::new(&path).start_failure();
            fs::create_dir_all(record.parent().unwrap()).unwrap();
            fs::write(&record, "an earlier reason\n").unwrap();
            // The service answers once its listener runs, which can be before `serve_in` removes the
            // record, so the record is looked at only after `serve_in` returns: it removes the record
            // before it waits for the quit.
            let probe = path.clone();
            let quitter = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(20);
                while Instant::now() < deadline {
                    if crate::cli::client::status_at(&probe).is_ok() {
                        let _ = crate::cli::client::control_at(
                            &probe,
                            ghostlight_relay::protocol::Control::Quit {
                                reason: ghostlight_relay::protocol::QuitReason::Replace,
                            },
                        );
                        return true;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                false
            });
            serve_in(
                &path,
                StartReason::Command,
                |_| Ok(()),
                |home, _| ServiceHost::start_for_test(home),
            )
            .unwrap();
            assert!(quitter.join().unwrap(), "the service answered");
            assert!(
                !record.exists(),
                "a service that published removed the earlier start's reason"
            );
        }
    }
}
