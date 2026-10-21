//! The words of the install journey: what the window's setup view says at each stage, and what
//! `ghostlight install` and `ghostlight uninstall` print when they finish.
//!
//! One pure module with no I/O, shaped like [`super::readiness`]: a closed vocabulary, a facts
//! struct, a pure resolver and tests. The workbench reads the machine and calls [`compose`]; the
//! install reads the same facts and calls [`install_lines`]. Neither words anything itself, so the
//! window and the terminal cannot disagree about what comes next (ADR-0200 Decisions 5, 8, 9, 10
//! and 12).
//!
//! The setup view has four stages, resolved in this order by [`stage`]: Done (setup is complete
//! and finished in this run), Retired (complete earlier, so the window shows the workbench),
//! Computer (the person is in an app container and must finish on the computer itself), Browser
//! (no browser is connected), and otherwise Agent. The view is data: every sentence, the three
//! steps, the code panels the page copies from, and the rows with their actions. The pending words
//! a row action shows after a click belong to the action and live in the page; the service keeps
//! no pending state.
//!
//! JQ8 is "launch": Add and Open make Ghostlight start the named browser, so both are service
//! actions ([`SetupAction::AddInBrowser`], [`SetupAction::OpenBrowser`]). JQ9 is "yes": Edge gets
//! Add from the Chrome Web Store like Chrome, Brave and Chromium, so every native browser gets one.
//!
//! # What the install prints
//!
//! A real install prints [`install_lines`] and nothing else about the happy path. Lines that need
//! a person print beside it, always: errors, registrations that need attention or are owned
//! elsewhere ([`super::deployment::client_keeps_entry`], `client_entry_unchangeable`),
//! `kept_entry`, `removal_failed`, `registration_failed`, `platform_entry_kept`,
//! `moved_earlier_installation` (once; it says where files went) and the container lines. Lines
//! that report mechanism print only in `--dry-run`: `platform_entry_updated`, `browsers_registered`,
//! `removed_version_directory`, `archived_version_audit`, `client_rewritten` and the per-browser
//! report.
//!
//! # What the uninstall prints
//!
//! A real uninstall prints [`uninstall_lines`] the same way: what it removed, whether the running
//! Ghostlight stopped, and for a whole uninstall the home it kept and the extension the browser
//! keeps. One that names browsers or agents also says that the command went with it and that
//! Ghostlight keeps running. Beside it print only the lines that need a person: errors, registrations that need
//! attention, and `platform_entry_kept`. What it would remove, the `Path` entry among it, prints
//! only in `--dry-run`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::environment::{run_by_full_path, Environment, OPEN_NEW_TERMINAL};

/// The Chrome Web Store listing of the Ghostlight extension.
pub const STORE_LISTING_URL: &str = "https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl";

/// The first request a person gives their agent. It works on a page Sylin keeps for the purpose.
pub const PROMPT: &str = "Open sylin.org/ghostlight/demo/desk and ring the bell.";

/// The line under the mascot when setup is done.
pub const TAGLINE: &str = "A light left burning, so the halls stay safe.";

/// The three step names, in order.
pub const STEP_NAMES: [&str; 3] = ["This computer", "Your browser", "Your agent"];

/// Appended to a step name for a screen reader when the step is done.
const SR_DONE: &str = ", done";
/// Appended to a step name for a screen reader when the step is the current one.
const SR_CURRENT: &str = ", current step";

/// The id of the browser family the Get Google Chrome row stands for.
const CHROME_ID: &str = "chrome";
/// The name of the browser Ghostlight sends a person to when no supported browser is native.
const CHROME_NAME: &str = "Google Chrome";
/// What the eyebrow says while setup is under way.
const EYEBROW_SETUP: &str = "Set up Ghostlight";
/// What the eyebrow says once setup is done.
const EYEBROW_READY: &str = "Ready";
/// A step word for a done first step.
const WORD_INSTALLED: &str = "Installed";
/// The word for a connection that is made: a step word without a name, a row that is done, and an
/// agent whose Ghostlight entry is in place, as Integrations says it (`AGENT_STATE_WORDS.installed`
/// in `ui/lib/words.js`).
pub const WORD_CONNECTED: &str = "Connected";
/// The name used for an agent whose label is not known.
const FALLBACK_AGENT_NAME: &str = "Your agent";
/// The live line while the person is on the agent step.
const LIVE_AGENT: &str = "Waiting for your agent.";
/// The accessible and visible name of the copy button for a command.
const COPY_COMMAND: &str = "Copy command";
/// The accessible and visible name of the copy button for the prompt.
const COPY_PROMPT: &str = "Copy prompt";
/// The label of the button that sends a person to the Chrome download.
const GET_CHROME_LABEL: &str = "Get Google Chrome";
/// The label of the button that adds the extension.
const ADD_LABEL: &str = "Add";
/// What an agent row says when its entry was written by an older Ghostlight. The Integrations page
/// says the same for the same state (`AGENT_STATE_WORDS.updatable` in `ui/lib/words.js`).
pub const WORD_NEEDS_UPDATE: &str = "Needs an update";
/// The label of the button that brings an older entry up to date, as the Integrations page labels
/// its own (`AGENT_ACTS.update`).
pub const UPDATE_LABEL: &str = "Update";
/// What an agent row says when Ghostlight never edits its file, as the Integrations page says it
/// (`AGENT_STATE_WORDS.by_hand`).
pub const WORD_BY_HAND: &str = "Found, set up by hand";
/// The label of the button that copies an agent's setup, as the Integrations page labels its own
/// (`AGENT_ACTS.copy`).
pub const COPY_SETUP_LABEL: &str = "Copy setup";

/// The computer Ghostlight runs on, as far as the finishing command is concerned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// Windows, where the command runs in PowerShell.
    Windows,
    /// Linux, where the command runs in a terminal.
    Linux,
}

/// How a browser family is installed on this computer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserForm {
    /// An ordinary installation that can start the Ghostlight edge.
    Native,
    /// A Snap package, which cannot.
    Snap,
    /// A Flatpak package, which cannot.
    Flatpak,
    /// Both a Snap and a Flatpak package, and no ordinary installation.
    SnapAndFlatpak,
    /// Not installed.
    Missing,
}

/// The profile of a browser that holds the Ghostlight extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileFact {
    /// The profile's directory name, such as `Profile 1`.
    pub directory: String,
    /// The name the person gave the profile, when there is one.
    pub name: Option<String>,
    /// How many profiles the browser has.
    pub profiles: usize,
}

/// What is known about one browser family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserFact {
    /// The stable browser identifier.
    pub id: String,
    /// The full name, such as `Google Chrome`.
    pub name: String,
    /// The short name for running text, such as `Chrome`.
    pub short: String,
    /// How the browser is installed.
    pub form: BrowserForm,
    /// Whether this browser is connected to Ghostlight now.
    pub connected: bool,
    /// The first profile that holds the extension, when one does.
    pub extension: Option<ProfileFact>,
}

/// What the integrations scan found for one agent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentState {
    /// The agent is on this computer and has no Ghostlight entry.
    Found,
    /// The agent has Ghostlight's entry.
    Written,
    /// The agent has an entry an older Ghostlight wrote, which an update brings up to date.
    Updatable,
    /// The agent has an entry that needs a person to look at it.
    NeedsAttention,
}

/// What is known about one agent that is on this computer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentFact {
    /// The stable agent identifier.
    pub id: String,
    /// The name a person knows the agent by.
    pub label: String,
    /// What the scan found.
    pub state: AgentState,
    /// Whether Ghostlight can write or update the agent's entry now, as the client roster says. It
    /// is false while the app container this service runs in keeps the agent's configuration
    /// private.
    pub can_install: bool,
    /// Whether Ghostlight never edits the agent's file, so copying its setup is the only way in.
    pub copy_only: bool,
    /// Whether the agent is in use: a live session of it, with this entry in place. The workbench
    /// decides it once for the setup view and the Integrations page alike.
    pub in_use: bool,
}

