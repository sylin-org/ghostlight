//! Pointer, viewport, zoom, and window-geometry execution.

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome, ClickShape};
use serde_json::json;

use crate::events::DomainEvent;
use crate::governance::{Capability, CapabilitySet};
use crate::language::{
    outcome::{Outcome, TargetRole},
    Click, Drag, Hover, ScrollPage,
};
use crate::workspace::WorkspaceLease;

use super::{
    action_subject, observed_host, readiness, ApplicationExecutor, Clicked, Effect,
    InvocationContext, ResolvedLocation, Terminal,
};

impl ApplicationExecutor {
    pub(super) fn perform_click(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Click,
    ) -> Terminal {
        let location = match self.resolve_location(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            value.view.as_deref(),
            value.x,
            value.y,
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let selected = location.tab();
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
        let (command, facts, clicked) = match location {
            ResolvedLocation::Target { tab, target } => {
                let clicked = Clicked::Target(target.role);
                self.emit(DomainEvent::TargetIndicated {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    physical_id: tab.physical_id,
                    locator: target.locator.clone(),
                    click: Some(ClickShape {
                        clicks: value.click_count,
                        button: value.button.clone(),
                    }),
                });
                (
                    BrowserCommand::Activate {
                        tab_id: tab.physical_id,
                        locator: target.locator,
                        button: value.button.clone(),
                        click_count: value.click_count,
                    },
                    json!({"tab":tab.handle.as_str(),"target":target.handle.as_str(),"activated":true}),
                    clicked,
                )
            }
            ResolvedLocation::Point { tab, view, point } => (
                BrowserCommand::ActivatePoint {
                    tab_id: tab.physical_id,
                    point,
                    expected_viewport: view.viewport,
                    button: value.button.clone(),
                    click_count: value.click_count,
                },
                json!({"tab":tab.handle.as_str(),"view":view.handle.as_str(),"activated":true}),
                Clicked::Point(point),
            ),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Activated {
                tab,
                subject,
                committed_urls,
            }) => {
                let host = observed_host(&tab.url);
                let outcome = match clicked {
                    Clicked::Target(role) => Outcome::TargetClicked {
                        host,
                        subject: action_subject(context, subject, Some(role))
                            .expect("a semantic click has a fallback subject"),
                    },
                    Clicked::Point(point) => Outcome::PointClicked {
                        host,
                        x: point.x.round().max(0.0) as u32,
                        y: point.y.round().max(0.0) as u32,
                        subject: action_subject(context, subject, None),
                    },
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    &selected,
                    &tab,
                    &committed_urls,
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

    pub(super) fn perform_scroll(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &ScrollPage,
    ) -> Terminal {
        let (selected, locator, revealed_role) = match self.resolve_optional_target(
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
        match self.dispatch(
            context,
            BrowserCommand::Scroll {
                tab_id: selected.physical_id,
                locator,
                direction: value
                    .target
                    .is_none()
                    .then(|| value.direction.clone().unwrap_or_else(|| "down".into())),
                amount: value
                    .target
                    .is_none()
                    .then(|| value.amount.clone().unwrap_or_else(|| "medium".into())),
            },
        ) {
            Ok(BrowserOutcome::Scrolled {
                tab_id,
                x,
                y,
                subject,
            }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.invalidate_views(&selected.handle) {
                    return self.workspace_failure(context, error);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    value.target.is_some(),
                    if value.target.is_some() {
                        Outcome::TargetRevealed {
                            host: observed_host(&selected.url),
                            subject: action_subject(
                                context,
                                subject,
                                Some(revealed_role.unwrap_or(TargetRole::Control)),
                            )
                            .expect("a semantic reveal has a fallback subject"),
                        }
                    } else {
                        Outcome::PageScrolled {
                            host: observed_host(&selected.url),
                            direction: value
                                .direction
                                .clone()
                                .unwrap_or_else(|| "down".into()),
                        }
                    },
                    json!({"tab":selected.handle.as_str(),"target":value.target,"scrolled":true,"x":x,"y":y}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn set_zoom(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        percent: u16,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
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
            BrowserCommand::SetZoom {
                tab_id: selected.physical_id,
                zoom: f64::from(percent) / 100.0,
            },
        ) {
            Ok(BrowserOutcome::Zoomed { tab_id, zoom }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.invalidate_views(&selected.handle) {
                    return self.workspace_failure(context, error);
                }
                let actual_percent = (zoom * 100.0).round() as u16;
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::ZoomSet {
                        percent: actual_percent,
                        host: observed_host(&selected.url),
                    },
                    json!({"tab":selected.handle.as_str(),"action":"zoom","requested_percent":percent,"percent":actual_percent,"zoomed":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn resize_window(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        width: u32,
        height: u32,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
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
            BrowserCommand::ResizeWindow {
                tab_id: selected.physical_id,
                width,
                height,
            },
        ) {
            Ok(BrowserOutcome::WindowResized {
                tab_id,
                width: observed_width,
                height: observed_height,
                affected_tab_ids,
            }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.invalidate_views_for_physical(&affected_tab_ids) {
                    return self.workspace_failure(context, error);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::WindowResized {
                        width: observed_width,
                        height: observed_height,
                    },
                    json!({"tab":selected.handle.as_str(),"action":"resize","requested_width":width,"requested_height":height,"width":observed_width,"height":observed_height,"resized":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn perform_hover(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Hover,
    ) -> Terminal {
        let location = match self.resolve_location(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            value.view.as_deref(),
            value.x,
            value.y,
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let selected = location.tab();
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
        let hovered_role = match &location {
            ResolvedLocation::Target { target, .. } => Some(target.role),
            ResolvedLocation::Point { .. } => None,
        };
        let (command, facts) = match location {
            ResolvedLocation::Target { tab, target } => {
                self.emit(DomainEvent::TargetIndicated {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    physical_id: tab.physical_id,
                    locator: target.locator.clone(),
                    click: None,
                });
                (
                    BrowserCommand::Hover {
                        tab_id: tab.physical_id,
                        locator: target.locator,
                    },
                    json!({"tab":tab.handle.as_str(),"target":target.handle.as_str(),"hovered":true}),
                )
            }
            ResolvedLocation::Point { tab, view, point } => (
                BrowserCommand::HoverPoint {
                    tab_id: tab.physical_id,
                    point,
                    expected_viewport: view.viewport,
                },
                json!({"tab":tab.handle.as_str(),"view":view.handle.as_str(),"hovered":true}),
            ),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Hovered { tab_id, subject }) if tab_id == selected.physical_id => {
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::Hovered {
                        host: observed_host(&selected.url),
                        subject: action_subject(context, subject, hovered_role),
                    },
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn perform_drag(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Drag,
    ) -> Terminal {
        let mut dragged_from = None;
        let mut dragged_onto = None;
        let (selected, command, facts) = if let (Some(source), Some(destination)) = (
            value.source_target.as_deref(),
            value.destination_target.as_deref(),
        ) {
            let (selected, source) = match self.resolve_target(lease, value.tab.as_deref(), source)
            {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let (_, destination) =
                match self.resolve_target(lease, Some(selected.handle.as_str()), destination) {
                    Ok(value) => value,
                    Err(error) => return self.workspace_failure(context, error),
                };
            self.emit(DomainEvent::TargetIndicated {
                invocation: context.invocation.into(),
                workspace: context.workspace.as_str().into(),
                physical_id: selected.physical_id,
                locator: source.locator.clone(),
                click: None,
            });
            dragged_from = Some(source.role);
            dragged_onto = Some(destination.role);
            let facts = json!({"tab":selected.handle.as_str(),"source_target":source.handle.as_str(),"destination_target":destination.handle.as_str(),"dragged":true});
            let command = BrowserCommand::Drag {
                tab_id: selected.physical_id,
                source_locator: source.locator,
                destination_locator: destination.locator,
            };
            (selected, command, facts)
        } else {
            let view_handle = value.view.as_deref().expect("language validated view");
            let start_location = match self.resolve_location(
                lease,
                value.tab.as_deref(),
                None,
                Some(view_handle),
                value.start_x,
                value.start_y,
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let ResolvedLocation::Point {
                tab: selected,
                view,
                point: start,
            } = start_location
            else {
                unreachable!("view input resolves to a point")
            };
            let (_, end) = match lease.resolve_view_point(
                view_handle,
                Some(&selected),
                value.end_x.expect("language validated end_x"),
                value.end_y.expect("language validated end_y"),
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let facts =
                json!({"tab":selected.handle.as_str(),"view":view.handle.as_str(),"dragged":true});
            let command = BrowserCommand::DragPoints {
                tab_id: selected.physical_id,
                start,
                end,
                expected_viewport: view.viewport,
            };
            (selected, command, facts)
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
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Dragged {
                tab,
                source_subject,
                destination_subject,
                committed_urls,
            }) => {
                let outcome = Outcome::Dragged {
                    host: observed_host(&tab.url),
                    source: action_subject(context, source_subject, dragged_from),
                    destination: action_subject(context, destination_subject, dragged_onto),
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    &selected,
                    &tab,
                    &committed_urls,
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
}
