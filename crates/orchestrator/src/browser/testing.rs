//! The test double of the browser port: a fake browser that answers the commands a test
//! scripts, records what it was asked, and reports the facts a test gives it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::browser::adapter::{BrowserCommand, BrowserOutcome, PhysicalWait, RuntimeControlState};
use crate::sync::lock;

use super::{BrowserError, BrowserPort, BrowserSummary};

/// The browser a fake stands in for when a test does not care which one it is.
pub const FAKE_BROWSER: &str = "browser_fake";

/// Deterministic browser port for executor contract tests.
#[derive(Debug)]
pub struct FakeBrowser {
    documents: Mutex<
        std::collections::HashMap<u64, crate::browser::adapter::documents::DocumentInventory>,
    >,
    scopes: Mutex<Vec<crate::browser::adapter::documents::DocumentScope>>,
    calls: Mutex<Vec<BrowserCommand>>,
    routed: Mutex<Vec<String>>,
    outcomes: Mutex<VecDeque<Queued>>,
    control_states: Mutex<Vec<RuntimeControlState>>,
    connected: Mutex<Vec<BrowserSummary>>,
    returning: Mutex<bool>,
    last_connected: Mutex<Option<String>>,
    installed: Mutex<std::collections::HashMap<String, String>>,
    arrivals: Mutex<std::collections::HashMap<String, u64>>,
    lives: Mutex<std::collections::HashMap<String, String>>,
    launches: Mutex<super::LaunchGuard>,
}

/// One answer the fake will give, and whether it wrote the command to the browser first.
#[derive(Debug)]
struct Queued {
    outcome: Result<BrowserOutcome, BrowserError>,
    written: bool,
}

impl Default for FakeBrowser {
    fn default() -> Self {
        Self {
            documents: Mutex::default(),
            scopes: Mutex::default(),
            calls: Mutex::default(),
            routed: Mutex::default(),
            outcomes: Mutex::default(),
            control_states: Mutex::default(),
            connected: Mutex::new(vec![summary(FAKE_BROWSER, true)]),
            returning: Mutex::new(false),
            last_connected: Mutex::new(None),
            installed: Mutex::default(),
            arrivals: Mutex::default(),
            lives: Mutex::default(),
            launches: Mutex::default(),
        }
    }
}

/// One connected browser, as a test wants to describe it.
pub fn summary(id: &str, attended: bool) -> BrowserSummary {
    BrowserSummary {
        id: id.into(),
        name: super::UNBRANDED_BROWSER_NAME.into(),
        adapter_version: "1.0.0".into(),
        attended,
    }
}

impl FakeBrowser {
    /// Supply explicit document evidence for a governed executor fixture.
    pub fn set_documents(
        &self,
        tab: u64,
        inventory: crate::browser::adapter::documents::DocumentInventory,
    ) {
        lock(&self.documents).insert(tab, inventory);
    }
    /// Document constraints sent to this browser, separate from physical operation calls.
    pub fn scopes(&self) -> Vec<crate::browser::adapter::documents::DocumentScope> {
        lock(&self.scopes).clone()
    }
    /// Queue the next answer of a command the fake writes to the browser, whatever the
    /// answer says.
    pub fn push(&self, outcome: Result<BrowserOutcome, BrowserError>) {
        lock(&self.outcomes).push_back(Queued {
            outcome,
            written: true,
        });
    }
    /// Queue the next answer of a command the fake fails before writing any of it, as a port
    /// does when the connection is gone before its first frame.
    pub fn push_unwritten(&self, outcome: Result<BrowserOutcome, BrowserError>) {
        lock(&self.outcomes).push_back(Queued {
            outcome,
            written: false,
        });
    }
    pub fn calls(&self) -> Vec<BrowserCommand> {
        lock(&self.calls).clone()
    }
    pub fn control_states(&self) -> Vec<RuntimeControlState> {
        lock(&self.control_states).clone()
    }
    /// Which browser each dispatch was routed to, in order.
    pub fn routed(&self) -> Vec<String> {
        lock(&self.routed).clone()
    }
    /// Replace the connected inventory this port reports. Each listed browser that was
    /// reported as an installed browser counts as one arrival of that installed browser.
    pub fn connect(&self, browsers: Vec<BrowserSummary>) {
        {
            let installed = lock(&self.installed);
            let mut arrivals = lock(&self.arrivals);
            for browser in &browsers {
                if let Some(installed) = installed.get(&browser.id) {
                    *arrivals.entry(installed.clone()).or_default() += 1;
                }
            }
        }
        *lock(&self.connected) = browsers;
    }
    /// Report evidence that a browser is about to connect, as a relay does after a service
    /// start or while a connection initializes.
    pub fn set_returning(&self, returning: bool) {
        *lock(&self.returning) = returning;
    }
    /// Remember `browser`, a native-host id, as the installed browser that last connected.
    pub fn remember_last_connected(&self, browser: &str) {
        *lock(&self.last_connected) = Some(browser.into());
    }
    /// Report that the browser identity `browser` connected as the installed browser
    /// `installed`, a native-host id.
    pub fn remember_installed(&self, browser: &str, installed: &str) {
        lock(&self.installed).insert(browser.into(), installed.into());
    }
    /// Report `life` as the extension life the connection of `browser` named in its hello,
    /// or no life for `None`.
    pub fn set_life(&self, browser: &str, life: Option<&str>) {
        let mut lives = lock(&self.lives);
        match life {
            Some(life) => lives.insert(browser.into(), life.into()),
            None => lives.remove(browser),
        };
    }
}