/// The observations the setup view is composed from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupFacts {
    /// Whether setup is complete.
    pub complete: bool,
    /// Whether setup was completed in this run of the service.
    pub finished: bool,
    /// The computer Ghostlight runs on.
    pub platform: Platform,
    /// Whether the person must finish on the computer itself, because this service runs in an app
    /// container and sees only its own view of the registry.
    pub finish_on_this_computer: bool,
    /// The browser families, in table order.
    pub browsers: Vec<BrowserFact>,
    /// The agents that are on this computer.
    pub agents: Vec<AgentFact>,
    /// The command an agent starts to run Ghostlight's MCP server.
    pub mcp_command: String,
    /// The name of the agent whose request completed setup, when it is known.
    pub agent_name: Option<String>,
}

/// Where the person is in the journey.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The person must finish installing on the computer itself.
    Computer,
    /// No browser is connected.
    Browser,
    /// A browser is connected and the agent is next.
    Agent,
    /// Setup was completed in this run.
    Done,
    /// Setup was completed earlier; the window shows the workbench.
    Retired,
}

/// The state of one step in the stepper.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    /// The step is done.
    Done,
    /// The step is the one the person is on.
    Current,
    /// The step is still ahead.
    Todo,
}

/// One of the three steps.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Step {
    /// The step's name.
    pub name: String,
    /// Where the step stands.
    pub state: StepState,
    /// The word under the name; empty when there is none.
    pub word: String,
    /// The name with its state, as a screen reader reads it.
    pub sr: String,
}

impl Step {
    fn new(index: usize, state: StepState, word: impl Into<String>) -> Self {
        let name = STEP_NAMES[index];
        let suffix = match state {
            StepState::Done => SR_DONE,
            StepState::Current => SR_CURRENT,
            StepState::Todo => "",
        };
        Self {
            name: name.to_owned(),
            state,
            word: word.into(),
            sr: format!("{name}{suffix}"),
        }
    }
}

/// How the mascot's lantern looks.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    /// Waiting for the person to finish a step.
    Waiting,
    /// Lit.
    Lit,
}

/// The headline: the page draws the accent in the italic face.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Headline {
    /// The words before the accent.
    pub lead: String,
    /// The accent phrase.
    pub accent: String,
    /// The words after the accent.
    pub trail: String,
}

impl Headline {
    fn new(lead: &str, accent: &str, trail: &str) -> Self {
        Self {
            lead: lead.to_owned(),
            accent: accent.to_owned(),
            trail: trail.to_owned(),
        }
    }
}

/// A command or prompt the page shows in a panel and copies itself.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CodePanel {
    /// The label above the text.
    pub label: String,
    /// The text to copy.
    pub text: String,
    /// The name of the copy button.
    pub copy_name: String,
}

impl CodePanel {
    fn new(label: &str, text: &str, copy_name: &str) -> Self {
        Self {
            label: label.to_owned(),
            text: text.to_owned(),
            copy_name: copy_name.to_owned(),
        }
    }
}

/// The line that says what Ghostlight is waiting for.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Live {
    /// What it says.
    pub text: String,
    /// Whether the thing waited for has arrived. Composed views are waiting lines, so this is
    /// false; the page sets it when it swaps in the connected line.
    pub done: bool,
}

impl Live {
    fn waiting(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            done: false,
        }
    }
}

/// How a row reads.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowTone {
    /// Done.
    Done,
    /// Still to do.
    Todo,
    /// Needs the person's attention.
    Attention,
    /// Not available here.
    Off,
}

/// What a row's button asks for. The page posts the kinds the service performs, opens the link
/// for `get_chrome`, sends `review_agent` to the integrations tab, and puts the agent's setup on
/// the clipboard for `copy_setup`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetupAction {
    /// Start the browser on the Chrome Web Store listing.
    AddInBrowser {
        /// The browser's identifier.
        browser: String,
    },
    /// Start the browser, in the profile that holds the extension when it has several.
    OpenBrowser {
        /// The browser's identifier.
        browser: String,
        /// The profile directory, or none for the browser's own choice.
        #[serde(default)]
        profile: Option<String>,
    },
    /// Open the Google Chrome download page.
    GetChrome,
    /// Write Ghostlight's entry into the agent: add it, or bring an older one up to date.
    ConnectAgent {
        /// The agent's identifier.
        agent: String,
    },
    /// Go to the integrations tab to look at the agent's entry.
    ReviewAgent {
        /// The agent's identifier.
        agent: String,
    },
    /// Put the agent's setup on the clipboard, for an agent whose file Ghostlight never edits.
    CopySetup {
        /// The agent's identifier.
        agent: String,
    },
}

/// What the page shows between a click and the next update that changes the row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pending {
    /// The row's detail while the action runs.
    pub detail: String,
    /// The live line while the action runs, when it changes.
    pub live: Option<String>,
}

/// A row's button.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RowAction {
    /// What it asks for.
    pub action: SetupAction,
    /// The text on the button.
    pub label: String,
    /// The accessible name.
    pub name: String,
    /// Whether the button is the primary one (filled) and not outlined.
    pub primary: bool,
    /// Whether the button leaves Ghostlight, so it carries the external mark.
    pub external: bool,
    /// What the page shows after a click.
    pub pending: Option<Pending>,
}

/// One row: a browser or an agent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Row {
    /// The browser or agent identifier.
    pub id: String,
    /// The name.
    pub name: String,
    /// The word beside the name.
    pub detail: String,
    /// How the row reads.
    pub tone: RowTone,
    /// The row's button, when it has one.
    pub action: Option<RowAction>,
}

/// The setup view: everything the window draws, in words.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SetupView {
    /// Where the person is.
    pub stage: Stage,
    /// The small line above the headline.
    pub eyebrow: String,
    /// The headline.
    pub headline: Headline,
    /// The sentence under the headline, when there is one.
    pub lead: Option<String>,
    /// The three steps.
    pub steps: [Step; 3],
    /// The lantern.
    pub light: Light,
    /// The main code panel, when there is one.
    pub code: Option<CodePanel>,
    /// The label above the rows, when they have one.
    pub rows_label: Option<String>,
    /// The browsers or agents.
    pub rows: Vec<Row>,
    /// A second code panel below the rows, when there is one.
    pub extra_code: Option<CodePanel>,
    /// What Ghostlight is waiting for, when it waits.
    pub live: Option<Live>,
    /// What the live line says once a browser connects, when a browser is connected.
    pub connected_line: Option<String>,
    /// The line under the mascot, when setup is done.
    pub tagline: Option<String>,
}

impl SetupView {
    fn new(
        stage: Stage,
        eyebrow: &str,
        headline: Headline,
        steps: [Step; 3],
        light: Light,
    ) -> Self {
        Self {
            stage,
            eyebrow: eyebrow.to_owned(),
            headline,
            lead: None,
            steps,
            light,
            code: None,
            rows_label: None,
            rows: Vec::new(),
            extra_code: None,
            live: None,
            connected_line: None,
            tagline: None,
        }
    }
}

/// Where the person is in the journey, resolved from the facts.
#[must_use]
pub fn stage(facts: &SetupFacts) -> Stage {
    if facts.complete && facts.finished {
        Stage::Done
    } else if facts.complete {
        Stage::Retired
    } else if facts.finish_on_this_computer {
        Stage::Computer
    } else if facts.browsers.iter().any(|browser| browser.connected) {
        Stage::Agent
    } else {
        Stage::Browser
    }
}

/// Compose the setup view for the facts.
#[must_use]
pub fn compose(facts: &SetupFacts) -> SetupView {
    match stage(facts) {
        Stage::Computer => computer(facts),
        Stage::Browser => browser(facts),
        Stage::Agent => agent(facts),
        Stage::Done => done(facts),
        Stage::Retired => retired(facts),
    }
}

fn computer(facts: &SetupFacts) -> SetupView {
    let (label, text) = match facts.platform {
        Platform::Windows => (
            "Paste into PowerShell from the Start menu",
            "~\\.ghostlight\\bin\\ghostlight.exe install",
        ),
        Platform::Linux => (
            "Paste into a terminal",
            "~/.ghostlight/bin/ghostlight install",
        ),
    };
    let mut view = SetupView::new(
        Stage::Computer,
        EYEBROW_SETUP,
        Headline::new("Finish installing on ", "this computer", "."),
        [
            Step::new(0, StepState::Current, ""),
            Step::new(1, StepState::Todo, ""),
            Step::new(2, StepState::Todo, ""),
        ],
        Light::Waiting,
    );
    view.lead = Some("Run this once from a regular terminal.".to_owned());
    view.code = Some(CodePanel::new(label, text, COPY_COMMAND));
    view.live = Some(Live::waiting("Waiting for this computer."));
    view
}

