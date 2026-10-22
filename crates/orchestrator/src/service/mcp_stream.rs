//! One MCP client session on an accepted `mcp` stream: its engine and the stream its lines go out
//! on, the resume tokens it hands the edge, and once it is initialized its workspace, queue,
//! workers and the catalog its client was given.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use ghostlight_relay::endpoint::Connection;
use ghostlight_relay::protocol::{self, GoodbyeReason, McpFrame, Role};
use serde_json::Value;

use crate::browser::transport::{SocketReader, SocketWriter};
use crate::browser::BrowserPort;
use crate::governance::AuthoritySnapshot;
use crate::intake::listener;
use crate::intake::mcp::{McpSession, OwnedTab, ResumeState, Seating, Step, OLDEST_REVISION};
use crate::intake::session::{
    IntakeChannel, ServiceRequest, ServiceResponse, SessionMarker, ToolDefinition,
};
use crate::language::catalog_for;
use crate::language::intake::OpeningRefusal;
use crate::language::outcome::CancelCause;
use crate::provenance::{Attribution, ConnectionEvidence};
use crate::sync::lock;
use crate::work::{Admission, PreparedInvocation};
use crate::workspace::{UrlDigest, WorkspaceId, WorkspaceStore};

use super::admission::{Refused, SessionQueue};
use super::connections::ServiceContext;
use super::release::reap_finished_sessions;

/// What a queued call is counted to hold beyond its input, its id and its tool's name.
const QUEUED_METADATA_BYTES: usize = 512;

/// Serve one MCP client session on an accepted `mcp` stream.
///
/// The opening names the channel and, for the command line, the process that owns the session.
/// Once the opening is answered, the connection's attribution is minted with the accept time, and
/// the app that started the session is read once from the stream's peer process. The opening's
/// answer never waits on that read, and nothing the client sends takes part in it. The client's
/// `initialize` does wait on it: whether the session stands by, because another connection of
/// the same agent already offers the tools, decides its instructions ([`toolset`](super::toolset)).
/// A resume token continues an earlier session: its revision, client and owned tabs come back at
/// once, without waiting for any browser. Each tab is pending until its own browser lists a tab
/// with its id at the URL the session last knew, which the session's first call asks for
/// (ADR-0203 Decision 1). Until then the tab stays in every token the session hands out.
pub(super) fn serve_mcp(
    connection: Connection,
    context: &Arc<ServiceContext>,
    requested: &protocol::Opening,
) -> Result<()> {
    let Some(_session_permit) = context.sessions.acquire(1) else {
        listener::refuse(&connection, OpeningRefusal::Capacity);
        return Ok(());
    };
    let marker = match requested
        .session
        .clone()
        .map(serde_json::from_value::<SessionMarker>)
    {
        None => None,
        Some(Ok(marker)) => Some(marker),
        Some(Err(_)) => {
            listener::refuse(&connection, OpeningRefusal::Malformed);
            return Ok(());
        }
    };
    // A client that sent a token initialized with an earlier service and never initializes
    // again, so a token that cannot be read still resumes an initialized session.
    let resumed = requested.resume.as_deref().map(|token| {
        ResumeState::decode(token).unwrap_or_else(|error| {
            context.diagnostics.sink().emit(
                ghostlight_bridge::diagnostics::event::HARNESS_ATTACHED,
                ghostlight_bridge::diagnostics::Level::Warn,
                None,
                &format!(
                    "{error}; the session resumes at MCP revision {OLDEST_REVISION} with no tab"
                ),
            );
            ResumeState::unreadable()
        })
    });
    listener::accept(&connection, &context.accepted(Role::Mcp))?;
    let attribution =
        Attribution::accepted(IntakeChannel::of_opening(requested.channel.as_deref()));
    let observation = context.sources.read(connection.peer_process_id());
    let accepted = Accepted {
        attribution: attribution.observed(observation.as_ref()),
        marker,
    };
    let mut reader =
        SocketReader::new(connection.try_clone()?).interrupted_by(Arc::clone(&context.stop));
    let writer = Arc::new(SocketWriter::new(connection)?);
    let (session, resumed_tabs) = match resumed {
        Some(state) => {
            let kept = KeptTabs {
                owned: state.owned_tabs.clone(),
                lives: state.lives.clone(),
            };
            (McpSession::resumed(state), Some(kept))
        }
        None => (McpSession::new(), None),
    };
    let wire = Arc::new(McpWire::new(Arc::clone(&writer), session));
    let _enrolled = context.quit.enroll(Arc::clone(&wire));
    let seat = context.toolsets.enter(Arc::clone(&wire));
    wire.seat(seat.seating(accepted.attribution.channel, observation.as_ref()));

    let mut live = match resumed_tabs {
        Some(kept) => {
            let live = LiveSession::open(context, &wire, &accepted)?;
            // Its client listed tools from an earlier service and will not list them again, so
            // the session watch tells it to, unless the session stands by with no tools.
            if !wire.stands_by() {
                live.advertised.given_earlier();
            }
            remember_resumed_tabs(&context.workspaces, &live.workspace, &kept);
            wire.publish_resume(
                &context.workspaces,
                context.browser_port.as_ref(),
                &live.workspace,
            );
            Some(live)
        }
        None => None,
    };

    // Every way this connection can end has to reach the teardown below. A reset socket, an
    // oversized frame and one malformed frame are all ordinary ways for a client to go away.
    let served = (|| -> Result<()> {
        while let Some(frame) = reader.native::<McpFrame>(None)? {
            let McpFrame::Message { line } = frame else {
                continue;
            };
            let mut opened = false;
            // A fresh session opens once it is initialized and only after the lines written so
            // far, so its `initialize` answer never waits on opening its workspace.
            let mut open_when_initialized = |live: &mut Option<LiveSession>| -> Result<()> {
                if live.is_none() && wire.is_initialized() {
                    *live = Some(LiveSession::open(context, &wire, &accepted)?);
                    opened = true;
                }
                Ok(())
            };
            let initialized_before = wire.is_initialized();
            let steps = wire.accept(&line);
            if !initialized_before && wire.is_initialized() {
                wire.announce_initialized();
            }
            for step in steps {
                match step {
                    Step::Reply(reply) => wire.send(&reply),
                    Step::Forward(request) => {
                        open_when_initialized(&mut live)?;
                        if let Some(live) = &live {
                            live.handle(context, &wire, request);
                        }
                    }
                }
            }
            open_when_initialized(&mut live)?;
            if let (true, Some(live)) = (opened, &live) {
                wire.publish_resume(
                    &context.workspaces,
                    context.browser_port.as_ref(),
                    &live.workspace,
                );
            }
        }
        Ok(())
    })();

    writer.close();
    if let Some(live) = live {
        live.close(context);
    }
    drop(seat);
    served
}

