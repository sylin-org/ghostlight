//! Form, text, file, script, keyboard, and wait execution.

use std::time::Instant;

use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, FieldNow, ObservedTarget, PhysicalActionSubject, PhysicalField,
    PhysicalValue,
};
use crate::browser::BrowserError;
use crate::governance::Decision;
use serde_json::{json, Value};

use crate::events::DomainEvent;
use crate::governance::{Capability, CapabilitySet};
use crate::language::{
    field_echo::{self, HeldField},
    field_problem::{self, ControlRef, FieldProblem, RejectedControl, RejectedField},
    outcome::{Observed, Outcome, Refusal, TargetRole},
    FillForm, PressKey, RunScript, TypeText, UploadFiles, Wait,
};
use crate::workspace::{SelectedTab, WorkspaceError, WorkspaceLease};

use super::result::Readiness;
use super::{
    action_subject, adapter_budget_ms, bounded, load_physical_files, named_key, observed_host,
    readiness, ApplicationExecutor, Effect, InvocationContext, InvocationResult, ResolvedLocation,
    Status, Terminal,
};
use crate::browser::adapter::PhysicalFile;

impl ApplicationExecutor {
    pub(super) fn perform_fill(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &FillForm,
    ) -> Terminal {
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
        let given: Vec<(String, PhysicalValue)> = resolved
            .iter()
            .map(|(target, value)| (target.handle.as_str().to_owned(), value.clone()))
            .collect();
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
        let described = match self.dispatch(
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
                if !value.user_authorized_credentials {
                    if let Some(index) = targets.iter().position(|target| target.credential_class) {
                        let control = credential_control(
                            context,
                            &targets[index],
                            given.get(index).map(|(handle, _)| handle.as_str()),
                        );
                        return self.credential_guidance(context, decision, &selected, control);
                    }
                }
                targets
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        };
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
                held,
                submitted,
                committed_urls,
            }) => {
                let Some(held) = held_fields(context, &held, &given, &described) else {
                    return self.protocol_failure(context, decision, Some(selected.physical_id));
                };
                let mut facts = json!({"tab":selected.handle.as_str(),"filled_count":filled_count,"submitted":submitted});
                if let Some(fields) = field_echo::facts(&held) {
                    facts["fields"] = fields;
                }
                self.finish_with_expectation(
                    context,
                    lease,
                    decision,
                    requirements,
                    &selected,
                    &tab,
                    &committed_urls,
                    Outcome::FormFilled {
                        fields: filled_count,
                        submitted,
                        host: observed_host(&tab.url),
                        held,
                    },
                    facts,
                    value.expect.as_ref(),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(BrowserError::FieldsRejected { problems }) => self.fields_rejected(
                context,
                decision,
                selected.physical_id,
                problems,
                &given,
                &described,
            ),
            Err(BrowserError::FieldsReverted { set, problems }) => self.fields_reverted(
                context,
                decision,
                selected.physical_id,
                set,
                problems,
                &given,
                &described,
            ),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    /// The refusal of a fill that the page declined before any edit, with what would succeed.
    ///
    /// Each problem names the field by its position in the call; the service pairs it with that
    /// field's handle, given value and last observed role and name. A position the call does
    /// not have is a page that disagrees with the contract.
    fn fields_rejected(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: u64,
        problems: Vec<FieldProblem>,
        given: &[(String, PhysicalValue)],
        described: &[ObservedTarget],
    ) -> Terminal {
        let Some(rejected) = rejected_fields(context, problems, given, described) else {
            return self.protocol_failure(context, decision, Some(physical_id));
        };
        let facts = field_problem::rejected_facts(&rejected);
        self.failed(
            context,
            decision,
            Some(physical_id),
            Refusal::FieldValuesRejected {
                fields: rejected,
                total: given.len(),
            },
            facts,
        )
    }

    /// The result of a fill the page partly undid: some fields hold their values and the named
    /// ones went back after the page reacted. The effect is partial and repeating is not safe,
    /// because the fields that held would be filled again.
    #[allow(clippy::too_many_arguments)]
    fn fields_reverted(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: u64,
        set: usize,
        problems: Vec<FieldProblem>,
        given: &[(String, PhysicalValue)],
        described: &[ObservedTarget],
    ) -> Terminal {
        let Some(reverted) = rejected_fields(context, problems, given, described) else {
            return self.protocol_failure(context, decision, Some(physical_id));
        };
        let facts = field_problem::reverted_facts(set, &reverted);
        let refusal = Refusal::FieldsReverted { set, reverted };
        let summary = refusal.summary();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Failed,
                Effect::Partial,
                Readiness::Unknown,
                false,
                &summary,
                facts,
                refusal.next_steps(),
            ),
            decision,
            physical_id: Some(physical_id),
            observed: Observed::default(),
            audit: refusal.audit(),
        }
    }

    pub(super) fn perform_type_text(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TypeText,
    ) -> Terminal {
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
                    let control =
                        credential_control(context, &targets[0], Some(target.handle.as_str()));
                    return self.credential_guidance(context, decision, &selected, control);
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
    ) -> Terminal {
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
    ) -> Terminal {
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
    ) -> Terminal {
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
                let mut completed = 0usize;
                let mut last = None;
                for _ in 0..repetitions {
                    for stroke in &strokes {
                        if context.cancellation.is_cancelled() {
                            let error = if completed == 0 {
                                crate::browser::BrowserError::CancelledBeforeDispatch
                            } else {
                                crate::browser::BrowserError::CancelledAfterDispatch
                            };
                            return self.browser_failure(
                                context,
                                decision,
                                error,
                                Some(selected.physical_id),
                            );
                        }
                        match self.dispatch(
                            context,
                            BrowserCommand::PressKey {
                                tab_id: selected.physical_id,
                                locator: locator.clone(),
                                key: stroke.clone(),
                                modifiers: value.modifiers.clone(),
                            },
                        ) {
                            Ok(receipt @ BrowserOutcome::KeyPressed { .. }) => {
                                last = Some(receipt);
                                completed += 1;
                            }
                            Ok(_) => {
                                return self.protocol_failure(
                                    context,
                                    decision,
                                    Some(selected.physical_id),
                                )
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
                }
                let BrowserOutcome::KeyPressed {
                    tab,
                    key,
                    subject,
                    committed_urls,
                } = last.expect("at least one stroke is dispatched")
                else {
                    unreachable!("the stroke loop produced a key receipt");
                };
                let outcome = Outcome::KeyboardSent {
                    host: observed_host(&tab.url),
                    key: named_key(&key),
                    subject: action_subject(context, subject, focused_role),
                };
                let mut facts = json!({"tab":selected.handle.as_str(),"key":key,"pressed":true});
                if total > 1 {
                    facts["strokes_completed"] = json!(completed);
                    facts["total_expected"] = json!(total);
                }
                self.finish_with_expectation(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    facts,
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
    ) -> Terminal {
        self.with_authorized_optional_target(
            context,
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            Capability::Read,
            |selected, locator, _target_role, decision| {
                if value.condition == "duration" {
                    let milliseconds: u64 = value
                        .value
                        .as_deref()
                        .and_then(|raw| raw.parse().ok())
                        .unwrap_or(0);
                    let started = Instant::now();
                    loop {
                        if context.cancellation.is_cancelled() {
                            return self.browser_failure(
                                context,
                                decision,
                                crate::browser::BrowserError::CancelledBeforeDispatch,
                                Some(selected.physical_id),
                            );
                        }
                        if started.elapsed().as_millis() >= u128::from(milliseconds) {
                            break;
                        }
                        if Instant::now() >= context.deadline {
                            return self.browser_failure(
                                context,
                                decision,
                                crate::browser::BrowserError::DeadlineAfterDispatch,
                                Some(selected.physical_id),
                            );
                        }
                        std::thread::sleep(std::cmp::min(
                            std::time::Duration::from_millis(20),
                            context.deadline.saturating_duration_since(Instant::now()),
                        ));
                    }
                    let elapsed_ms =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let should_settle = value.visual_settle != Some(false);
                    let settlement = if should_settle {
                        match self.settle_wait(
                            context,
                            lease,
                            selected,
                            locator.clone(),
                            value.timeout_ms.saturating_sub(elapsed_ms),
                        ) {
                            Ok(observation) => observation,
                            Err(error) => {
                                return self.browser_failure(
                                    context,
                                    decision,
                                    error,
                                    Some(selected.physical_id),
                                )
                            }
                        }
                    } else {
                        None
                    };
                    let visual_settled = settlement.map(|(satisfied, _)| satisfied);
                    let final_satisfied = true;
                    let final_elapsed_ms =
                        elapsed_ms.saturating_add(settlement.map_or(0, |(_, elapsed)| elapsed));
                    let status = if final_satisfied {
                        Status::Succeeded
                    } else {
                        Status::Failed
                    };
                    let outcome = Outcome::Waited {
                        condition: value.condition.clone(),
                        elapsed_ms: final_elapsed_ms,
                        satisfied: final_satisfied,
                        host: observed_host(&selected.url),
                    };
                    let facts = json!({
                        "tab": selected.handle.as_str(),
                        "condition": "duration",
                        "satisfied": final_satisfied,
                        "elapsed_ms": final_elapsed_ms,
                        "readiness": readiness(selected.readiness),
                        "visual_settle": should_settle,
                        "visual_settled": visual_settled,
                    });
                    return Terminal {
                        result: InvocationResult::new(
                            context.invocation,
                            status,
                            Effect::None,
                            readiness(selected.readiness),
                            true,
                            outcome.summary().as_str(),
                            facts,
                            outcome.next_steps(),
                        ),
                        decision,
                        physical_id: Some(selected.physical_id),
                        observed: outcome.observed(),
                        audit: outcome.audit(),
                    };
                }
                // Waiting on what the page calls a control: polled executor-side through the same
                // semantic query every selector-based action uses, so no handle pre-resolution is
                // needed and a stale handle can never be required to wait.
                if value.condition == "selector_present" {
                    let Some(selector) = value.selector.as_ref() else {
                        return self.failed(
                            context,
                            decision,
                            Some(selected.physical_id),
                            Refusal::InvalidRequest,
                            json!({"reason":"invalid_request"}),
                        );
                    };
                    let started = Instant::now();
                    let budget_end = started
                        + std::cmp::min(
                            std::time::Duration::from_millis(value.timeout_ms),
                            context.deadline.saturating_duration_since(started),
                        );
                    let mut satisfied = false;
                    loop {
                        if context.cancellation.is_cancelled() {
                            return self.browser_failure(
                                context,
                                decision,
                                crate::browser::BrowserError::CancelledBeforeDispatch,
                                Some(selected.physical_id),
                            );
                        }
                        match self.dispatch(
                            context,
                            BrowserCommand::QuerySemantic {
                                tab_id: selected.physical_id,
                                name: selector.name.clone(),
                                role: selector.role.clone(),
                                exact: selector.exact,
                                form_scope: false,
                            },
                        ) {
                            Ok(BrowserOutcome::Targets { targets, .. }) if !targets.is_empty() => {
                                satisfied = true;
                                break;
                            }
                            Ok(_) => {}
                            Err(error) => {
                                return self.browser_failure(
                                    context,
                                    decision,
                                    error,
                                    Some(selected.physical_id),
                                )
                            }
                        }
                        if Instant::now() >= budget_end || Instant::now() >= context.deadline {
                            break;
                        }
                        std::thread::sleep(std::cmp::min(
                            std::time::Duration::from_millis(100),
                            context.deadline.saturating_duration_since(Instant::now()),
                        ));
                    }
                    let elapsed_ms =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let should_settle = value.visual_settle != Some(false);
                    let settlement = if satisfied && should_settle {
                        match self.settle_wait(
                            context,
                            lease,
                            selected,
                            locator.clone(),
                            value.timeout_ms.saturating_sub(elapsed_ms),
                        ) {
                            Ok(observation) => observation,
                            Err(error) => {
                                return self.browser_failure(
                                    context,
                                    decision,
                                    error,
                                    Some(selected.physical_id),
                                )
                            }
                        }
                    } else {
                        None
                    };
                    let visual_settled = settlement.map(|(satisfied, _)| satisfied);
                    let final_satisfied = satisfied;
                    let final_elapsed_ms =
                        elapsed_ms.saturating_add(settlement.map_or(0, |(_, elapsed)| elapsed));
                    let status = if final_satisfied {
                        Status::Succeeded
                    } else {
                        Status::Failed
                    };
                    let outcome = Outcome::Waited {
                        condition: value.condition.clone(),
                        elapsed_ms: final_elapsed_ms,
                        satisfied: final_satisfied,
                        host: observed_host(&selected.url),
                    };
                    let facts = json!({
                        "tab": selected.handle.as_str(),
                        "condition": "selector_present",
                        "satisfied": final_satisfied,
                        "elapsed_ms": final_elapsed_ms,
                        "visual_settle": should_settle,
                        "visual_settled": visual_settled,
                    });
                    return Terminal {
                        result: InvocationResult::new(
                            context.invocation,
                            status,
                            Effect::None,
                            readiness(selected.readiness),
                            true,
                            outcome.summary().as_str(),
                            facts,
                            outcome.next_steps(),
                        ),
                        decision,
                        physical_id: Some(selected.physical_id),
                        observed: outcome.observed(),
                        audit: outcome.audit(),
                    };
                }
                match self.dispatch(
                    context,
                    BrowserCommand::Observe {
                        tab_id: selected.physical_id,
                        condition: value.condition.clone(),
                        value: value.value.clone(),
                        locator: locator.clone(),
                        timeout_ms: adapter_budget_ms(
                            value.timeout_ms,
                            context.deadline.saturating_duration_since(Instant::now()),
                        ),
                    },
                ) {
                    Ok(BrowserOutcome::Observed {
                        tab_id,
                        satisfied,
                        elapsed_ms,
                        readiness: browser_readiness,
                    }) if tab_id == selected.physical_id => {
                        let _ = lease.update_readiness(&selected.handle, browser_readiness);
                        let is_composite_default =
                            value.condition == "load_ready" || value.condition == "target_present";
                        let should_settle = if value.condition == "visual_settle" {
                            false
                        } else if is_composite_default {
                            value.visual_settle != Some(false)
                        } else {
                            value.visual_settle == Some(true)
                        };
                        let settlement = if satisfied && should_settle {
                            match self.settle_wait(
                                context,
                                lease,
                                selected,
                                locator.clone(),
                                value.timeout_ms.saturating_sub(elapsed_ms),
                            ) {
                                Ok(observation) => observation,
                                Err(error) => {
                                    return self.browser_failure(
                                        context,
                                        decision,
                                        error,
                                        Some(selected.physical_id),
                                    )
                                }
                            }
                        } else {
                            None
                        };
                        let visual_settled = settlement.map(|(satisfied, _)| satisfied);
                        let final_satisfied = satisfied;
                        let final_elapsed_ms =
                            elapsed_ms.saturating_add(settlement.map_or(0, |(_, elapsed)| elapsed));
                        let status = if final_satisfied {
                            Status::Succeeded
                        } else {
                            Status::Failed
                        };
                        // The condition is a closed vocabulary and its value is not: only the name of the
                        // condition joins the sentence that reaches audit.
                        let outcome = Outcome::Waited {
                            condition: value.condition.clone(),
                            elapsed_ms: final_elapsed_ms,
                            satisfied: final_satisfied,
                            host: observed_host(&selected.url),
                        };
                        let summary = outcome.summary();
                        let next_steps = outcome.next_steps();
                        let outcome_observed = outcome.observed();
                        let facts = json!({
                            "tab": selected.handle.as_str(),
                            "condition": value.condition,
                            "satisfied": final_satisfied,
                            "elapsed_ms": final_elapsed_ms,
                            "readiness": readiness(browser_readiness),
                            "visual_settle": should_settle,
                        "visual_settled": visual_settled,
                        });
                        Terminal {
                            result: InvocationResult::new(
                                context.invocation,
                                status,
                                Effect::None,
                                readiness(browser_readiness),
                                true,
                                &summary,
                                facts,
                                next_steps,
                            ),
                            decision,
                            physical_id: Some(tab_id),
                            observed: outcome_observed,
                            audit: outcome.audit(),
                        }
                    }
                    Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
                    Err(error) => {
                        self.browser_failure(context, decision, error, Some(selected.physical_id))
                    }
                }
            },
        )
    }

    /// Best-effort visual preparation never changes a satisfied primary wait condition.
    fn settle_wait(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        selected: &SelectedTab,
        locator: Option<String>,
        budget_ms: u64,
    ) -> Result<Option<(bool, u64)>, crate::browser::BrowserError> {
        let budget = budget_ms.min(crate::language::settlement::MAX_WAIT_MS);
        if budget == 0 {
            return Ok(None);
        }
        match self.dispatch(
            context,
            BrowserCommand::Observe {
                tab_id: selected.physical_id,
                condition: "visual_settle".into(),
                value: None,
                locator,
                timeout_ms: adapter_budget_ms(
                    budget,
                    context.deadline.saturating_duration_since(Instant::now()),
                ),
            },
        )? {
            BrowserOutcome::Observed {
                tab_id,
                satisfied,
                elapsed_ms,
                readiness,
            } if tab_id == selected.physical_id => {
                let _ = lease.update_readiness(&selected.handle, readiness);
                Ok(Some((satisfied, elapsed_ms)))
            }
            _ => Err(crate::browser::BrowserError::Protocol(
                "expected visual settlement observation".into(),
            )),
        }
    }

    fn type_focused(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TypeText,
    ) -> Terminal {
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
                            let control = credential_control(context, &targets[0], None);
                            return self.credential_guidance(context, decision, selected, control);
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

/// The control a credential refusal names, from what the page last reported about it.
fn credential_control(
    context: &InvocationContext<'_>,
    observed: &ObservedTarget,
    handle: Option<&str>,
) -> ControlRef {
    ControlRef::new(
        TargetRole::classify(&observed.role),
        &observed.name,
        handle,
        context.snapshot.preserves_target_names(),
    )
}

/// Pair each problem the page reported with the field it names: that field's handle, given
/// value, and last observed role and name. A position the call does not have is a page that
/// disagrees with the contract, and gives none.
fn rejected_fields(
    context: &InvocationContext<'_>,
    problems: Vec<FieldProblem>,
    given: &[(String, PhysicalValue)],
    described: &[ObservedTarget],
) -> Option<Vec<RejectedField>> {
    let preserve = context.snapshot.preserves_target_names();
    let mut rejected = Vec::with_capacity(problems.len());
    for problem in problems {
        let ((target, value), observed) =
            (given.get(problem.field)?, described.get(problem.field)?);
        let follows_choice = described[..problem.field]
            .iter()
            .any(|earlier| is_choice(TargetRole::classify(&earlier.role)));
        let control = RejectedControl {
            target: target.clone(),
            role: TargetRole::classify(&observed.role),
            name: observed.name.clone(),
            given: value.quoted(problem.value_index),
            credential: observed.credential_class,
            follows_choice,
        };
        rejected.push(RejectedField::new(problem, control, preserve));
    }
    Some(rejected)
}

/// Pair each field the page read back with the handle and the last observed role and name of
/// the field it belongs to. A credential-class field is never read back, and a position the call
/// does not have is a page that disagrees with the contract, and gives none.
fn held_fields(
    context: &InvocationContext<'_>,
    held: &[FieldNow],
    given: &[(String, PhysicalValue)],
    described: &[ObservedTarget],
) -> Option<Vec<HeldField>> {
    let preserve = context.snapshot.preserves_target_names();
    let mut fields = Vec::with_capacity(held.len());
    for reading in held {
        let ((target, value), observed) =
            (given.get(reading.field)?, described.get(reading.field)?);
        if observed.credential_class {
            continue;
        }
        let held = HeldField::new(
            target,
            TargetRole::classify(&observed.role),
            &observed.name,
            &reading.now,
            reading.tolerated,
            preserve,
        );
        let held = match reading.matched {
            Some(how) => held.matched(how, &value.quoted(None)),
            None => held,
        };
        fields.push(match &reading.picked {
            Some(label) => held.picked(label, reading.group.as_deref()),
            None => held,
        });
    }
    Some(fields)
}

/// Whether answering this control can change the options another control offers.
const fn is_choice(role: TargetRole) -> bool {
    matches!(
        role,
        TargetRole::Combobox | TargetRole::ListBox | TargetRole::Radio
    )
}

/// What a field's value is on the wire: a boolean stays a boolean, a list stays a list, and a
/// number is written out as text.
fn form_field_wire(value: &crate::language::FormFieldValue) -> PhysicalValue {
    match value {
        crate::language::FormFieldValue::Text(text) => PhysicalValue::Text(text.clone()),
        crate::language::FormFieldValue::Flag(flag) => PhysicalValue::Flag(*flag),
        crate::language::FormFieldValue::Choices(choices) => {
            PhysicalValue::Choices(choices.clone())
        }
        crate::language::FormFieldValue::Number(number) => {
            PhysicalValue::Text(if number.fract() == 0.0 && number.abs() < 1.0e15 {
                format!("{}", *number as i64)
            } else {
                number.to_string()
            })
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
