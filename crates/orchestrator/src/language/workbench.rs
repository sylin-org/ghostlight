//! The words the workbench server says: who can see a view, why an act is not available here, and
//! how a refused request reads.
//!
//! The page asks the service what context it is in and renders what it is told. Every sentence a
//! request can end in lives here, so the server routes and decides and never words anything
//! itself (ADR-0199, amendment of 2026-10-15). It also holds the lines `status` and `doctor` print
//! about the service, the workbench and anything an older installation left running.

use serde::{Deserialize, Serialize};

/// The word for the context that carries the right key: the Ghostlight window.
pub const WORD_CONTROL: &str = "Full control";

/// The word for the context of a browser tab while watching is on.
pub const WORD_WATCH: &str = "Watching";

/// The word for the context of a browser tab while watching is off.
pub const WORD_CLOSED: &str = "Watching is off";

/// What the window says while other screens can watch.
pub const SENTENCE_CONTROL_WATCHED: &str =
    "Others on this computer can watch this view in a browser.";

/// What the window says while only the window shows Ghostlight.
pub const SENTENCE_CONTROL_PRIVATE: &str = "Watching is off: only this window shows Ghostlight.";

/// What a watching tab says about itself.
pub const SENTENCE_WATCH: &str =
    "Anyone on this computer can open this view. Changes are made in the Ghostlight window.";

/// Why a view shows no data: watching is off.
pub const WATCHING_OFF: &str =
    "Watching is off on this computer. Turn it on in the Ghostlight window.";

/// Why an act is not available in a view that only watches.
pub const CHANGE_IN_WINDOW: &str = "Change this in the Ghostlight window.";

/// A request carried a key that is not this service start's key.
pub const WRONG_KEY: &str = "This window's key is out of date. Reopen the Ghostlight window.";

/// A view asked for more than it may in a short time.
pub const TOO_MANY: &str = "Too many requests from this view. Try again in a moment.";

/// The rules on disk changed after the draft was made.
pub const STALE_POLICY: &str = "Your rules changed in another window; your draft is kept.";

/// A client's setup changed after the view was drawn.
pub const STALE_HARNESS: &str = "This client changed since this view was drawn. Look again.";

/// A request the server could not read.
pub const BAD_REQUEST: &str = "Ghostlight could not read that request.";

/// A request body over the route's size.
pub const TOO_LARGE: &str = "That request is too large.";

/// A request whose body is not JSON.
pub const UNSUPPORTED_TYPE: &str = "Ghostlight reads requests sent as JSON.";

/// A request that came from another site.
pub const CROSS_SITE: &str = "That request came from another site, so Ghostlight refused it.";

/// An address that is not served.
pub const NOT_FOUND: &str = "There is nothing at this address.";

/// An address that does not take this kind of request.
pub const METHOD_NOT_ALLOWED: &str = "This address does not take that kind of request.";

/// A request addressed to a host name this view is not served under.
pub const MISDIRECTED: &str = "This view is served only at its own address.";

/// The license shown on the About page.
pub const PRODUCT_LICENSE: &str = "Apache-2.0 OR MIT";

/// Confirmation after copying a harness's MCP command.
pub const COPIED_COMMAND: &str = "MCP command copied.";

/// Confirmation after copying a harness's manual setup.
pub const COPIED_SETUP: &str = "Manual setup copied.";

/// Why the runtime is held, when the pause came from the Ghostlight window.
pub const PAUSED_IN_WINDOW: &str = "Paused in the Ghostlight window.";

/// Why the runtime is held, when the pause came from a screen that is watching.
pub const PAUSED_FROM_WATCHER: &str = "Paused from a screen that is watching.";

/// Why the runtime is held, when the pause came from the tray.
pub const PAUSED_FROM_TRAY: &str = "Paused from the tray.";

/// Why the runtime is held, when the pause came from the browser toolbar.
pub const PAUSED_FROM_BROWSER: &str = "Paused from the browser toolbar.";

/// What a screen is told when every place for a live view of its kind is taken.
pub const TOO_MANY_SCREENS: &str = "Too many screens are watching. Close one and try again.";

