//! Which browser a call goes to: the session's own, waited for or started again, the one the
//! agent names, or the only one connected once its own stayed away, and the refusal when none can
//! take it (ADR-0196 Decision 12 and its amendment of 2026-11-13).

use super::*;

impl ApplicationExecutor {
    /// Admit this invocation's next browser crossing and name the browser it goes to.
    ///
    /// Runtime control and audit health are checked first, so a paused session never starts
    /// browser recovery just to learn where it would have gone.
    pub(super) fn admitted_browser(
        &self,
        context: &InvocationContext<'_>,
    ) -> Result<String, BrowserError> {
        self.admit_dispatch(context)?;
        self.target_browser(context)
    }

    /// Decide which browser this invocation's next crossing goes to.
    ///
    /// Resolution happens at the seam rather than at admission because a call that never reaches
    /// a browser must not need one: listing an empty session's tabs answers truthfully with no
    /// browser connected at all.
    ///
    /// The session's browser is the one its first tab bound, when [`WorkspaceLease::add_tab`] or
    /// [`WorkspaceLease::restore_tab`] took it, or [`WorkspaceStore::confirm`] kept a pending one.
    /// A crossing reads that binding and never writes it, so a crossing that binds no tab, such as
    /// a recording status, leaves the session free to open its first tab in any browser. Until a
    /// tab binds one, the frame keeps its own first choice, so one frame never crosses to two
    /// browsers. When the chosen browser is gone, the call waits for it and may start that same
    /// browser again. A session whose browser stayed away leaves it in one place,
    /// [`Self::move_session`]: for the connected browser the agent named, or for the only browser
    /// connected, which is no choice and so is settled here and never told to the caller. Among
    /// several connected browsers the choice is real, and the call is refused.
    pub(super) fn target_browser(
        &self,
        context: &InvocationContext<'_>,
    ) -> Result<String, BrowserError> {
        // The workspace vanished between admission and dispatch. Nothing physical should happen
        // for a workspace that no longer exists.
        if !self.workspaces.contains(context.workspace.as_str()) {
            return Err(BrowserError::CancelledBeforeDispatch);
        }
        let mut pinned = self
            .workspaces
            .browser_of(context.workspace.as_str())
            .or_else(|| context.findings.targeted());
        // The agent named a connected browser while the one this session works in is gone: the
        // session moves there, since it can never work in a browser that left (W-D5). While that
        // browser is connected, naming another stays refused, so a workspace lives in one browser.
        let connected = self.browser.browsers();
        if let (Some(requested), Some(gone)) = (context.requested_browser, pinned.as_deref()) {
            if requested != gone
                && connected.iter().any(|browser| browser.id == requested)
                && !connected.iter().any(|browser| browser.id == gone)
            {
                self.move_session(context, gone, requested, SessionMove::Named);
                pinned = None;
            }
        }
        let chosen = match choose_browser(context.requested_browser, pinned.as_deref(), &connected)
        {
            Ok(chosen) => chosen,
            Err(BrowserError::DisconnectedBeforeDispatch) => match pinned.as_deref() {
                Some(pinned) => match self.await_pinned_browser(context, pinned) {
                    Err(BrowserError::WrongProfile) => {
                        self.move_to_only_browser(context, pinned)?
                    }
                    waited => waited?,
                },
                None => self.await_or_recover_browser(context)?,
            },
            Err(error) => return Err(error),
        };
        context.findings.target(&chosen);
        Ok(chosen)
    }

    /// Let the session leave `gone`, the browser it works in, which stayed away, for the only
    /// browser connected, and name that browser (ADR-0196 Decision 12 amendment, 2026-11-13).
    ///
    /// The connected browsers are read once, here, and [`after_absence`] decides among them. A
    /// call that names a browser never comes here, so a named browser is always the one used.
    fn move_to_only_browser(
        &self,
        context: &InvocationContext<'_>,
        gone: &str,
    ) -> Result<String, BrowserError> {
        let to = after_absence(gone, &self.browser.browsers())?;
        if to != gone {
            self.move_session(context, gone, &to, SessionMove::OnlyConnected);
        }
        Ok(to)
    }

