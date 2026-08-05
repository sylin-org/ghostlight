// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `ghostlight-mcp-connector`: the small protocol-versioned stdio shore for Ghostlight.
//!
//! MCP JSON-RPC ends here. The persistent service receives only typed product operations and
//! returns protocol-neutral projections and outcomes.

mod bridge;
mod jsonrpc;
mod mcp_2025_11_25;
mod mcp_2026_07_28;

use bridge::{
    sequence_of, BridgeClient, BridgeEvent, Correlation, DisconnectedPending, Effects, Observation,
};
use ghostlight_transport::bridge::ServiceMessage;
use ghostlight_transport::instance::{self, Instance};
use ghostlight_transport::{ipc, proc, watchdog};
use jsonrpc::{error_response, parse_line, write_line, ParsedLine, Request, INVALID_REQUEST};
use serde_json::Value;
use std::collections::VecDeque;
use tokio::io::{AsyncBufReadExt, AsyncWrite, BufReader, Lines};

const TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS: &str = "If your MCP client reports `Transport closed`, stop. Reconnect Ghostlight through the current MCP client's normal connection mechanism. Starting ghostlight-mcp-connector as a standalone process does not repair that client's closed stdio connection and may create a different browser workspace. Before retrying effectful work, inspect browser state because the prior outcome may be unknown.";

enum SelectedHandler {
    Unselected,
    Mcp2025(mcp_2025_11_25::Handler),
    Mcp2026(mcp_2026_07_28::Handler),
}

struct Machine {
    handler: SelectedHandler,
    correlation: Correlation,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            handler: SelectedHandler::Unselected,
            correlation: Correlation::default(),
        }
    }
}

impl Machine {
    fn on_request(&mut self, request: Request) -> Effects {
        if request.method == "initialize" && request.id.is_none() {
            return Effects::default();
        }
        if request.method == "server/discover"
            && matches!(&self.handler, SelectedHandler::Unselected)
        {
            return mcp_2026_07_28::discover(&request);
        }

        match &mut self.handler {
            SelectedHandler::Unselected => {
                if request.method == "initialize" {
                    if initialize_names_2026(&request) {
                        return mixed_era_error(
                            &request,
                            "MCP 2026-07-28 has no initialize lifecycle",
                        );
                    }
                    return match mcp_2025_11_25::Handler::select(&request, &mut self.correlation) {
                        Ok((handler, effects)) => {
                            self.handler = SelectedHandler::Mcp2025(handler);
                            effects
                        }
                        Err(response) => Effects::output(response),
                    };
                }
                if mcp_2026_07_28::valid_selector(&request) {
                    let mut handler = mcp_2026_07_28::Handler::new();
                    let effects = handler.handle(&request, &mut self.correlation);
                    self.handler = SelectedHandler::Mcp2026(handler);
                    return effects;
                }
                if has_protocol_metadata(&request) {
                    return mcp_2026_07_28::selector_error(&request);
                }
                if request.id.is_none() {
                    return Effects::default();
                }
                Effects::output(error_response(
                    request.id.as_ref(),
                    INVALID_REQUEST,
                    "select MCP 2025-11-25 with initialize, or send exact 2026-07-28 metadata on every request",
                    None,
                ))
            }
            SelectedHandler::Mcp2025(handler) => {
                if has_protocol_metadata(&request) {
                    mixed_era_error(
                        &request,
                        "per-request protocol metadata cannot be mixed into an MCP 2025-11-25 lifecycle",
                    )
                } else {
                    handler.handle(&request, &mut self.correlation)
                }
            }
            SelectedHandler::Mcp2026(handler) => {
                if request.method == "initialize" {
                    mixed_era_error(&request, "initialize cannot be mixed into MCP 2026-07-28")
                } else {
                    handler.handle(&request, &mut self.correlation)
                }
            }
        }
    }

    fn on_service(&mut self, message: ServiceMessage) -> Effects {
        match self.correlation.observe(message) {
            Observation::None => Effects::default(),
            Observation::Cancel(message) => Effects::service(message),
            Observation::CatalogChanged(generation) => match &mut self.handler {
                SelectedHandler::Unselected => Effects::default(),
                SelectedHandler::Mcp2025(handler) => handler.catalog_changed(generation),
                SelectedHandler::Mcp2026(handler) => handler.catalog_changed(generation),
            },
            Observation::Resolved(resolution) => match &mut self.handler {
                SelectedHandler::Unselected => Effects::default(),
                SelectedHandler::Mcp2025(handler) => {
                    handler.on_resolution(resolution, &mut self.correlation)
                }
                SelectedHandler::Mcp2026(handler) => handler.on_resolution(resolution),
            },
        }
    }

