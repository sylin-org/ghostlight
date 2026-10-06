//! Form, text, file, script, keyboard, and wait execution.

use std::time::Instant;

use ghostlight_bridge::browser::{
    BrowserCommand, BrowserOutcome, PhysicalActionSubject, PhysicalField,
};
use serde_json::{json, Value};

use crate::events::DomainEvent;
use crate::governance::{Capability, CapabilitySet};
use crate::language::{
    outcome::{Outcome, Refusal},
    FillForm, PressKey, RunScript, TypeText, UploadFiles, Wait,
};
use crate::workspace::{SelectedTab, WorkspaceError, WorkspaceLease};

use super::{
    action_subject, adapter_budget_ms, bounded, load_physical_files, named_key, observed_host,
    readiness, ApplicationExecutor, Conclusion, Effect, InvocationContext, ResolvedLocation,
    Status, WorkEvidence,
};
use ghostlight_bridge::browser::PhysicalFile;

struct WaitObservation {
    satisfied: bool,
    elapsed_ms: u64,
    readiness: ghostlight_bridge::browser::BrowserReadiness,
}

fn wait_budget_ms(context: &InvocationContext<'_>) -> u64 {
    u64::try_from(
        context
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

impl ApplicationExecutor {
    pub(super) fn perform_fill(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &FillForm,
    ) -> WorkEvidence {
        let mut resolved = Vec::with_capacity(value.fields.len());
        let mut selected: Option<SelectedTab> = None;
        if value.fields.iter().any(|field| field.selector.is_some()) {
            let established = match lease.select_tab(value.tab.as_deref()) {
                Ok(tab) => tab,
                Err(error) => return self.workspace_failure(context, error),
            };
            let pre = self.authorize(context, CapabilitySet::READ, Some(established.url.as_str()));
            if !pre.allowed {
                return self.blocked(
                    context,
                    pre,
                    Some(established.physical_id),
                    Effect::None,
                    true,
                    json!({"reason":pre.reason.as_str()}),
                );
            }
            selected = Some(established);
        }
        for field in &value.fields {
            let (tab, target) = if let Some(selector) = &field.selector {
                match self.resolve_semantic(
                    context,
                    lease,
                    value
                        .tab
                        .as_deref()
                        .or_else(|| selected.as_ref().map(|tab| tab.handle.as_str())),
                    selector,
                ) {
                    Ok(value) => value,
                    Err(terminal) => return *terminal,
                }
            } else {
                match self.resolve_target(
                    context,
                    lease,
                    value
                        .tab
                        .as_deref()
                        .or_else(|| selected.as_ref().map(|tab| tab.handle.as_str())),
                    field
                        .target
                        .as_deref()
                        .expect("validated target or selector"),
                ) {
                    Ok(value) => value,
                    Err(error) => return self.workspace_failure(context, error),
                }
            };
            if let Some(current) = &selected {
                if current.handle != tab.handle {
                    return self.workspace_failure(context, WorkspaceError::TargetTabMismatch);
                }
            } else {
                selected = Some(tab);
            }
            resolved.push((target, form_field_wire(&field.value)));
        }
        let selected = selected.expect("validated non-empty fields");
        let submit = match value.submit_target.as_deref() {
            Some(handle) => {
                match self.resolve_target(context, lease, Some(selected.handle.as_str()), handle) {
                    Ok((_, target)) => Some(target),
                    Err(error) => return self.workspace_failure(context, error),
                }
            }
            None => None,
        };
        let requirements = if submit.is_some() {
            CapabilitySet::READ
                .union(CapabilitySet::WRITE)
                .union(CapabilitySet::ACTION)
        } else {
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        };
        let decision = self.authorize(context, requirements, Some(selected.url.as_str()));
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
        let mut locators: Vec<_> = resolved
            .iter()
            .map(|(target, _)| target.locator.clone())
            .collect();
        if let Some(target) = &submit {
            locators.push(target.locator.clone());
        }
        match self.dispatch(
            context,
            BrowserCommand::DescribeTargets {
                tab_id: selected.physical_id,
                locators: locators.clone(),
            },
        ) {
            Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                if tab_id == selected.physical_id =>
            {
                if targets.len() != locators.len() {
                    return self.protocol_failure(context, decision, Some(tab_id));
                }
                if !value.user_authorized_credentials
                    && targets.iter().any(|target| target.credential_class)
                {
                    return self.credential_guidance(context, decision, &selected);
                }
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
        let fields = resolved
            .into_iter()
            .map(|(target, value)| PhysicalField {
                locator: target.locator,
                value,
            })
            .collect();
        match self.dispatch(
            context,
            BrowserCommand::Fill {
                tab_id: selected.physical_id,
                fields,
                allow_credentials: value.user_authorized_credentials,
                submit_locator: submit.map(|target| target.locator),
                timeout_ms: adapter_budget_ms(
                    value.timeout_ms,
                    context.deadline.saturating_duration_since(Instant::now()),
                ),
            },
        ) {
            Ok(BrowserOutcome::Filled {
                tab,
                filled_count,
                submitted,
                committed_urls,
            }) => self.finish_with_expectation(context, lease, decision, requirements, &selected, &tab, &committed_urls, Outcome::FormFilled { fields: filled_count, submitted, host: observed_host(&tab.url) }, json!({"tab":selected.handle.as_str(),"filled_count":filled_count,"submitted":submitted}), value.expect.as_ref()),
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn perform_type_text(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TypeText,
    ) -> WorkEvidence {
        if value.focused {
            return self.type_focused(context, lease, value);
        }
        let (selected, target) = if let Some(selector) = &value.selector {
            match self.resolve_semantic(context, lease, value.tab.as_deref(), selector) {
                Ok(value) => value,
                Err(terminal) => return *terminal,
            }
        } else {
            match self.resolve_target(context, lease, value.tab.as_deref(), &value.target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            }
        };
        let typed_role = target.role;
        let decision = self.authorize(context, context.requirements, Some(selected.url.as_str()));
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
            BrowserCommand::DescribeTargets {
                tab_id: selected.physical_id,
                locators: vec![target.locator.clone()],
            },
        ) {
            Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                if tab_id == selected.physical_id && targets.len() == 1 =>
            {
                if targets[0].credential_class && !value.user_authorized_credentials {
                    return self.credential_guidance(context, decision, &selected);
                }
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
        self.emit(DomainEvent::TargetIndicated {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: selected.physical_id,
            locator: target.locator.clone(),
            click: None,
        });
        match self.dispatch(
            context,
            BrowserCommand::TypeText {
                tab_id: selected.physical_id,
                locator: target.locator,
                text: value.text.clone(),
                clear_first: value.clear_first,
                allow_credentials: value.user_authorized_credentials,
            },
        ) {
            Ok(BrowserOutcome::Typed {
                tab,
                character_count,
                subject,
                committed_urls,
            }) => {
                let outcome = Outcome::TextTyped {
                    host: observed_host(&tab.url),
                    subject: action_subject(context, subject, Some(typed_role))
                        .expect("typing has a fallback subject"),
                    characters: character_count,
                };
                self.finish_with_expectation(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    &selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    json!({"tab":selected.handle.as_str(),"target":target.handle.as_str(),"typed":true,"character_count":character_count}),
                    value.expect.as_ref(),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn upload_files(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &UploadFiles,
    ) -> WorkEvidence {
        let dropping = value.view.is_some();
        let (selected, target_locator, target_role, drop) = if dropping {
            let location = match self.resolve_location(
                context,
                lease,
                value.tab.as_deref(),
                None,
                value.view.as_deref(),
                value.x,
                value.y,
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            match location {
                ResolvedLocation::Point { tab, view, point } => {
                    (tab, None, None, Some((point, view.viewport)))
                }
                ResolvedLocation::Target { .. } => {
                    unreachable!("a view input resolves to a point")
                }
            }
        } else if let Some(selector) = &value.selector {
            match self.resolve_semantic(context, lease, value.tab.as_deref(), selector) {
                Ok((tab, target)) => {
                    let locator = target.locator.clone();
                    let role = target.role;
                    (tab, Some(locator), Some(role), None)
                }
                Err(terminal) => return *terminal,
            }
        } else {
            match self.resolve_target(
                context,
                lease,
                value.tab.as_deref(),
                value.target.as_deref().expect("validated target"),
            ) {
                Ok((tab, target)) => {
                    let locator = target.locator.clone();
                    let role = target.role;
                    (tab, Some(locator), Some(role), None)
                }
                Err(error) => return self.workspace_failure(context, error),
            }
        };
        let decision = self.authorize(context, Capability::Write, Some(selected.url.as_str()));
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
        if !dropping {
            match self.dispatch(
                context,
                BrowserCommand::DescribeTargets {
                    tab_id: selected.physical_id,
                    locators: vec![target_locator.clone().expect("attach has a locator")],
                },
            ) {
                Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                    if tab_id == selected.physical_id && targets.len() == 1 => {}
                Ok(_) => {
                    return self.protocol_failure(context, decision, Some(selected.physical_id))
                }
                Err(error) => {
                    return self.browser_failure(
                        context,
                        decision,
                        error,
                        Some(selected.physical_id),
                    )
                }
            }
        }
        let (files, total) = if !value.paths.is_empty() {
            match load_physical_files(&value.paths) {
                Ok(value) => value,
                Err(reason) => {
                    return self.failed(
                        context,
                        decision,
                        Some(selected.physical_id),
                        Refusal::FilesUnreadable,
                        json!({"reason":reason}),
                    )
                }
            }
        } else if !value.files.is_empty() {
            match decode_inline_files(&value.files) {
                Ok(value) => value,
                Err(reason) => {
                    return self.failed(
                        context,
                        decision,
                        Some(selected.physical_id),
                        Refusal::FilesUnreadable,
                        json!({"reason":reason}),
                    )
                }
            }
        } else {
            let image = value
                .source_image
                .as_deref()
                .expect("validated source_image");
            match lease.take_image(image, &selected) {
                Some((mime_type, data)) => {
                    let decoded = (data.len() / 4 * 3) as u64;
                    (
                        vec![PhysicalFile {
                            name: "capture".into(),
                            media_type: mime_type,
                            data,
                            size: decoded,
                        }],
                        decoded,
                    )
                }
                None => {
                    return self.failed(
                        context,
                        decision,
                        Some(selected.physical_id),
                        Refusal::FilesUnreadable,
                        json!({"reason":"the captured image is stale or belongs elsewhere"}),
                    )
                }
            }
        };
        let expected_count = files.len();
        let dispatch = if dropping {
            let (point, viewport) = drop.expect("validated drop geometry");
            BrowserCommand::DropImageAt {
                tab_id: selected.physical_id,
                point,
                expected_viewport: viewport,
                file: files.into_iter().next().expect("one image"),
            }
        } else {
            BrowserCommand::UploadFiles {
                tab_id: selected.physical_id,
                locator: target_locator.expect("attach has a locator"),
                files,
            }
        };
        match self.dispatch(context, dispatch) {
            Ok(BrowserOutcome::FilesUploaded {
                tab_id,
                uploaded_count,
                uploaded_bytes,
                subject,
            }) if tab_id == selected.physical_id
                && uploaded_count == expected_count
                && uploaded_bytes == total =>
            {
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    false,
                    Outcome::FilesUploaded {
                        count: uploaded_count,
                        host: observed_host(&selected.url),
                        subject: action_subject(context, subject, target_role),
                    },
                    json!({"tab":selected.handle.as_str(),"dropped":dropping,"uploaded_count":uploaded_count,"uploaded_bytes":uploaded_bytes}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn run_script(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &RunScript,
    ) -> WorkEvidence {
        self.with_authorized_tab(
            context,
            lease,
            value.tab.as_deref(),
            Capability::Execute,
            |selected, decision| {
                match self.dispatch(
                    context,
                    BrowserCommand::EvaluateScript {
                        tab_id: selected.physical_id,
                        script: value.script.clone(),
                        max_result_chars: value.max_result_chars,
                    },
                ) {
                    Ok(BrowserOutcome::ScriptEvaluated {
                        tab,
                        value,
                        truncated,
                        committed_urls,
                    }) => {
                        let rendered = serde_json::from_str(&value).unwrap_or(Value::String(value));
                        let outcome = Outcome::ScriptEvaluated {
                            host: observed_host(&tab.url),
                        };
                        self.action_success(
                            context,
                            lease,
                            decision,
                            Capability::Execute,
                            selected,
                            &tab,
                            &committed_urls,
                            outcome,
                            json!({"tab":selected.handle.as_str(),"value":rendered,"truncated":truncated}),
                        )
                    }
                    Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
                    Err(error) => {
                        self.browser_failure(context, decision, error, Some(selected.physical_id))
                    }
                }
            },
        )
    }

    pub(super) fn perform_key(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &PressKey,
    ) -> WorkEvidence {
        self.with_authorized_optional_target(
            context,
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            context.requirements,
            |selected, locator, focused_role, decision| {
                let strokes: Vec<String> = if value.strokes.is_empty() {
                    vec![value.key.clone()]
                } else {
                    value.strokes.clone()
                };
                let repetitions = usize::from(value.repeat.max(1));
                let total = strokes.len().saturating_mul(repetitions);
                context
                    .execution
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .expect(total);
                let mut current = selected.clone();
                let mut last = None;
                for _ in 0..repetitions {
                    for stroke in &strokes {
                        if context.cancellation.is_cancelled() {
                            return self.browser_failure(
                                context,
                                decision,
                                crate::browser::BrowserError::CancelledBeforeDispatch,
                                Some(current.physical_id),
                            );
                        }
                        // A prior Enter may have navigated. Use its governed landing and refuse
                        // a locator from an earlier generation before sending another stroke.
                        if locator.is_some() && current.generation != selected.generation {
                            return self.workspace_failure(context, WorkspaceError::StaleTarget);
                        }
                        let decision =
                            self.authorize(context, context.requirements, Some(&current.url));
                        if !decision.allowed {
                            return self.blocked(
                                context,
                                decision,
                                Some(current.physical_id),
                                Effect::None,
                                false,
                                json!({"reason":decision.reason.as_str()}),
                            );
                        }
                        let receipt = match self.dispatch(
                            context,
                            BrowserCommand::PressKey {
                                tab_id: current.physical_id,
                                locator: locator.clone(),
                                key: stroke.clone(),
                                modifiers: value.modifiers.clone(),
                            },
                        ) {
                            Ok(receipt @ BrowserOutcome::KeyPressed { .. }) => receipt,
                            Ok(_) => {
                                return self.protocol_failure(
                                    context,
                                    decision,
                                    Some(current.physical_id),
                                )
                            }
                            Err(error) => {
                                return self.browser_failure(
                                    context,
                                    decision,
                                    error,
                                    Some(current.physical_id),
                                )
                            }
                        };
                        let BrowserOutcome::KeyPressed {
                            tab,
                            key,
                            subject,
                            committed_urls,
                        } = receipt
                        else {
                            unreachable!("the validated stroke receipt is a key receipt");
                        };
                        let outcome = Outcome::KeyboardSent {
                            host: observed_host(&tab.url),
                            key: named_key(&key),
                            subject: action_subject(context, subject, focused_role),
                        };
                        let facts = json!({"tab":current.handle.as_str(),"key":key,"pressed":true});
                        let applied = self.action_success(
                            context,
                            lease,
                            decision,
                            Capability::Action,
                            &current,
                            &tab,
                            &committed_urls,
                            outcome,
                            facts,
                        );
                        if !applied.completed() {
                            return applied;
                        }
                        current = match lease.select_tab(Some(current.handle.as_str())) {
                            Ok(tab) => tab,
                            Err(error) => return self.workspace_failure(context, error),
                        };
                        last = Some(applied);
                    }
                }
                self.verify_action(
                    context,
                    &current,
                    last.expect("at least one stroke was dispatched"),
                    value.expect.as_ref(),
                )
            },
        )
    }

    pub(super) fn perform_wait(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Wait,
    ) -> WorkEvidence {
        self.with_authorized_optional_target(
            context,
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            Capability::Read,
            |selected, locator, _target_role, decision| {
                let context = InvocationContext {
                    deadline: context
                        .deadline
                        .min(Instant::now() + std::time::Duration::from_millis(value.timeout_ms)),
                    ..*context
                };
                self.workbench.wait_progress(
                    context.invocation,
                    context.workspace.as_str(),
                    crate::language::progress::WaitProgress::condition(
                        value,
                        wait_budget_ms(&context),
                    ),
                );
                let mut observed =
                    match self.wait_condition(&context, selected, locator.clone(), value) {
                        Ok(observed) => observed,
                        Err(error) => {
                            return self.browser_failure(
                                &context,
                                decision,
                                error,
                                Some(selected.physical_id),
                            )
                        }
                    };
                let should_settle = match value.condition.as_str() {
                    "duration" | "selector_present" | "load_ready" | "target_present" => {
                        value.visual_settle != Some(false)
                    }
                    "visual_settle" | "layout_stable" => false,
                    _ => value.visual_settle == Some(true),
                };
                let mut visual_settled = None;
                if observed.satisfied && should_settle && wait_budget_ms(&context) > 0 {
                    self.workbench.wait_progress(
                        context.invocation,
                        context.workspace.as_str(),
                        crate::language::progress::WaitProgress::Settlement {
                            budget_ms: wait_budget_ms(&context)
                                .min(crate::language::settlement::MAX_WAIT_MS),
                        },
                    );
                    match self.observe_wait(
                        &context,
                        selected,
                        locator,
                        "visual_settle",
                        None,
                        crate::language::settlement::MAX_WAIT_MS,
                    ) {
                        Ok(settled) => {
                            visual_settled = Some(settled.satisfied);
                            observed.elapsed_ms =
                                observed.elapsed_ms.saturating_add(settled.elapsed_ms);
                            observed.readiness = settled.readiness;
                        }
                        Err(error) => {
                            let mut failed = self.browser_failure(
                                &context,
                                decision,
                                error,
                                Some(selected.physical_id),
                            );
                            failed.payload.facts["condition_satisfied"] = json!(true);
                            failed.payload.facts["wait_phase"] = json!("visual_settle");
                            return failed;
                        }
                    }
                }
                if let Err(error) = lease.update_readiness(&selected.handle, observed.readiness) {
                    return self.workspace_failure(&context, error);
                }
                WorkEvidence::new(
                    context.invocation,
                    if observed.satisfied {
                        Status::Succeeded
                    } else {
                        Status::Failed
                    },
                    Effect::None,
                    readiness(observed.readiness),
                    true,
                    Conclusion::Outcome(Outcome::Waited {
                        condition: value.condition.clone(),
                        elapsed_ms: observed.elapsed_ms,
                        satisfied: observed.satisfied,
                        host: observed_host(&selected.url),
                    }),
                    json!({
                        "tab":selected.handle.as_str(),
                        "condition":value.condition,
                        "satisfied":observed.satisfied,
                        "elapsed_ms":observed.elapsed_ms,
                        "readiness":readiness(observed.readiness),
                        "visual_settle":should_settle,
                        "visual_settled":visual_settled,
                    }),
                    decision,
                    Some(selected.physical_id),
                )
            },
        )
    }

    /// Execute the requested condition within its original deadline, without an extra call.
    fn wait_condition(
        &self,
        context: &InvocationContext<'_>,
        selected: &SelectedTab,
        locator: Option<String>,
        value: &Wait,
    ) -> Result<WaitObservation, crate::browser::BrowserError> {
        use crate::browser::BrowserError;
        let started = Instant::now();
        if value.condition == "duration" {
            let milliseconds: u64 = value
                .value
                .as_deref()
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(0);
            loop {
                if context.cancellation.is_cancelled() {
                    return Err(BrowserError::CancelledBeforeDispatch);
                }
                if Instant::now() >= context.deadline {
                    return Err(BrowserError::DeadlineBeforeDispatch);
                }
                if started.elapsed().as_millis() >= u128::from(milliseconds) {
                    return Ok(WaitObservation {
                        satisfied: true,
                        elapsed_ms: u64::try_from(started.elapsed().as_millis())
                            .unwrap_or(u64::MAX),
                        readiness: selected.readiness,
                    });
                }
                std::thread::sleep(
                    std::time::Duration::from_millis(20)
                        .min(context.deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
        if value.condition == "selector_present" {
            let selector = value
                .selector
                .as_ref()
                .expect("Language validated the selector");
            loop {
                match self.dispatch(
                    context,
                    BrowserCommand::QuerySemantic {
                        tab_id: selected.physical_id,
                        name: selector.name.clone(),
                        role: selector.role.clone(),
                        exact: selector.exact,
                        form_scope: false,
                    },
                )? {
                    BrowserOutcome::Targets { tab_id, targets }
                        if tab_id == selected.physical_id =>
                    {
                        if !targets.is_empty() {
                            return Ok(WaitObservation {
                                satisfied: true,
                                elapsed_ms: u64::try_from(started.elapsed().as_millis())
                                    .unwrap_or(u64::MAX),
                                readiness: selected.readiness,
                            });
                        }
                    }
                    _ => {
                        return Err(BrowserError::Protocol(
                            "Incompatible wait observation.".into(),
                        ))
                    }
                }
                if Instant::now() >= context.deadline {
                    return Ok(WaitObservation {
                        satisfied: false,
                        elapsed_ms: u64::try_from(started.elapsed().as_millis())
                            .unwrap_or(u64::MAX),
                        readiness: selected.readiness,
                    });
                }
                std::thread::sleep(
                    std::time::Duration::from_millis(100)
                        .min(context.deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
        self.observe_wait(
            context,
            selected,
            locator,
            &value.condition,
            value.value.clone(),
            wait_budget_ms(context),
        )
    }

    /// Read one exact observation receipt. Settlement uses the same validation and failure path.
    fn observe_wait(
        &self,
        context: &InvocationContext<'_>,
        selected: &SelectedTab,
        locator: Option<String>,
        condition: &str,
        value: Option<String>,
        budget_ms: u64,
    ) -> Result<WaitObservation, crate::browser::BrowserError> {
        match self.dispatch(
            context,
            BrowserCommand::Observe {
                tab_id: selected.physical_id,
                condition: condition.into(),
                value,
                locator,
                timeout_ms: adapter_budget_ms(
                    budget_ms.min(wait_budget_ms(context)),
                    context.deadline.saturating_duration_since(Instant::now()),
                ),
            },
        )? {
            BrowserOutcome::Observed {
                tab_id,
                satisfied,
                elapsed_ms,
                readiness,
            } if tab_id == selected.physical_id => Ok(WaitObservation {
                satisfied,
                elapsed_ms,
                readiness,
            }),
            _ => Err(crate::browser::BrowserError::Protocol(
                "Incompatible wait observation.".into(),
            )),
        }
    }

    fn type_focused(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TypeText,
    ) -> WorkEvidence {
        self.with_authorized_tab(
            context,
            lease,
            value.tab.as_deref(),
            context.requirements,
            |selected, decision| {
                // The describe step is not only a credential gate: the control it observes is the
                // typing receipt's subject. The typed outcome itself carries no subject, so without
                // this fallback a fully successful focused typing panicked after the effect landed
                // (the work done, the operation never settled).
                let focused = match self.dispatch(
                    context,
                    BrowserCommand::DescribeFocused {
                        tab_id: selected.physical_id,
                    },
                ) {
                    Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                        if tab_id == selected.physical_id && targets.len() == 1 =>
                    {
                        if targets[0].credential_class && !value.user_authorized_credentials {
                            return self.credential_guidance(context, decision, selected);
                        }
                        Some(PhysicalActionSubject {
                            role: targets[0].role.clone(),
                            name: targets[0].name.clone(),
                        })
                    }
                    Ok(_) => {
                        return self.protocol_failure(context, decision, Some(selected.physical_id))
                    }
                    Err(error) => {
                        return self.browser_failure(
                            context,
                            decision,
                            error,
                            Some(selected.physical_id),
                        )
                    }
                };
                match self.dispatch(
                    context,
                    BrowserCommand::TypeFocused {
                        tab_id: selected.physical_id,
                        text: value.text.clone(),
                        clear_first: value.clear_first,
                        allow_credentials: value.user_authorized_credentials,
                    },
                ) {
                    Ok(BrowserOutcome::Typed {
                        tab,
                        character_count,
                        subject,
                        committed_urls,
                    }) => {
                        let outcome = Outcome::TextTyped {
                            host: observed_host(&tab.url),
                            subject: action_subject(context, subject.or(focused), None)
                                .expect("focused typing always names its control"),
                            characters: character_count,
                        };
                        self.finish_with_expectation(
                            context,
                            lease,
                            decision,
                            Capability::Action,
                            selected,
                            &tab,
                            &committed_urls,
                            outcome,
                            json!({"tab":selected.handle.as_str(),"focused":true,"typed":true,"character_count":character_count}),
                            value.expect.as_ref(),
                        )
                    }
                    Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
                    Err(error) => {
                        self.browser_failure(context, decision, error, Some(selected.physical_id))
                    }
                }
            },
        )
    }
}

fn form_field_wire(value: &crate::language::FormFieldValue) -> String {
    match value {
        crate::language::FormFieldValue::Text(text) => text.clone(),
        crate::language::FormFieldValue::Flag(flag) => {
            if *flag {
                "true".into()
            } else {
                "false".into()
            }
        }
        crate::language::FormFieldValue::Number(number) => {
            if number.fract() == 0.0 && number.abs() < 1.0e15 {
                format!("{}", *number as i64)
            } else {
                number.to_string()
            }
        }
    }
}

fn decode_inline_files(
    files: &[crate::language::InlineFile],
) -> Result<(Vec<PhysicalFile>, u64), String> {
    use base64::Engine as _;
    const UPLOAD_AGGREGATE_BYTES: usize = 5_000_000;
    let mut decoded = Vec::with_capacity(files.len());
    let mut total = 0u64;
    for file in files {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&file.data_base64)
            .map_err(|_| "an inline file is not valid base64".to_string())?;
        total = total.saturating_add(bytes.len() as u64);
        if total > UPLOAD_AGGREGATE_BYTES as u64 {
            return Err("inline files exceed the upload ceiling".into());
        }
        decoded.push(PhysicalFile {
            name: bounded(&file.name, 255),
            media_type: bounded(&file.media_type, 100),
            data: file.data_base64.clone(),
            size: bytes.len() as u64,
        });
    }
    Ok((decoded, total))
}
