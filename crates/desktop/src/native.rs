//! What the shell does on this computer for the service: show a notice, open a link or a folder,
//! ask for a file, and open the window at a section (ADR-0199 Decision 7 and its amendment of
//! 2026-10-15, shell milestone 2).
//!
//! The service decides what to show or open and when; the shell only carries it out. The pure
//! parts live here and are unit tested: the address check, the address a window loads to open at
//! a section ([`section_url`]), and [`carry_out`], which turns one act into its result. [`ThisComputer`] is the thin glue to the operating system (the opener, the
//! file dialog) and [`show_notice`] is the notification call; both are checked live.
//!
//! An act is answered through a callback, because a file dialog is not always finished when it is
//! asked for. On Linux the dialog is a GTK one that belongs to the event-loop thread, which must
//! stay free to run GTK: the dialog is shown, the call returns, and the answer comes from the
//! dialog's own response. Windows shows its dialog modally and answers before it returns.
//!
//! The shell starts a process only while the session is not ending
//! ([`Session::may_start_process`]), and a link or a folder is opened without a console
//! window: the shell is a graphical program and the opener asks the shell of the system, never a
//! command interpreter.

#[cfg(target_os = "linux")]
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::rc::Rc;

use tao::window::Window;

use crate::link::{ActOutcome, NativeAct};
use crate::session::Session;

/// The shell's AppUserModelID. Windows groups the window with a Start-menu shortcut that carries
/// the same id, and shows a notification from an unpackaged program only then (ADR-0199
/// Decision 7).
pub const SHELL_APP_ID: &str = "Sylin.Ghostlight";

/// The name notices are shown under where the system asks for one.
const NOTICE_APP_NAME: &str = "Ghostlight";

/// The longest address the shell accepts, in characters.
const ADDRESS_LIMIT: usize = 48;

/// The start of every address the shell opens in the person's browser.
const SECURE_SCHEME: &str = "https://";

/// The name of the thread that shows one notice, so a slow notification service never holds the
/// window.
const NOTICE_THREAD_NAME: &str = "ghostlight-desktop-notice";

/// Name the process [`SHELL_APP_ID`]. Call it before any window is made. A refusal is one line on
/// standard error, as every shell error is.
pub fn prepare_identity() {
    if let Err(error) = ghostlight_relay::os::set_app_user_model_id(SHELL_APP_ID) {
        eprintln!("ghostlight-desktop: {error}");
    }
}

/// Whether `address` is safe to put after the `#` of the workbench address: one to
/// [`ADDRESS_LIMIT`] characters of `a` to `z`, `/` and `-`, and nothing else.
///
/// This keeps the address safe and nothing more. Which addresses the page has is the service's to
/// say; it checks every address before it sends one.
pub fn is_safe_address(address: &str) -> bool {
    !address.is_empty()
        && address.len() <= ADDRESS_LIMIT
        && address
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || matches!(byte, b'/' | b'-'))
}

/// The address a new window loads: the workbench address, with `address` after the `#` when it
/// passes [`is_safe_address`].
pub fn start_url(workbench: &str, address: Option<&str>) -> String {
    match address.filter(|address| is_safe_address(address)) {
        Some(address) => format!("{workbench}#{address}"),
        None => workbench.to_owned(),
    }
}

/// The address an open window navigates to in order to show `address`, or `None` when the address
/// is not safe.
///
/// `origin` is the workbench origin, with no trailing slash. The window is moved by navigating to
/// this address, which the webview does as a change of the address after the `#` and the page
/// reads as such. A script run in the page would be lost when the page is still loading, which is
/// how a request for a section made right after the window opened was dropped.
pub fn section_url(origin: &str, address: &str) -> Option<String> {
    is_safe_address(address).then(|| start_url(&format!("{origin}/"), Some(address)))
}

/// Where the result of one act goes. It is called exactly once, on the thread that carried the
/// act out, possibly after the call that started the act has returned.
pub type Answer = Box<dyn FnOnce(ActOutcome)>;

/// Where the file the person chose goes, or `None` when they closed the dialog. Called exactly
/// once, on the thread that asked, possibly after [`Computer::pick_file`] has returned.
pub type Chosen = Box<dyn FnOnce(Option<PathBuf>)>;

/// What only this computer can do, as [`carry_out`] asks for it.
pub trait Computer {
    /// Open an address in the person's browser.
    fn open_url(&self, url: &str) -> io::Result<()>;

    /// Show a folder in the file manager.
    fn open_folder(&self, path: &Path) -> io::Result<()>;

