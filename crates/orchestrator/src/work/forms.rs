//! Form, text, file, script, keyboard, and wait execution.

use std::time::Instant;

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome, PhysicalField};
use serde_json::{json, Value};

use crate::events::DomainEvent;
use crate::governance::{Capability, CapabilitySet};
use crate::language::{
    outcome::{Outcome, Refusal},
    FillForm, PressKey, RunScript, TypeText, UploadFiles, Wait,
};
use crate::workspace::{SelectedTab, WorkspaceError, WorkspaceLease};

use super::{
    action_subject, bounded, load_physical_files, named_key, observation_budget_ms, observed_host,
    readiness, ApplicationExecutor, Effect, InvocationContext, InvocationResult, ResolvedLocation,
    Status, Terminal,
};
use ghostlight_bridge::browser::PhysicalFile;

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
                    true,
                ) {
                    Ok(value) => value,
                    Err(terminal) => return terminal,
                }
            } else {
                match self.resolve_target(
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
                match self.resolve_target(lease, Some(selected.handle.as_str()), handle) {
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
                if targets.iter().any(|target| target.credential_class) {
                    return self.credential_handoff(context, decision, &selected);
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
                submit_locator: submit.map(|target| target.locator),
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
    ) -> Terminal {
        if value.focused {
            return self.type_focused(context, lease, value);
        }
        let (selected, target) = if let Some(selector) = &value.selector {
            match self.resolve_semantic(context, lease, value.tab.as_deref(), selector, false) {
                Ok(value) => value,
                Err(terminal) => return terminal,
            }
        } else {
            match self.resolve_target(lease, value.tab.as_deref(), &value.target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            }
        };
        let typed_role = target.role;
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
                if targets[0].credential_class {
                    return self.credential_handoff(context, decision, &selected);
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
                    Capability::Write,
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
            match self.resolve_semantic(context, lease, value.tab.as_deref(), selector, true) {
                Ok((tab, target)) => {
                    let locator = target.locator.clone();
                    let role = target.role;
                    (tab, Some(locator), Some(role), None)
                }
                Err(terminal) => return terminal,
            }
        } else {
            match self.resolve_target(
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
                    if tab_id == selected.physical_id && targets.len() == 1 =>
                {
                    if targets[0].credential_class {
                        return self.credential_handoff(context, decision, &selected);
                    }
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
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Execute, Some(selected.url.as_str()));
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
                    &selected,
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
    }

    pub(super) fn perform_key(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &PressKey,
    ) -> Terminal {
        let (selected, locator, focused_role) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
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
            &selected,
            &tab,
            &committed_urls,
            outcome,
            facts,
            value.expect.as_ref(),
        )
    }

    pub(super) fn perform_wait(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Wait,
    ) -> Terminal {
        let (selected, locator, _target_role) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
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
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let outcome = Outcome::Waited {
                condition: value.condition.clone(),
                elapsed_ms,
                satisfied: true,
                host: observed_host(&selected.url),
            };
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    Status::Succeeded,
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    outcome.summary().as_str(),
                    json!({"tab":selected.handle.as_str(),"condition":"duration","satisfied":true,"elapsed_ms":elapsed_ms,"readiness":readiness(selected.readiness)}),
                    outcome.next_steps(),
                ),
                decision,
                physical_id: Some(selected.physical_id),
                observed: outcome.observed(),
            };
        }
        match self.dispatch(
            context,
            BrowserCommand::Observe {
                tab_id: selected.physical_id,
                condition: value.condition.clone(),
                value: value.value.clone(),
                locator,
                timeout_ms: observation_budget_ms(
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
                let status = if satisfied {
                    Status::Succeeded
                } else {
                    Status::Failed
                };
                // The condition is a closed vocabulary and its value is not: only the name of the
                // condition joins the sentence that reaches audit.
                let outcome = Outcome::Waited {
                    condition: value.condition.clone(),
                    elapsed_ms,
                    satisfied,
                    host: observed_host(&selected.url),
                };
                let summary = outcome.summary();
                let next_steps = outcome.next_steps();
                let outcome_observed = outcome.observed();
                Terminal {
                    result: InvocationResult::new(
                        context.invocation,
                        status,
                        Effect::None,
                        readiness(browser_readiness),
                        true,
                        &summary,
                        json!({"tab":selected.handle.as_str(),"condition":value.condition,"satisfied":satisfied,"elapsed_ms":elapsed_ms,"readiness":readiness(browser_readiness)}),
                        next_steps,
                    ),
                    decision,
                    physical_id: Some(tab_id),
                    observed: outcome_observed,
                }
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn type_focused(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TypeText,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
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
        match self.dispatch(
            context,
            BrowserCommand::DescribeFocused {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                if tab_id == selected.physical_id && targets.len() == 1 =>
            {
                if targets[0].credential_class {
                    return self.credential_handoff(context, decision, &selected);
                }
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
        match self.dispatch(
            context,
            BrowserCommand::TypeFocused {
                tab_id: selected.physical_id,
                text: value.text.clone(),
                clear_first: value.clear_first,
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
                    subject: action_subject(context, subject, None)
                        .expect("typing has a fallback subject"),
                    characters: character_count,
                };
                self.finish_with_expectation(
                    context,
                    lease,
                    decision,
                    Capability::Write,
                    &selected,
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