    fn on_connected(&mut self) -> Effects {
        match &mut self.handler {
            SelectedHandler::Mcp2025(handler) => handler.bridge_connected(&mut self.correlation),
            SelectedHandler::Unselected | SelectedHandler::Mcp2026(_) => Effects::default(),
        }
    }

    fn on_disconnect(&mut self, reason: &str) -> Effects {
        if let SelectedHandler::Mcp2025(handler) = &mut self.handler {
            handler.bridge_disconnected();
        }
        let mut effects = Effects::default();
        for disconnected in self.correlation.disconnect() {
            effects.extend(self.render_disconnect(disconnected, reason));
        }
        effects
    }

    fn on_unsent(
        &mut self,
        sequence: ghostlight_transport::bridge::BridgeSequence,
        possibly_written: bool,
        reason: &str,
    ) -> Effects {
        let Some(pending) = self.correlation.take_unsent(sequence) else {
            return Effects::default();
        };
        let may_have_started = possibly_written
            && matches!(
                pending.kind,
                bridge::PendingKind::CallTool2025 | bridge::PendingKind::CallTool2026 { .. }
            );
        self.render_disconnect(
            DisconnectedPending {
                pending,
                may_have_started,
            },
            reason,
        )
    }

    fn shutdown(&mut self) -> Effects {
        match &mut self.handler {
            SelectedHandler::Mcp2025(handler) => handler.shutdown(&mut self.correlation),
            SelectedHandler::Mcp2026(handler) => handler.shutdown(),
            SelectedHandler::Unselected => Effects::default(),
        }
    }

    fn render_disconnect(&mut self, disconnected: DisconnectedPending, reason: &str) -> Effects {
        match &mut self.handler {
            SelectedHandler::Unselected => Effects::default(),
            SelectedHandler::Mcp2025(handler) => handler.bridge_failure(disconnected, reason),
            SelectedHandler::Mcp2026(handler) => handler.bridge_failure(disconnected, reason),
        }
    }

    #[cfg(test)]
    fn selected_date(&self) -> Option<&'static str> {
        match self.handler {
            SelectedHandler::Unselected => None,
            SelectedHandler::Mcp2025(_) => Some(mcp_2025_11_25::PROTOCOL_VERSION),
            SelectedHandler::Mcp2026(_) => Some(mcp_2026_07_28::PROTOCOL_VERSION),
        }
    }
}

fn has_protocol_metadata(request: &Request) -> bool {
    request
        .params
        .as_object()
        .and_then(|params| params.get("_meta"))
        .and_then(Value::as_object)
        .is_some_and(|meta| meta.contains_key(mcp_2026_07_28::PROTOCOL_VERSION_META))
}

fn initialize_names_2026(request: &Request) -> bool {
    request
        .params
        .as_object()
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        == Some(mcp_2026_07_28::PROTOCOL_VERSION)
}

fn mixed_era_error(request: &Request, message: &str) -> Effects {
    request.id.as_ref().map_or_else(Effects::default, |id| {
        Effects::output(error_response(Some(id), INVALID_REQUEST, message, None))
    })
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let instance = match resolve_instance(&args) {
        Ok(instance) => instance,
        Err(error) => {
            eprintln!("ghostlight-mcp-connector: {error}");
            std::process::exit(2);
        }
    };
    pin_instance_environment(&instance);
    let debug = std::env::var_os("GHOSTLIGHT_DEBUG").is_some()
        || args.iter().any(|argument| argument == "--debug");
    ghostlight_transport::init_tracing(debug);
    let parent = proc::parent();
    let runtime = tokio::runtime::Runtime::new().expect("build the MCP edge tokio runtime");
    let code = runtime.block_on(run(parent));
    std::process::exit(code);
}