fn browser(facts: &SetupFacts) -> SetupView {
    let rows = browser_rows(facts);
    let offers = rows.iter().filter(|row| row.action.is_some()).count();
    let mut view = SetupView::new(
        Stage::Browser,
        EYEBROW_SETUP,
        Headline::new("Add Ghostlight to ", "your browser", "."),
        [
            Step::new(0, StepState::Done, WORD_INSTALLED),
            Step::new(1, StepState::Current, ""),
            Step::new(2, StepState::Todo, ""),
        ],
        Light::Waiting,
    );
    if offers >= 2 {
        view.lead = Some("One browser is enough.".to_owned());
    }
    view.rows = rows;
    view.live = Some(Live::waiting("Waiting for a browser."));
    view
}

fn agent(facts: &SetupFacts) -> SetupView {
    let connected = connected_short(facts);
    let connected_word = connected.map_or_else(
        || WORD_CONNECTED.to_owned(),
        |short| format!("Connected in {short}"),
    );
    let steps = [
        Step::new(0, StepState::Done, WORD_INSTALLED),
        Step::new(1, StepState::Done, connected_word),
        Step::new(2, StepState::Current, ""),
    ];
    let mut view = if facts.agents.is_empty() {
        let mut view = SetupView::new(
            Stage::Agent,
            EYEBROW_SETUP,
            Headline::new("Add Ghostlight to ", "your agent", "."),
            steps,
            Light::Lit,
        );
        view.lead = Some("No agents were found on this computer.".to_owned());
        view.code = Some(CodePanel::new(
            "MCP server command",
            &facts.mcp_command,
            COPY_COMMAND,
        ));
        view.extra_code = Some(CodePanel::new("Then ask it", PROMPT, COPY_PROMPT));
        view
    } else {
        let mut view = SetupView::new(
            Stage::Agent,
            EYEBROW_SETUP,
            Headline::new("Ask ", "your agent", "."),
            steps,
            Light::Lit,
        );
        view.code = Some(CodePanel::new("Paste into your agent", PROMPT, COPY_PROMPT));
        view.rows_label = Some("Agents on this computer".to_owned());
        view.rows = facts.agents.iter().map(agent_row).collect();
        view
    };
    view.live = Some(Live::waiting(LIVE_AGENT));
    view.connected_line = connected.map(|short| format!("{short} is connected."));
    view
}

fn done(facts: &SetupFacts) -> SetupView {
    let named = facts.agent_name.as_deref();
    let name = named.unwrap_or(FALLBACK_AGENT_NAME);
    let mut view = SetupView::new(
        Stage::Done,
        EYEBROW_READY,
        Headline::new(
            &format!("{name} just used your browser. "),
            "You're set.",
            "",
        ),
        finished_steps(facts, named.unwrap_or(WORD_CONNECTED)),
        Light::Lit,
    );
    view.tagline = Some(TAGLINE.to_owned());
    view
}

/// Setup was completed earlier, so the window shows the workbench and this view says nothing.
fn retired(facts: &SetupFacts) -> SetupView {
    SetupView::new(
        Stage::Retired,
        "",
        Headline::default(),
        finished_steps(facts, WORD_CONNECTED),
        Light::Lit,
    )
}

fn finished_steps(facts: &SetupFacts, third: &str) -> [Step; 3] {
    let second = connected_short(facts).map_or_else(
        || WORD_CONNECTED.to_owned(),
        |short| format!("Connected in {short}"),
    );
    [
        Step::new(0, StepState::Done, WORD_INSTALLED),
        Step::new(1, StepState::Done, second),
        Step::new(2, StepState::Done, third),
    ]
}

fn connected_short(facts: &SetupFacts) -> Option<&str> {
    ordered_browsers(&facts.browsers)
        .into_iter()
        .find(|browser| browser.connected)
        .map(|browser| browser.short.as_str())
}

/// The installed browsers in the order the rows show them: connected ones first, then Google
/// Chrome, then the rest in table order.
fn ordered_browsers(browsers: &[BrowserFact]) -> Vec<&BrowserFact> {
    let mut installed: Vec<&BrowserFact> = browsers
        .iter()
        .filter(|browser| {
            browser.connected || browser.extension.is_some() || browser.form != BrowserForm::Missing
        })
        .collect();
    installed.sort_by_key(|browser| {
        if browser.connected {
            0
        } else if browser.id == CHROME_ID {
            1
        } else {
            2
        }
    });
    installed
}

/// Whether the browser can run the Ghostlight edge: it is connected, holds the extension, or is
/// an ordinary installation.
fn is_native(browser: &BrowserFact) -> bool {
    browser.connected || browser.extension.is_some() || browser.form == BrowserForm::Native
}

/// The first native browser in row order, or none. The install says "Next: add Ghostlight to"
/// this browser.
fn first_native(browsers: &[BrowserFact]) -> Option<&BrowserFact> {
    ordered_browsers(browsers)
        .into_iter()
        .find(|b| is_native(b))
}

fn browser_rows(facts: &SetupFacts) -> Vec<Row> {
    let ordered = ordered_browsers(&facts.browsers);
    let any_native = ordered.iter().any(|browser| is_native(browser));
    let mut rows = Vec::new();
    let mut primary_given = false;
    for browser in ordered {
        let row = if browser.connected {
            Row {
                id: browser.id.clone(),
                name: browser.name.clone(),
                detail: WORD_CONNECTED.to_owned(),
                tone: RowTone::Done,
                action: None,
            }
        } else if let Some(profile) = &browser.extension {
            open_row(browser, profile)
        } else if browser.form == BrowserForm::Native {
            let row = add_row(browser, !primary_given);
            primary_given = true;
            row
        } else if !any_native {
            let row = sandboxed_row(browser, !primary_given);
            primary_given = true;
            row
        } else {
            continue;
        };
        rows.push(row);
    }
    if rows.is_empty() {
        rows.push(Row {
            id: CHROME_ID.to_owned(),
            name: CHROME_NAME.to_owned(),
            detail: "Not installed".to_owned(),
            tone: RowTone::Todo,
            action: Some(get_chrome_action(true)),
        });
    }
    rows
}

fn open_row(browser: &BrowserFact, profile: &ProfileFact) -> Row {
    let several = profile.profiles >= 2;
    let detail = match profile.name.as_deref().filter(|_| several) {
        Some(name) => format!(
            "Added in your {name} profile. Open {} to finish.",
            browser.short
        ),
        None => format!("Added. Open {} to finish.", browser.short),
    };
    Row {
        id: browser.id.clone(),
        name: browser.name.clone(),
        detail,
        tone: RowTone::Todo,
        action: Some(RowAction {
            action: SetupAction::OpenBrowser {
                browser: browser.id.clone(),
                profile: several.then(|| profile.directory.clone()),
            },
            label: format!("Open {}", browser.short),
            name: format!("Open {}", browser.name),
            primary: false,
            external: false,
            pending: Some(Pending {
                detail: format!("Opening {}.", browser.short),
                live: Some(format!("Waiting for {}.", browser.short)),
            }),
        }),
    }
}

fn add_row(browser: &BrowserFact, primary: bool) -> Row {
    Row {
        id: browser.id.clone(),
        name: browser.name.clone(),
        detail: "Not added".to_owned(),
        tone: RowTone::Todo,
        action: Some(RowAction {
            action: SetupAction::AddInBrowser {
                browser: browser.id.clone(),
            },
            label: ADD_LABEL.to_owned(),
            name: format!("Add to {}", browser.name),
            primary,
            external: true,
            pending: Some(Pending {
                detail: "Finish in the Chrome Web Store.".to_owned(),
                live: Some(format!("Waiting for {}.", browser.short)),
            }),
        }),
    }
}

fn sandboxed_row(browser: &BrowserFact, primary: bool) -> Row {
    let packages = match browser.form {
        BrowserForm::Snap => "Snap keeps it",
        BrowserForm::Flatpak => "Flatpak keeps it",
        _ => "Snap and Flatpak keep it",
    };
    Row {
        id: browser.id.clone(),
        name: browser.name.clone(),
        detail: format!("{packages} sealed off from the rest of this computer."),
        tone: RowTone::Off,
        action: Some(get_chrome_action(primary)),
    }
}

