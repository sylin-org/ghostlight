//! The words the workbench server says: who can see a view, why an act is not available here, and
//! how a refused request reads.
//!
//! The page asks the service what context it is in and renders what it is told. Every sentence a
//! request can end in lives here, so the server routes and decides and never words anything
//! itself (ADR-0199, amendment of 2026-10-15). It also holds the checks the Status page shows, and
//! the lines `status` and `doctor` print about the service, the workbench and anything an older
//! installation left running. The window, the tray and `status` count one thing, the agents, the
//! way At a glance groups them (ADR-0199, amendment of 2026-10-18).

use serde::{Deserialize, Serialize};

use crate::events::DenialPresentation;
use crate::install::HarnessState;
use crate::language::channel_switch::ChannelsOff;
use crate::language::history::OutcomeClass;
use crate::language::readiness::Readiness;
use crate::language::setup::{WORD_BY_HAND, WORD_CONNECTED, WORD_NEEDS_UPDATE};

/// The word the Ghostlight window wears while others can watch it, the state Settings > Sharing
/// turns off. While watching is off the window wears no word.
pub const WORD_SHARED: &str = "Shared";

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

/// Why the runtime is held, when the pause came from the Ghostlight window.
pub const PAUSED_IN_WINDOW: &str = "Paused in the Ghostlight window.";

/// Why the runtime is held, when the pause came over loopback without the window's key while
/// watching is on: a watching page, or any local program, since nothing proves which.
pub const PAUSED_FROM_WATCHER: &str = "Paused from a watching page or a local program.";

/// Why the runtime is held, when the pause came from the tray.
pub const PAUSED_FROM_TRAY: &str = "Paused from the tray.";

/// Why the runtime is held, when the pause came from the browser toolbar.
pub const PAUSED_FROM_BROWSER: &str = "Paused from the browser toolbar.";

/// What a screen is told when every place for a live view of its kind is taken.
pub const TOO_MANY_SCREENS: &str = "Too many screens are watching. Close one and try again.";

/// The tray entry that brings the Ghostlight window forward.
pub const TRAY_OPEN: &str = "Open Ghostlight";

/// The tray entry that pauses the agents' browser work: the runtime act's words
/// ([`super::control::PAUSE`]), which the band, Settings and the browser toolbar use too.
pub const TRAY_HOLD: &str = super::control::PAUSE;

/// The tray entry that lets the agents' browser work continue ([`super::control::RESUME`]).
pub const TRAY_RESUME: &str = super::control::RESUME;

/// The tray entry that starts a new session once one ended ([`super::control::START_SESSION`]).
pub const TRAY_START_SESSION: &str = super::control::START_SESSION;

/// The tray entry that quits Ghostlight.
pub const TRAY_QUIT: &str = "Quit Ghostlight";

/// The title of the one notice that explains the icon, shown the first time the window is closed.
pub const TRAY_HINT_TITLE: &str = "Ghostlight is still running";

/// The body of that notice: where the icon is and how to open the window again.
///
/// Windows 11 hides a new icon under the notification area's arrow, so the notice names that place
/// and the Start menu too.
#[cfg(windows)]
pub const TRAY_HINT_BODY: &str = "Open it again from the Ghostlight icon in the notification area (under ^ if it is hidden) or from the Start menu.";

/// The body of that notice: where the icon is and how to open the window again.
#[cfg(not(windows))]
pub const TRAY_HINT_BODY: &str =
    "Open it again from the Ghostlight icon in the system tray or from Applications.";

/// Why there is, or is not, a Ghostlight window, as `status` reports it.
///
/// The service decides it (`shown` and `tray` from the shell it has, the rest from what keeps it
/// from bringing one up) and `status --json` carries the word.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowReason {
    /// The window is open.
    Shown,
    /// The window is closed and the icon is there.
    Tray,
    /// No display is reachable from the service.
    NoDisplay,
    /// The service runs where no one can see a window: a remote session or a background session.
    NoInteractiveDesktop,
    /// The window program is missing from the installation.
    NoShellFile,
    /// The computer has no web view runtime for the window.
    NoWebview,
    /// The runtime is there, but the window could not be built.
    WindowFailed,
    /// The desktop shows no tray icons, so the window program does not stay.
    NoTray,
    /// The window program started and ended again and again, so the service stopped starting it.
    DidNotStay,
}

