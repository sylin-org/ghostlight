//! The frame's admission checks: each records its decision as the frame's evidence, then asks
//! governance whether a channel, the audit, a dispatch, a landing, a document or a tab close may
//! go ahead.

use super::*;

impl ApplicationExecutor {
    pub(super) fn audit_decision(&self, context: &InvocationContext<'_>) -> Decision {
        if !context.snapshot.requires_audit() {
            return Decision::permitted();
        }
        context
            .snapshot
            .authorize_audit(!self.audit.health().unavailable())
    }

    /// Refuse a request whose channel a policy turned off, before any browser work and for every
    /// operation, `policy_explain` included (ADR-0202 Decision 22a).
    pub(super) fn channel_preflight(&self, context: &InvocationContext<'_>) -> Option<Terminal> {
        let channel = context.provenance.attribution().channel;
        let (decision, author) = context.snapshot.channel_refusal(channel)?;
        context.findings.permit(context.snapshot.decision_evidence(
            CapabilitySet::EMPTY,
            None,
            decision,
        ));
        Some(Terminal::refused(
            context.invocation,
            Shape {
                status: Status::Blocked,
                effect: Effect::None,
                readiness: Readiness::NotApplicable,
                repeat_safe: true,
            },
            &Refusal::ChannelOff { channel, author },
            with_denial_facts(decision, json!({})),
            decision,
            None,
        ))
    }

    pub(super) fn audit_preflight(&self, context: &InvocationContext<'_>) -> Option<Terminal> {
        // Existing human-control paths keep their precedence and language.
        if !self.governance.runtime_decision().allowed {
            return None;
        }
        let decision = self.audit_decision(context);
        if decision.allowed {
            return None;
        }
        context.findings.permit(context.snapshot.decision_evidence(
            context.requirements,
            None,
            decision,
        ));
        Some(self.blocked(context, decision, None, Effect::None, true, json!({})))
    }

    pub(super) fn admit_dispatch(
        &self,
        context: &InvocationContext<'_>,
    ) -> Result<(), BrowserError> {
        let mut decision = self.governance.runtime_decision();
        if decision.allowed {
            decision = self.audit_decision(context);
        }
        if decision.allowed {
            Ok(())
        } else {
            context.findings.permit(context.snapshot.decision_evidence(
                CapabilitySet::EMPTY,
                None,
                decision,
            ));
            Err(BrowserError::RuntimeControl(decision.reason))
        }
    }

    /// Authorize one operation against its real destination whenever it names one.
    /// `url: None` decides the requirements with no host, for work that acts on no page, such as
    /// a listing, a flow or a recording's status; with no requirements it asks only the person's
    /// runtime controls. Every operation that acts on a tab's page must pass `Some(&tab.url)`,
    /// the tab's raw string as tracked right now, **even when that string is empty** because the
    /// tab's first landing has not been governed yet. An empty or otherwise unparseable string
    /// falls straight through to `check_landing`, which denies it as `HostDenied` -- there is
    /// no host-blind fallback. A tab whose destination is not yet known is treated as denied,
    /// never as having no destination; otherwise the operator's host allowlist would be bypassed
    /// for exactly the tabs it exists to cover.
    pub(super) fn authorize(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        url: Option<&str>,
    ) -> Decision {
        let requirements = requirements.into();
        let runtime = self.governance.runtime_decision();
        let (decision, evidence) = if runtime.allowed {
            context.snapshot.authorize_with_evidence(requirements, url)
        } else {
            (
                runtime,
                context
                    .snapshot
                    .decision_evidence(requirements, url, runtime),
            )
        };
        context.findings.permit(evidence);
        decision
    }

    pub(super) fn authorize_commits(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        tab: &PhysicalTab,
        commits: &[String],
    ) -> Decision {
        let requirements = requirements.into();
        for url in commits.iter().chain(std::iter::once(&tab.url)) {
            let decision = self.check_landing(context, requirements, url);
            if !decision.allowed {
                return decision;
            }
        }
        Decision::permitted()
    }

    /// Govern a completed observation's destination. Runtime controls guard the next dispatch;
    /// they cannot turn an already admitted receipt into a permanent policy hold on the tab.
    pub(super) fn check_landing(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        url: &str,
    ) -> Decision {
        let (decision, evidence) = context
            .snapshot
            .authorize_with_evidence(requirements.into(), Some(url));
        context.findings.permit(evidence);
        decision
    }

    /// Govern one document that may be embedded in its tab, keeping the check on the frame's
    /// trace. The check carries no host: an embedded document's host never enters the durable
    /// record (ADR-0203 Decision 6.1), and the tab's own admission already names the top
    /// document's host.
    pub(super) fn authorize_document(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        url: &str,
    ) -> Decision {
        let (decision, mut evidence) = context
            .snapshot
            .authorize_with_evidence(requirements.into(), Some(url));
        evidence.host = None;
        context.findings.permit(evidence);
        decision
    }

    /// Admit the model's close of a tab (ADR-0203 Decision 7.6).
    ///
    /// `page` is the address the close takes the tab away from, and the close needs Action on its
    /// host, then the tab-close setting. It is none for a held tab, whose close undoes
    /// Ghostlight's refused landing (Decision 2.4), and for a tab already gone, which the close
    /// cannot touch: the setting alone decides those, after the person's runtime controls.
    pub(super) fn check_tab_close(
        &self,
        context: &InvocationContext<'_>,
        page: Option<&str>,
    ) -> Decision {
        let admitted = match page {
            Some(url) => self.authorize(context, context.requirements, Some(url)),
            None => self.authorize(context, CapabilitySet::EMPTY, None),
        };
        if !admitted.allowed {
            return admitted;
        }
        let close = self.authorize_close_setting(context, page);
        if close.allowed {
            admitted
        } else {
            close
        }
    }

    /// Decide the tab-close setting alone, keeping its evidence when it refuses. Undoing
    /// Ghostlight's own refused landing asks only this (ADR-0203 Decision 2.4).
    pub(super) fn authorize_close_setting(
        &self,
        context: &InvocationContext<'_>,
        page: Option<&str>,
    ) -> Decision {
        let close = context.snapshot.authorize_tab_close();
        if !close.allowed {
            context.findings.permit(context.snapshot.decision_evidence(
                CapabilitySet::ACTION,
                page,
                close,
            ));
        }
        close
    }
}
