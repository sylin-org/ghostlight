//! The three contexts a workbench request can be in, and what each may do.
//!
//! Every request is in one context. Control carries the right key: the Ghostlight window. Watch
//! carries none while "Others can watch" is on: a browser tab. Closed carries none while watching
//! is off. `GET /api/context` tells the page which one it is in and which acts it may offer, so
//! the page renders what it is told and never guesses (ADR-0199, amendment of 2026-10-15).

use serde::Serialize;
use serde_json::{json, Map, Value};

use super::parts::Audience;
use super::WorkbenchDestination;
use crate::language::workbench as words;

/// Who is asking, as far as access goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebContext {
    /// The request carries this start's key.
    Control,
    /// No key, and watching is on.
    Watch,
    /// No key, and watching is off.
    Closed,
}

impl WebContext {
    /// The token the page reads.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Watch => "watch",
            Self::Closed => "closed",
        }
    }

    /// What a projection is made for in this context. Closed never receives data.
    #[must_use]
    pub const fn audience(self) -> Option<Audience> {
        match self {
            Self::Control => Some(Audience::Control),
            Self::Watch => Some(Audience::Watch),
            Self::Closed => None,
        }
    }

    /// The short word for this context.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Control => words::WORD_CONTROL,
            Self::Watch => words::WORD_WATCH,
            Self::Closed => words::WORD_CLOSED,
        }
    }

    /// The sentence that says who can see this view.
    #[must_use]
    pub const fn sentence(self, watching: bool) -> &'static str {
        match self {
            Self::Control if watching => words::SENTENCE_CONTROL_WATCHED,
            Self::Control => words::SENTENCE_CONTROL_PRIVATE,
            Self::Watch => words::SENTENCE_WATCH,
            Self::Closed => words::WATCHING_OFF,
        }
    }
}

/// Every act a view may offer. A closed list: an act missing here is not offered anywhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebAct {
    /// Search the workbench.
    Search,
    /// Check a draft policy against recorded work.
    CheckPolicy,
    /// Look again for supported clients.
    Rescan,
    /// Bring the Ghostlight window forward.
    ContinueInWindow,
    /// Pause agents.
    Hold,
    /// Let agents work again.
    Resume,
    /// Start a fresh session.
    StartSession,
    /// End the session.
    EndSession,
    /// Apply the person's rules.
    ApplyPolicy,
    /// Remove the person's rules.
    RemovePolicy,
    /// Turn process diagnostics on or off.
    SetDiagnostics,
    /// Turn "Others can watch" on or off.
    SetWatching,
    /// Connect every client Ghostlight can.
    SetupAll,
    /// Install, fix or remove one client's setup.
    HarnessAct,
    /// Show a test notification.
    TestNotification,
    /// Open a link or a client's download page in the person's browser.
    Open,
    /// Show the diagnostics folder in the file manager.
    RevealDiagnostics,
    /// Ask the person for a client's application or settings file.
    Locate,
    /// Start a browser on a step of the setup view.
    SetupAct,
    /// End Ghostlight, as the tray's Quit does.
    Quit,
}

impl WebAct {
    /// Every act, in the order the context document lists them.
    pub const ALL: [Self; 20] = [
        Self::Search,
        Self::CheckPolicy,
        Self::Rescan,
        Self::ContinueInWindow,
        Self::Hold,
        Self::Resume,
        Self::StartSession,
        Self::EndSession,
        Self::ApplyPolicy,
        Self::RemovePolicy,
        Self::SetDiagnostics,
        Self::SetWatching,
        Self::SetupAll,
        Self::HarnessAct,
        Self::TestNotification,
        Self::Open,
        Self::RevealDiagnostics,
        Self::Locate,
        Self::SetupAct,
        Self::Quit,
    ];

    /// The key the page names this act by.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::CheckPolicy => "check_policy",
            Self::Rescan => "rescan",
            Self::ContinueInWindow => "continue_in_window",
            Self::Hold => "hold",
            Self::Resume => "resume",
            Self::StartSession => "start_session",
            Self::EndSession => "end_session",
            Self::ApplyPolicy => "apply_policy",
            Self::RemovePolicy => "remove_policy",
            Self::SetDiagnostics => "set_diagnostics",
            Self::SetWatching => "set_watching",
            Self::SetupAll => "setup_all",
            Self::HarnessAct => "harness_act",
            Self::TestNotification => "test_notification",
            Self::Open => "open",
            Self::RevealDiagnostics => "reveal_diagnostics",
            Self::Locate => "locate",
            Self::SetupAct => "setup_act",
            Self::Quit => "quit",
        }
    }

    /// Whether a view that only watches may do this act.
    const fn open_to_watchers(self) -> bool {
        matches!(
            self,
            Self::Search | Self::CheckPolicy | Self::Rescan | Self::ContinueInWindow | Self::Hold
        )
    }
}

/// Whether one act is available in a context, and the sentence for the page when it is not.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Availability {
    /// Whether the view may offer the act.
    pub available: bool,
    /// Why not, when it may not.
    pub why_not: Option<String>,
}

/// What `act` is in `context`.
#[must_use]
pub fn availability(act: WebAct, context: WebContext) -> Availability {
    let why_not = match context {
        WebContext::Control => None,
        WebContext::Watch if act.open_to_watchers() => None,
        WebContext::Watch => Some(words::CHANGE_IN_WINDOW),
        WebContext::Closed => Some(words::WATCHING_OFF),
    };
    Availability {
        available: why_not.is_none(),
        why_not: why_not.map(str::to_owned),
    }
}