    /// Ask the person for a file and hand the choice to `chosen`.
    ///
    /// It must not run a nested event loop or block the thread that runs the window: that thread
    /// owns the toolkit, and a dialog that waits on it can never be answered.
    fn pick_file(&self, title: &str, chosen: Chosen);
}

/// Carry out `act` on `computer` and say how it ended through `answer`.
///
/// A link or a folder is opened only while the session is not ending, because opening one starts a
/// process. A link that is not a secure address and an act this build does not know fail.
pub fn carry_out(act: &NativeAct, session: &dyn Session, computer: &dyn Computer, answer: Answer) {
    let opened = |result: io::Result<()>| match result {
        Ok(()) => ActOutcome::Done,
        Err(error) => {
            eprintln!("ghostlight-desktop: {error}");
            ActOutcome::Failed
        }
    };
    match act {
        NativeAct::OpenUrl { url } => answer(
            if url.starts_with(SECURE_SCHEME) && session.may_start_process() {
                opened(computer.open_url(url))
            } else {
                ActOutcome::Failed
            },
        ),
        NativeAct::OpenFolder { path } => answer(if session.may_start_process() {
            opened(computer.open_folder(Path::new(path)))
        } else {
            ActOutcome::Failed
        }),
        NativeAct::PickFile { title } => computer.pick_file(
            title,
            Box::new(move |chosen| answer(picked_outcome(chosen))),
        ),
        NativeAct::Unknown => answer(ActOutcome::Failed),
    }
}

/// How a file dialog ended: the path chosen, or cancelled when none was.
fn picked_outcome(chosen: Option<PathBuf>) -> ActOutcome {
    match chosen {
        Some(path) => match path.into_os_string().into_string() {
            Ok(path) => ActOutcome::Picked { path },
            Err(_) => ActOutcome::Failed,
        },
        None => ActOutcome::Cancelled,
    }
}

/// The operating system's opener and file dialog. The dialog belongs to `parent` when the window
/// is open.
pub struct ThisComputer<'a> {
    /// The window the file dialog is parented to.
    pub parent: Option<&'a Window>,
}

impl Computer for ThisComputer<'_> {
    fn open_url(&self, url: &str) -> io::Result<()> {
        open::that_detached(url)
    }

    fn open_folder(&self, path: &Path) -> io::Result<()> {
        open::that_detached(path)
    }

    #[cfg(windows)]
    fn pick_file(&self, title: &str, chosen: Chosen) {
        let dialog = rfd::FileDialog::new().set_title(title);
        chosen(match self.parent {
            Some(parent) => dialog.set_parent(parent).pick_file(),
            None => dialog.pick_file(),
        });
    }

    #[cfg(target_os = "linux")]
    fn pick_file(&self, title: &str, chosen: Chosen) {
        use gtk::prelude::{FileChooserExt, NativeDialogExt};
        use tao::platform::unix::WindowExtUnix;

        let parent = self.parent.map(WindowExtUnix::gtk_window);
        let dialog = gtk::FileChooserNative::new(
            Some(title),
            parent,
            gtk::FileChooserAction::Open,
            None,
            None,
        );
        dialog.set_modal(parent.is_some());
        // The toolkit keeps a native dialog only while something holds it. The response handler
        // holds it and, when it answers, lets go of it together with the callback.
        let held = Rc::new(RefCell::new(Some((dialog.clone(), chosen))));
        dialog.connect_response(move |dialog, response| {
            let Some((_, chosen)) = held.borrow_mut().take() else {
                return;
            };
            chosen(
                (response == gtk::ResponseType::Accept)
                    .then(|| dialog.filename())
                    .flatten(),
            );
        });
        dialog.show();
    }
}