impl WindowReason {
    /// The word `status --json` carries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shown => "shown",
            Self::Tray => "tray",
            Self::NoDisplay => "no_display",
            Self::NoInteractiveDesktop => "no_interactive_desktop",
            Self::NoShellFile => "no_shell_file",
            Self::NoWebview => "no_webview",
            Self::WindowFailed => "window_failed",
            Self::NoTray => "no_tray",
            Self::DidNotStay => "did_not_stay",
        }
    }

    /// The sentence `status` and `doctor` print for this reason, or `None` when the window is
    /// open and there is nothing to say.
    #[must_use]
    pub const fn sentence(self) -> Option<&'static str> {
        Some(match self {
            Self::Shown => return None,
            Self::Tray => WINDOW_TRAY,
            Self::NoDisplay => "No Ghostlight window: no display is reachable from the service.",
            Self::NoInteractiveDesktop => {
                "No Ghostlight window: the service is not running in a desktop session a person can see."
            }
            Self::NoShellFile => {
                "No Ghostlight window: the window program is missing from the Ghostlight folder. Install Ghostlight again to restore it."
            }
            Self::NoWebview => WINDOW_NO_WEBVIEW,
            Self::WindowFailed => {
                "No Ghostlight window: the window could not be built on this computer. Run ghostlight open to try again."
            }
            Self::NoTray => WINDOW_NO_TRAY,
            Self::DidNotStay => WINDOW_DID_NOT_STAY,
        })
    }
}

/// What the tray's tooltip says while the window cannot be built.
pub const TRAY_WINDOW_FAILS: &str = "Ghostlight cannot open its window on this computer.";

#[cfg(windows)]
const WINDOW_TRAY: &str = "The Ghostlight window is closed. Its icon is in the notification area.";
#[cfg(not(windows))]
const WINDOW_TRAY: &str = "The Ghostlight window is closed. Its icon is in the system tray.";

#[cfg(windows)]
const WINDOW_NO_WEBVIEW: &str =
    "No Ghostlight window: the Microsoft WebView2 runtime is not installed.";
#[cfg(not(windows))]
const WINDOW_NO_WEBVIEW: &str =
    "No Ghostlight window: the WebKitGTK 4.1 web view is not installed.";

#[cfg(windows)]
const WINDOW_NO_TRAY: &str =
    "No Ghostlight window: the notification area is not available to show the icon.";
#[cfg(not(windows))]
const WINDOW_NO_TRAY: &str = "No Ghostlight window: this desktop shows no tray icons. Install a tray extension (on GNOME, AppIndicator) or open Ghostlight from Applications.";

#[cfg(windows)]
const WINDOW_DID_NOT_STAY: &str = "No Ghostlight window: the window program ended again and again, so Ghostlight stopped starting it. Run ghostlight open to try again.";
#[cfg(not(windows))]
const WINDOW_DID_NOT_STAY: &str = "No Ghostlight window: the window program ended again and again, so Ghostlight stopped starting it. Run ghostlight open to try again; if it ends again, check that WebKitGTK 4.1 (libwebkit2gtk-4.1) is installed.";

/// Why the diagnostics folder cannot be shown: diagnostics are off.
pub const DIAGNOSTICS_OFF: &str = "Diagnostics are off; turn them on first.";

/// A link the service would not open because it is not a secure address.
pub const LINK_NOT_SECURE: &str = "Ghostlight opens only secure links.";

/// The title of the dialog where a person points at a client's application or settings file.
pub const LOCATE_DIALOG_TITLE: &str = "Locate the harness application or settings file";

/// The title of the notice that says Ghostlight blocked an action.
const NOTICE_BLOCKED_TITLE: &str = "Ghostlight blocked an action";

/// The body of the notice that says Ghostlight blocked an action. It names no site and no action.
const NOTICE_BLOCKED_BODY: &str = "A configured guardrail prevented browser work.";

/// The title of the notice that says Ghostlight kept a tab open that a call would have closed.
const NOTICE_KEPT_TITLE: &str = "Ghostlight kept a tab open";