/// An accepted `mcp` stream: what the service knew of it once it answered the opening and read
/// its source.
struct Accepted {
    /// The connection's identity, accept time, declared channel and source.
    attribution: Attribution,
    /// The session owner the opening declared, for the command line.
    marker: Option<SessionMarker>,
}

/// One session's engine and the stream its lines go out on.
pub(super) struct McpWire {
    writer: Arc<SocketWriter>,
    session: Mutex<McpSession>,
    resume: Mutex<Resume>,
    /// The session's workspace and queue once it opened, which a service end decides.
    pub(super) live: OnceLock<LiveEnd>,
}

/// The resume token a stream was last handed, and whether a service end handed it its last.
#[derive(Default)]
struct Resume {
    last: Option<String>,
    /// The tabs the last token restores, so a promotion writes the same tabs again.
    kept: KeptTabs,
    /// Nothing is published once a service end has handed the stream its last token.
    sealed: bool,
}

/// What a service end needs of an opened session: its workspace, and its queue to cancel.
pub(super) struct LiveEnd {
    pub(super) workspace: WorkspaceId,
    pub(super) queue: Arc<SessionQueue>,
}

impl McpWire {
    fn new(writer: Arc<SocketWriter>, session: McpSession) -> Self {
        Self {
            writer,
            session: Mutex::new(session),
            resume: Mutex::default(),
            live: OnceLock::new(),
        }
    }

    fn accept(&self, line: &str) -> Vec<Step> {
        lock(&self.session).accept(line)
    }

    fn is_initialized(&self) -> bool {
        lock(&self.session).is_initialized()
    }

    /// The name the session's client claimed, if any.
    fn claim(&self) -> Option<String> {
        lock(&self.session).claim().map(str::to_owned)
    }

    /// Let `seating` decide whether the session stands by ([`McpSession::seat`]).
    fn seat(&self, seating: Seating) {
        lock(&self.session).seat(seating);
    }

    /// Whether the session stands by, so its catalog is empty.
    fn stands_by(&self) -> bool {
        lock(&self.session).stands_by()
    }

