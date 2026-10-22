//! The workbench search: what a person can find from the window's search palette, worded the
//! way the window shows it.
//!
//! The facade's [`WorkbenchFacade::search`] matches a query against one look at the service and
//! the recorded history, and answers [`SearchHit`]s that name the page that shows each one.

use serde::Serialize;

use crate::intake::session::IntakeChannel;
use crate::language::workbench as words;

use super::{parts, PageAddress, WorkbenchFacade};

const SEARCH_LIMIT: usize = 100;

impl WorkbenchFacade {
    /// Search what the window shows: its pages, the work in flight, the agents and browsers
    /// connected now, the agents on this computer, the browser-work setting, the checks, and the
    /// recorded actions, newest first.
    ///
    /// A hit matches on the words a person reads for it, never on an opaque id: a recorded action
    /// on its tool, its sentence, the host it landed on, its agent, its browser and its outcome,
    /// so "sylin" finds the actions on sylin.org. Matching allocates nothing per record and only a
    /// match is worded, so a long history stays quick to search. At most [`SEARCH_LIMIT`] hits.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let look = self.look();
        let gathered = look.gathered();
        let mut hits = Hits::new(&query);
        for (page, view) in [
            (&words::SEARCH_GLANCE, PageAddress::Monitor),
            (&words::SEARCH_INTEGRATIONS, PageAddress::Integrations),
            (&words::SEARCH_STATUS, PageAddress::Status),
            (&words::SEARCH_POLICY, PageAddress::Policy),
            (&words::SEARCH_SETTINGS, PageAddress::Settings),
        ] {
            if hits.finds(&[page.title, page.detail]) {
                hits.push(SearchHit::new(
                    SearchKind::Destination,
                    view.as_str(),
                    page.title,
                    page.detail.to_owned(),
                    view,
                ));
            }
        }
        for operation in &gathered.operations {
            let phase = operation.phase.word();
            if hits.finds_action(&operation.tool, &[&operation.agent.name, phase]) {
                hits.push(SearchHit::new(
                    SearchKind::Operation,
                    &operation.invocation,
                    &words::tool_words(&operation.tool),
                    words::search_operation_detail(&operation.agent.name, phase),
                    PageAddress::Monitor,
                ));
            }
        }
        for session in &gathered.sessions {
            let line = words::session_line(
                session.tab_count,
                session.browser.as_deref(),
                session.running_operations > 0,
                session.channel == IntakeChannel::Cli,
            );
            if hits.finds(&[&session.agent.name, &line]) {
                hits.push(SearchHit::new(
                    SearchKind::Session,
                    &session.id,
                    &session.agent.name,
                    line,
                    PageAddress::Monitor,
                ));
            }
        }
        for browser in &gathered.browsers {
            let detail = words::search_browser_detail(browser.tab_count);
            if hits.finds(&[&browser.family, &detail]) {
                hits.push(SearchHit::new(
                    SearchKind::Browser,
                    &browser.id,
                    &browser.family,
                    detail,
                    PageAddress::Monitor,
                ));
            }
        }
        for harness in &gathered.harnesses {
            let word = words::harness_state_word(
                harness.state,
                harness.copy_only,
                parts::in_use(harness, &gathered.sessions),
            );
            if hits.finds(&[&harness.label, word]) {
                hits.push(SearchHit::new(
                    SearchKind::Installation,
                    &harness.id,
                    &harness.label,
                    word.to_owned(),
                    PageAddress::Integrations,
                ));
            }
        }
        let runtime = crate::language::control::state_word(gathered.runtime_state);
        if hits.finds(&[words::CONTROLS_RUNTIME_LABEL, runtime]) {
            hits.push(SearchHit::new(
                SearchKind::Configuration,
                RUNTIME_SETTING_ID,
                words::CONTROLS_RUNTIME_LABEL,
                runtime.to_owned(),
                PageAddress::SettingsBrowserWork,
            ));
        }
        for diagnostic in &gathered.checks() {
            if hits.finds(&[&diagnostic.label, &diagnostic.detail]) {
                hits.push(SearchHit::new(
                    SearchKind::Diagnostic,
                    diagnostic.id.as_str(),
                    &diagnostic.label,
                    diagnostic.detail.clone(),
                    PageAddress::Status,
                ));
            }
        }
        for item in &self.projection.history() {
            let outcome = words::search_outcome(item.outcome);
            let browser = item.browser.as_deref().unwrap_or_default();
            if hits.finds_action(
                &item.tool,
                &[
                    &item.summary,
                    item.observed.host.as_deref().unwrap_or_default(),
                    &item.agent.name,
                    browser,
                    outcome.unwrap_or_default(),
                ],
            ) {
                hits.push(SearchHit {
                    outcome: outcome.map(|_| item.outcome),
                    timestamp_ms: Some(item.timestamp_ms),
                    ..SearchHit::new(
                        SearchKind::History,
                        &item.invocation,
                        &words::search_action_title(&item.tool, &item.summary),
                        words::search_action_detail(&item.agent.name, browser),
                        PageAddress::Monitor,
                    )
                });
            }
        }
        hits.found
    }
}

