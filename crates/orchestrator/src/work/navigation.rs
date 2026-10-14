//! Controlled-tab listing, activation, navigation, and closure execution.

use crate::browser::adapter::{BrowserCommand, BrowserOutcome};
use serde_json::{json, Value};

use crate::browser::BrowserError;
use crate::events::DomainEvent;
use crate::governance::{Capability, CapabilitySet, Decision};
use crate::language::outcome::{Outcome, Refusal};
use crate::language::ReusePolicy;
use crate::workspace::{SelectedTab, WorkspaceError, WorkspaceLease};

use super::{
    bounded, observed_host, readiness, ApplicationExecutor, CloseCompensation, Effect,
    InvocationContext, InvocationResult, Readiness, Status, Terminal,
};

/// How the governed tab came to exist, which the receipt reports truthfully (ADR-0137).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenedTabKind {
    /// Created fresh by this open.
    Created,
    /// Reused: a same-host tab Ghostlight opened and a released session of the same client left
    /// open (ADR-0196 Decision 9).
    Adopted,
    /// Recreated fresh under a recovered stale handle.
    Recovered,
}

/// The browser group every controlled tab of one client sits in: `Ghostlight - <client label>`.
///
/// The title depends on the client label alone (ADR-0066). Default reuse relies on that: it hands
/// a released tab only to a session of the client that released it, so the tab already sits in
/// the caller's group and is never regrouped (ADR-0196 Decision 9).
fn group_title(client_label: &str) -> String {
    format!("Ghostlight - {}", bounded(client_label, 80))
}

impl ApplicationExecutor {
    pub(super) fn list_tabs(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Read, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        // Listing is a current read of real state: the tabs come from the live browser through
        // a dispatching query, and only this workspace's bound tabs are named -- the person's
        // unbound tabs stay private. A browser on its way back, such as an idle MV3 worker that
        // suspended its relay, gets the same bounded wait at the browser seam that every other
        // crossing gets, before the same policy-owned recovery.
        let live = match self.dispatch(context, BrowserCommand::ListTabs) {
            Ok(BrowserOutcome::Tabs { tabs }) => tabs,
            Ok(_) => return self.protocol_failure(context, decision, None),
            Err(error) => return self.browser_failure(context, decision, error, None),
        };
        match lease.tabs() {
            Ok(bindings) => {
                let facts: Vec<_> = bindings
                    .into_iter()
                    .filter_map(|tab| {
                        let current = live
                            .iter()
                            .find(|physical| physical.tab_id == tab.physical_id)?;
                        Some(json!({
                            "tab":tab.handle.as_str(),
                            "title":current.title,
                            "url":current.url,
                            "active":current.active,
                            "readiness":readiness(current.readiness),
                        }))
                    })
                    .collect();
                let outcome = Outcome::TabsListed { count: facts.len() };
                // Listing tabs is also how a caller discovers where tabs can be opened. A model
                // asked to choose a browser needs the choices in front of it, and this is the one
                // read that already answers "what is there".
                let browsers: Vec<_> = self
                    .browser
                    .browsers()
                    .into_iter()
                    .map(|browser| {
                        json!({"browser":browser.id,"name":browser.name,"attended":browser.attended})
                    })
                    .collect();
                self.succeeded(
                    context,
                    decision,
                    None,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    outcome,
                    json!({"tabs":facts,"browsers":browsers}),
                )
            }
            Err(error) => self.workspace_failure(context, error),
        }
    }