    /// The one way a session changes browser: it leaves `from`, which is not connected, so its
    /// next tab binds `to`. Every tab it held in `from` reads as no longer open from here on
    /// ([`WorkspaceStore::leave_browser`]). The move goes to the process log, where a person can
    /// find it, and never to the result: the audit record and the window already name the browser
    /// each action ran in.
    ///
    /// A session that does not work in `from` has nothing to leave and nothing is logged: a
    /// frame's own first choice bound no tab, and another call of the session may have moved it
    /// already.
    fn move_session(
        &self,
        context: &InvocationContext<'_>,
        from: &str,
        to: &str,
        cause: SessionMove,
    ) {
        if !self
            .workspaces
            .leave_browser(context.workspace.as_str(), from)
        {
            return;
        }
        self.diagnostics.sink().emit(
            crate::diagnostics::SESSION_BROWSER_MOVED,
            ghostlight_bridge::diagnostics::Level::Info,
            Some(context.invocation.as_str()),
            &format!(
                "{} left {from}, not connected, for {to}, {}",
                context.workspace.as_str(),
                cause.detail()
            ),
        );
    }

    /// Wait a bounded grace for the browser this session already works in, then start that
    /// browser again when Ghostlight can, and otherwise say what is true.
    ///
    /// The same browser reconnecting, or still installing its page runtime, is what the grace is
    /// for, so the grace always comes first and a reconnecting browser never gets a second
    /// window. When the session's browser is one installed browser this service saw it connect
    /// as, and `browser.startup` is `on_demand`, the grace ends once nothing is evidently
    /// returning, which leaves the start the call's budget, and recovery then starts that browser
    /// and no other (ADR-0196 Decision 12 amendment, 2026-10-17). It starts nothing while that
    /// installed browser is already connected as another identity, since a start would only open a
    /// window there. Otherwise the session waits the whole grace. A browser that stayed away is
    /// answered as not connected, or as [`BrowserError::WrongProfile`] while another browser is,
    /// which [`Self::target_browser`] settles.
    pub(super) fn await_pinned_browser(
        &self,
        context: &InvocationContext<'_>,
        pinned: &str,
    ) -> Result<String, BrowserError> {
        let cancelled = context.cancellation.flag();
        let restartable = self
            .browser
            .installed_browser_of(pinned)
            .filter(|_| self.governance.browser_startup() == BrowserStartup::OnDemand);
        let grace_end = return_grace_end(context.deadline);
        let availability = if restartable.is_some() {
            self.browser.await_returning(pinned, grace_end, cancelled)
        } else {
            self.browser.await_available(pinned, grace_end, cancelled)
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err(BrowserError::CancelledBeforeDispatch);
        }
        match (availability, restartable) {
            (Availability::Available, _) => Ok(pinned.into()),
            (Availability::NotAnswering, _) => Err(BrowserError::NotAnsweringBeforeDispatch),
            (Availability::Absent, Some(installed)) if !self.runs_as_another(&installed) => {
                self.restart_pinned_browser(context, pinned, &installed)
            }
            (Availability::Absent, _) => Err(self.pinned_browser_absent()),
        }
    }

    /// Whether the installed browser `installed` is connected now, under any browser identity.
    fn runs_as_another(&self, installed: &str) -> bool {
        self.browser.browsers().iter().any(|connected| {
            self.browser.installed_browser_of(&connected.id).as_deref() == Some(installed)
        })
    }