/// How the page opens a link in the window: the service opens it.
const OPENER_SERVICE: &str = "service";

/// How the page opens a link in a browser tab: an ordinary anchor.
const OPENER_ANCHOR: &str = "anchor";

/// Every address the product points at, by the key of its destination.
///
/// Every context gets the table: a tab opens a destination as an ordinary anchor, and the About
/// page shows in full while watching is off. The page holds no address of its own.
fn links() -> Map<String, Value> {
    WorkbenchDestination::all()
        .into_iter()
        .map(|destination| (destination.key().to_owned(), json!(destination.url())))
        .collect()
}

/// The body of `GET /api/context`.
///
/// Closed holds only the context, its word and sentence, the product and the links: nothing about
/// this computer's state.
#[must_use]
pub fn context_document(context: WebContext, watching: bool, instance: &str) -> Value {
    let mut document = Map::new();
    document.insert("context".into(), json!(context.token()));
    document.insert("word".into(), json!(context.word()));
    document.insert("sentence".into(), json!(context.sentence(watching)));
    if context != WebContext::Closed {
        let acts: Map<String, Value> = WebAct::ALL
            .into_iter()
            .map(|act| {
                (
                    act.key().to_owned(),
                    serde_json::to_value(availability(act, context))
                        .expect("an availability is plain data"),
                )
            })
            .collect();
        document.insert("instance".into(), json!(instance));
        document.insert("acts".into(), Value::Object(acts));
        let opener = if context == WebContext::Control {
            OPENER_SERVICE
        } else {
            OPENER_ANCHOR
        };
        document.insert("opener".into(), json!(opener));
    }
    document.insert(
        "product".into(),
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "license": words::PRODUCT_LICENSE,
        }),
    );
    document.insert("links".into(), Value::Object(links()));
    Value::Object(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_has_every_act() {
        for act in WebAct::ALL {
            assert!(availability(act, WebContext::Control).available, "{act:?}");
        }
    }

    #[test]
    fn watch_has_the_harmless_acts_and_says_where_to_change_the_rest() {
        let open = [
            WebAct::Search,
            WebAct::CheckPolicy,
            WebAct::Rescan,
            WebAct::ContinueInWindow,
            WebAct::Hold,
        ];
        for act in WebAct::ALL {
            let answer = availability(act, WebContext::Watch);
            assert_eq!(answer.available, open.contains(&act), "{act:?}");
            if !answer.available {
                assert_eq!(answer.why_not.as_deref(), Some(words::CHANGE_IN_WINDOW));
            }
        }
    }

    #[test]
    fn quit_is_the_windows_alone() {
        assert_eq!(WebAct::ALL.len(), 20);
        assert_eq!(WebAct::ALL.last().map(|act| act.key()), Some("quit"));
        assert!(availability(WebAct::Quit, WebContext::Control).available);
        let watcher = availability(WebAct::Quit, WebContext::Watch);
        assert!(!watcher.available);
        assert_eq!(watcher.why_not.as_deref(), Some(words::CHANGE_IN_WINDOW));
        let closed = availability(WebAct::Quit, WebContext::Closed);
        assert!(!closed.available);
        assert_eq!(closed.why_not.as_deref(), Some(words::WATCHING_OFF));
    }

    #[test]
    fn closed_has_no_act_and_says_watching_is_off() {
        for act in WebAct::ALL {
            let answer = availability(act, WebContext::Closed);
            assert!(!answer.available);
            assert_eq!(answer.why_not.as_deref(), Some(words::WATCHING_OFF));
        }
    }

    #[test]
    fn act_keys_are_distinct_snake_case_words() {
        let mut keys: Vec<_> = WebAct::ALL.iter().map(|act| act.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), WebAct::ALL.len());
        for key in keys {
            assert!(key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_'));
        }
    }

    #[test]
    fn the_window_is_told_who_can_see_it() {
        let watched = context_document(WebContext::Control, true, "instance_1");
        let private = context_document(WebContext::Control, false, "instance_1");
        assert_eq!(watched["sentence"], words::SENTENCE_CONTROL_WATCHED);
        assert_eq!(private["sentence"], words::SENTENCE_CONTROL_PRIVATE);
        assert_eq!(watched["opener"], OPENER_SERVICE);
        assert_eq!(watched["instance"], "instance_1");
        assert_eq!(watched["acts"]["hold"]["available"], true);
    }

    #[test]
    fn a_tab_opens_links_as_anchors() {
        let document = context_document(WebContext::Watch, true, "instance_1");
        assert_eq!(document["opener"], OPENER_ANCHOR);
        assert_eq!(document["acts"]["resume"]["available"], false);
        assert_eq!(
            document["acts"]["resume"]["why_not"],
            words::CHANGE_IN_WINDOW
        );
    }

    #[test]
    fn every_context_gets_the_whole_table_of_links() {
        for context in [WebContext::Control, WebContext::Watch, WebContext::Closed] {
            let document = context_document(context, true, "instance_1");
            let links = document["links"].as_object().unwrap();
            assert_eq!(links.len(), WorkbenchDestination::all().len());
            for destination in WorkbenchDestination::all() {
                assert_eq!(links[destination.key()], destination.url());
                assert!(destination.url().starts_with("https://"));
            }
        }
    }

    #[test]
    fn a_closed_document_holds_only_the_product() {
        let document = context_document(WebContext::Closed, false, "instance_1");
        let mut keys: Vec<_> = document.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["context", "links", "product", "sentence", "word"]);
        assert_eq!(document["product"]["version"], env!("CARGO_PKG_VERSION"));
    }
}