/// The hover text of the tray icon.
pub const TRAY_TOOLTIP: &str = "Ghostlight";

/// The tray entry that brings the Ghostlight window forward.
pub const TRAY_OPEN: &str = "Open Ghostlight";

/// The tray entry that pauses the agents' browser work.
pub const TRAY_HOLD: &str = "Pause browser work";

/// The tray entry that lets the agents' browser work continue.
pub const TRAY_RESUME: &str = "Resume browser work";

/// The tray entry that quits Ghostlight.
pub const TRAY_QUIT: &str = "Quit Ghostlight";

/// Why the diagnostics folder cannot be shown: diagnostics are off.
pub const DIAGNOSTICS_OFF: &str = "Diagnostics are off; turn them on first.";

/// A link the service would not open because it is not a secure address.
pub const LINK_NOT_SECURE: &str = "Ghostlight opens only secure links.";

/// The title of the dialog where a person points at a client's application or settings file.
pub const LOCATE_DIALOG_TITLE: &str = "Locate the harness application or settings file";

/// The label of the Controls row for agents' browser work.
pub const CONTROLS_RUNTIME_LABEL: &str = "Browser work";

/// The state word of browser work that agents may ask for.
pub const CONTROLS_RUNTIME_ACTIVE: &str = "Running";

/// The state word of browser work that is paused.
pub const CONTROLS_RUNTIME_HELD: &str = "Paused";

/// The state word of browser work whose session has ended.
pub const CONTROLS_RUNTIME_ENDED: &str = "Session ended";

/// The label of the Controls row for "Others can watch".
pub const CONTROLS_VIEWING_LABEL: &str = "Others can watch";

/// The state word of "Others can watch" while it is on.
pub const CONTROLS_VIEWING_ON: &str = "On -- anyone on this computer can open this view";

/// The state word of "Others can watch" while it is off.
pub const CONTROLS_VIEWING_OFF: &str = "Off -- only this window shows Ghostlight";

/// The label of the Controls row for process diagnostics.
pub const CONTROLS_DIAGNOSTICS_LABEL: &str = "Process diagnostics";

/// The state word of process diagnostics while they are off.
pub const CONTROLS_DIAGNOSTICS_OFF: &str = "Off";

/// The state word of process diagnostics while they are on, naming what turned them on.
#[must_use]
pub fn controls_diagnostics_on(layer: &str) -> String {
    format!("On ({layer})")
}

/// The label of the Controls row for the notification test.
pub const CONTROLS_NOTIFICATIONS_LABEL: &str = "Notifications";

/// What the notification test does.
pub const CONTROLS_NOTIFICATIONS_WORD: &str =
    "Send one local checkup notification. Nothing leaves this device.";

/// Why watching cannot be changed: no watch view is being served.
pub const WATCHING_NOT_SERVED: &str = "Ghostlight is not serving a watch view on this computer.";

/// Why watching was not changed: the setting could not be saved.
pub const WATCHING_NOT_SAVED: &str = "Ghostlight could not save that setting. Try again.";

/// How many of each kind of connection the service has right now.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConnectionCounts {
    /// Connections whose channel is MCP, across every session.
    pub mcp_clients: usize,
    /// Connections whose channel is the command line, across every session.
    pub command_line: usize,
    /// Connected browser instances.
    pub browsers: usize,
}

