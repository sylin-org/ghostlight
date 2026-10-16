//! What the shell does on this computer for the service: show a notice, open a link or a folder,
//! ask for a file, and open the window at a section (ADR-0199 Decision 7 and its amendment of
//! 2026-10-15, shell milestone 2).
//!
//! The service decides what to show or open and when; the shell only carries it out. The pure
//! parts live here and are unit tested: the address check that keeps the script the window runs
//! safe, the start address of a window opened at a section, and [`carry_out`], which turns one
//! act into its result. [`ThisComputer`] is the thin glue to the operating system (the opener, the
//! file dialog) and [`show_notice`] is the notification call; both are checked live.
//!
//! The shell starts a process only while the session is not ending
//! ([`SessionState::may_start_process`]), and a link or a folder is opened without a console
//! window: the shell is a graphical program and the opener asks the shell of the system, never a
//! command interpreter.

use std::io;
use std::path::{Path, PathBuf};

use tao::window::Window;

use crate::link::{ActOutcome, NativeAct};
use crate::session::SessionState;

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

/// Whether `address` is safe to put in the script that moves the window: one to
/// [`ADDRESS_LIMIT`] characters of `a` to `z`, `/` and `-`, and nothing else.
///
/// This keeps the script safe and nothing more. Which addresses the page has is the service's to
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

/// The script that moves an open window to `address`, or `None` when the address is not safe.
pub fn move_script(address: &str) -> Option<String> {
    is_safe_address(address).then(|| format!("window.location.hash = \"{address}\";"))
}

/// What only this computer can do, as [`carry_out`] asks for it.
pub trait Computer {
    /// Open an address in the person's browser.
    fn open_url(&self, url: &str) -> io::Result<()>;

    /// Show a folder in the file manager.
    fn open_folder(&self, path: &Path) -> io::Result<()>;

    /// Ask the person for a file, or `None` when they close the dialog. It waits for them.
    fn pick_file(&self, title: &str) -> Option<PathBuf>;
}

/// Carry out `act` on `computer` and say how it ended.
///
/// A link or a folder is opened only while the session is not ending, because opening one starts a
/// process. A link that is not a secure address and an act this build does not know fail.
pub fn carry_out(act: &NativeAct, session: &SessionState, computer: &dyn Computer) -> ActOutcome {
    let opened = |result: io::Result<()>| match result {
        Ok(()) => ActOutcome::Done,
        Err(error) => {
            eprintln!("ghostlight-desktop: {error}");
            ActOutcome::Failed
        }
    };
    match act {
        NativeAct::OpenUrl { url } => {
            if url.starts_with(SECURE_SCHEME) && session.may_start_process() {
                opened(computer.open_url(url))
            } else {
                ActOutcome::Failed
            }
        }
        NativeAct::OpenFolder { path } => {
            if session.may_start_process() {
                opened(computer.open_folder(Path::new(path)))
            } else {
                ActOutcome::Failed
            }
        }
        NativeAct::PickFile { title } => match computer.pick_file(title) {
            Some(path) => match path.into_os_string().into_string() {
                Ok(path) => ActOutcome::Picked { path },
                Err(_) => ActOutcome::Failed,
            },
            None => ActOutcome::Cancelled,
        },
        NativeAct::Unknown => ActOutcome::Failed,
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

    fn pick_file(&self, title: &str) -> Option<PathBuf> {
        let dialog = rfd::FileDialog::new().set_title(title);
        match self.parent {
            Some(parent) => dialog.set_parent(parent).pick_file(),
            None => dialog.pick_file(),
        }
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

    use super::*;
    use crate::session::SessionSignal;

    /// A computer that records what it was asked and answers as told.
    #[derive(Default)]
    struct Recording {
        asked: RefCell<Vec<String>>,
        refuses: bool,
        chooses: Option<PathBuf>,
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

        fn pick_file(&self, title: &str) -> Option<PathBuf> {
            self.asked.borrow_mut().push(format!("pick {title}"));
            self.chooses.clone()
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

    fn ending() -> SessionState {
        let state = SessionState::new();
        state.on(SessionSignal::Query);
        state
    }

    #[test]
    fn the_app_id_is_the_one_the_shortcut_will_carry() {
        assert_eq!(SHELL_APP_ID, "Sylin.Ghostlight");
    }

    #[test]
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

    #[test]
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

    #[test]
    fn the_script_that_moves_the_window_holds_the_address_and_nothing_else() {
        assert_eq!(
            move_script("settings/sharing").as_deref(),
            Some("window.location.hash = \"settings/sharing\";")
        );
        assert_eq!(move_script("x\"; alert(1); \""), None);
        assert_eq!(move_script(""), None);
    }

    #[test]
    fn a_link_and_a_folder_are_opened_and_reported_done() {
        let computer = Recording::default();
        let session = SessionState::new();
        let link = NativeAct::OpenUrl {
            url: "https://example.com/a".to_owned(),
        };
        let folder = NativeAct::OpenFolder {
            path: "logs".to_owned(),
        };
        assert_eq!(carry_out(&link, &session, &computer), ActOutcome::Done);
        assert_eq!(carry_out(&folder, &session, &computer), ActOutcome::Done);
        assert_eq!(
            *computer.asked.borrow(),
            ["url https://example.com/a", "folder logs"]
        );
    }

    #[test]
    fn an_open_the_computer_refuses_is_reported_failed() {
        let computer = Recording {
            refuses: true,
            ..Recording::default()
        };
        let session = SessionState::new();
        for act in [
            NativeAct::OpenUrl {
                url: "https://example.com".to_owned(),
            },
            NativeAct::OpenFolder {
                path: "logs".to_owned(),
            },
        ] {
            assert_eq!(carry_out(&act, &session, &computer), ActOutcome::Failed);
        }
    }

    #[test]
    fn while_the_session_is_ending_nothing_is_started_and_the_act_fails() {
        let computer = Recording::default();
        let session = ending();
        for act in [
            NativeAct::OpenUrl {
                url: "https://example.com".to_owned(),
            },
            NativeAct::OpenFolder {
                path: "logs".to_owned(),
            },
        ] {
            assert_eq!(carry_out(&act, &session, &computer), ActOutcome::Failed);
        }
        assert!(computer.asked.borrow().is_empty(), "nothing was started");
    }

    #[test]
    fn a_link_that_is_not_secure_is_never_opened() {
        let computer = Recording::default();
        let session = SessionState::new();
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
            assert_eq!(carry_out(&act, &session, &computer), ActOutcome::Failed);
        }
        assert!(computer.asked.borrow().is_empty());
    }

    #[test]
    fn a_picked_file_is_reported_with_its_path_and_a_closed_dialog_is_cancelled() {
        let session = SessionState::new();
        let act = NativeAct::PickFile {
            title: "Choose".to_owned(),
        };
        let picks = Recording {
            chooses: Some(PathBuf::from("chosen.json")),
            ..Recording::default()
        };
        assert_eq!(
            carry_out(&act, &session, &picks),
            ActOutcome::Picked {
                path: "chosen.json".to_owned()
            }
        );
        assert_eq!(*picks.asked.borrow(), ["pick Choose"]);
        assert_eq!(
            carry_out(&act, &session, &Recording::default()),
            ActOutcome::Cancelled
        );
    }

    #[test]
    fn an_act_this_build_does_not_know_fails() {
        assert_eq!(
            carry_out(
                &NativeAct::Unknown,
                &SessionState::new(),
                &Recording::default()
            ),
            ActOutcome::Failed
        );
    }
}