    pub(super) fn activate_tab(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: &str,
    ) -> Terminal {
        let selected = match lease.select_tab(Some(requested_tab)) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, CapabilitySet::EMPTY, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::FocusTab {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::TabFocused {
                tab_id,
                active,
                window_focused,
            }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.mark_active(&selected.handle) {
                    return self.workspace_failure_after_effect(context, error, tab_id);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::TabActivated {
                        host: observed_host(&selected.url),
                    },
                    json!({"tab":selected.handle.as_str(),"active":active,"window_focused":window_focused}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn open_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        url: &str,
        reuse: ReusePolicy,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Read, Some(url));
        if !decision.allowed {
            return self.blocked_at(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
                observed_host(url),
            );
        }
        let client_label = match self.workspaces.client_label(context.workspace) {
            Ok(label) => label,
            Err(error) => return self.workspace_failure(context, error),
        };
        if reuse == ReusePolicy::Domain {
            if let Some(terminal) =
                self.reuse_released_tab(context, lease, decision, &client_label, url)
            {
                return terminal;
            }
        }
        let (tab, commits, reused) = match self.dispatch(
            context,
            BrowserCommand::OpenTab {
                url: url.into(),
                group_title: group_title(&client_label),
                // The service alone chooses which tab to reuse (ADR-0196 Decision 9). An adapter
                // told `never` runs no reuse ladder of its own, so even an adapter that still has
                // one can never adopt a tab the person opened.
                reuse: ReusePolicy::Never.as_str().into(),
                focus_window: holds_no_tab(lease),
            },
        ) {
            Ok(BrowserOutcome::TabOpened {
                tab,
                committed_urls,
                reused,
            }) => (tab, committed_urls, reused),
            Ok(_) => return self.protocol_failure(context, decision, None),
            Err(error) => return self.browser_failure(context, decision, error, None),
        };
        let controlled = match lease.add_tab(&tab) {
            Ok(tab) => tab,
            // The browser already opened the page, so the refusal must not claim nothing happened.
            Err(error) => return self.workspace_failure_after_effect(context, error, tab.tab_id),
        };
        self.settle_opened_tab(
            context,
            lease,
            tab,
            controlled,
            commits,
            if reused {
                OpenedTabKind::Adopted
            } else {
                OpenedTabKind::Created
            },
        )
    }

    /// Reuse a tab Ghostlight opened and a released session of the same client left open, when
    /// one fits this open.
    ///
    /// The service makes the whole choice and sends the adapter an exact tab id (ADR-0196
    /// Decision 9). Candidates come only from the released pool, in the browser this session
    /// works in, on the requested host, so a tab the person opened is never one. They were also
    /// released by a session with this session's `client_label`, so a reused tab already sits in
    /// this client's group: `Navigate` does not move it, and another client's tab would stay
    /// under that client's group title. A candidate is used only while the live browser still
    /// shows it at its release address; one that closed or moved on is dropped and the next is
    /// tried. Returns None when nothing fits, and the open then creates a fresh tab.
    fn reuse_released_tab(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        decision: Decision,
        client_label: &str,
        url: &str,
    ) -> Option<Terminal> {
        let browser = match self.admitted_browser(context) {
            Ok(browser) => browser,
            Err(error) => return Some(self.browser_failure(context, decision, error, None)),
        };
        let mut candidate = self
            .workspaces
            .claim_released(&browser, client_label, url)?;
        let live = match self.dispatch(context, BrowserCommand::ListTabs) {
            Ok(BrowserOutcome::Tabs { tabs }) => tabs,
            Ok(_) => return Some(self.protocol_failure(context, decision, None)),
            Err(error) => return Some(self.browser_failure(context, decision, error, None)),
        };
        let released = loop {
            if let Some(tab) = live
                .iter()
                .find(|tab| tab.tab_id == candidate.physical_id && tab.url == candidate.url)
            {
                break tab.clone();
            }
            candidate = self
                .workspaces
                .claim_released(&browser, client_label, url)?;
        };
        let controlled = match lease.add_tab(&released) {
            Ok(controlled) => controlled,
            // Another session already took this tab; nothing happened yet, so open a fresh one.
            Err(WorkspaceError::PhysicalTabOwned) => return None,
            Err(error) => return Some(self.workspace_failure(context, error)),
        };
        Some(
            match self.dispatch(
                context,
                BrowserCommand::Navigate {
                    tab_id: released.tab_id,
                    url: url.into(),
                },
            ) {
                Ok(BrowserOutcome::Navigated {
                    tab,
                    committed_urls,
                }) if tab.tab_id == released.tab_id => self.settle_opened_tab(
                    context,
                    lease,
                    tab,
                    controlled,
                    committed_urls,
                    OpenedTabKind::Adopted,
                ),
                Ok(_) => self.protocol_failure(context, decision, Some(released.tab_id)),
                Err(error) => self.browser_failure(context, decision, error, Some(released.tab_id)),
            },
        )
    }

    /// Govern the just-opened tab: landing authorization with close compensation, governed
    /// landing, and the opened-page receipt. Shared by first opens and dead-tab recovery so
    /// both ride exactly the same governance.
    fn settle_opened_tab(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        tab: crate::browser::adapter::PhysicalTab,
        controlled: SelectedTab,
        commits: Vec<String>,
        kind: OpenedTabKind,
    ) -> Terminal {
        self.emit(DomainEvent::TabCreated {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            tab: controlled.handle.clone(),
            physical_id: controlled.physical_id,
        });
        let landing = self.authorize_commits(context, Capability::Read, &tab, &commits);
        if !landing.allowed {
            return match self.compensate_close(context, lease, &controlled) {
                CloseCompensation::Closed => self.blocked_at(
                    context,
                    landing,
                    Some(controlled.physical_id),
                    Effect::None,
                    true,
                    json!({"reason":landing.reason.as_str(),"compensated":true}),
                    observed_host(&controlled.url),
                ),
                CloseCompensation::Retained => self.blocked_at(
                    context,
                    landing,
                    Some(controlled.physical_id),
                    Effect::Applied,
                    false,
                    json!({"reason":landing.reason.as_str(),"compensated":false,"retained":true}),
                    observed_host(&controlled.url),
                ),
                CloseCompensation::Unknown => self.unknown(
                    context,
                    landing,
                    Some(controlled.physical_id),
                    Refusal::LandingDeniedUnknown,
                    json!({"reason":landing.reason.as_str(),"compensated":false}),
                ),
            };
        }
        let governed = match lease.apply_landing(&controlled.handle, &tab) {
            Ok(tab) => tab,
            Err(error) => {
                return self.workspace_failure_after_effect(context, error, controlled.physical_id)
            }
        };
        self.emit(DomainEvent::DocumentCommitted {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            tab: governed.handle.clone(),
            physical_id: governed.physical_id,
        });
        let reused = kind == OpenedTabKind::Adopted;
        let created = kind != OpenedTabKind::Adopted;
        let outcome = if reused {
            Outcome::PageReused {
                host: observed_host(&governed.url),
            }
        } else {
            Outcome::PageOpened {
                host: observed_host(&governed.url),
            }
        };
        self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, outcome, json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"created":created,"reused":reused,"document_generation":governed.generation}))
    }

    pub(super) fn navigate_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        url: &str,
        discard_beforeunload: bool,
        reuse: ReusePolicy,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Read, Some(url));
        if !decision.allowed {
            return self.blocked_at(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
                observed_host(url),
            );
        }
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(WorkspaceError::NoTab) if requested_tab.is_none() => {
                return self.open_page(context, lease, url, reuse)
            }
            // The caller's durable tab handle points at a tab that is gone. Recreate it
            // through the governed open path under the same handle, and say so plainly.
            Err(WorkspaceError::StaleTab) if requested_tab.is_some() => {
                let handle = requested_tab.expect("guarded above");
                let client_label = match self.workspaces.client_label(context.workspace) {
                    Ok(label) => label,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let decision = self.authorize(context, Capability::Read, Some(url));
                if !decision.allowed {
                    return self.blocked_at(
                        context,
                        decision,
                        None,
                        Effect::None,
                        true,
                        json!({"reason":decision.reason.as_str()}),
                        observed_host(url),
                    );
                }
                let (tab, commits) = match self.dispatch(
                    context,
                    BrowserCommand::OpenTab {
                        url: url.into(),
                        group_title: group_title(&client_label),
                        // A recovered stale handle must be a fresh tab under that handle, not
                        // an adoption of an unrelated same-host tab.
                        reuse: ReusePolicy::Never.as_str().into(),
                        focus_window: holds_no_tab(lease),
                    },
                ) {
                    Ok(BrowserOutcome::TabOpened {
                        tab,
                        committed_urls,
                        ..
                    }) => (tab, committed_urls),
                    Ok(_) => return self.protocol_failure(context, decision, None),
                    Err(error) => return self.browser_failure(context, decision, error, None),
                };
                let controlled = match lease.restore_tab(handle, &tab) {
                    Ok(controlled) => controlled,
                    Err(error) => {
                        return self.workspace_failure_after_effect(context, error, tab.tab_id)
                    }
                };
                let mut terminal = self.settle_opened_tab(
                    context,
                    lease,
                    tab,
                    controlled,
                    commits,
                    OpenedTabKind::Recovered,
                );
                if terminal.result.status == Status::Succeeded {
                    terminal.result.repeat_safe = false;
                    let outcome = Outcome::PageRecovered {
                        host: terminal.observed.host.clone(),
                    };
                    terminal.result.summary = outcome.summary();
                    terminal.audit = outcome.audit();
                    if let Some(facts) = terminal.result.facts.as_object_mut() {
                        facts.insert("recovered".into(), json!("new_tab"));
                    }
                }
                return terminal;
            }
            Err(error) => return self.workspace_failure(context, error),
        };
        let command = if discard_beforeunload {
            BrowserCommand::NavigateDiscardingBeforeUnload {
                tab_id: selected.physical_id,
                url: url.into(),
            }
        } else {
            BrowserCommand::Navigate {
                tab_id: selected.physical_id,
                url: url.into(),
            }
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Navigated {
                tab,
                committed_urls,
            }) => {
                let landing =
                    self.authorize_commits(context, Capability::Read, &tab, &committed_urls);
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    self.emit(DomainEvent::HoldEntered {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        physical_id: selected.physical_id,
                    });
                    return self.blocked_at(context, landing, Some(selected.physical_id), Effect::Applied, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}), observed_host(&tab.url));
                }
                let governed = match lease.apply_landing(&selected.handle, &tab) {
                    Ok(tab) => tab,
                    Err(error) => {
                        return self.workspace_failure_after_effect(
                            context,
                            error,
                            selected.physical_id,
                        )
                    }
                };
                self.emit(DomainEvent::DocumentCommitted {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    tab: governed.handle.clone(),
                    physical_id: governed.physical_id,
                });
                self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, Outcome::PageNavigated { host: observed_host(&governed.url) }, json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"created":false,"document_generation":governed.generation}))
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn navigate_history(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        direction: &str,
    ) -> Terminal {
        self.with_authorized_tab(
            context,
            lease,
            requested_tab,
            Capability::Action,
            |selected, decision| {
                let outcome = self.dispatch(
                    context,
                    BrowserCommand::TraverseHistory {
                        tab_id: selected.physical_id,
                        direction: direction.into(),
                    },
                );
                self.complete_navigation(
                    context,
                    lease,
                    selected,
                    decision,
                    outcome,
                    |host| Outcome::HistoryTraversed {
                        direction: direction.into(),
                        host,
                    },
                    json!({"action":direction}),
                )
            },
        )
    }

    pub(super) fn reload_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        bypass_cache: bool,
    ) -> Terminal {
        self.with_authorized_tab(
            context,
            lease,
            requested_tab,
            Capability::Action,
            |selected, decision| {
                let outcome = self.dispatch(
                    context,
                    BrowserCommand::Reload {
                        tab_id: selected.physical_id,
                        bypass_cache,
                    },
                );
                self.complete_navigation(
                    context,
                    lease,
                    selected,
                    decision,
                    outcome,
                    |host| Outcome::PageReloaded { host },
                    json!({"action":"reload","bypass_cache":bypass_cache}),
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_navigation<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        selected: &SelectedTab,
        decision: Decision,
        outcome: Result<BrowserOutcome, BrowserError>,
        make_outcome: F,
        mut facts: Value,
    ) -> Terminal
    where
        F: FnOnce(Option<String>) -> Outcome,
    {
        match outcome {
            Ok(BrowserOutcome::Navigated {
                tab,
                committed_urls,
            }) => {
                let landing =
                    self.authorize_commits(context, Capability::Action, &tab, &committed_urls);
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    self.emit(DomainEvent::HoldEntered {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        physical_id: selected.physical_id,
                    });
                    return self.blocked_at(
                        context,
                        landing,
                        Some(selected.physical_id),
                        Effect::Applied,
                        false,
                        json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}),
                        observed_host(&tab.url),
                    );
                }
                let governed = match lease.apply_landing(&selected.handle, &tab) {
                    Ok(tab) => tab,
                    Err(error) => {
                        return self.workspace_failure_after_effect(
                            context,
                            error,
                            selected.physical_id,
                        )
                    }
                };
                let outcome = make_outcome(observed_host(&governed.url));
                if let Some(object) = facts.as_object_mut() {
                    object.insert("tab".into(), json!(governed.handle.as_str()));
                    object.insert("url".into(), json!(governed.url));
                    object.insert("title".into(), json!(governed.title));
                    object.insert("document_generation".into(), json!(governed.generation));
                }
                self.emit(DomainEvent::DocumentCommitted {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    tab: governed.handle.clone(),
                    physical_id: governed.physical_id,
                });
                self.succeeded(
                    context,
                    landing,
                    Some(governed.physical_id),
                    Effect::Applied,
                    readiness(governed.readiness),
                    false,
                    outcome,
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn close_tab(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested: &str,
    ) -> Terminal {
        // Closing what is already gone is the desired state achieved, not an error.
        if matches!(
            lease.select_tab(Some(requested)),
            Err(WorkspaceError::StaleTab)
        ) {
            let decision = self.authorize_tab_close(context);
            if !decision.allowed {
                return self.blocked(
                    context,
                    decision,
                    None,
                    Effect::None,
                    true,
                    json!({"tab":requested,"reason":decision.reason.as_str()}),
                );
            }
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    Status::Succeeded,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    Outcome::TabAlreadyClosed.summary().as_str(),
                    json!({"tab":requested,"closed":false,"already_gone":true}),
                    vec![],
                ),
                decision,
                physical_id: None,
                observed: Default::default(),
                audit: Outcome::TabAlreadyClosed.audit(),
            };
        }
        let selected = match lease.select_tab(Some(requested)) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize_tab_close(context);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"tab":selected.handle.as_str(),"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::CloseTab {
                tab_id: selected.physical_id,
                released: false,
            },
        ) {
            Ok(BrowserOutcome::TabClosed { tab_id }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.confirm_tab_closed(&selected.handle) {
                    return self.workspace_failure_after_effect(context, error, tab_id);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    Readiness::NotApplicable,
                    false,
                    Outcome::TabClosed,
                    json!({"tab":selected.handle.as_str(),"closed":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }
}

/// Whether the workspace holds no tab yet, so the tab it opens should bring its window into view.
fn holds_no_tab(lease: &WorkspaceLease) -> bool {
    lease.tabs().is_ok_and(|tabs| tabs.is_empty())
}