async fn run(parent: Option<proc::ProcId>) -> i32 {
    let endpoint = ipc::adapter_endpoint_name(&ipc::default_endpoint());
    let mut client = BridgeClient::spawn(endpoint);

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    if let Some(parent) = parent {
        tokio::spawn(async move {
            watchdog::wait_until_orphaned(parent).await;
            let _ = shutdown_tx.send(()).await;
        });
    }

    let stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    match serve(stdin, &mut stdout, &mut client, &mut shutdown_rx).await {
        Ok(()) => 0,
        Err(error) => {
            tracing::error!(%error, "MCP edge ended with an error");
            1
        }
    }
}

async fn serve<W>(
    mut lines: Lines<BufReader<tokio::io::Stdin>>,
    stdout: &mut W,
    client: &mut BridgeClient,
    shutdown: &mut tokio::sync::mpsc::Receiver<()>,
) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    let mut machine = Machine::default();
    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line.map_err(|error| error.to_string())? {
                    Some(line) => {
                        let effects = match parse_line(&line) {
                            ParsedLine::Empty => Effects::default(),
                            ParsedLine::Error(response) => Effects::output(response),
                            ParsedLine::Request(request) => machine.on_request(request),
                        };
                        apply_effects(&mut machine, client, stdout, effects).await?;
                    }
                    None => {
                        let effects = machine.shutdown();
                        apply_effects(&mut machine, client, stdout, effects).await?;
                        return Ok(());
                    }
                }
            }
            event = client.next_event() => {
                let Some(event) = event else {
                    let effects = machine.on_disconnect("Ghostlight bridge task stopped");
                    apply_effects(&mut machine, client, stdout, effects).await?;
                    return Err("Ghostlight bridge task stopped".into());
                };
                let effects = match event {
                    BridgeEvent::Connected => machine.on_connected(),
                    BridgeEvent::Message(message) => machine.on_service(message),
                    BridgeEvent::Disconnected(reason) => machine.on_disconnect(&reason),
                    BridgeEvent::Fatal(reason) => {
                        let effects = machine.on_disconnect(&reason);
                        apply_effects(&mut machine, client, stdout, effects).await?;
                        return Err(reason);
                    }
                };
                apply_effects(&mut machine, client, stdout, effects).await?;
            }
            _ = shutdown.recv() => {
                let effects = machine.shutdown();
                apply_effects(&mut machine, client, stdout, effects).await?;
                return Ok(());
            }
        }
    }
}