/// Show one notice, best effort, on a thread of its own. A failure is one line on standard
/// error and is never shown to the person.
///
/// Clicking the notice does nothing: a program that is not packaged has nothing for the system
/// to activate.
pub fn show_notice(title: String, body: String) {
    let spawned = std::thread::Builder::new()
        .name(NOTICE_THREAD_NAME.to_owned())
        .spawn(move || {
            if let Err(error) = notify(&title, &body) {
                eprintln!("ghostlight-desktop: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("ghostlight-desktop: {error}");
    }
}

/// Hand one notice to the system's notification service.
fn notify(title: &str, body: &str) -> Result<(), notify_rust::error::Error> {
    let mut notice = notify_rust::Notification::new();
    notice.appname(NOTICE_APP_NAME).summary(title).body(body);
    #[cfg(target_os = "windows")]
    notice.app_id(SHELL_APP_ID);
    notice.show().map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::session::fake::FakeSession;

    /// A computer that records what it was asked and answers as told. A computer that `defers`
    /// keeps the dialog's callback until the test finishes the dialog.
    #[derive(Default)]
    struct Recording {
        asked: RefCell<Vec<String>>,
        refuses: bool,
        chooses: Option<PathBuf>,
        defers: bool,
        pending: RefCell<Option<Chosen>>,
    }

    impl Computer for Recording {
        fn open_url(&self, url: &str) -> io::Result<()> {
            self.asked.borrow_mut().push(format!("url {url}"));
            self.answer()
        }

        fn open_folder(&self, path: &Path) -> io::Result<()> {
            self.asked
                .borrow_mut()
                .push(format!("folder {}", path.display()));
            self.answer()
        }

        fn pick_file(&self, title: &str, chosen: Chosen) {
            self.asked.borrow_mut().push(format!("pick {title}"));
            if self.defers {
                *self.pending.borrow_mut() = Some(chosen);
            } else {
                chosen(self.chooses.clone());
            }
        }
    }

    impl Recording {
        fn answer(&self) -> io::Result<()> {
            if self.refuses {
                Err(io::Error::other("refused"))
            } else {
                Ok(())
            }
        }
    }

    /// Carry out `act`, and return every outcome it was answered with so far.
    fn answered(
        act: &NativeAct,
        session: &dyn Session,
        computer: &dyn Computer,
    ) -> Rc<RefCell<Vec<ActOutcome>>> {
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&outcomes);
        carry_out(
            act,
            session,
            computer,
            Box::new(move |outcome| sink.borrow_mut().push(outcome)),
        );
        outcomes
    }

    /// The one outcome `act` is answered with straight away.
    fn outcome_of(act: &NativeAct, session: &dyn Session, computer: &dyn Computer) -> ActOutcome {
        let outcomes = answered(act, session, computer);
        let outcomes = outcomes.borrow();
        assert_eq!(outcomes.len(), 1, "answered exactly once");
        outcomes[0].clone()
    }

    fn an_address_is_lowercase_letters_slashes_and_hyphens_and_at_most_48_long() {
        for address in [
            "monitor",
            "settings",
            "settings/diagnostics",
            "settings/browser-work",
            &"a".repeat(48),
        ] {
            assert!(is_safe_address(address), "{address}");
        }
        for address in [
            "",
            &"a".repeat(49),
            "Settings",
            "settings/Diagnostics",
            "settings_diagnostics",
            "settings diagnostics",
            "monitor1",
            "#monitor",
            "monitor\"",
            "monitor\";alert(1);\"",
            "monitor'",
            "monitor\\",
            "monitor\n",
            "monitor\u{e9}",
            "monitor?x=1",
            "../monitor",
        ] {
            assert!(!is_safe_address(address), "{address:?}");
        }
    }

    fn a_window_opened_at_a_section_loads_it_after_the_hash() {
        let base = "http://127.0.0.1:41234/";
        assert_eq!(
            start_url(base, Some("settings/diagnostics")),
            "http://127.0.0.1:41234/#settings/diagnostics"
        );
        assert_eq!(start_url(base, None), base);
        assert_eq!(start_url(base, Some("bad\"address")), base);
        assert_eq!(start_url(base, Some("")), base);
    }

    /// A show moves the window to a section by navigating to its address, never by running a
    /// script, so it reaches a page that is still loading, and a window opened at a section loads it
    /// after the hash; an address that is not script-safe (lowercase letters, slashes and hyphens, at
    /// most 48 long) names no section (commit 613f01fb; ARCHITECTURE).
    #[test]
    fn a_show_moves_the_window_to_a_section_by_navigating_never_by_a_script() {
        let origin = "http://127.0.0.1:41234";
        // The first request and every later one take the same road, whatever the window shows.
        for address in ["settings/diagnostics", "settings/sharing", "monitor"] {
            assert_eq!(
                section_url(origin, address),
                Some(format!("{origin}/#{address}")),
                "{address}"
            );
        }
        assert_eq!(section_url(origin, "x\"; alert(1); \""), None);
        assert_eq!(section_url(origin, ""), None);
        assert_eq!(
            section_url(origin, "settings/sharing"),
            Some(start_url(&format!("{origin}/"), Some("settings/sharing"))),
            "a window opened at a section and one moved to it load the same address"
        );

        an_address_is_lowercase_letters_slashes_and_hyphens_and_at_most_48_long();
        a_window_opened_at_a_section_loads_it_after_the_hash();
    }

    fn a_link_and_a_folder_are_opened_and_reported_done() {
        let computer = Recording::default();
        let session = FakeSession::new();
        let link = NativeAct::OpenUrl {
            url: "https://example.com/a".to_owned(),
        };
        let folder = NativeAct::OpenFolder {
            path: "logs".to_owned(),
        };
        assert_eq!(outcome_of(&link, &session, &computer), ActOutcome::Done);
        assert_eq!(outcome_of(&folder, &session, &computer), ActOutcome::Done);
        assert_eq!(
            *computer.asked.borrow(),
            ["url https://example.com/a", "folder logs"]
        );
    }

    fn an_open_the_computer_refuses_is_reported_failed() {
        let computer = Recording {
            refuses: true,
            ..Recording::default()
        };
        let session = FakeSession::new();
        for act in [
            NativeAct::OpenUrl {
                url: "https://example.com".to_owned(),
            },
            NativeAct::OpenFolder {
                path: "logs".to_owned(),
            },
        ] {
            assert_eq!(outcome_of(&act, &session, &computer), ActOutcome::Failed);
        }
    }

    /// The shell starts a process only to open a secure link or a folder, reported done, or failed
    /// when the computer refuses; a link that is not secure, an act it does not know, and anything
    /// once the session is ending start nothing and fail (ACCEPTANCE, desktop 1 and 11).
    #[test]
    fn the_shell_opens_only_a_secure_link_or_a_folder_while_the_session_lives() {
        let computer = Recording::default();
        let session = FakeSession::ending();
        for act in [
            NativeAct::OpenUrl {
                url: "https://example.com".to_owned(),
            },
            NativeAct::OpenFolder {
                path: "logs".to_owned(),
            },
        ] {
            assert_eq!(outcome_of(&act, &session, &computer), ActOutcome::Failed);
        }
        assert!(computer.asked.borrow().is_empty(), "nothing was started");

        a_link_and_a_folder_are_opened_and_reported_done();
        an_open_the_computer_refuses_is_reported_failed();
        a_link_that_is_not_secure_is_never_opened();
        an_act_this_build_does_not_know_fails();
    }

    fn a_link_that_is_not_secure_is_never_opened() {
        let computer = Recording::default();
        let session = FakeSession::new();
        for url in [
            "http://example.com",
            "file:///etc/passwd",
            "ms-settings:",
            "calc.exe",
            "",
        ] {
            let act = NativeAct::OpenUrl {
                url: url.to_owned(),
            };
            assert_eq!(outcome_of(&act, &session, &computer), ActOutcome::Failed);
        }
        assert!(computer.asked.borrow().is_empty());
    }

    fn a_picked_file_is_reported_with_its_path_and_a_closed_dialog_is_cancelled() {
        let session = FakeSession::new();
        let act = NativeAct::PickFile {
            title: "Choose".to_owned(),
        };
        let picks = Recording {
            chooses: Some(PathBuf::from("chosen.json")),
            ..Recording::default()
        };
        assert_eq!(
            outcome_of(&act, &session, &picks),
            ActOutcome::Picked {
                path: "chosen.json".to_owned()
            }
        );
        assert_eq!(*picks.asked.borrow(), ["pick Choose"]);
        assert_eq!(
            outcome_of(&act, &session, &Recording::default()),
            ActOutcome::Cancelled
        );
    }

    /// A file pick is answered from the dialog's own callback, with the picked path or as
    /// cancelled, so the event loop never blocks while the person decides (commit ebbbcf49).
    #[test]
    fn a_dialog_that_is_still_open_leaves_the_act_unanswered_until_the_person_decides() {
        let session = FakeSession::new();
        let act = NativeAct::PickFile {
            title: "Choose".to_owned(),
        };
        let computer = Recording {
            defers: true,
            ..Recording::default()
        };
        let outcomes = answered(&act, &session, &computer);
        assert!(
            outcomes.borrow().is_empty(),
            "the call returned with the dialog still open"
        );
        let chosen = computer
            .pending
            .borrow_mut()
            .take()
            .expect("dialog is open");
        chosen(Some(PathBuf::from("later.json")));
        assert_eq!(
            *outcomes.borrow(),
            [ActOutcome::Picked {
                path: "later.json".to_owned()
            }]
        );

        a_picked_file_is_reported_with_its_path_and_a_closed_dialog_is_cancelled();
    }

    fn an_act_this_build_does_not_know_fails() {
        assert_eq!(
            outcome_of(
                &NativeAct::Unknown,
                &FakeSession::new(),
                &Recording::default()
            ),
            ActOutcome::Failed
        );
    }
}