    /// Stop standing by, hand the edge a token that says so, and then tell the client its tool
    /// list changed. A restart after the promotion resumes the session offering the tools its
    /// client was told about.
    pub(super) fn promote(&self) {
        let lines = lock(&self.session).promote();
        let mut resume = lock(&self.resume);
        if !resume.sealed {
            let kept = resume.kept.clone();
            self.write_resume(&mut resume, kept);
        }
        drop(resume);
        for line in &lines {
            self.send(line);
        }
    }

    /// The workspace of the session, once it opened.
    pub(super) fn workspace(&self) -> Option<WorkspaceId> {
        self.live.get().map(|live| live.workspace.clone())
    }

    /// Write one MCP line to the client.
    fn send(&self, line: &Value) {
        let _ = self.writer.native(&McpFrame::Message {
            line: line.to_string(),
        });
    }

    /// Say goodbye with `reason` on this stream.
    pub(super) fn goodbye(&self, reason: GoodbyeReason) {
        let _ = self.writer.native(&McpFrame::Goodbye { reason });
    }

    /// Render one answer from the session queue and write its lines.
    fn render(&self, response: ServiceResponse) {
        let lines = lock(&self.session).render(response);
        for line in &lines {
            self.send(line);
        }
    }

    /// Hand the edge a fresh resume token when what it would restore has changed: a binding was
    /// confirmed or forgotten, or its URL moved. Once a service end handed the stream its last
    /// token, nothing more is published.
    ///
    /// Each tab is written with its own binding's browser, a pending one included, so a call that
    /// completes before that browser returns keeps the tab in the token. A tab carries the digest
    /// of its URL, never the URL, so the edge holds no page address, and each browser carries the
    /// extension life its tabs were confirmed in, read from `port`.
    fn publish_resume(
        &self,
        workspaces: &WorkspaceStore,
        port: &dyn BrowserPort,
        workspace: &WorkspaceId,
    ) {
        let mut resume = lock(&self.resume);
        if !resume.sealed {
            self.write_resume(&mut resume, kept_tabs(workspaces, port, workspace));
        }
    }

    /// Hand the edge the first token of a session that has just been initialized, ahead of the
    /// `initialize` answer, so a client that holds its answer always has a token to resume with
    /// (ADR-0203 Decision 1). A fresh session holds no tab, so this token is the one a service end
    /// would hand it, and it is written even after an end handed the stream its last.
    fn announce_initialized(&self) {
        let mut resume = lock(&self.resume);
        self.write_resume(&mut resume, KeptTabs::default());
    }

    /// Hand the edge the last resume token of this service, with the session's tabs only when
    /// `keeps_tabs` says its workspace stays bound, and publish nothing after it.
    ///
    /// A session that ends at a quit resumes later with no tab, so its handles read as no longer
    /// open instead of rebinding tabs the release left behind.
    pub(super) fn hand_last_resume(
        &self,
        workspaces: &WorkspaceStore,
        port: &dyn BrowserPort,
        keeps_tabs: bool,
    ) {
        let mut resume = lock(&self.resume);
        if resume.sealed {
            return;
        }
        let kept = match self.workspace() {
            Some(workspace) if keeps_tabs => kept_tabs(workspaces, port, &workspace),
            _ => KeptTabs::default(),
        };
        self.write_resume(&mut resume, kept);
        resume.sealed = true;
    }

    /// Write the token that restores `kept`, unless it is the one last written or the session is
    /// not initialized yet.
    fn write_resume(&self, resume: &mut Resume, kept: KeptTabs) {
        let Some(state) = lock(&self.session).resume_state(kept.owned.clone(), kept.lives.clone())
        else {
            return;
        };
        let token = state.encode();
        if resume.last.as_deref() != Some(token.as_str()) {
            if self
                .writer
                .native(&McpFrame::Resume {
                    token: token.clone(),
                })
                .is_err()
            {
                return;
            }
            resume.last = Some(token);
        }
        resume.kept = kept;
    }
}

/// What a resume token restores: the tabs, and the extension life of each of their browsers.
#[derive(Clone, Default)]
struct KeptTabs {
    owned: Vec<OwnedTab>,
    lives: BTreeMap<String, String>,
}