impl FakeBrowser {
    /// Answer one command, with whether the fake wrote it to the browser. Every command is
    /// written except one answered from [`Self::push_unwritten`].
    fn answer(
        &self,
        browser: &str,
        command: BrowserCommand,
    ) -> (Result<BrowserOutcome, BrowserError>, bool) {
        use crate::browser::adapter::documents::{
            DocumentInventory, DocumentObservation, PhysicalDocument,
        };
        if let BrowserCommand::DescribeDocuments {
            tab_id,
            locators,
            points,
            focused,
            ..
        } = &command
        {
            let mut inventory = lock(&self.documents)
                .get(tab_id)
                .cloned()
                .unwrap_or_default();
            if locators.is_empty() && points.is_empty() && !focused {
                inventory.subjects.clear();
            } else if inventory.subjects.is_empty() {
                inventory.subjects = inventory
                    .documents
                    .first()
                    .map(|document| vec![document.id.clone()])
                    .unwrap_or_default();
            }
            return (
                Ok(BrowserOutcome::Documents {
                    tab_id: *tab_id,
                    inventory,
                }),
                true,
            );
        }
        if let BrowserCommand::InDocuments { scope, primitive } = command {
            lock(&self.scopes).push(scope.clone());
            let (outcome, written) = self.answer(browser, *primitive);
            return (
                outcome.map(|result| BrowserOutcome::InDocuments {
                    observation: DocumentObservation {
                        visited: scope.allowed,
                        ..DocumentObservation::default()
                    },
                    result: Box::new(result),
                }),
                written,
            );
        }
        if matches!(
            &command,
            BrowserCommand::Observe(PhysicalWait {
                condition: crate::language::PageCondition::VisualSettle,
                ..
            })
        ) {
            let tab_id = match &command {
                BrowserCommand::Observe(PhysicalWait { tab_id, .. }) => *tab_id,
                _ => unreachable!(),
            };
            lock(&self.routed).push(browser.into());
            lock(&self.calls).push(command);
            if let Some(queued) = lock(&self.outcomes).pop_front() {
                return (queued.outcome, queued.written);
            }
            return (
                Ok(BrowserOutcome::Observed {
                    tab_id,
                    satisfied: true,
                    elapsed_ms: 0,
                    readiness: crate::browser::adapter::BrowserReadiness::Complete,
                }),
                true,
            );
        }
        lock(&self.routed).push(browser.into());
        lock(&self.calls).push(command);
        let Queued {
            outcome: result,
            written,
        } = lock(&self.outcomes).pop_front().unwrap_or_else(|| Queued {
            outcome: Err(BrowserError::primitive("no fake outcome")),
            written: true,
        });
        if let Ok(
            BrowserOutcome::TabOpened { tab, .. }
            | BrowserOutcome::Navigated { tab, .. }
            | BrowserOutcome::Typed { tab, .. },
        ) = &result
        {
            lock(&self.documents).insert(
                tab.tab_id,
                DocumentInventory {
                    documents: vec![PhysicalDocument {
                        id: format!("document_{}", tab.tab_id),
                        url: tab.url.clone(),
                        parent: None,
                        supported: true,
                    }],
                    ..DocumentInventory::default()
                },
            );
        }
        (result, written)
    }
}

impl BrowserPort for FakeBrowser {
    fn call(
        &self,
        browser: &str,
        command: BrowserCommand,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.answer(browser, command).0
    }

    fn call_guarded(
        &self,
        browser: &str,
        command: BrowserCommand,
        dispatch: super::BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        (dispatch.admit)()?;
        let (outcome, written) = self.answer(browser, command);
        if written {
            dispatch.written.store(true, Ordering::SeqCst);
        }
        outcome
    }

    fn browsers(&self) -> Vec<BrowserSummary> {
        lock(&self.connected).clone()
    }

    fn browser_returning(&self) -> bool {
        *lock(&self.returning)
    }

    fn last_connected_browser(&self) -> Option<String> {
        lock(&self.last_connected).clone()
    }

    fn installed_browser_of(&self, browser: &str) -> Option<String> {
        lock(&self.installed).get(browser).cloned()
    }

    fn arrivals_of(&self, installed: &str) -> u64 {
        lock(&self.arrivals).get(installed).copied().unwrap_or(0)
    }

    fn note_start(&self, installed: &str, now: Instant, window: std::time::Duration) -> bool {
        let arrivals = self.arrivals_of(installed);
        lock(&self.launches).note(installed, arrivals, now, window)
    }

    fn forget_start(&self, installed: &str) {
        lock(&self.launches).forget(installed);
    }

    fn life_of(&self, browser: &str) -> Option<String> {
        lock(&self.lives).get(browser).cloned()
    }

    fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        lock(&self.control_states).push(state);
        Ok(())
    }
}