fn get_chrome_action(primary: bool) -> RowAction {
    RowAction {
        action: SetupAction::GetChrome,
        label: GET_CHROME_LABEL.to_owned(),
        name: GET_CHROME_LABEL.to_owned(),
        primary,
        external: true,
        pending: None,
    }
}

fn agent_row(agent: &AgentFact) -> Row {
    let row = |detail: String, tone: RowTone, action: Option<RowAction>| Row {
        id: agent.id.clone(),
        name: agent.label.clone(),
        detail,
        tone,
        action,
    };
    if agent.in_use {
        return row(WORD_CONNECTED.to_owned(), RowTone::Done, None);
    }
    let writes = agent.can_install;
    match agent.state {
        AgentState::Written => row(
            format!("Restart {} to finish.", agent.label),
            RowTone::Attention,
            None,
        ),
        AgentState::Found if agent.copy_only => row(
            WORD_BY_HAND.to_owned(),
            RowTone::Todo,
            Some(RowAction {
                action: SetupAction::CopySetup {
                    agent: agent.id.clone(),
                },
                label: COPY_SETUP_LABEL.to_owned(),
                name: format!("Copy setup for {}", agent.label),
                primary: false,
                external: false,
                pending: None,
            }),
        ),
        AgentState::Found => row(
            "Found".to_owned(),
            RowTone::Todo,
            writes.then(|| write_action(agent, "Connect", "Connecting")),
        ),
        AgentState::Updatable => row(
            WORD_NEEDS_UPDATE.to_owned(),
            RowTone::Todo,
            writes.then(|| write_action(agent, UPDATE_LABEL, "Updating")),
        ),
        AgentState::NeedsAttention => row(
            "Needs attention".to_owned(),
            RowTone::Attention,
            Some(RowAction {
                action: SetupAction::ReviewAgent {
                    agent: agent.id.clone(),
                },
                label: "Review".to_owned(),
                name: format!("Review {}", agent.label),
                primary: false,
                external: false,
                pending: None,
            }),
        ),
    }
}

/// The button that writes the agent's entry, labelled `label`, with `working` shown while it runs.
fn write_action(agent: &AgentFact, label: &str, working: &str) -> RowAction {
    RowAction {
        action: SetupAction::ConnectAgent {
            agent: agent.id.clone(),
        },
        label: label.to_owned(),
        name: format!("{label} {}", agent.label),
        primary: false,
        external: false,
        pending: Some(Pending {
            detail: working.to_owned(),
            live: None,
        }),
    }
}

/// One agent as the install saw it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallAgent {
    /// The name a person knows the agent by.
    pub label: String,
    /// Whether the agent has Ghostlight's entry after this install.
    pub has_entry: bool,
    /// Whether this install wrote or changed that entry, so the agent must restart.
    pub changed: bool,
}

/// What an install knows when it finishes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFacts {
    /// The installed version.
    pub version: String,
    /// The desktop the install ran on.
    pub environment: Environment,
    /// Whether this install wrote the user `Path`, so an open terminal does not have it yet.
    pub path_written: bool,
    /// The `ghostlight` command's full path, when its directory is not on the `PATH` the install
    /// ran with, so typing `ghostlight` in that terminal finds nothing.
    pub command_off_path: Option<PathBuf>,
    /// The agents that are on this computer.
    pub agents: Vec<InstallAgent>,
    /// The command an agent starts to run Ghostlight's MCP server.
    pub mcp_command: String,
    /// The browser families, in table order.
    pub browsers: Vec<BrowserFact>,
    /// Whether setup is complete.
    pub complete: bool,
    /// Whether the install showed the Ghostlight window.
    pub window_shown: bool,
}

/// The whole success output of a real install, one line per entry, in order.
///
/// While setup is incomplete exactly one `Next:` line says what to do. The lines never describe
/// mechanism; see the module documentation for the lines that print beside these.
#[must_use]
pub fn install_lines(facts: &InstallFacts) -> Vec<String> {
    let mut lines = vec![format!("Ghostlight {} is installed.", facts.version)];
    lines.push(facts.environment.location().to_owned());
    if let Some(caveat) = facts.environment.caveat() {
        lines.push(caveat.to_owned());
    }
    if facts.path_written {
        lines.push(OPEN_NEW_TERMINAL.to_owned());
    }
    if let Some(command) = &facts.command_off_path {
        lines.push(run_by_full_path(command));
    }
    let changed: Vec<&str> = facts
        .agents
        .iter()
        .filter(|agent| agent.changed)
        .map(|agent| agent.label.as_str())
        .collect();
    let with_entry: Vec<&str> = facts
        .agents
        .iter()
        .filter(|agent| agent.has_entry)
        .map(|agent| agent.label.as_str())
        .collect();
    // The browser step is done when a browser is connected, as in the window: a profile that
    // holds the extension still has to be opened.
    let connected = facts.browsers.iter().any(|browser| browser.connected);
    let restart_first = !facts.complete && !connected && !changed.is_empty();
    if facts.agents.is_empty() {
        lines.push(format!(
            "No agents were found on this computer. Ghostlight's MCP server command: {}",
            facts.mcp_command
        ));
    } else if !with_entry.is_empty() {
        let note = if restart_first {
            let target = if changed.len() == with_entry.len() {
                (if changed.len() == 1 { "it" } else { "them" }).to_owned()
            } else {
                join_list(&changed)
            };
            format!(" (restart {target} before you ask)")
        } else {
            String::new()
        };
        lines.push(format!("Agents: {}{note}.", join_list(&with_entry)));
    }
    if facts.complete {
        if !changed.is_empty() {
            lines.push(format!(
                "Restart {} to use Ghostlight.",
                join_list(&changed)
            ));
        }
    } else {
        lines.push(next_line(facts, connected, &changed));
    }
    lines
}

/// The one `Next:` line: the step the window shows. A connected browser asks for the prompt, a
/// profile that holds the extension asks for that browser to be opened, and otherwise the person
/// adds Ghostlight to the first native browser, or gets Chrome.
fn next_line(facts: &InstallFacts, connected: bool, changed: &[&str]) -> String {
    if connected {
        if changed.is_empty() {
            return format!("Next: ask your agent: {PROMPT}");
        }
        let pronoun = if changed.len() == 1 { "it" } else { "one" };
        return format!(
            "Next: restart {}, then ask {pronoun}: {PROMPT}",
            join_list(changed)
        );
    }
    if let Some(browser) = ordered_browsers(&facts.browsers)
        .into_iter()
        .find(|browser| browser.extension.is_some())
    {
        return format!(
            "Next: open {}, where Ghostlight is already added.",
            browser.short
        );
    }
    match (first_native(&facts.browsers), facts.window_shown) {
        (Some(browser), true) => format!(
            "Next: add Ghostlight to {} in the Ghostlight window.",
            browser.short
        ),
        (Some(browser), false) => {
            format!(
                "Next: add Ghostlight to {}: {STORE_LISTING_URL}",
                browser.short
            )
        }
        (None, true) => "Next: get Google Chrome from the Ghostlight window.".to_owned(),
        (None, false) => {
            format!("Next: install Google Chrome, then add Ghostlight there: {STORE_LISTING_URL}")
        }
    }
}

/// The first line of a whole uninstall.
const UNINSTALLED: &str = "Ghostlight is uninstalled.";
/// What an uninstall says when no agent or browser held Ghostlight's registration.
const NOTHING_REMOVED: &str = "No agent or browser had Ghostlight to remove.";
/// What a whole uninstall says after the Ghostlight that was running stopped.
const RUNNING_STOPPED: &str = "The running Ghostlight was stopped.";
/// What a whole uninstall says when the running Ghostlight did not stop in time.
const RUNNING_DID_NOT_STOP: &str =
    "The running Ghostlight did not stop in time; quit it from its icon, or sign out.";
/// What an uninstall that named browsers or agents says when it also removed the command.
const COMMAND_REMOVED: &str = "The ghostlight command was removed too.";
/// What an uninstall that named browsers or agents says while Ghostlight still runs.
const LEFT_RUNNING: &str =
    "Ghostlight keeps running for the agents and browsers that still have it.";
/// The last line of a whole uninstall: the extension is the browser's to remove.
const EXTENSION_STAYS: &str =
    "Ghostlight in Browser stays in your browser until you remove it from the browser's extensions page.";

