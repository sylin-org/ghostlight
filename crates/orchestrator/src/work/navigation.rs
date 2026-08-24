//! Controlled-tab listing, activation, navigation, and closure execution.

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome};
use serde_json::{json, Value};

use crate::browser::BrowserError;
use crate::events::DomainEvent;
use crate::governance::{Capability, CapabilitySet, Decision};
use crate::language::outcome::{Outcome, Refusal};
use crate::workspace::{SelectedTab, WorkspaceError, WorkspaceLease};

use super::{
    bounded, observed_host, readiness, ApplicationExecutor, CloseCompensation, Effect,
    InvocationContext, Readiness, Terminal,
};

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
        // unbound tabs stay private. An idle MV3 worker suspends its relay silently, so the
        // read gives a waking adapter one bounded window to reattach before answering from
        // absence.
        if self.browser.browsers().is_empty() && !self.wait_for_any_browser(context) {
            return self.failed(
                context,
                decision,
                None,
                Refusal::BrowserStartupManual { browser: None },
                json!({"reason":"browser_startup_manual"}),
            );
        }
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
                    return self.workspace_failure(context, error);
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
        let group_title = format!("Ghostlight - {}", bounded(&client_label, 80));
        let (tab, commits) = match self.dispatch(
            context,
            BrowserCommand::OpenTab {
                url: url.into(),
                group_title,
            },
        ) {
            Ok(BrowserOutcome::TabOpened {
                tab,
                committed_urls,
            }) => (tab, committed_urls),
            Ok(_) => return self.protocol_failure(context, decision, None),
            Err(error) => return self.browser_failure(context, decision, error, None),
        };
        let controlled = match lease.add_tab(&tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
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
                    Some(tab.tab_id),
                    Effect::None,
                    true,
                    json!({"reason":landing.reason.as_str(),"compensated":true}),
                    observed_host(&tab.url),
                ),
                CloseCompensation::Retained => self.blocked_at(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Effect::Applied,
                    false,
                    json!({"reason":landing.reason.as_str(),"compensated":false,"retained":true}),
                    observed_host(&tab.url),
                ),
                CloseCompensation::Unknown => self.unknown(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Refusal::LandingDeniedUnknown,
                    json!({"reason":landing.reason.as_str(),"compensated":false}),
                ),
            };
        }
        let governed = match lease.apply_landing(&controlled.handle, &tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        self.emit(DomainEvent::DocumentCommitted {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            tab: governed.handle.clone(),
            physical_id: governed.physical_id,
        });
        self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, Outcome::PageOpened { host: observed_host(&governed.url) }, json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"created":true,"document_generation":governed.generation}))
    }

    pub(super) fn navigate_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        url: &str,
        discard_beforeunload: bool,
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
                return self.open_page(context, lease, url)
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
                    Err(error) => return self.workspace_failure(context, error),
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
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
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
            &selected,
            decision,
            outcome,
            |host| Outcome::HistoryTraversed {
                direction: direction.into(),
                host,
            },
            json!({"action":direction}),
        )
    }

    pub(super) fn reload_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        bypass_cache: bool,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
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
            &selected,
            decision,
            outcome,
            |host| Outcome::PageReloaded { host },
            json!({"action":"reload","bypass_cache":bypass_cache}),
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
                    Err(error) => return self.workspace_failure(context, error),
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
            },
        ) {
            Ok(BrowserOutcome::TabClosed { tab_id }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.confirm_tab_closed(&selected.handle) {
                    return self.workspace_failure(context, error);
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