/// The title and body of the notice a blocked browser job asks for, chosen by how the job was
/// blocked, the typed `presentation` the in-page notice is chosen by too (ADR-0203 Decision 5.6).
/// A kept tab says a tab was kept, with the in-page notice's reason
/// ([`super::presentation::denial_lines`]), not that a guardrail blocked work. Neither names a
/// site or an action.
#[must_use]
pub const fn blocked_notice(presentation: DenialPresentation) -> (&'static str, &'static str) {
    match presentation {
        DenialPresentation::Guardrail => (NOTICE_BLOCKED_TITLE, NOTICE_BLOCKED_BODY),
        DenialPresentation::TabKeptOpenByPolicy | DenialPresentation::TabKeptOpenBySetting => (
            NOTICE_KEPT_TITLE,
            super::presentation::denial_lines(presentation).1,
        ),
    }
}

/// The title of the notice a person asks for to check that notices arrive.
pub const NOTICE_CHECKUP_TITLE: &str = "Ghostlight checkup";

/// The body of the checkup notice.
pub const NOTICE_CHECKUP_BODY: &str = "Notifications are ready.";

/// Why a link, a folder or a file dialog did not happen on this computer.
pub const NATIVE_ACT_FAILED: &str = "This computer would not do that.";

/// The label of the Controls row for agents' browser work. Its state is worded by
/// [`super::control::state_word`].
pub const CONTROLS_RUNTIME_LABEL: &str = "Browser work";

/// The label of the Controls row for "Others can watch".
pub const CONTROLS_VIEWING_LABEL: &str = "Others can watch";

/// The state word of "Others can watch" while it is on.
pub const CONTROLS_VIEWING_ON: &str = "On: anyone on this computer can open this view";

/// The state word of "Others can watch" while it is off.
pub const CONTROLS_VIEWING_OFF: &str = "Off: only this window shows Ghostlight";

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

/// The state word of notifications, which Ghostlight always sends: the Settings rail says it.
pub const CONTROLS_NOTIFICATIONS_ON: &str = "On";

/// What the notification test does.
pub const CONTROLS_NOTIFICATIONS_WORD: &str =
    "Send one local checkup notification. Nothing leaves this device.";

/// Where the browser's own settings that Settings > This screen names are changed. The service
/// never learns their current values, so their lines say how each starts and claim no more.
pub const BROWSER_SET_IN: &str = "Set in the browser";

/// The label of the This screen line for the browser's agent activity effects, as the
/// extension's settings name them.
pub const BROWSER_EFFECTS_LABEL: &str = "Agent activity effects";

/// What the browser's agent activity effects are, and how they start.
pub const BROWSER_EFFECTS_WORD: &str =
    "The cursor, click ripples and scan-lines on a page an agent works in. They start on.";

/// The label of the This screen line for the browser's action captions, as the extension's
/// settings name them.
pub const BROWSER_CAPTIONS_LABEL: &str = "Action captions";

/// What the browser's action captions are, and how they start.
pub const BROWSER_CAPTIONS_WORD: &str =
    "A caption at the bottom of the page names each action. They start off.";

/// What the page says once Quit has been accepted, until the service ends.
pub const QUITTING: &str = "Ghostlight is quitting.";

/// Why Quit did nothing: no service is attached to end.
pub const QUIT_UNAVAILABLE: &str = "Ghostlight cannot quit from this view.";

/// Why a setup step was not carried out: the browser or profile it names is not what this computer
/// has now.
pub const SETUP_STEP_NOT_OFFERED: &str = "That setup step is no longer offered.";

/// Why a setup step started nothing: the person is signing out.
pub const SETUP_SESSION_ENDING: &str = "Ghostlight starts nothing while you sign out.";

/// Why a setup step did not start the browser.
pub const SETUP_BROWSER_NOT_STARTED: &str = "Ghostlight could not start that browser.";

/// Why watching cannot be changed: no watch view is being served.
pub const WATCHING_NOT_SERVED: &str = "Ghostlight is not serving a watch view on this computer.";

/// Why watching was not changed: the setting could not be saved.
pub const WATCHING_NOT_SAVED: &str = "Ghostlight could not save that setting. Try again.";

/// What the Browsers shelf of At a glance says while no browser is connected and Ghostlight would
/// start none on request.
pub const NO_BROWSER: &str = "No browser connected";

/// What the Browsers shelf of At a glance says while no browser is connected: the browser an
/// agent's request starts, when Ghostlight would start `starts` (its name), and otherwise
/// [`NO_BROWSER`].
#[must_use]
pub fn no_browser(starts: Option<&str>) -> String {
    starts.map_or_else(
        || NO_BROWSER.to_owned(),
        |browser| format!("Starts {browser} on request"),
    )
}