/// The tabs a resume token restores for `workspace`: every binding, a pending one included, by
/// its own browser and the digest of its URL, and the extension life of each of their browsers.
///
/// A pending binding remembers the life it was confirmed in. A confirmed one is in the life its
/// browser's connection reports, so those lives are read from `port` before the bindings are:
/// a gap marks a browser's bindings pending before a new connection takes its place, so a binding
/// still confirmed after that read was confirmed in the life it returned. A browser whose tabs
/// name two lives, or none, carries no life, and its tabs are confirmed by their URL digests.
fn kept_tabs(
    workspaces: &WorkspaceStore,
    port: &dyn BrowserPort,
    workspace: &WorkspaceId,
) -> KeptTabs {
    let browsers: BTreeSet<String> = workspaces
        .tab_bindings(workspace.as_str())
        .into_iter()
        .map(|binding| binding.browser)
        .collect();
    let current: BTreeMap<String, Option<String>> = browsers
        .into_iter()
        .map(|browser| {
            let life = port.life_of(&browser);
            (browser, life)
        })
        .collect();
    let bindings = workspaces.tab_bindings(workspace.as_str());
    let mut lives: BTreeMap<String, Option<String>> = BTreeMap::new();
    for binding in &bindings {
        let life = if binding.pending {
            binding.life.clone()
        } else {
            current.get(&binding.browser).cloned().flatten()
        };
        lives
            .entry(binding.browser.clone())
            .and_modify(|known| {
                if *known != life {
                    *known = None;
                }
            })
            .or_insert(life);
    }
    KeptTabs {
        owned: bindings
            .into_iter()
            .map(|binding| OwnedTab {
                browser_id: binding.browser,
                tab_id: binding.physical_id,
                handle: binding.handle,
                url_digest: Some(binding.url.as_str().to_owned()),
            })
            .collect(),
        lives: lives
            .into_iter()
            .filter_map(|(browser, life)| Some((browser, life?)))
            .collect(),
    }
}

/// The tool catalog one session's client was last given, with the authority it was built from.
#[derive(Default)]
pub(super) struct AdvertisedCatalog(Mutex<Option<Advertised>>);

struct Advertised {
    authority: String,
    /// None when an earlier service gave the client its catalog, which this one never saw.
    tools: Option<Vec<ToolDefinition>>,
}

impl AdvertisedCatalog {
    /// Build the catalog `authority` admits and remember it as what the client now has.
    pub(super) fn advertise(&self, authority: &AuthoritySnapshot) -> Vec<ToolDefinition> {
        let tools = catalog_for(authority);
        *lock(&self.0) = Some(Advertised {
            authority: authority.id().to_owned(),
            tools: Some(tools.clone()),
        });
        tools
    }

    /// Remember that an earlier service gave the client a catalog this one never saw: the client
    /// of a resumed session listed its tools before the restart and does not list them again on
    /// its own. The next comparison therefore tells it to, whatever the authority is now.
    pub(super) fn given_earlier(&self) {
        *lock(&self.0) = Some(Advertised {
            authority: String::new(),
            tools: None,
        });
    }

    /// The catalog `authority` admits when it differs from the one given, and `None` otherwise.
    ///
    /// Nothing is compared until the client has a catalog, from this service or an earlier one, and
    /// the catalog is rebuilt only when the authority itself changed.
    pub(super) fn changed_by(&self, authority: &AuthoritySnapshot) -> Option<Vec<ToolDefinition>> {
        let mut guard = lock(&self.0);
        let advertised = guard.as_mut()?;
        if advertised.authority == authority.id() {
            return None;
        }
        advertised.authority = authority.id().to_owned();
        let tools = catalog_for(authority);
        if advertised.tools.as_ref() == Some(&tools) {
            return None;
        }
        advertised.tools = Some(tools.clone());
        Some(tools)
    }
}

/// The diagnostics detail of a session that attached: its connection id, its channel and the file
/// name of the app that started it, when one was seen. Never the client's claim (ADR-0161).
fn attached(attribution: &Attribution) -> String {
    let connection = attribution.connection_id.as_deref().unwrap_or_default();
    let channel = attribution.channel.as_str();
    match &attribution.source {
        Some(source) => format!("{connection} via {channel} from {}", source.image),
        None => format!("{connection} via {channel}"),
    }
}

/// The workspace, queue and workers of a session that has been initialized.
struct LiveSession {
    workspace: WorkspaceId,
    connection: Arc<ConnectionEvidence>,
    queue: Arc<SessionQueue>,
    workers: Vec<JoinHandle<()>>,
    watch: Option<JoinHandle<()>>,
    watch_stop: Arc<AtomicBool>,
    advertised: Arc<AdvertisedCatalog>,
}

