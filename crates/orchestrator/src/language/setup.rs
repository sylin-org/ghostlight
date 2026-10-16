//! The words of the install journey: what the window's setup view says at each stage, and what
//! `ghostlight install` prints when it finishes.
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

use serde::{Deserialize, Serialize};

use super::environment::{Environment, OPEN_NEW_TERMINAL};

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
/// The word for a connection that is made: a step word without a name, and a row that is done.
const WORD_CONNECTED: &str = "Connected";
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
    /// An ordinary installation that can start Ghostlight's connector.
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
    /// Whether the agent runs in an app container, where Ghostlight cannot write its entry.
    pub private_view: bool,
    /// Whether the service has seen a session from this agent.
    pub seen: bool,
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
/// for `get_chrome`, and sends `review_agent` to the integrations tab.
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
    /// Write Ghostlight's entry into the agent.
    ConnectAgent {
        /// The agent's identifier.
        agent: String,
    },
    /// Go to the integrations tab to look at the agent's entry.
    ReviewAgent {
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

/// Whether the browser can run Ghostlight's connector: it is connected, holds the extension, or is
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
    if agent.seen {
        return row(WORD_CONNECTED.to_owned(), RowTone::Done, None);
    }
    match agent.state {
        AgentState::Written => row(
            format!("Restart {} to finish.", agent.label),
            RowTone::Attention,
            None,
        ),
        AgentState::Found => row(
            "Found".to_owned(),
            RowTone::Todo,
            (!agent.private_view).then(|| RowAction {
                action: SetupAction::ConnectAgent {
                    agent: agent.id.clone(),
                },
                label: "Connect".to_owned(),
                name: format!("Connect {}", agent.label),
                primary: false,
                external: false,
                pending: Some(Pending {
                    detail: "Connecting".to_owned(),
                    live: None,
                }),
            }),
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
    let browser_added = facts
        .browsers
        .iter()
        .any(|browser| browser.connected || browser.extension.is_some());
    let restart_first = !facts.complete && !browser_added && !changed.is_empty();
    if facts.agents.is_empty() {
        lines.push(format!(
            "No agents were found on this computer. Ghostlight's MCP server command: {}",
            facts.mcp_command
        ));
    } else if !with_entry.is_empty() {
        let note = if restart_first {
            let pronoun = if changed.len() == 1 { "it" } else { "them" };
            format!(" (restart {pronoun} before you ask)")
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
        lines.push(next_line(facts, browser_added, &changed));
    }
    lines
}

fn next_line(facts: &InstallFacts, browser_added: bool, changed: &[&str]) -> String {
    if browser_added {
        if changed.is_empty() {
            return format!("Next: ask your agent: {PROMPT}");
        }
        let pronoun = if changed.len() == 1 { "it" } else { "one" };
        return format!(
            "Next: restart {}, then ask {pronoun}: {PROMPT}",
            join_list(changed)
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
            directory: "Profile 1".to_owned(),
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
            private_view: false,
            seen: false,
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

    fn step_states(view: &SetupView) -> Vec<StepState> {
        view.steps.iter().map(|step| step.state).collect()
    }

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

    #[test]
    fn the_computer_stage_gives_the_finishing_command_for_the_platform() {
        let mut state = facts(vec![], vec![]);
        state.finish_on_this_computer = true;
        let view = compose(&state);
        assert_eq!(view.stage, Stage::Computer);
        assert_eq!(view.eyebrow, "Set up Ghostlight");
        assert_eq!(
            view.headline,
            Headline::new("Finish installing on ", "this computer", ".")
        );
        assert_eq!(
            view.lead.as_deref(),
            Some("Run this once from a regular terminal.")
        );
        let code = view.code.as_ref().unwrap();
        assert_eq!(code.label, "Paste into PowerShell from the Start menu");
        assert_eq!(code.text, "~\\.ghostlight\\bin\\ghostlight.exe install");
        assert_eq!(code.copy_name, "Copy command");
        assert_eq!(
            view.live.as_ref().unwrap().text,
            "Waiting for this computer."
        );
        assert_eq!(
            step_states(&view),
            [StepState::Current, StepState::Todo, StepState::Todo]
        );
        assert_eq!(view.light, Light::Waiting);
        assert!(view.rows.is_empty());

        state.platform = Platform::Linux;
        let code = compose(&state).code.unwrap();
        assert_eq!(code.label, "Paste into a terminal");
        assert_eq!(code.text, "~/.ghostlight/bin/ghostlight install");
    }

    #[test]
    fn the_browser_stage_asks_for_a_browser() {
        let view = compose(&facts(vec![chrome(BrowserForm::Native)], vec![]));
        assert_eq!(view.stage, Stage::Browser);
        assert_eq!(
            view.headline,
            Headline::new("Add Ghostlight to ", "your browser", ".")
        );
        assert_eq!(view.live.as_ref().unwrap().text, "Waiting for a browser.");
        assert!(!view.live.as_ref().unwrap().done);
        assert_eq!(view.light, Light::Waiting);
        assert_eq!(
            step_states(&view),
            [StepState::Done, StepState::Current, StepState::Todo]
        );
        assert_eq!(view.steps[0].word, "Installed");
        assert!(view.code.is_none());
        assert!(view.connected_line.is_none());
    }

    #[test]
    fn a_native_browser_without_the_extension_offers_add() {
        let view = compose(&facts(
            vec![chrome(BrowserForm::Native), brave(BrowserForm::Native)],
            vec![],
        ));
        let chrome_row = row(&view, "chrome");
        assert_eq!(chrome_row.detail, "Not added");
        assert_eq!(chrome_row.tone, RowTone::Todo);
        let action = chrome_row.action.as_ref().unwrap();
        assert_eq!(
            action.action,
            SetupAction::AddInBrowser {
                browser: "chrome".to_owned()
            }
        );
        assert_eq!(action.label, "Add");
        assert_eq!(action.name, "Add to Google Chrome");
        assert!(action.primary && action.external);
        let pending = action.pending.as_ref().unwrap();
        assert_eq!(pending.detail, "Finish in the Chrome Web Store.");
        assert_eq!(pending.live.as_deref(), Some("Waiting for Chrome."));
        let brave_action = row(&view, "brave").action.as_ref().unwrap();
        assert!(!brave_action.primary && brave_action.external);
        assert_eq!(
            brave_action.pending.as_ref().unwrap().live.as_deref(),
            Some("Waiting for Brave.")
        );
    }

    #[test]
    fn edge_gets_add_from_the_chrome_web_store() {
        let view = compose(&facts(vec![edge(BrowserForm::Native)], vec![]));
        let action = row(&view, "edge").action.as_ref().unwrap();
        assert_eq!(
            action.action,
            SetupAction::AddInBrowser {
                browser: "edge".to_owned()
            }
        );
        assert_eq!(action.name, "Add to Microsoft Edge");
        assert!(action.primary);
    }

    #[test]
    fn rows_put_connected_first_then_chrome_then_the_table_order() {
        let rows = browser_rows(&facts(
            vec![
                edge(BrowserForm::Native),
                chrome(BrowserForm::Native),
                brave(BrowserForm::Native),
                connected(browser(
                    "chromium",
                    "Chromium",
                    "Chromium",
                    BrowserForm::Native,
                )),
            ],
            vec![],
        ));
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["chromium", "chrome", "edge", "brave"]);
        assert_eq!(rows[0].detail, "Connected");
        assert_eq!(rows[0].tone, RowTone::Done);
        assert!(rows[0].action.is_none());
    }

    #[test]
    fn a_browser_that_is_not_installed_has_no_row() {
        let view = compose(&facts(
            vec![chrome(BrowserForm::Missing), brave(BrowserForm::Native)],
            vec![],
        ));
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0].id, "brave");
    }

    #[test]
    fn an_extension_in_a_profile_offers_open() {
        let several = compose(&facts(
            vec![with_extension(chrome(BrowserForm::Native), Some("Work"), 3)],
            vec![],
        ));
        let chrome_row = row(&several, "chrome");
        assert_eq!(
            chrome_row.detail,
            "Added in your Work profile. Open Chrome to finish."
        );
        assert_eq!(chrome_row.tone, RowTone::Todo);
        let action = chrome_row.action.as_ref().unwrap();
        assert_eq!(
            action.action,
            SetupAction::OpenBrowser {
                browser: "chrome".to_owned(),
                profile: Some("Profile 1".to_owned())
            }
        );
        assert_eq!(action.label, "Open Chrome");
        assert_eq!(action.name, "Open Google Chrome");
        assert!(!action.primary && !action.external);
        let pending = action.pending.as_ref().unwrap();
        assert_eq!(pending.detail, "Opening Chrome.");
        assert_eq!(pending.live.as_deref(), Some("Waiting for Chrome."));

        let unnamed = compose(&facts(
            vec![with_extension(chrome(BrowserForm::Native), None, 3)],
            vec![],
        ));
        assert_eq!(
            row(&unnamed, "chrome").detail,
            "Added. Open Chrome to finish."
        );

        let single = compose(&facts(
            vec![with_extension(
                chrome(BrowserForm::Native),
                Some("Person 1"),
                1,
            )],
            vec![],
        ));
        let single_row = row(&single, "chrome");
        assert_eq!(single_row.detail, "Added. Open Chrome to finish.");
        assert_eq!(
            single_row.action.as_ref().unwrap().action,
            SetupAction::OpenBrowser {
                browser: "chrome".to_owned(),
                profile: None
            }
        );
    }

    #[test]
    fn sandboxed_browsers_show_only_when_none_is_native() {
        let sandboxed = compose(&facts(
            vec![
                chrome(BrowserForm::Snap),
                brave(BrowserForm::Flatpak),
                edge(BrowserForm::SnapAndFlatpak),
            ],
            vec![],
        ));
        assert_eq!(
            row(&sandboxed, "chrome").detail,
            "Snap keeps it sealed off from the rest of this computer."
        );
        assert_eq!(
            row(&sandboxed, "brave").detail,
            "Flatpak keeps it sealed off from the rest of this computer."
        );
        assert_eq!(
            row(&sandboxed, "edge").detail,
            "Snap and Flatpak keep it sealed off from the rest of this computer."
        );
        for item in &sandboxed.rows {
            assert_eq!(item.tone, RowTone::Off);
            let action = item.action.as_ref().unwrap();
            assert_eq!(action.action, SetupAction::GetChrome);
            assert_eq!(action.label, "Get Google Chrome");
            assert!(action.external);
        }
        assert!(sandboxed.rows[0].action.as_ref().unwrap().primary);
        assert!(!sandboxed.rows[1].action.as_ref().unwrap().primary);

        let mixed = compose(&facts(
            vec![chrome(BrowserForm::Snap), brave(BrowserForm::Native)],
            vec![],
        ));
        assert_eq!(mixed.rows.len(), 1);
        assert_eq!(mixed.rows[0].id, "brave");
    }

    #[test]
    fn no_browser_at_all_offers_google_chrome() {
        for browsers in [
            vec![],
            vec![chrome(BrowserForm::Missing), edge(BrowserForm::Missing)],
        ] {
            let view = compose(&facts(browsers, vec![]));
            assert_eq!(view.rows.len(), 1);
            let only = &view.rows[0];
            assert_eq!(only.name, "Google Chrome");
            assert_eq!(only.detail, "Not installed");
            let action = only.action.as_ref().unwrap();
            assert_eq!(action.action, SetupAction::GetChrome);
            assert!(action.primary && action.external);
            assert!(view.lead.is_none());
        }
    }

    #[test]
    fn one_browser_is_enough_appears_with_two_actions() {
        let one = compose(&facts(vec![chrome(BrowserForm::Native)], vec![]));
        assert!(one.lead.is_none());
        let two = compose(&facts(
            vec![chrome(BrowserForm::Native), edge(BrowserForm::Native)],
            vec![],
        ));
        assert_eq!(two.lead.as_deref(), Some("One browser is enough."));
        let open_and_add = compose(&facts(
            vec![
                with_extension(chrome(BrowserForm::Native), None, 1),
                brave(BrowserForm::Native),
            ],
            vec![],
        ));
        assert_eq!(open_and_add.lead.as_deref(), Some("One browser is enough."));
    }

    #[test]
    fn the_agent_stage_asks_the_agent() {
        let mut found = agent("claude-code", "Claude Code", AgentState::Found);
        found.private_view = false;
        let view = compose(&facts(
            vec![connected(chrome(BrowserForm::Native))],
            vec![found],
        ));
        assert_eq!(view.stage, Stage::Agent);
        assert_eq!(view.headline, Headline::new("Ask ", "your agent", "."));
        let code = view.code.as_ref().unwrap();
        assert_eq!(code.label, "Paste into your agent");
        assert_eq!(
            code.text,
            "Open sylin.org/ghostlight/demo/desk and ring the bell."
        );
        assert_eq!(code.copy_name, "Copy prompt");
        assert_eq!(view.live.as_ref().unwrap().text, "Waiting for your agent.");
        assert_eq!(view.rows_label.as_deref(), Some("Agents on this computer"));
        assert_eq!(view.light, Light::Lit);
        assert_eq!(view.connected_line.as_deref(), Some("Chrome is connected."));
        assert_eq!(
            step_states(&view),
            [StepState::Done, StepState::Done, StepState::Current]
        );
        assert_eq!(view.steps[1].word, "Connected in Chrome");
        assert_eq!(view.steps[1].sr, "Your browser, done");
        assert_eq!(view.steps[2].sr, "Your agent, current step");
        assert_eq!(view.steps[0].sr, "This computer, done");
        assert!(view.extra_code.is_none());
    }

    #[test]
    fn with_no_agent_found_the_mcp_command_is_the_action() {
        let view = compose(&facts(vec![connected(chrome(BrowserForm::Native))], vec![]));
        assert_eq!(view.stage, Stage::Agent);
        assert_eq!(
            view.headline,
            Headline::new("Add Ghostlight to ", "your agent", ".")
        );
        assert_eq!(
            view.lead.as_deref(),
            Some("No agents were found on this computer.")
        );
        let code = view.code.as_ref().unwrap();
        assert_eq!(code.label, "MCP server command");
        assert_eq!(code.text, "ghostlight mcp");
        assert_eq!(code.copy_name, "Copy command");
        let extra = view.extra_code.as_ref().unwrap();
        assert_eq!(extra.label, "Then ask it");
        assert_eq!(extra.text, PROMPT);
        assert_eq!(extra.copy_name, "Copy prompt");
        assert_eq!(view.live.as_ref().unwrap().text, "Waiting for your agent.");
        assert!(view.rows.is_empty());
        assert!(view.rows_label.is_none());
    }

    #[test]
    fn an_agent_row_appearing_returns_the_plain_form() {
        let mut state = facts(vec![connected(chrome(BrowserForm::Native))], vec![]);
        assert!(compose(&state).extra_code.is_some());
        state
            .agents
            .push(agent("codex", "Codex", AgentState::Found));
        let view = compose(&state);
        assert!(view.extra_code.is_none());
        assert_eq!(view.headline.lead, "Ask ");
    }

    #[test]
    fn every_agent_row_reads_its_state() {
        let mut seen = agent("claude-code", "Claude Code", AgentState::Written);
        seen.seen = true;
        let written = agent("codex", "Codex", AgentState::Written);
        let found = agent("cursor", "Cursor", AgentState::Found);
        let mut private = agent("desktop", "Claude Desktop", AgentState::Found);
        private.private_view = true;
        let attention = agent("zed", "Zed", AgentState::NeedsAttention);
        let view = compose(&facts(
            vec![connected(chrome(BrowserForm::Native))],
            vec![seen, written, found, private, attention],
        ));

        let connected_row = row(&view, "claude-code");
        assert_eq!(connected_row.detail, "Connected");
        assert_eq!(connected_row.tone, RowTone::Done);
        assert!(connected_row.action.is_none());

        let restart = row(&view, "codex");
        assert_eq!(restart.detail, "Restart Codex to finish.");
        assert_eq!(restart.tone, RowTone::Attention);
        assert!(restart.action.is_none());

        let found_row = row(&view, "cursor");
        assert_eq!(found_row.detail, "Found");
        assert_eq!(found_row.tone, RowTone::Todo);
        let connect = found_row.action.as_ref().unwrap();
        assert_eq!(
            connect.action,
            SetupAction::ConnectAgent {
                agent: "cursor".to_owned()
            }
        );
        assert_eq!(connect.label, "Connect");
        assert_eq!(connect.name, "Connect Cursor");
        assert!(!connect.primary && !connect.external);
        assert_eq!(
            connect.pending,
            Some(Pending {
                detail: "Connecting".to_owned(),
                live: None
            })
        );

        let private_row = row(&view, "desktop");
        assert_eq!(private_row.detail, "Found");
        assert!(private_row.action.is_none());

        let review_row = row(&view, "zed");
        assert_eq!(review_row.detail, "Needs attention");
        assert_eq!(review_row.tone, RowTone::Attention);
        let review = review_row.action.as_ref().unwrap();
        assert_eq!(
            review.action,
            SetupAction::ReviewAgent {
                agent: "zed".to_owned()
            }
        );
        assert_eq!(review.label, "Review");
        assert_eq!(review.name, "Review Zed");
        assert!(review.pending.is_none());
    }

    #[test]
    fn done_names_the_agent_or_falls_back() {
        let mut state = facts(vec![connected(chrome(BrowserForm::Native))], vec![]);
        state.complete = true;
        state.finished = true;
        state.agent_name = Some("Claude Code".to_owned());
        let named = compose(&state);
        assert_eq!(named.stage, Stage::Done);
        assert_eq!(named.eyebrow, "Ready");
        assert_eq!(
            named.headline,
            Headline::new("Claude Code just used your browser. ", "You're set.", "")
        );
        assert_eq!(named.steps[2].word, "Claude Code");
        assert_eq!(named.tagline.as_deref(), Some(TAGLINE));
        assert_eq!(named.light, Light::Lit);
        assert!(named.live.is_none());
        assert_eq!(
            step_states(&named),
            [StepState::Done, StepState::Done, StepState::Done]
        );

        state.agent_name = None;
        let unnamed = compose(&state);
        assert_eq!(unnamed.headline.lead, "Your agent just used your browser. ");
        assert_eq!(unnamed.steps[2].word, "Connected");
    }

    #[test]
    fn retired_says_nothing() {
        let mut state = facts(vec![], vec![]);
        state.complete = true;
        let view = compose(&state);
        assert_eq!(view.stage, Stage::Retired);
        assert!(view.headline.lead.is_empty());
        assert!(view.rows.is_empty());
        assert!(view.live.is_none() && view.code.is_none() && view.tagline.is_none());
    }

    #[test]
    fn the_view_serializes_with_the_tags_the_page_reads() {
        let view = compose(&facts(vec![chrome(BrowserForm::Native)], vec![]));
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["stage"], "browser");
        assert_eq!(json["light"], "waiting");
        assert_eq!(json["steps"][1]["state"], "current");
        assert_eq!(json["rows"][0]["tone"], "todo");
        assert_eq!(
            json["rows"][0]["action"]["action"]["kind"],
            "add_in_browser"
        );
        assert_eq!(json["rows"][0]["action"]["action"]["browser"], "chrome");
        let parsed: SetupAction =
            serde_json::from_str(r#"{"kind":"open_browser","browser":"edge"}"#).unwrap();
        assert_eq!(
            parsed,
            SetupAction::OpenBrowser {
                browser: "edge".to_owned(),
                profile: None
            }
        );
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
            agents,
            mcp_command: "ghostlight mcp".to_owned(),
            browsers,
            complete: false,
            window_shown: true,
        }
    }

    fn next_lines(lines: &[String]) -> Vec<&String> {
        lines
            .iter()
            .filter(|line| line.starts_with("Next:"))
            .collect()
    }

    #[test]
    fn a_first_install_with_the_window_points_at_the_window() {
        let lines = install_lines(&install(
            vec![chrome(BrowserForm::Native)],
            agents_with_entries(&[("Claude Code", true), ("Codex", true)]),
        ));
        assert_eq!(
            lines,
            [
                "Ghostlight 2.0.0 is installed.",
                "Ghostlight is in your notification area, and in the Start menu.",
                "Agents: Claude Code and Codex (restart them before you ask).",
                "Next: add Ghostlight to Chrome in the Ghostlight window.",
            ]
        );
    }

    #[test]
    fn a_first_install_without_the_window_gives_the_listing() {
        let mut facts = install(
            vec![chrome(BrowserForm::Native)],
            agents_with_entries(&[("Claude Code", true)]),
        );
        facts.window_shown = false;
        let lines = install_lines(&facts);
        assert_eq!(lines[2], "Agents: Claude Code (restart it before you ask).");
        assert_eq!(
            lines[3],
            format!("Next: add Ghostlight to Chrome: {STORE_LISTING_URL}")
        );
    }

    #[test]
    fn the_next_line_names_the_first_native_browser_in_row_order() {
        let lines = install_lines(&install(
            vec![
                edge(BrowserForm::Native),
                chrome(BrowserForm::Native),
                brave(BrowserForm::Native),
            ],
            agents_with_entries(&[("Codex", false)]),
        ));
        assert_eq!(lines[2], "Agents: Codex.");
        assert_eq!(
            lines[3],
            "Next: add Ghostlight to Chrome in the Ghostlight window."
        );
        let lines = install_lines(&install(
            vec![edge(BrowserForm::Native), chrome(BrowserForm::Snap)],
            vec![],
        ));
        assert!(lines
            .iter()
            .any(|line| line.contains("add Ghostlight to Edge")));
    }

    #[test]
    fn an_extension_in_a_profile_asks_for_the_prompt() {
        let browsers = vec![with_extension(chrome(BrowserForm::Native), None, 1)];
        let changed = install_lines(&install(
            browsers.clone(),
            agents_with_entries(&[("Claude Code", true), ("Codex", true), ("Cursor", true)]),
        ));
        assert_eq!(changed[2], "Agents: Claude Code, Codex and Cursor.");
        assert_eq!(
            changed[3],
            format!("Next: restart Claude Code, Codex and Cursor, then ask one: {PROMPT}")
        );
        let one = install_lines(&install(
            browsers.clone(),
            agents_with_entries(&[("Codex", true)]),
        ));
        assert_eq!(
            one[3],
            format!("Next: restart Codex, then ask it: {PROMPT}")
        );
        let unchanged = install_lines(&install(browsers, agents_with_entries(&[("Codex", false)])));
        assert_eq!(unchanged[3], format!("Next: ask your agent: {PROMPT}"));
    }

    #[test]
    fn a_complete_reinstall_has_no_next_line() {
        let mut facts = install(
            vec![connected(chrome(BrowserForm::Native))],
            agents_with_entries(&[("Codex", false)]),
        );
        facts.complete = true;
        let quiet = install_lines(&facts);
        assert_eq!(quiet.len(), 3);
        assert!(next_lines(&quiet).is_empty());

        facts.agents = agents_with_entries(&[("Claude Code", true), ("Codex", true)]);
        let restart = install_lines(&facts);
        assert_eq!(
            restart.last().unwrap(),
            "Restart Claude Code and Codex to use Ghostlight."
        );
        assert!(next_lines(&restart).is_empty());
    }

    #[test]
    fn with_no_agents_the_install_gives_the_mcp_command() {
        let lines = install_lines(&install(vec![chrome(BrowserForm::Native)], vec![]));
        assert_eq!(
            lines[2],
            "No agents were found on this computer. Ghostlight's MCP server command: ghostlight mcp"
        );
        assert_eq!(next_lines(&lines).len(), 1);
    }

    #[test]
    fn a_detected_agent_without_an_entry_is_not_listed() {
        let lines = install_lines(&install(
            vec![chrome(BrowserForm::Native)],
            vec![InstallAgent {
                label: "Zed".to_owned(),
                has_entry: false,
                changed: false,
            }],
        ));
        assert!(!lines.iter().any(|line| line.starts_with("Agents:")));
        assert!(!lines.iter().any(|line| line.starts_with("No agents")));
    }

    #[test]
    fn a_path_just_written_asks_for_a_new_terminal() {
        let mut facts = install(vec![chrome(BrowserForm::Native)], vec![]);
        facts.path_written = true;
        let lines = install_lines(&facts);
        assert_eq!(lines[2], OPEN_NEW_TERMINAL);
    }

    #[test]
    fn the_environment_caveat_follows_the_location() {
        let mut facts = install(vec![chrome(BrowserForm::Native)], vec![]);
        facts.environment = Environment::Wsl;
        let lines = install_lines(&facts);
        assert_eq!(lines[1], Environment::Wsl.location());
        assert_eq!(lines[2], Environment::Wsl.caveat().unwrap());
    }

    #[test]
    fn with_no_native_browser_the_install_points_at_chrome() {
        let mut facts = install(vec![chrome(BrowserForm::Snap)], vec![]);
        let shown = install_lines(&facts);
        assert_eq!(
            shown.last().unwrap(),
            "Next: get Google Chrome from the Ghostlight window."
        );
        facts.window_shown = false;
        let hidden = install_lines(&facts);
        assert_eq!(
            hidden.last().unwrap(),
            &format!("Next: install Google Chrome, then add Ghostlight there: {STORE_LISTING_URL}")
        );
        facts.browsers = vec![];
        assert_eq!(install_lines(&facts), hidden);
    }

    #[test]
    fn an_incomplete_install_has_exactly_one_next_line_and_never_says_to_finish() {
        let browser_sets = [
            vec![],
            vec![chrome(BrowserForm::Native)],
            vec![chrome(BrowserForm::Snap)],
            vec![with_extension(chrome(BrowserForm::Native), Some("Work"), 2)],
            vec![connected(chrome(BrowserForm::Native))],
        ];
        let agent_sets = [
            vec![],
            agents_with_entries(&[("Codex", true)]),
            agents_with_entries(&[("Codex", false), ("Cursor", true)]),
        ];
        for browsers in &browser_sets {
            for agents in &agent_sets {
                for window_shown in [true, false] {
                    for path_written in [true, false] {
                        let mut facts = install(browsers.clone(), agents.clone());
                        facts.window_shown = window_shown;
                        facts.path_written = path_written;
                        let lines = install_lines(&facts);
                        assert_eq!(next_lines(&lines).len(), 1, "{lines:?}");
                        assert!(lines.iter().all(|line| !line.contains("to finish")));
                        assert!(lines.iter().all(|line| line.is_ascii()));
                    }
                }
            }
        }
    }

    #[test]
    fn lists_join_the_way_a_person_says_them() {
        assert_eq!(join_list(&[]), "");
        assert_eq!(join_list(&["A"]), "A");
        assert_eq!(join_list(&["A", "B"]), "A and B");
        assert_eq!(join_list(&["A", "B", "C"]), "A, B and C");
        assert_eq!(join_list(&["A", "B", "C", "D"]), "A, B, C and D");
    }

    #[test]
    fn the_listing_url_names_the_store_extension() {
        assert!(STORE_LISTING_URL.ends_with(STORE_EXTENSION_ID));
    }

    #[test]
    fn every_string_is_ascii() {
        let mut state = facts(
            vec![
                connected(chrome(BrowserForm::Native)),
                with_extension(edge(BrowserForm::Native), Some("Work"), 2),
                brave(BrowserForm::Native),
            ],
            vec![
                agent("a", "Alpha", AgentState::Found),
                agent("b", "Beta", AgentState::Written),
                agent("c", "Gamma", AgentState::NeedsAttention),
            ],
        );
        state.agent_name = Some("Alpha".to_owned());
        let mut views = Vec::new();
        for (complete, finished, finish_here) in [
            (false, false, false),
            (false, false, true),
            (true, false, false),
            (true, true, false),
        ] {
            state.complete = complete;
            state.finished = finished;
            state.finish_on_this_computer = finish_here;
            for platform in [Platform::Windows, Platform::Linux] {
                state.platform = platform;
                views.push(compose(&state));
            }
        }
        state.complete = false;
        state.finish_on_this_computer = false;
        state.browsers = vec![chrome(BrowserForm::Snap)];
        views.push(compose(&state));
        state.browsers = vec![];
        views.push(compose(&state));
        state.browsers = vec![connected(chrome(BrowserForm::Native))];
        state.agents = vec![];
        views.push(compose(&state));
        for view in views {
            let json = serde_json::to_string(&view).unwrap();
            assert!(json.is_ascii(), "{json}");
        }
        assert!(STEP_NAMES.iter().all(|name| name.is_ascii()));
        assert!(STORE_LISTING_URL.is_ascii() && PROMPT.is_ascii() && TAGLINE.is_ascii());
    }
}