/// What became of the Ghostlight that was running when an uninstall finished.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunningAfterUninstall {
    /// No Ghostlight was running.
    Unchanged,
    /// The uninstall removed only part of Ghostlight and left the running one as it was.
    LeftRunning,
    /// The running Ghostlight stopped.
    Stopped,
    /// The running Ghostlight was asked to stop and was still running when the wait ended.
    StillRunning,
}

/// What a real uninstall knows when it finishes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallFacts {
    /// Whether the uninstall removed Ghostlight as a whole, rather than from named browsers or
    /// agents only.
    pub whole: bool,
    /// The agents Ghostlight's entry was removed from, by the names a person knows them by.
    pub agents: Vec<String>,
    /// The browsers whose connection to Ghostlight was removed, by name.
    pub browsers: Vec<String>,
    /// Whether the uninstall removed the `ghostlight` command. Only a partial uninstall says so,
    /// since a whole one removes everything it registered.
    pub command_removed: bool,
    /// What became of the running Ghostlight.
    pub running: RunningAfterUninstall,
    /// The Ghostlight home the uninstall left in place, as a person types it.
    pub home: String,
}

/// The whole success output of a real uninstall, one line per entry, in order: what was removed,
/// whether the running Ghostlight stopped, and, for a whole uninstall, what stays and how to
/// remove it. The lines never describe mechanism; `--dry-run` lists that.
#[must_use]
pub fn uninstall_lines(facts: &UninstallFacts) -> Vec<String> {
    let mut lines = Vec::new();
    if facts.whole {
        lines.push(UNINSTALLED.to_owned());
    }
    if !facts.agents.is_empty() {
        let names: Vec<&str> = facts.agents.iter().map(String::as_str).collect();
        // A whole uninstall ends the running Ghostlight, and with it every agent's connection.
        let note = if facts.whole {
            String::new()
        } else {
            let target = if names.len() == 1 { "it" } else { "them" };
            format!(" (restart {target} to unload Ghostlight's tools)")
        };
        lines.push(format!("Removed from agents: {}{note}.", join_list(&names)));
    }
    if !facts.browsers.is_empty() {
        let names: Vec<&str> = facts.browsers.iter().map(String::as_str).collect();
        lines.push(format!("Removed from browsers: {}.", join_list(&names)));
    }
    if facts.agents.is_empty() && facts.browsers.is_empty() {
        lines.push(NOTHING_REMOVED.to_owned());
    }
    if !facts.whole && facts.command_removed {
        lines.push(COMMAND_REMOVED.to_owned());
    }
    match facts.running {
        RunningAfterUninstall::Unchanged => {}
        RunningAfterUninstall::LeftRunning => lines.push(LEFT_RUNNING.to_owned()),
        RunningAfterUninstall::Stopped => lines.push(RUNNING_STOPPED.to_owned()),
        RunningAfterUninstall::StillRunning => lines.push(RUNNING_DID_NOT_STOP.to_owned()),
    }
    if facts.whole {
        lines.push(format!(
            "Kept {}, with the Ghostlight programs, your audit history and your settings. Delete that folder to remove them too.",
            facts.home
        ));
        lines.push(EXTENSION_STAYS.to_owned());
    }
    lines
}

