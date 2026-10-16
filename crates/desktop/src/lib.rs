//! The Ghostlight window process (ADR-0199 Decisions 1 and 7, and its amendment of 2026-10-15).
//!
//! `ghostlight-desktop` is the third executable of one build, next to the service and the
//! connectors. It owns the window, the tray, and the acts only this computer can do for the service
//! (a notice, a link or a folder opened, a file picked), and nothing else. It decides nothing and
//! opens no network port: the window renders the service's loopback workbench, and every fact it shows or
//! acts on comes from the service. The shell learns the workbench address and the bearer that
//! makes the page the window's over the relay's `desktop` stream, and hands the bearer to that
//! origin's page only.
//!
//! The shell is built on `wry` (the operating system webview), `tao` (window and event loop) and
//! `tray-icon` (tray and menu). It is not built on Tauri: Tauri brings `tokio`, `reqwest` and
//! `http`, plus its runtime, configuration, capability codegen, build step and plugins, which is a
//! larger binary and more upgrade surface for a process that must decide nothing. It is not built
//! on raw platform bindings either (WebView2 COM calls, Win32 windows and `Shell_NotifyIcon`,
//! hand-driven WebKitGTK): that is hundreds of lines of unsafe code, which the workspace forbids
//! outside its audited FFI crate. `wry`, `tao` and `tray-icon` come from the same upstream as
//! Tauri and are already locked at these exact versions in this tree, so the shell adds no
//! package to `Cargo.lock` except itself.
//!
//! The service's native acts use `notify-rust` (notices; D-Bus on Linux, a toast on Windows),
//! `open` (the system opener, through `ShellExecute` on Windows so no console window opens) and
//! `rfd` (the file dialog; GTK on Linux, the common dialog on Windows). Each is already locked at
//! an exact version, and each is built without the features that bring an async runtime, D-Bus in
//! pure Rust, or the desktop portal.
//!
//! The dependency tree holds exactly one name on the network-crate ban list of ADR-0028 and
//! ADR-0197: `http`, which `wry`'s own API types bring. The shell registers no custom scheme and
//! no IPC handler, does no I/O of its own over it and opens no socket.
//! `crates/desktop/guards/network.test.mjs` names it as the one exception, fails if it ever
//! leaves the tree, and fails if `zbus`, `ashpd`, `async-std` or any other network or async
//! runtime crate enters it.

use std::ffi::OsString;

pub mod app;
pub mod link;
pub mod machine;
pub mod native;
pub mod session;
pub mod states;
pub mod tray;
pub mod webview;
pub mod window;

/// The exit code of a run with arguments the program does not accept.
pub const EXIT_USAGE: u8 = 2;

/// The argument of a shell the service started to show its tray.
pub const TRAY_ARGUMENT: &str = "--tray";

/// The argument of a shell that reopens the window after an update.
pub const UPDATING_ARGUMENT: &str = "--updating";

/// How a shell was started. It decides what the shell may do before it first attaches to the
/// service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// A person started it, from the Start menu or a file manager. It opens the window and may
    /// start the service.
    Person,
    /// The service started it to show the tray (`--tray`). It opens no window and never starts the
    /// service, so a start that races a person's Quit cannot undo the Quit.
    Tray,
    /// The window reopening after an update (`--updating`). It opens the window on the Updating
    /// screen and never starts the service.
    Updating,
}

impl Launch {
    /// The launch the command line, without the program name, asks for, or `None` for a command
    /// line the program does not accept.
    pub fn from_arguments(args: &[OsString]) -> Option<Launch> {
        match args {
            [] => Some(Launch::Person),
            [only] if only == TRAY_ARGUMENT => Some(Launch::Tray),
            [only] if only == UPDATING_ARGUMENT => Some(Launch::Updating),
            _ => None,
        }
    }

    /// The argument that starts a shell with this launch, if it has one.
    pub fn argument(self) -> Option<&'static str> {
        match self {
            Launch::Person => None,
            Launch::Tray => Some(TRAY_ARGUMENT),
            Launch::Updating => Some(UPDATING_ARGUMENT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(args: &[&str]) -> Option<Launch> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        Launch::from_arguments(&args)
    }

    #[test]
    fn no_argument_is_a_persons_launch() {
        assert_eq!(launch(&[]), Some(Launch::Person));
    }

    #[test]
    fn the_two_arguments_the_shell_knows_are_accepted() {
        assert_eq!(launch(&["--tray"]), Some(Launch::Tray));
        assert_eq!(launch(&["--updating"]), Some(Launch::Updating));
    }

    #[test]
    fn anything_else_is_refused() {
        for args in [
            &["--help"][..],
            &[""],
            &["tray"],
            &["--TRAY"],
            &["--tray", "--updating"],
            &["--tray", "extra"],
            &["--tray="],
        ] {
            assert_eq!(launch(args), None, "{args:?}");
        }
    }

    #[test]
    fn a_launch_names_the_argument_that_makes_it() {
        for launch in [Launch::Person, Launch::Tray, Launch::Updating] {
            let args: Vec<OsString> = launch.argument().map(OsString::from).into_iter().collect();
            assert_eq!(Launch::from_arguments(&args), Some(launch));
        }
    }
}