/// The one sentence the window, `status` and `doctor` say about the service.
///
/// The service is zero or one, so this counts connections and never services. `None` means the
/// service is not running.
#[must_use]
pub fn service_line(counts: Option<ConnectionCounts>) -> String {
    let Some(counts) = counts else {
        return "Ghostlight service is not running.".to_owned();
    };
    let mut parts = Vec::new();
    for (count, one, many) in [
        (counts.mcp_clients, "MCP client", "MCP clients"),
        (
            counts.command_line,
            "command-line session",
            "command-line sessions",
        ),
        (counts.browsers, "browser", "browsers"),
    ] {
        match count {
            0 => {}
            1 => parts.push(format!("1 {one}")),
            _ => parts.push(format!("{count} {many}")),
        }
    }
    let connected = match parts.split_last() {
        None => return "Ghostlight service is up; nothing is connected yet.".to_owned(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    };
    format!("Ghostlight service is up; {connected} connected.")
}

/// What `status` says about a service that answered but did not report its connections.
pub const SERVICE_UP: &str = "Ghostlight service is up.";

/// What `status` adds to "not running" when nothing needs doing: the service starts by itself.
pub const SERVICE_STARTS_ON_DEMAND: &str = "It starts when a browser or an MCP client connects.";

/// The line that says the service is not running, with how it starts when `idle_is_ready`.
#[must_use]
pub fn service_not_running_line(idle_is_ready: bool) -> String {
    let line = service_line(None);
    if idle_is_ready {
        format!("{line} {SERVICE_STARTS_ON_DEMAND}")
    } else {
        line
    }
}

/// The line that says where the workbench is and whether others on this computer can watch it.
///
/// `usual_port` is false when the usual port was taken and the service chose another.
#[must_use]
pub fn workbench_line(url: &str, watching: bool, usual_port: bool) -> String {
    let who = if watching {
        "others on this computer can watch"
    } else {
        "watching is off"
    };
    let port = if usual_port {
        ""
    } else {
        " (the usual port was taken)"
    };
    format!("Workbench: {url} -- {who}{port}")
}

/// The line `doctor` prints for the endpoint the service listens on.
#[must_use]
pub fn endpoint_line(endpoint: &str) -> String {
    format!("Endpoint: {endpoint}")
}

/// The line `status` and `doctor` print when the installation in `bin` is a development build.
pub const DEVELOPMENT_CHANNEL_LINE: &str = "Channel: development";

/// The line that names something from an older Ghostlight installation that is still running.
#[must_use]
pub fn stray_line(image: &str) -> String {
    format!(
        "Something from an older Ghostlight installation is still running: {image}. Close it, or run ghostlight doctor --fix."
    )
}

/// The line `doctor --fix` prints for a stray it stopped.
#[must_use]
pub fn stray_stopped_line(image: &str) -> String {
    format!("Stopped {image}.")
}

/// The line `doctor --fix` prints for a stray that would not stop.
#[must_use]
pub fn stray_not_stopped_line(image: &str) -> String {
    format!("Could not stop {image}. Close it yourself.")
}

/// A search that is longer than the search box allows.
#[must_use]
pub fn search_too_long(limit: usize) -> String {
    format!("Search is limited to {limit} characters.")
}

/// A search that holds a character no search needs.
pub const SEARCH_CONTROL_CHARACTER: &str = "Search contains an unsupported control character.";

/// A policy document with nothing in it.
pub const POLICY_EMPTY: &str = "A policy document cannot be empty.";

/// A policy document longer than the window sends.
#[must_use]
pub fn policy_too_large(limit: usize) -> String {
    format!("A policy document is limited to {limit} bytes.")
}

/// How the policy page counts the sites marked never-touch for a view that may not name them.
#[must_use]
pub fn never_touch_count(count: usize) -> String {
    if count == 1 {
        "1 site is marked never-touch by policy.".to_owned()
    } else {
        format!("{count} sites are marked never-touch by policy.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sentence_is_plain_ascii_and_ends_a_sentence() {
        for sentence in [
            SENTENCE_CONTROL_WATCHED,
            SENTENCE_CONTROL_PRIVATE,
            SENTENCE_WATCH,
            WATCHING_OFF,
            CHANGE_IN_WINDOW,
            WRONG_KEY,
            TOO_MANY,
            STALE_POLICY,
            STALE_HARNESS,
            BAD_REQUEST,
            TOO_LARGE,
            UNSUPPORTED_TYPE,
            CROSS_SITE,
            NOT_FOUND,
            METHOD_NOT_ALLOWED,
            MISDIRECTED,
            PAUSED_IN_WINDOW,
            PAUSED_FROM_WATCHER,
            PAUSED_FROM_TRAY,
            PAUSED_FROM_BROWSER,
            TOO_MANY_SCREENS,
            COPIED_COMMAND,
            COPIED_SETUP,
            SEARCH_CONTROL_CHARACTER,
            POLICY_EMPTY,
            DIAGNOSTICS_OFF,
            LINK_NOT_SECURE,
            CONTROLS_NOTIFICATIONS_WORD,
            WATCHING_NOT_SERVED,
            WATCHING_NOT_SAVED,
            SERVICE_UP,
            SERVICE_STARTS_ON_DEMAND,
        ] {
            assert!(sentence.is_ascii(), "{sentence}");
            assert!(sentence.ends_with('.'), "{sentence}");
        }
    }

    #[test]
    fn the_never_touch_count_reads_in_the_singular_and_the_plural() {
        assert_eq!(
            never_touch_count(1),
            "1 site is marked never-touch by policy."
        );
        assert_eq!(
            never_touch_count(3),
            "3 sites are marked never-touch by policy."
        );
    }
    fn counts(
        mcp_clients: usize,
        command_line: usize,
        browsers: usize,
    ) -> Option<ConnectionCounts> {
        Some(ConnectionCounts {
            mcp_clients,
            command_line,
            browsers,
        })
    }

    #[test]
    fn the_workbench_line_says_who_can_watch_and_names_a_taken_port() {
        let url = "http://127.0.0.1:47600/";
        assert_eq!(
            workbench_line(url, true, true),
            "Workbench: http://127.0.0.1:47600/ -- others on this computer can watch"
        );
        assert_eq!(
            workbench_line(url, false, true),
            "Workbench: http://127.0.0.1:47600/ -- watching is off"
        );
        assert_eq!(
            workbench_line(url, true, false),
            "Workbench: http://127.0.0.1:47600/ -- others on this computer can watch (the usual port was taken)"
        );
    }

    #[test]
    fn an_idle_service_says_how_it_starts() {
        assert_eq!(
            service_not_running_line(false),
            "Ghostlight service is not running."
        );
        assert_eq!(
            service_not_running_line(true),
            "Ghostlight service is not running. It starts when a browser or an MCP client connects."
        );
    }

    #[test]
    fn the_stray_lines_name_the_image_and_the_fix() {
        assert_eq!(
            stray_line("C:/gl/bin/old.exe"),
            "Something from an older Ghostlight installation is still running: C:/gl/bin/old.exe. Close it, or run ghostlight doctor --fix."
        );
        assert_eq!(stray_stopped_line("old.exe"), "Stopped old.exe.");
        assert_eq!(
            stray_not_stopped_line("old.exe"),
            "Could not stop old.exe. Close it yourself."
        );
    }

    #[test]
    fn a_service_that_is_not_running_says_so() {
        assert_eq!(service_line(None), "Ghostlight service is not running.");
    }

    #[test]
    fn a_service_with_nothing_connected_says_so() {
        assert_eq!(
            service_line(counts(0, 0, 0)),
            "Ghostlight service is up; nothing is connected yet."
        );
    }

    #[test]
    fn one_connection_reads_in_the_singular() {
        assert_eq!(
            service_line(counts(1, 0, 0)),
            "Ghostlight service is up; 1 MCP client connected."
        );
    }

    #[test]
    fn two_kinds_are_joined_with_and() {
        assert_eq!(
            service_line(counts(2, 0, 2)),
            "Ghostlight service is up; 2 MCP clients and 2 browsers connected."
        );
    }

    #[test]
    fn three_kinds_are_listed_and_the_last_is_joined_with_and() {
        assert_eq!(
            service_line(counts(1, 1, 1)),
            "Ghostlight service is up; 1 MCP client, 1 command-line session and 1 browser connected."
        );
    }

    #[test]
    fn a_kind_with_none_connected_is_left_out() {
        assert_eq!(
            service_line(counts(0, 0, 3)),
            "Ghostlight service is up; 3 browsers connected."
        );
        assert_eq!(
            service_line(counts(0, 2, 0)),
            "Ghostlight service is up; 2 command-line sessions connected."
        );
    }
}