    /// Start `installed`, the installed browser this session's browser `pinned` connected as,
    /// and take the call there once `pinned` is back.
    ///
    /// Once that browser is connected, the call waits the return grace for `pinned` itself,
    /// since another profile of the browser may connect first, and keeps the rest of its deadline
    /// for the work. `pinned` still absent then, while another browser is connected, is
    /// [`BrowserError::WrongProfile`]. A browser that recovery leaves to the person gets the
    /// answer a session that waited for its browser always got.
    pub(super) fn restart_pinned_browser(
        &self,
        context: &InvocationContext<'_>,
        pinned: &str,
        installed: &str,
    ) -> Result<String, BrowserError> {
        let cancelled = context.cancellation.flag();
        match self
            .recovery
            .request_browser(installed, context.deadline, cancelled)
        {
            Ok(RecoveryDecision::Ready { .. }) => {
                match self.browser.await_available(
                    pinned,
                    return_grace_end(context.deadline),
                    cancelled,
                ) {
                    Availability::Available => Ok(pinned.into()),
                    _ if cancelled.load(Ordering::SeqCst) => {
                        Err(BrowserError::CancelledBeforeDispatch)
                    }
                    Availability::NotAnswering => Err(BrowserError::NotAnsweringBeforeDispatch),
                    Availability::Absent => Err(self.pinned_browser_absent()),
                }
            }
            Ok(RecoveryDecision::Manual { .. }) => Err(self.pinned_browser_absent()),
            Ok(RecoveryDecision::Failed { reason, details }) => {
                Err(BrowserError::RecoveryFailed { reason, details })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The answer for a session whose browser is not connected: not connected when no browser
    /// is, and the wrong profile when another browser is.
    pub(super) fn pinned_browser_absent(&self) -> BrowserError {
        if self.browser.browsers().is_empty() {
            BrowserError::DisconnectedBeforeDispatch
        } else {
            BrowserError::WrongProfile
        }
    }

    /// Find a browser for a session that works in none yet.
    ///
    /// While a browser is evidently on its way back -- a relay reconnecting right after the
    /// service published its endpoint, a connection still initializing or silent, or a
    /// connection that ended moments ago -- the call waits within the same bounded grace before
    /// recovery, so a service update never launches a browser the person already has open. The
    /// port reads that evidence again on every poll and ends the wait once it lapses, so a
    /// browser closed long ago, or a cold start with no browser, leaves recovery its launch
    /// budget.
    pub(super) fn await_or_recover_browser(
        &self,
        context: &InvocationContext<'_>,
    ) -> Result<String, BrowserError> {
        let cancelled = context.cancellation.flag();
        let returned = self
            .browser
            .await_any(return_grace_end(context.deadline), cancelled);
        if !returned.is_empty() {
            return choose_browser(context.requested_browser, None, &returned);
        }
        match self.recovery.request(context.deadline, cancelled) {
            Ok(RecoveryDecision::Manual { browsers }) => Err(BrowserError::RecoveryManual {
                browsers: browsers.into_iter().map(|browser| browser.name).collect(),
            }),
            Ok(RecoveryDecision::Failed { reason, details }) => {
                Err(BrowserError::RecoveryFailed { reason, details })
            }
            Ok(RecoveryDecision::Ready { browser }) => Ok(browser),
            Err(error) => Err(error.into()),
        }
    }
}

/// Why a session left its browser for another, as the process log says it.
#[derive(Clone, Copy)]
enum SessionMove {
    /// The agent named the browser it moved to.
    Named,
    /// It was the only browser connected.
    OnlyConnected,
}

impl SessionMove {
    const fn detail(self) -> &'static str {
        match self {
            Self::Named => "named by the agent",
            Self::OnlyConnected => "the only browser connected",
        }
    }
}

/// Where a call goes once the wait for `gone`, the browser its session works in, ended without
/// it, given the browsers `connected` now (ADR-0196 Decision 12 amendment, 2026-11-13).
///
/// The wait and this reading are two moments, so `gone` may be back: the call then goes to it and
/// the session moves nowhere, whatever else is connected. Otherwise one connected browser is no
/// choice and takes the call, several are a real choice between signed-in contexts and keep the
/// refusal, and none is not connected.
pub(super) fn after_absence(
    gone: &str,
    connected: &[crate::browser::BrowserSummary],
) -> Result<String, BrowserError> {
    if connected.iter().any(|browser| browser.id == gone) {
        return Ok(gone.into());
    }
    match connected {
        [] => Err(BrowserError::DisconnectedBeforeDispatch),
        [only] => Ok(only.id.clone()),
        _ => Err(BrowserError::WrongProfile),
    }
}

/// The refusal for a browser choice that could not be made, and the facts that explain it.
///
/// Returning candidates rather than a choice is the point: two connected browsers are two
/// different signed-in contexts, and guessing between them would put the person's work somewhere
/// they did not ask for.
pub(super) fn routing_refusal(
    error: &BrowserError,
    extension: impl FnOnce() -> ExtensionAdvice,
) -> Option<(Refusal, Value)> {
    match error {
        BrowserError::AmbiguousBrowser(candidates) => {
            Some((Refusal::BrowserAmbiguous, json!({"browsers":candidates})))
        }
        BrowserError::UnknownBrowser(_) => Some((Refusal::BrowserUnknown, json!({}))),
        BrowserError::BrowserPinned => Some((Refusal::BrowserPinned, json!({}))),
        BrowserError::Primitive { detail, cause } => Some((
            Refusal::BrowserPrimitive {
                detail: detail.clone(),
                cause: *cause,
            },
            json!({"detail":detail}),
        )),
        BrowserError::CaptureTooLarge { detail } => {
            Some((Refusal::CaptureTooLarge, json!({"detail":detail})))
        }
        BrowserError::RecoveryManual { browsers } => Some((
            Refusal::BrowserStartupManual {
                browsers: browsers.clone(),
                extension: extension(),
            },
            manual_browser_facts(browsers),
        )),
        BrowserError::RecoveryFailed { reason, details } => Some((
            Refusal::BrowserRecoveryFailed {
                reason: recovery_reason(*reason),
                // ExtensionAbsent names the store itself (S-D7); only a timeout depends on setup.
                extension: if *reason == RecoveryFailure::HandshakeTimeout {
                    extension()
                } else {
                    ExtensionAdvice::Plain
                },
            },
            json!({"details":details}),
        )),
        _ => None,
    }
}

fn manual_browser_facts(browsers: &[String]) -> Value {
    let mut facts = json!({
        "browsers": browsers,
    });
    if let [browser] = browsers {
        facts["browser"] = json!(browser);
    }
    facts
}

const fn recovery_reason(reason: RecoveryFailure) -> BrowserRecoveryReason {
    match reason {
        RecoveryFailure::BrowserAbsent => BrowserRecoveryReason::BrowserAbsent,
        RecoveryFailure::LaunchFailed => BrowserRecoveryReason::LaunchFailed,
        RecoveryFailure::SandboxedPackage => BrowserRecoveryReason::SandboxedPackage,
        RecoveryFailure::ExtensionAbsent => BrowserRecoveryReason::ExtensionAbsent,
        RecoveryFailure::NativeHostUnavailable => BrowserRecoveryReason::NativeHostUnavailable,
        RecoveryFailure::OwnedElsewhere => BrowserRecoveryReason::OwnedElsewhere,
        RecoveryFailure::HandshakeTimeout => BrowserRecoveryReason::HandshakeTimeout,
    }
}

/// When a call stops waiting for its browser to come back: after the return grace, and early
/// enough that a browser returning at the last moment still leaves the adapter receipt reserve
/// for the work itself.
fn return_grace_end(deadline: Instant) -> Instant {
    let now = Instant::now();
    let latest = deadline
        .checked_sub(Duration::from_millis(ADAPTER_RECEIPT_RESERVE_MS))
        .unwrap_or(now);
    (now + RETURN_GRACE).min(latest).max(now)
}