impl LiveSession {
    /// Open the session's workspace and start its workers and its catalog watch.
    ///
    /// This is where the connection's one evidence is built, from its observed attribution and
    /// its client's claim.
    fn open(
        context: &Arc<ServiceContext>,
        wire: &Arc<McpWire>,
        accepted: &Accepted,
    ) -> Result<Self> {
        // Before opening anything, release workspaces whose owner is gone and close the tabs they
        // still hold. Sweeping here rather than on a timer keeps the cost proportional to use.
        reap_finished_sessions(&context.workspaces, context.browser_port.as_ref());
        // Evidence belongs to this connection. Workspace continuity never replaces it.
        let connection = Arc::new(ConnectionEvidence::new(
            accepted.attribution.clone(),
            wire.claim().as_deref(),
        ));
        let workspace = context
            .workspaces
            .connect(connection.clone(), accepted.marker.clone());
        context.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::HARNESS_ATTACHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            &attached(connection.attribution()),
        );
        let queue = Arc::new(SessionQueue::new(
            context.budget.clone(),
            Arc::clone(&context.quit.drain),
        ));
        let advertised = Arc::new(AdvertisedCatalog::default());
        let watch_stop = Arc::new(AtomicBool::new(false));
        let _ = wire.live.set(LiveEnd {
            workspace: workspace.clone(),
            queue: Arc::clone(&queue),
        });
        let mut live = Self {
            workspace,
            connection,
            queue,
            workers: Vec::new(),
            watch: None,
            watch_stop,
            advertised,
        };
        live.watch = Some(live.spawn_watch(context, wire)?);
        for independent in [false, true] {
            let worker = live.spawn_worker(context, wire, independent);
            match worker {
                Ok(worker) => live.workers.push(worker),
                Err(error) => {
                    live.close(context);
                    return Err(error);
                }
            }
        }
        context.workbench.refresh_live_parts();
        Ok(live)
    }

    /// The once-a-second thread that advances cancelled work, keeps waiting work visible, hands
    /// the client a fresh resume token when a tab changed outside its calls, and tells the client
    /// when policy changes the catalog it was given.
    fn spawn_watch(
        &self,
        context: &Arc<ServiceContext>,
        wire: &Arc<McpWire>,
    ) -> Result<JoinHandle<()>> {
        let context = Arc::clone(context);
        let queue = Arc::clone(&self.queue);
        let executor = Arc::clone(&context.executor);
        let workspace = self.workspace.clone();
        let governance = context.governance.clone();
        let wire = Arc::clone(wire);
        let advertised = Arc::clone(&self.advertised);
        let stop = Arc::clone(&self.watch_stop);
        thread::Builder::new()
            .name("ghostlight-session-watch".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    if !queue.wait_idle_or_timeout(Duration::from_secs(1)) {
                        break;
                    }
                    queue.advance_cancelled();
                    for request in queue.admitted() {
                        executor.show_waiting(&request);
                    }
                    wire.publish_resume(
                        &context.workspaces,
                        context.browser_port.as_ref(),
                        &workspace,
                    );
                    if advertised.changed_by(&governance.snapshot()).is_some() {
                        wire.render(ServiceResponse::CatalogChanged);
                    }
                }
            })
            .context("start the session watch")
    }

    /// One bounded worker: ordinary work, or the independent lane that keeps controls moving.
    fn spawn_worker(
        &self,
        context: &Arc<ServiceContext>,
        wire: &Arc<McpWire>,
        independent: bool,
    ) -> Result<JoinHandle<()>> {
        let queue = Arc::clone(&self.queue);
        let executor = Arc::clone(&context.executor);
        let workspace = self.workspace.clone();
        let workspaces = context.workspaces.clone();
        let port = Arc::clone(&context.browser_port);
        let wire = Arc::clone(wire);
        thread::Builder::new()
            .name("ghostlight-session-work".into())
            .spawn(move || {
                while let Some(request) = queue.next(independent) {
                    let result = executor.execute_prepared(&request, Admission::Queued);
                    deliver_result(&wire, result);
                    wire.publish_resume(&workspaces, port.as_ref(), &workspace);
                    queue.complete(&request.id);
                }
            })
            .context("start bounded session worker")
    }

    /// Serve one request the engine forwarded.
    ///
    /// A call the queue does not take is answered here, on the thread that reads the stream, so
    /// its answer is written before the stream can close: over capacity, it is refused; while
    /// Ghostlight quits, it is cancelled before it started, with the quit's cause. Each goes
    /// through the one completion path and its audit line.
    fn handle(&self, context: &Arc<ServiceContext>, wire: &Arc<McpWire>, request: ServiceRequest) {
        match request {
            ServiceRequest::Catalog => {
                let tools = if wire.stands_by() {
                    Vec::new()
                } else {
                    self.advertised.advertise(&context.governance.snapshot())
                };
                wire.render(ServiceResponse::Catalog { tools });
            }
            ServiceRequest::Invoke { id, tool, input } => {
                // Bound retained payloads as well as queue count. Decoding occurs once at intake.
                let bytes = serde_json::to_vec(&input).map_or(0, |input| input.len())
                    + id.as_str().len()
                    + tool.len()
                    + QUEUED_METADATA_BYTES;
                let request = Arc::new(PreparedInvocation::new(
                    id,
                    &tool,
                    input,
                    self.workspace.clone(),
                    Arc::clone(&self.connection),
                ));
                let (admission, _flight) = match self.queue.admit(Arc::clone(&request), bytes) {
                    Ok(()) => return,
                    Err(Refused::Full) => match self.queue.begin_unqueued() {
                        Ok(flight) => (Admission::OverCapacity, Some(flight)),
                        Err(by) => (Admission::Cancelled(by), None),
                    },
                    Err(Refused::Cancelled(by)) => (Admission::Cancelled(by), None),
                };
                let result = context.executor.execute_prepared(&request, admission);
                deliver_result(wire, result);
                wire.publish_resume(
                    &context.workspaces,
                    context.browser_port.as_ref(),
                    &self.workspace,
                );
            }
            ServiceRequest::Cancel { id } => self.queue.cancel(&id),
        }
    }

    /// Stop the workers, detach the connection, and release a workspace nobody owns unless a
    /// service end has taken over, which then decides it.
    ///
    /// The work still admitted is cancelled because Ghostlight quit, while a quit drains, and
    /// because the session ended otherwise.
    fn close(self, context: &ServiceContext) {
        self.watch_stop.store(true, Ordering::SeqCst);
        self.queue.stop(
            context
                .quit
                .drain
                .cause()
                .map_or(CancelCause::SessionEnded, CancelCause::Quit),
        );
        if let Some(watch) = self.watch {
            let _ = watch.join();
        }
        for worker in self.workers {
            let _ = worker.join();
        }
        context.workspaces.disconnect(
            &self.workspace,
            self.connection
                .attribution()
                .connection_id
                .as_deref()
                .unwrap_or_default(),
        );
        // A workspace with an owner outlives this connection: the caller is still there and its
        // next call must reach the same tabs. It is released when its owner is gone or the
        // service ends, not when a socket is.
        if !context.workspaces.is_owned(&self.workspace) {
            context.quit.release_on_close(&self.workspace);
        }
        context.diagnostics.sink().emit(
            ghostlight_bridge::diagnostics::event::HARNESS_DETACHED,
            ghostlight_bridge::diagnostics::Level::Info,
            None,
            self.connection
                .attribution()
                .connection_id
                .as_deref()
                .unwrap_or_default(),
        );
        context.workbench.refresh_live_parts();
    }
}

