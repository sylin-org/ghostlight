//! The orchestrator's process-diagnostics hub: it owns this process's sink, performs the
//! person-facing toggle act for every surface, and maps sink state onto the wire contract.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ghostlight_bridge::browser::{DiagnosticsLayer, DiagnosticsState};
use ghostlight_bridge::diagnostics::{self, event, set_marker, Activation, Component, Level, Sink};

use crate::browser::AdapterLifecycleObserver;

/// The orchestrator-owned hub around the shared process sink (ADR-0145).
pub struct DiagnosticsHub {
    sink: Arc<Sink>,
    runtime_path: PathBuf,
}

impl DiagnosticsHub {
    /// Birth the hub's sink from the process environment and log the process start.
    pub fn birth(runtime_path: &Path) -> Arc<Self> {
        let sink = Sink::birth(
            Component::Orchestrator,
            env!("CARGO_PKG_VERSION"),
            runtime_path,
        );
        sink.emit(
            event::PROCESS_STARTED,
            Level::Info,
            None,
            "desktop authority starting",
        );
        Arc::new(Self {
            sink,
            runtime_path: runtime_path.to_path_buf(),
        })
    }

    /// A hub whose sink is off and silent, for test fixtures.
    pub fn for_tests() -> Arc<Self> {
        let sink = Sink::birth_with(
            Component::Orchestrator,
            env!("CARGO_PKG_VERSION"),
            Path::new("/ghostlight-test/ghostlight-runtime.json"),
            None,
            &diagnostics::OsMarkerWatcher,
            None,
        );
        Arc::new(Self {
            sink,
            runtime_path: PathBuf::from("/ghostlight-test/ghostlight-runtime.json"),
        })
    }

    /// The shared sink, for direct operational event emission at owned seams.
    pub fn sink(&self) -> Arc<Sink> {
        Arc::clone(&self.sink)
    }

    /// The wire-visible state.
    pub fn state(&self) -> DiagnosticsState {
        wire_state(&self.sink)
    }

    /// The directory diagnostics write into, when active.
    pub fn directory(&self) -> Option<PathBuf> {
        self.sink.activation().directory().map(Path::to_path_buf)
    }

    /// The person-facing toggle act: flip the marker, then let the sink re-evaluate. The
    /// sink's change callback publishes the new state to connected adapters.
    pub fn toggle(&self) -> DiagnosticsState {
        let on = matches!(self.sink.activation(), Activation::Off);
        self.sink.emit(
            event::DIAGNOSTICS_TOGGLE_REQUESTED,
            Level::Info,
            None,
            if on { "on" } else { "off" },
        );
        if let Err(error) = set_marker(&self.runtime_path, on) {
            self.sink.emit(
                event::DIAGNOSTICS_TOGGLED,
                Level::Warn,
                None,
                &format!("marker write failed: {error}"),
            );
        }
        self.sink.evaluate();
        self.sink.emit(
            event::DIAGNOSTICS_TOGGLED,
            Level::Info,
            None,
            &format!("layer now {}", self.sink.activation().layer()),
        );
        self.state()
    }
}

/// Map a sink's activation to the wire state.
pub fn wire_state(sink: &Sink) -> DiagnosticsState {
    let layer = match sink.activation() {
        Activation::Explicit { .. } => DiagnosticsLayer::Explicit,
        Activation::Marker { .. } => DiagnosticsLayer::Marker,
        Activation::Off => DiagnosticsLayer::Off,
    };
    DiagnosticsState { layer }
}

impl AdapterLifecycleObserver for DiagnosticsHub {
    fn adapter_attached(&self, browser_id: &str, replaced: bool) {
        let detail = if replaced {
            format!("{browser_id} replaced a prior connection")
        } else {
            format!("{browser_id} attached")
        };
        self.sink
            .emit(event::ADAPTER_ATTACHED, Level::Info, None, &detail);
    }

    fn adapter_detached(&self, browser_id: &str) {
        self.sink.emit(
            event::ADAPTER_DISCONNECTED,
            Level::Info,
            None,
            &format!("{browser_id} disconnected"),
        );
    }
}