/// What is connected to the service right now: the agents a person sees, and the connections
/// diagnostics count.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConnectionCounts {
    /// Connected agents, one per agent At a glance shows: every session of one agent counts once.
    /// A service that predates the count reports none.
    #[serde(default)]
    pub agents: usize,
    /// Connections whose channel is MCP, across every session.
    pub mcp_clients: usize,
    /// Connections whose channel is the command line, across every session.
    pub command_line: usize,
    /// Connected browser instances.
    pub browsers: usize,
}

/// The one sentence the window's Status page, the tray, `status` and `doctor` say about the
/// service.
///
/// The service is zero or one, so this counts agents and browsers and never services. `None` means
/// the service is not running.
#[must_use]
pub fn service_line(counts: Option<ConnectionCounts>) -> String {
    let Some(counts) = counts else {
        return SERVICE_NOT_RUNNING.to_owned();
    };
    let mut parts = Vec::new();
    for (count, one, many) in [
        (counts.agents, "agent", "agents"),
        (counts.browsers, "browser", "browsers"),
    ] {
        match count {
            0 => {}
            1 => parts.push(format!("1 {one}")),
            _ => parts.push(format!("{count} {many}")),
        }
    }
    if parts.is_empty() {
        return "Ghostlight is running; nothing is connected yet.".to_owned();
    }
    format!("Ghostlight is running; {} connected.", parts.join(" and "))
}

/// What the tray icon says on hover: the band's word for `readiness`, then [`service_line`] for
/// `counts`, then [`NO_BROWSER`] when an agent is connected but no browser is.
#[must_use]
pub fn tray_tooltip(readiness: Readiness, counts: ConnectionCounts) -> String {
    let line = service_line(Some(counts));
    if counts.browsers == 0 && counts.agents > 0 {
        format!("{}. {line} {NO_BROWSER}.", readiness.word())
    } else {
        format!("{}. {line}", readiness.word())
    }
}

/// What `status` says when no service answers.
const SERVICE_NOT_RUNNING: &str = "Ghostlight is not running.";

/// What `status` says about a service that answered but did not report its connections.
pub const SERVICE_UP: &str = "Ghostlight is running.";

/// What `status` adds to "not running" when nothing needs doing: the service starts by itself.
pub const SERVICE_STARTS_ON_DEMAND: &str = "It starts when a browser or an agent connects.";

/// The label of the Status check that says whether Ghostlight itself runs.
pub const STATUS_SERVICE_LABEL: &str = "Ghostlight";

/// What that check says while the service answers, which it does whenever the page has data.
pub const STATUS_SERVICE_RUNNING: &str = "Running and accepting agents.";

/// The label of the Status check that says whether a browser is connected.
pub const STATUS_BROWSER_LABEL: &str = "Browser";

/// What that check says while no browser is connected and Ghostlight would start none.
pub const STATUS_BROWSER_WAITING: &str = "Waiting for Ghostlight in Browser.";

/// What that check says while `browsers` (their names, one per connected browser) are connected.
/// A name is said once however many of that browser are connected.
#[must_use]
pub fn status_browser_connected(browsers: &[&str]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for name in browsers {
        if !names.contains(name) {
            names.push(name);
        }
    }
    let joined = match names.split_last() {
        None => return STATUS_BROWSER_WAITING.to_owned(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    };
    format!("Ghostlight in Browser is connected in {joined}.")
}

/// The label of the Status check that says whether the rules hold.
pub const STATUS_RULES_LABEL: &str = "Rules";

/// What that check says while a policy that applies cannot be read, so every request is refused.
pub const STATUS_RULES_INVALID: &str = "A policy that applies here is not valid, so Ghostlight refuses all browser work. A policy file still in schema 3 needs ghostlight policy migrate <file>.";

/// What that check says while the latest edit of the person's own rules is not valid.
pub const STATUS_RULES_EDIT_INVALID: &str =
    "Your latest rules are not valid. The last valid version stays in force.";

/// What that check says while every policy that applies is valid: whose rules apply, or that none
/// do, the same open case the Policy page names.
#[must_use]
pub const fn status_rules_valid(own: bool, organization: bool) -> &'static str {
    match (own, organization) {
        (false, false) => "No rules apply.",
        (true, _) => "Your rules are valid.",
        (false, true) => "Your organization's rules are valid.",
    }
}