/// Bring a resumed session's tabs back as pending bindings, each in its own browser and with the
/// extension life the token names for that browser.
///
/// No browser is asked here: the session's first call confirms each tab against its browser's
/// own list (ADR-0203 Decision 1). A tab written without a URL digest, by a build before digests,
/// cannot be confirmed and is not brought back, so its handle names no open tab.
fn remember_resumed_tabs(workspaces: &WorkspaceStore, workspace: &WorkspaceId, kept: &KeptTabs) {
    for tab in &kept.owned {
        if let Some(url) = tab.url_digest.as_deref().and_then(UrlDigest::parse) {
            workspaces.remember_tab(
                workspace,
                &tab.handle,
                &tab.browser_id,
                tab.tab_id,
                url,
                kept.lives.get(&tab.browser_id).map(String::as_str),
            );
        }
    }
}

/// Hand one finished invocation to the session as a result for the request it names, or end the
/// stream when the result cannot be encoded.
fn deliver_result(wire: &McpWire, mut result: crate::work::result::InvocationResult) {
    let text = result.model_text();
    let is_error = result.is_error();
    let content = std::mem::take(&mut result.content);
    match serde_json::to_value(&result) {
        Ok(value) => wire.render(ServiceResponse::Result {
            id: result.invocation,
            text,
            result: value,
            is_error,
            content,
        }),
        Err(_) => wire.writer.close(),
    }
}
