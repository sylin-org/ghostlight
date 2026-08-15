//! Reading, inspection, discovery, and screenshot execution.

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome};
use ghostlight_bridge::service::ServiceContent;
use serde_json::{json, Value};

use crate::governance::Capability;
use crate::language::outcome::{Outcome, Refusal, TargetNoun};
use crate::workspace::WorkspaceLease;

use super::{
    bounded, observed_host, readiness, word_count, ApplicationExecutor, Effect, InvocationContext,
    Terminal,
};

impl ApplicationExecutor {
    pub(super) fn read_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        max_chars: usize,
    ) -> Terminal {
        let (selected, locator, _) =
            match self.resolve_optional_target(lease, requested_tab, target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
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
            BrowserCommand::ReadText {
                tab_id: selected.physical_id,
                locator,
                max_chars,
            },
        ) {
            Ok(BrowserOutcome::Text {
                tab_id,
                text,
                truncated,
                title,
                url,
            }) if tab_id == selected.physical_id => {
                let landing = self.authorize(context, Capability::Read, Some(&url));
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    return self.blocked_at(context, landing, Some(tab_id), Effect::None, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}), observed_host(&url));
                }
                let words = word_count(&text);
                self.succeeded(context, landing, Some(tab_id), Effect::None, readiness(selected.readiness), true, Outcome::TextRead { words, host: observed_host(&url) }, json!({"tab":selected.handle.as_str(),"url":url,"title":bounded(&title,500),"text":bounded(&text,max_chars),"truncated":truncated || text.chars().count() > max_chars,"document_generation":selected.generation}))
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn inspect_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        kind: &str,
        max_items: usize,
    ) -> Terminal {
        self.targets_operation(
            context,
            lease,
            requested_tab,
            Capability::Read,
            BrowserCommand::Inspect {
                tab_id: 0,
                kind: kind.into(),
                max_items,
            },
            if kind == "controls" {
                TargetNoun::Control
            } else {
                TargetNoun::Item
            },
        )
    }

    pub(super) fn find(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        text: &str,
        kind: &str,
        max_results: usize,
    ) -> Terminal {
        self.targets_operation(
            context,
            lease,
            requested_tab,
            Capability::Read,
            BrowserCommand::Find {
                tab_id: 0,
                text: text.into(),
                kind: kind.into(),
                max_results,
            },
            TargetNoun::Match,
        )
    }

    /// One governed target retrieval.
    ///
    /// The closed noun chooses both the product sentence and the structured fact key.
    #[allow(clippy::too_many_arguments)]
    fn targets_operation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        capability: Capability,
        command: BrowserCommand,
        noun: TargetNoun,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, capability, Some(selected.url.as_str()));
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
        let command = match command {
            BrowserCommand::Inspect {
                kind, max_items, ..
            } => BrowserCommand::Inspect {
                tab_id: selected.physical_id,
                kind,
                max_items,
            },
            BrowserCommand::Find {
                text,
                kind,
                max_results,
                ..
            } => BrowserCommand::Find {
                tab_id: selected.physical_id,
                text,
                kind,
                max_results,
            },
            _ => unreachable!("target operations are closed"),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Targets { tab_id, targets }) if tab_id == selected.physical_id => {
                let mapped = match lease.register_targets(&selected, &targets) {
                    Ok(mapped) => mapped,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let items: Vec<_> = mapped.into_iter().map(|(handle, target)| json!({"target":handle.as_str(),"role":bounded(&target.role,100),"name":bounded(&target.name,500),"state":target.state,"credential_class":target.credential_class})).collect();
                let outcome = Outcome::TargetsListed {
                    noun,
                    count: items.len(),
                    host: observed_host(&selected.url),
                };
                let fact_key = match noun {
                    TargetNoun::Match => "matches",
                    TargetNoun::Control | TargetNoun::Item => "items",
                };
                let mut facts = serde_json::Map::new();
                facts.insert("tab".into(), json!(selected.handle.as_str()));
                facts.insert("document_generation".into(), json!(selected.generation));
                facts.insert(fact_key.into(), json!(items));
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    outcome,
                    Value::Object(facts),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn screenshot(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        full_page: bool,
    ) -> Terminal {
        let (selected, locator, _) =
            match self.resolve_optional_target(lease, requested_tab, target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
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
            BrowserCommand::Screenshot {
                tab_id: selected.physical_id,
                locator,
                full_page,
            },
        ) {
            Ok(BrowserOutcome::Screenshot {
                tab_id,
                mime_type,
                data,
                width,
                height,
                viewport,
            }) if tab_id == selected.physical_id => {
                if data.len() > 7_000_000 {
                    return self.failed(
                        context,
                        decision,
                        Some(tab_id),
                        Refusal::CaptureTooLarge,
                        json!({"reason":"screenshot_too_large"}),
                    );
                }
                let view = match lease.register_view(&selected, viewport, width, height) {
                    Ok(view) => view,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let outcome = Outcome::Captured {
                    full_page,
                    width,
                    height,
                };
                let mut terminal = self.succeeded(context, decision, Some(tab_id), Effect::None, readiness(selected.readiness), true, outcome, json!({"tab":selected.handle.as_str(),"view":view.as_str(),"mime_type":mime_type,"width":width,"height":height}));
                terminal.result = terminal
                    .result
                    .with_content(ServiceContent::Image { mime_type, data });
                terminal
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }
}