async fn apply_effects<W>(
    machine: &mut Machine,
    client: &BridgeClient,
    stdout: &mut W,
    effects: Effects,
) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    let mut queue = VecDeque::from([effects]);
    while let Some(effects) = queue.pop_front() {
        for output in effects.output {
            write_line(stdout, &output)
                .await
                .map_err(|error| error.to_string())?;
        }
        for message in effects.service {
            let sequence = sequence_of(&message);
            match client.send(message).await {
                Ok(()) => {
                    if let Some(sequence) = sequence {
                        machine.correlation.mark_sent(sequence);
                    }
                }
                Err(error) => {
                    if let Some(sequence) = sequence {
                        queue.push_back(machine.on_unsent(
                            sequence,
                            error.possibly_written(),
                            error.reason(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn resolve_instance(args: &[String]) -> Result<Instance, String> {
    let mut selected = None;
    let mut arguments = args.iter().skip(1);
    while let Some(argument) = arguments.next() {
        if let Some(value) = argument.strip_prefix("--instance=") {
            selected = Some(value.to_owned());
            break;
        }
        if argument == "--instance" {
            selected = arguments.next().cloned();
            break;
        }
    }
    instance::resolve_from(selected.as_deref())
}

fn pin_instance_environment(instance: &Instance) {
    if let Some(name) = instance.name() {
        std::env::set_var(Instance::ENV_VAR, name);
    } else {
        std::env::remove_var(Instance::ENV_VAR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpc::RequestId;
    use serde_json::{json, Map};

    #[test]
    fn transport_closed_recovery_instructions_pin_safe_behavior() {
        assert_eq!(
            TRANSPORT_CLOSED_RECOVERY_INSTRUCTIONS,
            concat!(
                "If your MCP client reports `Transport closed`, stop. ",
                "Reconnect Ghostlight through the current MCP client's normal connection mechanism. ",
                "Starting ghostlight-mcp-connector as a standalone process does not repair that client's closed stdio connection and may create a different browser workspace. ",
                "Before retrying effectful work, inspect browser state because the prior outcome may be unknown."
            )
        );
    }

    fn initialize(id: i64) -> Request {
        Request {
            id: Some(RequestId::Number(id.into())),
            method: "initialize".into(),
            params: json!({
                "protocolVersion": mcp_2025_11_25::PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1"}
            }),
        }
    }

    fn request_2026(id: i64, method: &str) -> Request {
        let mut meta = Map::new();
        meta.insert(
            mcp_2026_07_28::PROTOCOL_VERSION_META.into(),
            json!(mcp_2026_07_28::PROTOCOL_VERSION),
        );
        meta.insert(mcp_2026_07_28::CLIENT_CAPABILITIES_META.into(), json!({}));
        Request {
            id: Some(RequestId::Number(id.into())),
            method: method.into(),
            params: json!({"_meta": meta}),
        }
    }

    #[test]
    fn discover_does_not_bind_then_initialize_selects_2025() {
        let mut machine = Machine::default();
        let discovered = machine.on_request(request_2026(1, "server/discover"));
        assert_eq!(
            discovered.output[0]["result"]["supportedVersions"],
            json!([
                mcp_2026_07_28::PROTOCOL_VERSION,
                mcp_2025_11_25::PROTOCOL_VERSION,
            ])
        );
        assert_eq!(machine.selected_date(), None);
        let initialize = machine.on_request(initialize(2));
        assert!(matches!(
            initialize.service.first(),
            Some(ghostlight_transport::bridge::EdgeMessage::OpenWorkspace { .. })
        ));
        assert_eq!(
            machine.selected_date(),
            Some(mcp_2025_11_25::PROTOCOL_VERSION)
        );
    }

    #[test]
    fn exact_per_request_metadata_selects_2026_and_initialize_is_then_rejected() {
        let mut machine = Machine::default();
        let selected = machine.on_request(request_2026(1, "tools/list"));
        assert!(matches!(
            selected.service.first(),
            Some(ghostlight_transport::bridge::EdgeMessage::Catalog { .. })
        ));
        assert_eq!(
            machine.selected_date(),
            Some(mcp_2026_07_28::PROTOCOL_VERSION)
        );
        let mixed = machine.on_request(initialize(2));
        assert_eq!(mixed.output[0]["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn selected_2025_rejects_per_request_protocol_metadata() {
        let mut machine = Machine::default();
        machine.on_request(initialize(1));
        let mixed = machine.on_request(request_2026(2, "ping"));
        assert_eq!(mixed.output[0]["error"]["code"], INVALID_REQUEST);

        let mut malformed = request_2026(3, "ping");
        malformed.params["_meta"][mcp_2026_07_28::PROTOCOL_VERSION_META] = json!(7);
        let malformed = machine.on_request(malformed);
        assert_eq!(malformed.output[0]["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn missing_2026_metadata_does_not_bind_the_process() {
        let mut machine = Machine::default();
        let response = machine.on_request(Request {
            id: Some(RequestId::Number(1.into())),
            method: "tools/list".into(),
            params: json!({}),
        });
        assert_eq!(response.output[0]["error"]["code"], INVALID_REQUEST);
        assert_eq!(machine.selected_date(), None);
    }

    #[test]
    fn idless_initialize_notification_is_ignored_without_binding_or_service_work() {
        let mut machine = Machine::default();
        let mut notification = initialize(1);
        notification.id = None;

        let effects = machine.on_request(notification);

        assert!(effects.output.is_empty());
        assert!(effects.service.is_empty());
        assert_eq!(machine.selected_date(), None);
    }

    #[test]
    fn ambiguous_start_write_is_outcome_unknown_and_never_retry_safe() {
        let mut machine = Machine::default();
        let mut call = request_2026(12, "tools/call");
        call.params["name"] = json!("click");
        call.params["arguments"] = json!({"coordinate": [10, 20]});
        let start = machine.on_request(call);
        let ghostlight_transport::bridge::EdgeMessage::Start { sequence, .. } =
            start.service[0].clone()
        else {
            panic!("start expected");
        };

        let failed = machine.on_unsent(sequence, true, "ambiguous local write");
        assert_eq!(failed.output[0]["error"]["code"], -33003);
        assert_eq!(
            failed.output[0]["error"]["data"]["disposition"],
            "outcome_unknown"
        );
    }
}