/// What that check says while every policy that applies is valid: [`status_rules_valid`], then
/// the channels those rules turn off, in the words the Policy page's headline uses.
#[must_use]
pub fn status_rules_in_force(
    own: bool,
    organization: bool,
    channels_off: ChannelsOff<'_>,
) -> String {
    let valid = status_rules_valid(own, organization);
    match channels_off.policy_line() {
        Some(line) => format!("{valid} {line}"),
        None => valid.to_owned(),
    }
}

/// The label of the Status check that says whether history is saving.
pub const STATUS_HISTORY_LABEL: &str = "History";

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
        "Something from an older Ghostlight installation is still running: {image}. Close it, or run ghostlight install."
    )
}

/// The line `doctor` prints for a 1.x version directory still in `bin`.
#[must_use]
pub fn version_directory_line(directory: &str) -> String {
    format!("An older Ghostlight left {directory} in bin. Run ghostlight install to remove it.")
}

/// The line `ghostlight install`, a deploy and `doctor --fix` print for a stray that would not
/// stop.
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

/// A page of the window as the search palette offers it: the tab's own name, and what a person
/// finds there. The workbench's search pairs each with the page's address, which is also the
/// hit's identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchPage {
    /// The page's name, as its tab says it.
    pub title: &'static str,
    /// What the page shows, in a person's words.
    pub detail: &'static str,
}

/// At a glance, as the palette offers it.
pub const SEARCH_GLANCE: SearchPage = SearchPage {
    title: "At a glance",
    detail: "Whether Ghostlight is ready, what is running, and what just happened",
};

/// Integrations, as the palette offers it.
pub const SEARCH_INTEGRATIONS: SearchPage = SearchPage {
    title: "Integrations",
    detail: "The agents on this computer and whether they are connected",
};

/// Status, as the palette offers it.
pub const SEARCH_STATUS: SearchPage = SearchPage {
    title: "Status",
    detail: "Whether Ghostlight, the browser, the rules and history are working",
};

/// Policy, as the palette offers it.
pub const SEARCH_POLICY: SearchPage = SearchPage {
    title: "Policy",
    detail: "The rules for what agents may do, and on which sites",
};

/// Settings, as the palette offers it.
pub const SEARCH_SETTINGS: SearchPage = SearchPage {
    title: "Settings",
    detail: "This screen, sharing, browser work, notifications and diagnostics",
};

/// The start every browser tool's name shares, which the window leaves out when it names a tool
/// (`SHARED_TOOL_PREFIX` in `ui/lib/words.js`).
const SHARED_TOOL_PREFIX: &str = "browser_";

/// A catalog tool as the window names it, the way its activity list does (`toolWords` in
/// `ui/lib/words.js`): the shared start left out and the rest in words, so `browser_fill_form`
/// reads "fill form" and `policy_explain` reads "policy explain".
#[must_use]
pub fn tool_words(tool: &str) -> String {
    tool.strip_prefix(SHARED_TOOL_PREFIX)
        .unwrap_or(tool)
        .replace('_', " ")
}

/// The title of a recorded action in the search palette: the tool in words, then the sentence its
/// row says, as in "navigate: Opened sylin.org.". A record that kept no sentence is its tool alone.
#[must_use]
pub fn search_action_title(tool: &str, sentence: &str) -> String {
    let tool = tool_words(tool);
    if sentence.is_empty() {
        tool
    } else {
        format!("{tool}: {sentence}")
    }
}

/// The parts of a search hit's second line, joined the way a row joins its agent and age: the
/// parts it has, in order, between commas.
fn joined(parts: &[&str]) -> String {
    parts
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The second line of a recorded action in the search palette: the agent that drove it and the
/// browser that did it. The window adds how long ago it settled, and the outcome when it did not
/// succeed, in its own words.
#[must_use]
pub fn search_action_detail(agent: &str, browser: &str) -> String {
    joined(&[agent, browser])
}

/// The word a running card says for work that runs now (`PHASE_WORD` in `ui/lib/words.js`).
pub const SEARCH_RUNNING: &str = "running";

/// The word for accepted work waiting its turn.
pub const SEARCH_WAITING: &str = "waiting";

/// The word for accepted work a pause holds.
pub const SEARCH_HELD: &str = "held";

/// The word for work that was refused before its record arrived.
pub const SEARCH_BLOCKED: &str = "blocked";

/// The word for work that finished before its record arrived.
pub const SEARCH_FINISHED: &str = "finished";

/// The word a recorded action is found by for how it ended: the outcome word its row wears, in
/// lower case (`OUTCOME_WORD` in `ui/lib/words.js`), or `None` for an outcome its row says nothing
/// about, a success or a cancellation.
#[must_use]
pub const fn search_outcome(class: OutcomeClass) -> Option<&'static str> {
    match class {
        OutcomeClass::Refused => Some(SEARCH_BLOCKED),
        OutcomeClass::Kept => Some("kept"),
        OutcomeClass::NeedsYou => Some("needs you"),
        OutcomeClass::Failed => Some("failed"),
        OutcomeClass::Unknown => Some("unknown"),
        OutcomeClass::Succeeded | OutcomeClass::Cancelled => None,
    }
}