/// Join names as a person says them: `A`, `A and B`, `A, B and C`.
fn join_list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [head @ .., last] => format!("{} and {last}", head.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::native_host::STORE_EXTENSION_ID;
    use crate::language::self_check::golden::repository_path;

    /// The window's setup page, which renders the view and posts its acts back.
    const SETUP_PAGE: &str = "crates/orchestrator/ui/lib/setup-view.js";
    /// The extension's own setup view, which shows the same three steps.
    const EXTENSION_SETUP_VIEW: &str = "extension/lib/setup-view.js";
    /// The kit stylesheet, which styles the lantern.
    const KIT_CSS: &str = "extension/kit.css";
    /// The profile directory the test browsers keep the extension in.
    const PROFILE: &str = "Profile 1";
    /// What every `Next:` line starts with.
    const NEXT: &str = "Next:";

    fn browser(id: &str, name: &str, short: &str, form: BrowserForm) -> BrowserFact {
        BrowserFact {
            id: id.to_owned(),
            name: name.to_owned(),
            short: short.to_owned(),
            form,
            connected: false,
            extension: None,
        }
    }

    fn chrome(form: BrowserForm) -> BrowserFact {
        browser("chrome", "Google Chrome", "Chrome", form)
    }

    fn edge(form: BrowserForm) -> BrowserFact {
        browser("edge", "Microsoft Edge", "Edge", form)
    }

    fn brave(form: BrowserForm) -> BrowserFact {
        browser("brave", "Brave", "Brave", form)
    }

    fn connected(mut fact: BrowserFact) -> BrowserFact {
        fact.connected = true;
        fact
    }

    fn with_extension(mut fact: BrowserFact, name: Option<&str>, profiles: usize) -> BrowserFact {
        fact.extension = Some(ProfileFact {
            directory: PROFILE.to_owned(),
            name: name.map(str::to_owned),
            profiles,
        });
        fact
    }

    fn agent(id: &str, label: &str, state: AgentState) -> AgentFact {
        AgentFact {
            id: id.to_owned(),
            label: label.to_owned(),
            state,
            can_install: true,
            copy_only: false,
            in_use: false,
        }
    }

    fn facts(browsers: Vec<BrowserFact>, agents: Vec<AgentFact>) -> SetupFacts {
        SetupFacts {
            complete: false,
            finished: false,
            platform: Platform::Windows,
            finish_on_this_computer: false,
            browsers,
            agents,
            mcp_command: "ghostlight mcp".to_owned(),
            agent_name: None,
        }
    }

    fn row<'a>(view: &'a SetupView, id: &str) -> &'a Row {
        view.rows
            .iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("no row {id}"))
    }

    fn agents_with_entries(labels: &[(&str, bool)]) -> Vec<InstallAgent> {
        labels
            .iter()
            .map(|(label, changed)| InstallAgent {
                label: (*label).to_owned(),
                has_entry: true,
                changed: *changed,
            })
            .collect()
    }

    fn install(browsers: Vec<BrowserFact>, agents: Vec<InstallAgent>) -> InstallFacts {
        InstallFacts {
            version: "2.0.0".to_owned(),
            environment: Environment::Windows,
            path_written: false,
            command_off_path: None,
            agents,
            mcp_command: "ghostlight mcp".to_owned(),
            browsers,
            complete: false,
            window_shown: true,
        }
    }

    fn next_lines(lines: &[String]) -> Vec<&String> {
        lines.iter().filter(|line| line.starts_with(NEXT)).collect()
    }

    fn uninstall(agents: &[&str], browsers: &[&str]) -> UninstallFacts {
        UninstallFacts {
            whole: true,
            agents: agents.iter().map(|&name| name.to_owned()).collect(),
            browsers: browsers.iter().map(|&name| name.to_owned()).collect(),
            command_removed: true,
            running: RunningAfterUninstall::Stopped,
            home: r"C:\Users\person\.ghostlight".to_owned(),
        }
    }

    /// INTENT install journey and ACCEPTANCE D24: a setup finished in this run is Done, one
    /// finished earlier is Retired, an app container finishes on the computer, and otherwise a
    /// connected browser moves the person from the browser step to the agent step.
    #[test]
    fn stages_resolve_in_order() {
        let mut state = facts(vec![connected(chrome(BrowserForm::Native))], vec![]);
        assert_eq!(stage(&state), Stage::Agent);
        state.browsers.clear();
        assert_eq!(stage(&state), Stage::Browser);
        state.finish_on_this_computer = true;
        assert_eq!(stage(&state), Stage::Computer);
        state.complete = true;
        assert_eq!(stage(&state), Stage::Retired);
        state.finished = true;
        assert_eq!(stage(&state), Stage::Done);
        state.complete = false;
        assert_eq!(stage(&state), Stage::Computer);
    }

    /// ACCEPTANCE D23 and D16: every native browser is offered Add (Edge too, JQ9), a browser
    /// that holds the extension is offered Open, in that profile only when it has several, a
    /// browser that is not installed has no row, a sandboxed browser shows only when no browser
    /// is native, and Get Google Chrome appears only when no supported browser is installed.
    #[test]
    fn the_browser_step_offers_each_installed_browser_its_act() {
        let add = |fact: &BrowserFact| {
            Some(SetupAction::AddInBrowser {
                browser: fact.id.clone(),
            })
        };
        let open = |fact: &BrowserFact, profile: Option<&str>| {
            Some(SetupAction::OpenBrowser {
                browser: fact.id.clone(),
                profile: profile.map(str::to_owned),
            })
        };
        let get_chrome = Some(SetupAction::GetChrome);
        let native = chrome(BrowserForm::Native);
        let brave_native = brave(BrowserForm::Native);
        let edge_native = edge(BrowserForm::Native);
        let several = with_extension(chrome(BrowserForm::Native), Some("Work"), 3);
        let single = with_extension(chrome(BrowserForm::Native), Some("Person 1"), 1);
        let off = Some(RowTone::Off);
        // Per case, every row the step shows: its id, its act, and its tone where the case is
        // about the tone.
        type Shown<'a> = (&'a str, Option<SetupAction>, Option<RowTone>);
        let cases: Vec<(&str, Vec<BrowserFact>, Vec<Shown>)> = vec![
            (
                "two native browsers",
                vec![native.clone(), brave_native.clone()],
                vec![
                    ("chrome", add(&native), None),
                    ("brave", add(&brave_native), None),
                ],
            ),
            (
                "Edge",
                vec![edge_native.clone()],
                vec![("edge", add(&edge_native), None)],
            ),
            (
                "the extension in one of several profiles",
                vec![several.clone()],
                vec![("chrome", open(&several, Some(PROFILE)), None)],
            ),
            (
                "the extension in the only profile",
                vec![single.clone()],
                vec![("chrome", open(&single, None), None)],
            ),
            (
                "only sandboxed browsers",
                vec![
                    chrome(BrowserForm::Snap),
                    brave(BrowserForm::Flatpak),
                    edge(BrowserForm::SnapAndFlatpak),
                ],
                vec![
                    ("chrome", get_chrome.clone(), off),
                    ("brave", get_chrome.clone(), off),
                    ("edge", get_chrome.clone(), off),
                ],
            ),
            (
                "a sandboxed browser beside a native one",
                vec![chrome(BrowserForm::Snap), brave_native.clone()],
                vec![("brave", add(&brave_native), None)],
            ),
            (
                "no browser",
                vec![],
                vec![(CHROME_ID, get_chrome.clone(), None)],
            ),
            (
                "no browser installed",
                vec![chrome(BrowserForm::Missing), edge(BrowserForm::Missing)],
                vec![(CHROME_ID, get_chrome.clone(), None)],
            ),
            (
                "a browser that is not installed",
                vec![chrome(BrowserForm::Missing), brave_native.clone()],
                vec![("brave", add(&brave_native), None)],
            ),
        ];
        for (case, browsers, expected) in cases {
            let view = compose(&facts(browsers, vec![]));
            assert_eq!(view.rows.len(), expected.len(), "{case}: {:?}", view.rows);
            for (id, act, tone) in expected {
                let shown = row(&view, id);
                let button = shown.action.as_ref();
                assert_eq!(
                    button.map(|button| &button.action),
                    act.as_ref(),
                    "{case}: {id}"
                );
                if let Some(button) = button {
                    let leaves = matches!(
                        button.action,
                        SetupAction::AddInBrowser { .. } | SetupAction::GetChrome
                    );
                    assert_eq!(button.external, leaves, "{case}: {id} leaves Ghostlight");
                }
                if let Some(tone) = tone {
                    assert_eq!(shown.tone, tone, "{case}: {id}");
                }
                if act == get_chrome && tone.is_none() {
                    // The Get Google Chrome row of a computer with no supported browser.
                    assert_eq!(shown.name, CHROME_NAME, "{case}");
                }
            }
        }
    }

    /// ARCHITECTURE setup paragraph, 80c6f54a and c007d102: an agent row offers Connect or Update
    /// only where the roster says Ghostlight can install, a copy-only agent offers Copy setup, a
    /// written entry asks for a restart, and a row reads Connected only while the agent is in
    /// use.
    #[test]
    fn an_agent_row_offers_only_what_the_roster_allows() {
        /// What a row's detail must say.
        enum Says {
            /// Exactly these words.
            Exactly(&'static str),
            /// A sentence that names the agent.
            TheAgent,
            /// Whatever the state's own word is; the case is about the act.
            Anything,
        }
        let fact = |id: &str, state, can_install, copy_only, in_use| AgentFact {
            can_install,
            copy_only,
            in_use,
            ..agent(id, &format!("Agent {id}"), state)
        };
        let connect = |id: &str| SetupAction::ConnectAgent { agent: id.into() };
        let cases = [
            (
                fact("written-in-use", AgentState::Written, true, false, true),
                Says::Exactly(WORD_CONNECTED),
                Some(RowTone::Done),
                None,
            ),
            (
                fact("written", AgentState::Written, true, false, false),
                Says::TheAgent,
                Some(RowTone::Attention),
                None,
            ),
            (
                fact("found", AgentState::Found, true, false, false),
                Says::Anything,
                None,
                Some((connect("found"), "Connect")),
            ),
            (
                fact("found-private", AgentState::Found, false, false, false),
                Says::Anything,
                None,
                None,
            ),
            (
                fact("updatable", AgentState::Updatable, true, false, false),
                Says::Exactly(WORD_NEEDS_UPDATE),
                None,
                Some((connect("updatable"), UPDATE_LABEL)),
            ),
            (
                fact(
                    "updatable-private",
                    AgentState::Updatable,
                    false,
                    false,
                    false,
                ),
                Says::Exactly(WORD_NEEDS_UPDATE),
                None,
                None,
            ),
            (
                fact("attention", AgentState::NeedsAttention, true, false, false),
                Says::Anything,
                Some(RowTone::Attention),
                Some((
                    SetupAction::ReviewAgent {
                        agent: "attention".into(),
                    },
                    "Review",
                )),
            ),
            (
                fact("by-hand", AgentState::Found, false, true, false),
                Says::Exactly(WORD_BY_HAND),
                None,
                Some((
                    SetupAction::CopySetup {
                        agent: "by-hand".into(),
                    },
                    COPY_SETUP_LABEL,
                )),
            ),
            (
                fact("by-hand-in-use", AgentState::Found, false, true, true),
                Says::Exactly(WORD_CONNECTED),
                Some(RowTone::Done),
                None,
            ),
        ];
        let agents = cases.iter().map(|(fact, ..)| fact.clone()).collect();
        let view = compose(&facts(vec![connected(chrome(BrowserForm::Native))], agents));
        for (fact, says, tone, act) in cases {
            let shown = row(&view, &fact.id);
            match says {
                Says::Exactly(words) => assert_eq!(shown.detail, words, "{}", fact.id),
                Says::TheAgent => assert!(shown.detail.contains(&fact.label), "{}", fact.id),
                Says::Anything => {}
            }
            if let Some(tone) = tone {
                assert_eq!(shown.tone, tone, "{}", fact.id);
            }
            let button = shown
                .action
                .as_ref()
                .map(|button| (button.action.clone(), button.label.as_str()));
            assert_eq!(button, act, "{}", fact.id);
        }
    }

    /// Contract with `ui/lib/words.js` (80c6f54a) and `extension/lib/setup-view.js` (INTENT:
    /// every surface that shows the install journey shows the same three steps): each word
    /// another surface repeats is the one this module says.
    #[test]
    fn the_words_another_surface_repeats_are_the_services() {
        let page = include_str!("../../ui/lib/words.js");
        for line in [
            format!("updatable: \"{WORD_NEEDS_UPDATE}\""),
            format!("by_hand: \"{WORD_BY_HAND}\""),
            format!("update: \"{UPDATE_LABEL}\""),
            format!("copy: \"{COPY_SETUP_LABEL}\""),
        ] {
            assert!(page.contains(&line), "the page does not say {line}");
        }
        let extension = std::fs::read_to_string(repository_path(EXTENSION_SETUP_VIEW))
            .expect("the extension's setup view is readable");
        let steps = format!(
            "steps: Object.freeze([{}])",
            STEP_NAMES.map(|name| format!("\"{name}\"")).join(", ")
        );
        assert!(
            extension.contains(&steps),
            "{EXTENSION_SETUP_VIEW} does not say {steps}"
        );
    }

    /// Contract with the window's setup page: every stage, step state, row tone and action kind
    /// the view serializes is a value `ui/lib/setup-view.js` knows, every light is one
    /// `extension/kit.css` styles, and a step the page posts back to `/api/setup/act` without a
    /// profile decodes.
    #[test]
    fn the_view_speaks_the_vocabulary_the_page_reads() {
        let page = std::fs::read_to_string(repository_path(SETUP_PAGE))
            .expect("the setup page is readable");
        let kit = std::fs::read_to_string(repository_path(KIT_CSS)).expect("the kit is readable");
        let tag = |value: serde_json::Value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{value} is no string tag"))
                .to_owned()
        };
        let mut tags: Vec<String> = Vec::new();
        for stage in [
            Stage::Computer,
            Stage::Browser,
            Stage::Agent,
            Stage::Done,
            Stage::Retired,
        ] {
            tags.push(tag(serde_json::to_value(stage).unwrap()));
        }
        for state in [StepState::Done, StepState::Current, StepState::Todo] {
            tags.push(tag(serde_json::to_value(state).unwrap()));
        }
        for tone in [
            RowTone::Done,
            RowTone::Todo,
            RowTone::Attention,
            RowTone::Off,
        ] {
            tags.push(tag(serde_json::to_value(tone).unwrap()));
        }
        for action in [
            SetupAction::AddInBrowser {
                browser: "chrome".into(),
            },
            SetupAction::OpenBrowser {
                browser: "chrome".into(),
                profile: None,
            },
            SetupAction::GetChrome,
            SetupAction::ConnectAgent {
                agent: "codex".into(),
            },
            SetupAction::ReviewAgent {
                agent: "codex".into(),
            },
            SetupAction::CopySetup {
                agent: "codex".into(),
            },
        ] {
            tags.push(tag(serde_json::to_value(action).unwrap()["kind"].clone()));
        }
        for value in tags {
            assert!(
                page.contains(&format!("\"{value}\"")),
                "{SETUP_PAGE} does not know {value}"
            );
        }
        for light in [Light::Waiting, Light::Lit] {
            let value = tag(serde_json::to_value(light).unwrap());
            assert!(
                kit.contains(&format!("[data-light=\"{value}\"]")),
                "{KIT_CSS} does not style the {value} light"
            );
        }

        let posted: SetupAction =
            serde_json::from_str(r#"{"kind":"open_browser","browser":"edge"}"#).unwrap();
        assert_eq!(
            posted,
            SetupAction::OpenBrowser {
                browser: "edge".to_owned(),
                profile: None
            }
        );
        assert_eq!(
            serde_json::to_value(SetupAction::CopySetup {
                agent: "goose".to_owned(),
            })
            .unwrap(),
            serde_json::json!({ "kind": "copy_setup", "agent": "goose" }),
            "the page reads the agent to copy its setup"
        );
    }

    /// Contract with `install::native_host::STORE_EXTENSION_ID`: Add and every Next line send a
    /// person to the listing of the extension the native host admits.
    #[test]
    fn the_listing_url_names_the_store_extension() {
        assert!(STORE_LISTING_URL.ends_with(STORE_EXTENSION_ID));
    }

    /// ACCEPTANCE D21: a real install prints the version, where Ghostlight lives, which agents
    /// have it, then one Next line, or none once setup is complete; the agents line names the
    /// agents that must restart (ecc6da85), and a command a terminal cannot find yet says how to
    /// reach it (97217b28).
    #[test]
    fn an_install_prints_its_version_location_agents_and_next_step() {
        let chrome_native = || vec![chrome(BrowserForm::Native)];
        let first = install(
            chrome_native(),
            agents_with_entries(&[("Claude Code", true), ("Codex", true)]),
        );
        assert_eq!(
            install_lines(&first),
            [
                format!("Ghostlight {} is installed.", first.version),
                Environment::Windows.location().to_owned(),
                "Agents: Claude Code and Codex (restart them before you ask).".to_owned(),
                "Next: add Ghostlight to Chrome in the Ghostlight window.".to_owned(),
            ],
            "a first install with the window"
        );

        let with = |change: fn(&mut InstallFacts), facts: InstallFacts| {
            let mut facts = facts;
            change(&mut facts);
            facts
        };
        let wsl = with(
            |facts| facts.environment = Environment::Wsl,
            install(chrome_native(), agents_with_entries(&[("Codex", false)])),
        );
        let path_written = with(
            |facts| facts.path_written = true,
            install(chrome_native(), agents_with_entries(&[("Codex", false)])),
        );
        let off_path = with(
            |facts| {
                facts.environment = Environment::LinuxGnome;
                facts.command_off_path = Some(PathBuf::from("/home/p/.local/bin/ghostlight"));
            },
            install(chrome_native(), agents_with_entries(&[("Codex", false)])),
        );
        let no_agents = install(chrome_native(), vec![]);
        let complete = with(
            |facts| facts.complete = true,
            install(
                vec![connected(chrome(BrowserForm::Native))],
                agents_with_entries(&[("Codex", false)]),
            ),
        );
        let complete_changed = with(
            |facts| facts.complete = true,
            install(
                vec![connected(chrome(BrowserForm::Native))],
                agents_with_entries(&[("Claude Code", true), ("Codex", true)]),
            ),
        );
        // Per case, the lines after the version and the location, without the Next line.
        let cases: Vec<(&str, InstallFacts, Vec<String>)> = vec![
            (
                "only one agent changed",
                install(
                    chrome_native(),
                    agents_with_entries(&[("Claude Code", false), ("Codex", true)]),
                ),
                vec!["Agents: Claude Code and Codex (restart Codex before you ask).".into()],
            ),
            (
                "one agent, the window hidden",
                with(
                    |facts| facts.window_shown = false,
                    install(chrome_native(), agents_with_entries(&[("Claude Code", true)])),
                ),
                vec!["Agents: Claude Code (restart it before you ask).".into()],
            ),
            (
                "the extension in a profile",
                install(
                    vec![with_extension(chrome(BrowserForm::Native), None, 1)],
                    agents_with_entries(&[("Claude Code", true), ("Codex", true)]),
                ),
                vec!["Agents: Claude Code and Codex (restart them before you ask).".into()],
            ),
            (
                "a connected browser, where the Next line asks for the restart",
                install(
                    vec![connected(chrome(BrowserForm::Native))],
                    agents_with_entries(&[("Claude Code", true), ("Codex", true), ("Cursor", true)]),
                ),
                vec!["Agents: Claude Code, Codex and Cursor.".into()],
            ),
            (
                "an agent the install did not change",
                install(chrome_native(), agents_with_entries(&[("Codex", false)])),
                vec!["Agents: Codex.".into()],
            ),
            (
                "WSL",
                wsl,
                vec![
                    Environment::Wsl.caveat().unwrap().to_owned(),
                    "Agents: Codex.".into(),
                ],
            ),
            (
                "a Path just written",
                path_written,
                vec![OPEN_NEW_TERMINAL.to_owned(), "Agents: Codex.".into()],
            ),
            (
                "a command off the PATH",
                off_path,
                vec![
                    "This terminal's PATH does not include /home/p/.local/bin, so run ghostlight by its full path: /home/p/.local/bin/ghostlight".into(),
                    "Agents: Codex.".into(),
                ],
            ),
            (
                "no agents",
                no_agents.clone(),
                vec![format!(
                    "No agents were found on this computer. Ghostlight's MCP server command: {}",
                    no_agents.mcp_command
                )],
            ),
            (
                "a detected agent without an entry",
                install(
                    chrome_native(),
                    vec![InstallAgent {
                        label: "Zed".to_owned(),
                        has_entry: false,
                        changed: false,
                    }],
                ),
                vec![],
            ),
            ("a complete reinstall", complete, vec!["Agents: Codex.".into()]),
            (
                "a complete reinstall that changed agents",
                complete_changed,
                vec![
                    "Agents: Claude Code and Codex.".into(),
                    "Restart Claude Code and Codex to use Ghostlight.".into(),
                ],
            ),
        ];
        for (case, facts, rest) in cases {
            let lines = install_lines(&facts);
            let mut expected = vec![
                format!("Ghostlight {} is installed.", facts.version),
                facts.environment.location().to_owned(),
            ];
            expected.extend(rest);
            let without_next: Vec<String> = lines
                .iter()
                .filter(|line| !line.starts_with(NEXT))
                .cloned()
                .collect();
            assert_eq!(without_next, expected, "{case}");
            let nexts = usize::from(!facts.complete);
            assert_eq!(next_lines(&lines).len(), nexts, "{case}: {lines:?}");
            if !facts.complete {
                assert!(lines.last().unwrap().starts_with(NEXT), "{case}: {lines:?}");
            }
        }
    }

    /// 564bf635 (linux-1, L08) and ACCEPTANCE D21: while setup is incomplete the install prints
    /// exactly one Next line, never says "to finish", and names the step the window shows for
    /// the same browsers.
    #[test]
    fn the_next_line_is_the_step_the_window_shows() {
        let browser_sets = [
            vec![],
            vec![chrome(BrowserForm::Native)],
            vec![chrome(BrowserForm::Snap)],
            vec![with_extension(chrome(BrowserForm::Native), Some("Work"), 2)],
            vec![connected(chrome(BrowserForm::Native))],
            vec![
                edge(BrowserForm::Native),
                chrome(BrowserForm::Native),
                brave(BrowserForm::Native),
            ],
            vec![edge(BrowserForm::Native), chrome(BrowserForm::Snap)],
            vec![
                chrome(BrowserForm::Native),
                with_extension(edge(BrowserForm::Native), None, 1),
            ],
        ];
        let agent_sets = [
            vec![],
            agents_with_entries(&[("Codex", true)]),
            agents_with_entries(&[("Codex", false), ("Cursor", true)]),
        ];
        for browsers in &browser_sets {
            let view = compose(&facts(browsers.clone(), vec![]));
            let short = |id: &str| {
                browsers
                    .iter()
                    .find(|fact| fact.id == id)
                    .map_or_else(|| panic!("no browser {id}"), |fact| fact.short.clone())
            };
            let opened = view.rows.iter().find_map(|row| {
                row.action
                    .as_ref()
                    .filter(|button| matches!(button.action, SetupAction::OpenBrowser { .. }))
            });
            let primary = view
                .rows
                .iter()
                .find_map(|row| row.action.as_ref().filter(|button| button.primary));
            let shown = opened.or(primary).map(|button| &button.action);
            for agents in &agent_sets {
                for window_shown in [true, false] {
                    for path_written in [true, false] {
                        let mut facts = install(browsers.clone(), agents.clone());
                        facts.window_shown = window_shown;
                        facts.path_written = path_written;
                        let lines = install_lines(&facts);
                        let case = format!("{browsers:?} {agents:?} window {window_shown}");
                        let nexts = next_lines(&lines);
                        assert_eq!(nexts.len(), 1, "{case}: {lines:?}");
                        assert!(
                            lines.iter().all(|line| !line.contains("to finish")),
                            "{case}: {lines:?}"
                        );
                        let next = nexts[0];
                        if view.stage == Stage::Agent {
                            assert!(next.contains(PROMPT), "{case}: {next}");
                            continue;
                        }
                        let listing = |next: &str| {
                            assert_eq!(
                                next.contains(STORE_LISTING_URL),
                                !window_shown,
                                "{case}: {next}"
                            );
                        };
                        match shown {
                            Some(SetupAction::OpenBrowser { browser, .. }) => assert_eq!(
                                next,
                                &format!(
                                    "Next: open {}, where Ghostlight is already added.",
                                    short(browser)
                                ),
                                "{case}"
                            ),
                            Some(SetupAction::AddInBrowser { browser }) => {
                                let add = format!("add Ghostlight to {}", short(browser));
                                assert!(next.contains(&add), "{case}: {next}");
                                listing(next);
                            }
                            Some(SetupAction::GetChrome) => {
                                assert!(next.contains(CHROME_NAME), "{case}: {next}");
                                listing(next);
                            }
                            other => panic!("{case}: the window shows {other:?}"),
                        }
                    }
                }
            }
        }

        let browsers = vec![connected(chrome(BrowserForm::Native))];
        for (agents, next) in [
            (
                agents_with_entries(&[("Claude Code", true), ("Codex", true), ("Cursor", true)]),
                format!("Next: restart Claude Code, Codex and Cursor, then ask one: {PROMPT}"),
            ),
            (
                agents_with_entries(&[("Codex", true)]),
                format!("Next: restart Codex, then ask it: {PROMPT}"),
            ),
            (
                agents_with_entries(&[("Codex", false)]),
                format!("Next: ask your agent: {PROMPT}"),
            ),
        ] {
            let lines = install_lines(&install(browsers.clone(), agents));
            assert_eq!(next_lines(&lines), [&next]);
        }
        for browsers in browser_sets {
            let mut facts = install(browsers, agents_with_entries(&[("Codex", true)]));
            facts.complete = true;
            assert!(next_lines(&install_lines(&facts)).is_empty());
        }
    }

    /// 888d9323 and 8e5597e0 (install-7, docs-9; ADR-0199 Decision 20): an uninstall says what
    /// it removed, whether the running Ghostlight stopped, and for a whole uninstall the home it
    /// kept and the extension the browser keeps; a partial one says when the command went too
    /// and that Ghostlight keeps running.
    #[test]
    fn the_uninstall_summary_says_only_what_it_removed() {
        let kept = |facts: &UninstallFacts| {
            format!(
                "Kept {}, with the Ghostlight programs, your audit history and your settings. Delete that folder to remove them too.",
                facts.home
            )
        };
        let with = |change: fn(&mut UninstallFacts), facts: UninstallFacts| {
            let mut facts = facts;
            change(&mut facts);
            facts
        };
        let whole = uninstall(
            &["Claude Code", "Codex"],
            &["Google Chrome", "Microsoft Edge"],
        );
        let nothing = with(
            |facts| facts.running = RunningAfterUninstall::Unchanged,
            uninstall(&[], &[]),
        );
        let did_not_stop = with(
            |facts| facts.running = RunningAfterUninstall::StillRunning,
            uninstall(&[], &[]),
        );
        let with_command = uninstall(&["Zed"], &[]);
        let one_agent = with(
            |facts| {
                facts.whole = false;
                facts.command_removed = false;
                facts.running = RunningAfterUninstall::Unchanged;
            },
            uninstall(&["Zed"], &[]),
        );
        let partial = with(
            |facts| {
                facts.whole = false;
                facts.running = RunningAfterUninstall::LeftRunning;
            },
            uninstall(&["Zed", "Cursor"], &["Brave"]),
        );
        let cases = [
            (
                "a whole uninstall",
                vec![
                    UNINSTALLED.to_owned(),
                    "Removed from agents: Claude Code and Codex.".to_owned(),
                    "Removed from browsers: Google Chrome and Microsoft Edge.".to_owned(),
                    RUNNING_STOPPED.to_owned(),
                    kept(&whole),
                    EXTENSION_STAYS.to_owned(),
                ],
                whole,
            ),
            (
                "nothing removed and nothing running",
                vec![
                    UNINSTALLED.to_owned(),
                    NOTHING_REMOVED.to_owned(),
                    kept(&nothing),
                    EXTENSION_STAYS.to_owned(),
                ],
                nothing,
            ),
            (
                "a running Ghostlight that did not stop",
                vec![
                    UNINSTALLED.to_owned(),
                    NOTHING_REMOVED.to_owned(),
                    RUNNING_DID_NOT_STOP.to_owned(),
                    kept(&did_not_stop),
                    EXTENSION_STAYS.to_owned(),
                ],
                did_not_stop,
            ),
            (
                "a whole uninstall does not single out the command",
                vec![
                    UNINSTALLED.to_owned(),
                    "Removed from agents: Zed.".to_owned(),
                    RUNNING_STOPPED.to_owned(),
                    kept(&with_command),
                    EXTENSION_STAYS.to_owned(),
                ],
                with_command,
            ),
            (
                "a partial uninstall from one agent",
                vec!["Removed from agents: Zed (restart it to unload Ghostlight's tools).".to_owned()],
                one_agent,
            ),
            (
                "a partial uninstall that took the command",
                vec![
                    "Removed from agents: Zed and Cursor (restart them to unload Ghostlight's tools)."
                        .to_owned(),
                    "Removed from browsers: Brave.".to_owned(),
                    COMMAND_REMOVED.to_owned(),
                    LEFT_RUNNING.to_owned(),
                ],
                partial,
            ),
        ];
        for (case, expected, facts) in cases {
            assert_eq!(uninstall_lines(&facts), expected, "{case}");
        }
    }
}