/// One search result, worded for the person who searched (`crate::language::workbench`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchHit {
    /// Result category.
    pub kind: SearchKind,
    /// Opaque result identity: for a recorded action its invocation, so the window can open its
    /// row.
    pub id: String,
    /// What the hit is, in a person's words.
    pub title: String,
    /// The second line, in a person's words.
    pub detail: String,
    /// When a recorded action settled; the window says how long ago.
    pub timestamp_ms: Option<u64>,
    /// How a recorded action ended, present only when its row says so in a word
    /// ([`words::search_outcome`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<crate::language::history::OutcomeClass>,
    /// The page, and the section of Settings, that shows what the hit found.
    pub view: PageAddress,
}

impl SearchHit {
    fn new(kind: SearchKind, id: &str, title: &str, detail: String, view: PageAddress) -> Self {
        Self {
            kind,
            id: id.to_owned(),
            title: title.to_owned(),
            detail,
            timestamp_ms: None,
            outcome: None,
            view,
        }
    }
}

/// Closed search-result categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// A first-class workbench destination.
    Destination,
    /// Current operation.
    Operation,
    /// Admitted session.
    Session,
    /// Terminal history record.
    History,
    /// Connected browser instance.
    Browser,
    /// Checkup finding.
    Diagnostic,
    /// Runtime or authority configuration.
    Configuration,
    /// Supported development-harness registration.
    Installation,
}

/// The search identity of the browser-work setting.
const RUNTIME_SETTING_ID: &str = "runtime-control";

/// The hits of one search: what matches `query`, up to [`SEARCH_LIMIT`].
struct Hits<'a> {
    query: &'a str,
    found: Vec<SearchHit>,
}

impl<'a> Hits<'a> {
    const fn new(query: &'a str) -> Self {
        Self {
            query,
            found: Vec::new(),
        }
    }

    /// Whether there is room for another hit and one of `fields` holds the query, ignoring ASCII
    /// case. The query is already lower case.
    fn finds(&self, fields: &[&str]) -> bool {
        self.found.len() < SEARCH_LIMIT
            && fields
                .iter()
                .any(|field| holds_folded(field, self.query.as_bytes()))
    }

    /// Whether there is room for another hit and the catalog `tool` or one of `fields` holds the
    /// query. A tool matches by its name or by its words, so "fill form" and "browser_fill_form"
    /// both find `browser_fill_form`.
    fn finds_action(&self, tool: &str, fields: &[&str]) -> bool {
        self.finds(fields)
            || (self.found.len() < SEARCH_LIMIT && holds_tool(tool, self.query.as_bytes()))
    }

    fn push(&mut self, hit: SearchHit) {
        self.found.push(hit);
    }
}

/// Whether the catalog name `tool` holds `needle`, reading an underscore and a space as the same
/// character, so the tool's words match its name ([`words::tool_words`]).
fn holds_tool(tool: &str, needle: &[u8]) -> bool {
    let same = |left: u8, right: u8| {
        left.eq_ignore_ascii_case(&right) || matches!((left, right), (b'_', b' ') | (b' ', b'_'))
    };
    tool.as_bytes().windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(&left, &right)| same(left, right))
    })
}

/// Whether `field` holds `needle`, comparing ASCII letters without case and allocating nothing.
fn holds_folded(field: &str, needle: &[u8]) -> bool {
    field
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}