/// The second line of an action in flight in the search palette: the agent that asked for it and
/// the word for where it stands (`phase`, one of the `SEARCH_` phase words), as in
/// "Claude Code, running".
#[must_use]
pub fn search_operation_detail(agent: &str, phase: &str) -> String {
    joined(&[agent, phase])
}

/// How many tabs a browser or a session holds, as At a glance counts them (`tabsCount`).
fn tabs_count(count: usize) -> String {
    match count {
        0 => "no tabs".to_owned(),
        1 => "1 tab".to_owned(),
        _ => format!("{count} tabs"),
    }
}

/// What an idle session is, on its line (`GLANCE.idle`).
const SESSION_IDLE: &str = "idle";

/// What a session running work now is, on its line (`GLANCE.working`).
const SESSION_WORKING: &str = "working";

/// What a session from the command line adds to its line (`GLANCE.fromCommandLine`).
const SESSION_FROM_COMMAND_LINE: &str = "from the command line";

/// One session of an agent in words, the line its At a glance card shows (`sessionLine` in
/// `ui/lib/words.js`): its tabs and the browser it works in, whether it is running work now, and
/// the command line when that is where it came from, as in "2 tabs in Google Chrome, idle".
#[must_use]
pub fn session_line(
    tabs: usize,
    browser: Option<&str>,
    working: bool,
    command_line: bool,
) -> String {
    let counted = tabs_count(tabs);
    let place = match browser.filter(|name| !name.is_empty()) {
        Some(browser) if tabs > 0 => format!("{counted} in {browser}"),
        Some(browser) => format!("In {browser}, {counted}"),
        None => {
            let mut letters = counted.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + letters.as_str()
            })
        }
    };
    let state = if working {
        SESSION_WORKING
    } else {
        SESSION_IDLE
    };
    let from = if command_line {
        SESSION_FROM_COMMAND_LINE
    } else {
        ""
    };
    joined(&[&place, state, from])
}

/// The second line of a connected browser in the search palette, as in "Connected, 2 tabs".
#[must_use]
pub fn search_browser_detail(tabs: usize) -> String {
    format!("{WORD_CONNECTED}, {}", tabs_count(tabs))
}

/// The word for an agent that is not installed here (`AGENT_STATE_WORDS.not_detected`).
const WORD_NOT_HERE: &str = "Not on this computer";

/// The word for an agent installed here that has no Ghostlight entry
/// (`AGENT_STATE_WORDS.available`).
const WORD_FOUND: &str = "Found, not connected";

/// The word for an agent whose entry needs a person (`AGENT_STATE_WORDS.needs_attention`).
const WORD_NEEDS_ATTENTION: &str = "Needs your attention";

/// Where an agent stands, in the word its Integrations entry wears (`agentStanding` and
/// `AGENT_STATE_WORDS` in `ui/lib/words.js`). Ghostlight never reads a copy-only agent's file, so
/// such an agent is connected while one of its sessions is live (`in_use`), and one found here
/// that is not is set up by hand.
#[must_use]
pub const fn harness_state_word(
    state: HarnessState,
    copy_only: bool,
    in_use: bool,
) -> &'static str {
    match state {
        _ if copy_only && in_use => WORD_CONNECTED,
        HarnessState::Available if copy_only => WORD_BY_HAND,
        HarnessState::Installed => WORD_CONNECTED,
        HarnessState::NotDetected => WORD_NOT_HERE,
        HarnessState::Available => WORD_FOUND,
        HarnessState::Updatable => WORD_NEEDS_UPDATE,
        HarnessState::NeedsAttention => WORD_NEEDS_ATTENTION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::adapter::RuntimeControlState;
    use crate::language::control;
    use crate::language::setup::{COPY_SETUP_LABEL, STEP_NAMES, UPDATE_LABEL};

    /// The window's own vocabulary, which the search palette's words must say the same way.
    const PAGE_WORDS: &str = include_str!("../../ui/lib/words.js");

    /// Where [`PAGE_WORDS`] lives, as a failure names it.
    const PAGE_WORDS_FILE: &str = "crates/orchestrator/ui/lib/words.js";

    /// The browser toolbar's popup, which says the pause entries and the runtime states in the
    /// words of the tray.
    const TOOLBAR_WORDS: &str = "extension/lib/popup-view.js";

    /// The extension's own setup view, which shows the same three steps as the window.
    const EXTENSION_SETUP_VIEW: &str = "extension/lib/setup-view.js";

    /// Contract between the service and the page, the toolbar and the extension's setup view: a
    /// word the service says that the window or the browser also spells is spelled the same in
    /// both, so search finds a row by the words its card shows (desktop gate 3, n-d-04), the
    /// tray, the band and the toolbar say one word per act (ADR-0199 amendment of 2026-10-18),
    /// and the window and the toolbar say the runtime words the tray says: its acts and, as its
    /// tooltip's state word, the state Pause or End session left in force, all from
    /// `language::control` (ADR-0203 S5,
    /// `the_window_and_the_popup_say_the_runtime_words_the_tray_says`),
    /// the agent rows' Update and Copy setup acts read as the setup view names them (80c6f54a),
    /// and the setup page's steps are the service's three (INTENT: every surface that shows the
    /// install journey shows the same three steps).
    #[test]
    fn the_words_the_service_says_are_the_words_the_window_and_the_toolbar_show() {
        let read = |path| {
            std::fs::read_to_string(crate::language::self_check::golden::repository_path(path))
                .unwrap_or_else(|error| panic!("{path} is readable: {error}"))
        };
        let toolbar = read(TOOLBAR_WORDS);
        let setup_view = read(EXTENSION_SETUP_VIEW);
        let page = (PAGE_WORDS_FILE, PAGE_WORDS);
        let mut spelled = vec![
            (page, format!("update: \"{UPDATE_LABEL}\"")),
            (page, format!("copy: \"{COPY_SETUP_LABEL}\"")),
            (
                (EXTENSION_SETUP_VIEW, setup_view.as_str()),
                format!(
                    "steps: Object.freeze([{}])",
                    STEP_NAMES.map(|name| format!("\"{name}\"")).join(", ")
                ),
            ),
            (
                page,
                format!("SHARED_TOOL_PREFIX = \"{SHARED_TOOL_PREFIX}\""),
            ),
            (page, format!("working: \"{SESSION_WORKING}\"")),
            (page, format!("idle: \"{SESSION_IDLE}\"")),
            (
                page,
                format!("fromCommandLine: \"{SESSION_FROM_COMMAND_LINE}\""),
            ),
            (
                page,
                format!(
                    "by_hand: \"{}\"",
                    harness_state_word(HarnessState::Available, true, false)
                ),
            ),
        ];
        for (key, state) in [
            ("installed", HarnessState::Installed),
            ("not_detected", HarnessState::NotDetected),
            ("available", HarnessState::Available),
            ("updatable", HarnessState::Updatable),
            ("needs_attention", HarnessState::NeedsAttention),
        ] {
            let word = harness_state_word(state, false, false);
            spelled.push((page, format!("{key}: \"{word}\"")));
        }
        let popup = (TOOLBAR_WORDS, toolbar.as_str());
        for words in [page, popup] {
            spelled.push((words, format!("pause: \"{}\"", control::PAUSE)));
            spelled.push((words, format!("resume: \"{}\"", control::RESUME)));
            spelled.push((
                words,
                format!("startSession: \"{}\"", control::START_SESSION),
            ));
        }
        for (state, readiness) in [
            (RuntimeControlState::Held, Readiness::Paused),
            (RuntimeControlState::Ended, Readiness::SessionEnded),
        ] {
            let word = control::state_word(state);
            spelled.push((popup, format!("title: \"{word}\"")));
            assert_eq!(readiness.word(), word, "the tray's tooltip says {word}");
        }
        for ((file, text), entry) in spelled {
            assert!(text.contains(&entry), "{file} does not spell {entry}");
        }
    }
}
