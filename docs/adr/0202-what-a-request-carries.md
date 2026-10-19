# ADR-0202: What a request carries

Date: 2026-10-18. Status: Accepted, decided by the coordinator under the owner's delegation of
2026-10-18 ("you're authorized to evaluate the proposals and decide. I want this release to be an
opportunity to streamline architecture aspects like this one.").

Amends ADR-0161's amendment of 2026-10-18 (slice b3: when the source is read, when "in" may be
said, and how restored history names an agent), ADR-0161's "Capture evidence once for each
connection" (the observation time is the accept time) and its durable list (the claimed agent's catalog id),
ADR-0197 Decision 12 (what the resume token holds) and Decision 19 (the channel switches return,
now enforced on every request), and the schema-4 migration's handling of `channels.*.enabled`. Records three agent-facing changes inside flows under the owner's delegation of
2026-10-14 (ADR-0198). Keeps ADR-0105 Decision 2, ADR-0106, ADR-0196 and ADR-0199 as they are.

Decided and amended by ADR-0203 (2026-10-19): delegated item 5 (client-written tab bindings) is
decided there. Decision 1 (the reached browser's id joins the channel as a durable declared fact),
Decision 15 and slice S4 (the tab-group title reads the opener's agent, and reuse reads the
claim-derived agent) and Decision 23 (the browser port drops its workspace parameter) are amended.

Paths without a crate are under `crates/orchestrator/src`, and `ui/` is `crates/orchestrator/ui`.
Line numbers are the `dev` tree of 2026-10-18, with b3 landed as `feat(provenance): every
connection knows the app that started it`.

## Context

On 2026-10-18 the owner asked: "Do requests have a proper way to accumulate metadata as they are
onboarded into the processing pipeline?", and called it an "excellent moment for a proper
architecture analysis/proposal/break-and-rebuild change". On identity the owner's direction was
that every request carries where it came from, observed rather than claimed: "an MCP connector
that's invoked by claude-code.exe has a clear hint ... we just add a property: _source:
{channel:MCP, client: claude-code.exe} ... handling these _ fields properly, up to the surface",
and "even CLI requests can just do that - channel CLI, caller = whoever invoked." The coordinator
added the same principle for every intake: MCP, the command line, the browser stream, the desktop
shell, and the window's HTTP API on 127.0.0.1, where the operating system can name a loopback
connection's owner.

While the analysis ran, the owner set the shape: "meta fields are how different aspects of the
solution communicate values down the processing pipeline and back. the edge detects the callers,
attach the information to the request and passes it allong. It even helps in situations like the
Policy evaluation: if the MCP channel is disabled, any requests with _source.channel == 'mcp' should
be rejected. A much simpler take where the requests themselves are the boundary objects between
internal DDD 'domains' in the monolyth." And: "the stated response from the browser adapter can
simply be another state that we attach to the in-memory request in its way back to the client."
The coordinator answered that the stamp belongs at the service's intake, where the operating system
can name every peer, rather than in the edge, where any program speaking the relay could stamp
anything; that the meta is typed sections, not a bag of underscore fields; and that page content in
the adapter's reply stays memory-only. This decision takes that shape.

Three traces followed one request from the edge to the surface: intake, executor and completion.
The answer to the owner's question is: about half.

Facts about the connection have one typed carrier. `ConnectionEvidence` holds a durable
`Attribution` and a memory-only claim (`provenance.rs:33-56`). It is built once, shared through an
`Arc`, attached to every request before it queues (`service/mod.rs:1651-1656`), and leaves as the
audit's `provenance` and the window's `ConnectionDetails`. b3's `Source` lives there, which is the
right home.

Facts about the request have no carrier. The request is split across `ServiceRequest::Invoke`
(`intake/session.rs:151-172`), a `Job` (`service/admission.rs:74-79`), a `PreparedInvocation`
(`work/mod.rs:92-104`) and four arguments to `execute_prepared` (`work/mod.rs:320-326`). What its
stages learn goes into five tables on the shared executor, keyed by the invocation string
(`work/mod.rs:168-172`), and `complete_terminal` drains them once (`work/receipt.rs:19-131`). That
works because there is one completion path. Every new fact still costs a new table, a new drain and
a new field on each surface. The concrete gaps:

- **Three request ids.** The client's JSON-RPC id lives only in `McpSession`
  (`intake/mcp.rs:186-189`). A service id `mcp_<uuid>` is minted at `intake/mcp.rs:536` and keys
  the queue. `invocation_<uuid>` is minted at `work/mod.rs:139` and is the only one that reaches
  events and audit.
- **Flow steps share their flow's key.** `Reached` keeps two slots for it (`work/mod.rs:205-215`).
  Permissions are thrown away at every step's start (`work/receipt.rs:142`), so a flow's own
  admission check (`work/flow.rs:26`) never reaches its record.
- **Facts re-derived, not carried.** A step inherits its flow's requested browser, tab, target
  and repeat tool (`work/receipt.rs:144-152`), and for `RunFlow` they are all None
  (`work/mod.rs:2481-2552`). So a step's `browser` is ignored by `target_browser`
  (`work/mod.rs:1428-1433`), a removed target inside a flow is not marked or offered a replacement
  (`work/mod.rs:1830-1844`), and a step refused for another session's tab cannot name that session
  (`work/mod.rs:2119-2125`). No test runs a flow with `browser`; this was found by reading.
- **Connection evidence assembled by hand.** The opening is copied into a service-local `Opening`
  (`service/mod.rs:1329-1335`), and the claim is read back through a mutex (`1365-1367`). For a
  fresh session the evidence is minted after `initialize` (`1298-1302`), so `observed_at_ms`,
  documented as the accept time (`provenance.rs:47-48`), is not. b3 reads the source at `1245`,
  before the accept reply at `1256`: a process snapshot, up to 16 ancestors and an uncached
  `AppLocations::discover`, all inside the edge's 5 s `OPENING_DEADLINE`
  (`crates/relay/src/lib.rs:50-52`). A timeout there is an `AttemptFailed` and a service start
  (`crates/edge/src/forward.rs:375-392`).
- **One fact in many shapes.** The channel is a literal in the edge
  (`crates/edge/src/forward.rs:57-58`), two constants and a fold of unknown words in intake
  (`intake/mod.rs:22-38`), and a raw string in `McpSession` and the resume token
  (`intake/mcp.rs:181, 597-599`) that nothing in production reads.
- **One connection assumed among several.** A workspace copies `client_label` and `channel` from
  its first connection (`workspace/mod.rs:726-727`); a later one changes neither (`718-720`). The
  session card derives its agent from that copy (`workbench/live.rs:304-307, 492-494`). The public
  `ApplicationExecutor::execute` finds its evidence through `single_connection`
  (`work/mod.rs:273-288`, `workspace/mod.rs:749-754`), which answers None for two connections.
  Only tests call it.
- **The surface drops structure.** The agent is derived from (claim, channel) alone
  (`agent/mod.rs:217-226`) in four places (`workbench/mod.rs:1822, 1982`,
  `workbench/history.rs:55-58`, `workbench/live.rs:304-307`), so a restored MCP row is "Unknown
  agent" even when its record keeps a source. b3's words say "X, in Y." for any catalog claim beside
  any other agent's app (`language/provenance.rs:106-117`), so Cline started by Claude Desktop
  reads as "Cline, in Claude Desktop." `ui/lib/entries.js:21` reads `operation.browser`, a field
  that never existed (`workbench/mod.rs:1787-1806`).
- **Dead or dropped.** `deadline_ms` is always None (`intake/mcp.rs:543`), so the caller-deadline
  branch (`work/mod.rs:121-137`) runs only in tests. `Refused::Duplicate`
  (`service/admission.rs:131-134`) cannot happen: each call mints a fresh id and `McpSession`
  already refuses a duplicate client id (`intake/mcp.rs:532-534`). The token's `recording` is always
  None (`service/mod.rs:1410`). A cancel has no cause: one flag serves a client cancel, a disconnect
  and a quit (`work/mod.rs:73-90`, `service/admission.rs:196-201, 225-232`).
- **A sentence that over-claims.** "Paused from a screen that is watching."
  (`language/workbench.rs:87-88`) is said for any loopback request without the window's key while
  watching is on (`workbench/web/routes.rs:554-575, 1010-1014`), including a program's.

The owner's standing directives set the bar (`docs/MEMORY.md`). A seam changes only when a
separation-of-concerns analysis shows it must, and "it is frozen" is never the reason either way
("Get the architecture right; no artificial freezes"). Fewest meaningful moving parts. No check
that serves only the process. Lenient where nothing is lost, and a fresh look every time ("Be
adaptive as well"). What a person reads says only what they need ("Delight is clarity").

Three proposals answered the traces: finish the existing carriers and give each frame its own
findings; make the invocation one shared live record that events hand to the window; and type every
trust class, observe a source on every stream and add a loopback observer. Three independent
judgments chose the first. They found that the second couples the window to the executor's mutable
state, and that the third doubles the scope, reverses parts of the same-day amendment and adds new
platform code for a note. This decision is the first proposal with the judges' grafts and fixes.

## Decision

### The request is the boundary object

0. **One in-memory request crosses every domain, and comes back.** Intake builds the request:
   what came in, who sent it and on which channel (Decisions 3 to 7). Each domain it crosses reads
   what it needs from it and adds what it learns to its frame's findings, never to another domain's
   state and never through a lookup keyed by an id (Decisions 9 to 12): governance its checks, the
   workspace the tab and target it resolved, the browser port what it reached and the adapter's
   reply. The reply splits in two by type. Facts (what was reached, counts, effects, timings) are
   findings. Page content (text, images, form values) travels only in the operation's result to
   the agent and ends with it; no type lets it reach a record (`docs/MEMORY.md`, "Audit is
   metadata-only"). The single completion path turns the finished request into everything that
   leaves: the MCP result, the outcome sentence and next steps (AGENTS.md's one typed `Outcome`),
   the audit line, the history row and the window's events (Decisions 16 to 20). Surfaces receive
   values; none holds the live request.

### Where a request comes from

1. **Four classes of fact, each with one home.**
   - Observed, by the service or the operating system: connection id, accept time, `Source`,
     invocation id, timings, governance decisions and permission checks, tabs and targets resolved,
     the browser reached, what the page showed, and why a call was cancelled. Durable where a
     record needs them.
   - Declared, by Ghostlight's own peer over the peer-checked relay: role, channel, edge build,
     the command line's session marker, the resume token, the browser stream's origin, the
     extension's hello, and the shell's build and view. Only the channel is durable, as attribution.
     Any same-user program can open the relay and declare these (ADR-0199, web apps amendment), so
     none is authority.
   - Proven, by a secret: the window's bearer makes an HTTP request Control. It decides which acts
     a person's view offers, never what agents may do.
   - Claimed, by a client: `clientInfo.name` or the discovery name, JSON-RPC ids, `params._meta`
     and the cancel reason. Bounded, memory-only, shown as a claim. Never durable, never identity,
     never authority.

   A claim grants nothing and decides nothing. Governance decides by capability and destination,
   and, for one fact, by channel: the owner's channel switches (Decision 22a) admit or refuse a
   request by the channel its connection declared. The source is display and history evidence and
   decides nothing in this decision.

2. **The owner's `_source` is the service's stamp.** Every request carries `{channel, source}`
   because it rides its connection's `Attribution`: the channel from the opening Ghostlight's own
   peer sent, the source from the operating system. The claim sits beside it as a claim. Nothing a
   client writes takes part. There is no underscore-field passthrough and no generic bag. The MCP
   intake still reads only `name` and `arguments`; an `_source` in `params` is ignored, and one
   inside `arguments` is refused as an unknown field (`language/mod.rs:1596-1609`).

3. **One accepted connection, one evidence.**
   - `serve_mcp` decodes the marker, reads the channel word and writes the accept reply. Then it
     mints the `Attribution`: connection id, accept time and channel. It keeps the peer's process id
     beside it. One value, `Accepted { attribution, peer, marker }`, replaces the local `Opening`.
     A new connection fact is a field on `Attribution` (durable) or `ConnectionEvidence`
     (memory-only), never on a local struct.
   - The source is read once, in `LiveSession::open`: right after the accept reply for a resumed
     session, and after the `initialize` answer is written for a fresh one. Neither the edge's
     opening nor the client's handshake waits on the process read; the first request does. The
     sweep of finished sessions in `LiveSession::open` (`service/mod.rs:1509-1511`) leaves the
     handshake with it. Nothing the client sends takes part.
   - `ConnectionEvidence::new(attribution, claim)` is the one constructor. The evidence is never
     changed. One `Arc` each goes to the session, the workspace's live map, the workspace's opener
     (Decision 15) and every request on the connection.

   ```rust
   impl Attribution {
       fn accepted(channel: IntakeChannel) -> Self;   // connection id and accept time, no source
       fn observed(self, peer: Option<u32>) -> Self;  // the source, read once from the OS
   }
   impl ConnectionEvidence {
       fn new(attribution: Attribution, claim: &str) -> Self;  // the claim stays memory-only
   }
   ```

4. **Every intake says where it came from, through what it already has.**
   - `mcp` (`ghostlight mcp`): the edge declares channel `mcp`. The source is the app above the
     edge, for example Claude Desktop's `claude.exe`. The claim is `clientInfo.name`. Subagents
     of one client share one edge and so one source; a source does not tell callers on one
     connection apart (ADR-0198, amendment of 2026-10-18).
   - `cli` (`ghostlight call`): channel `cli`, and "caller = whoever invoked" has two answers that
     both stay. The source is the app the call came from ("A ghostlight call from
     WindowsTerminal.exe."). The session owner is the marker: `GHOSTLIGHT_SESSION`, else the nearest
     non-Ghostlight ancestor folded to its same-name run (`cli/session.rs:105-170`). One call can
     record `bash.exe` as owner and `claude.exe` as source, and both are right. The claim is
     Ghostlight's own "ghostlight call" (`cli/client.rs:194`), so the agent is Command line.
   - `browser`: identity is Ghostlight's own extension's hello. No source is read
     (ADR-0161 amendment: these streams carry no work to attribute). Acts carry
     `RuntimeSource::Browser`.
   - `desktop`: the same-build check admits it (`intake/listener.rs:93-95`). Acts carry
     `RuntimeSource::Tray`. No source.
   - `control`: the peer declares its operation. No source.
   - HTTP on 127.0.0.1: the bearer proves Control; its absence while watching is on means Watch
     (`workbench/web/routes.rs:554-575`). Acts carry `RuntimeSource::Window` or
     `RuntimeSource::Watcher`, and a watcher may only hold. The operating system could name the
     socket's owner, but no surface needs more than the true words of Decision 21, so no observer is
     built now (question 3).

5. **What a client may write:** the tool name and arguments, which the decoder checks against
   each tool's closed field list; its own JSON-RPC ids; a cancel of its own outstanding ids
   (`intake/mcp.rs:413-432`); a bounded name; the protocol version; and, through Ghostlight's
   command line, `GHOSTLIGHT_SESSION` for continuity only (ADR-0106). **What it may not write:** the
   channel, the source, the connection id, any time, the invocation id, the workspace, the deadline
   beyond its own bounded `timeout_ms`, any decision, or anything in `Attribution` or in a frame's
   findings. `params._meta` stays unread. If MCP progress is built, `progressToken` lives in
   `McpSession` beside the client id. If a client ever sends the per-caller marker ADR-0198's
   amendment allows, it is one named, bounded, memory-only field and only scopes the tab default.

### What a request is

6. **One request id.** `InvocationId` is a type in `work`, which owns invocation ids
   (`docs/1.0/ARCHITECTURE.md:488`): `invocation_<32 hex>`, its prefix in one place.
   `McpSession::call` mints it and keeps its two maps between the client's id and it. The queue,
   events, the MCP result, the audit line and the diagnostics carry the same id. The MCP result
   already names `invocation` (`work/result.rs:67-94`, kept by the lean projection,
   `intake/mcp/lean.rs:40-53`), so a client's result joins its audit line with no client value made
   durable.

7. **One request value.** `PreparedInvocation` is built once in `LiveSession::handle` and owns
   everything the request is: id, tool, workspace, its connection's evidence (required, no longer
   `Option`), its cancellation, the intake instant and wall time, the deadline, whether a longer
   `timeout_ms` would help (now just the decoded operation's `timeout_can_grow`), the decoded
   operation, and the waiting stage. The queue holds `Arc<PreparedInvocation>`, and `Job` goes. The
   executor's door is `execute_prepared(&prepared, admission)` with a closed
   `Admission { Queued, OverCapacity }` in place of a bool. The deadline counts from service intake;
   time a line spent held at the edge is not counted, a known and accepted limit.

   ```rust
   pub(crate) struct InvocationId(String);           // "invocation_<32 hex>", minted once
   pub(crate) struct PreparedInvocation {
       id: InvocationId,
       tool: String,
       workspace: WorkspaceId,
       provenance: Arc<ConnectionEvidence>,           // every request has a caller
       cancellation: CancellationToken,
       received: Instant,
       received_at_ms: u64,                           // the live row's start
       deadline: Instant,
       deadline_adjustable: bool,
       decoded: Result<Operation, LanguageError>,
       stage: Mutex<InvocationStage>,                 // read by the session's watch thread
   }
   ```

8. **A cancel says who cancelled.** The token gains a cause set once beside its flag:
   `Client`, `SessionEnded` or `Quit`. A client's `notifications/cancelled` sets `Client`.
   `LiveSession::close` sets `Quit` while a quit drains (`intake.quit.drain.is_quitting()`) and
   `SessionEnded` otherwise. The flag stays an `AtomicBool`, so `BrowserDispatch.cancelled`
   (`browser/mod.rs:86-99`) is unchanged. `language::outcome` words each cause. This slice is
   optional (S9).

### What a request learns

9. **Each frame keeps its own findings.** A frame is a request's top-level operation, or one flow
   step. What its stages learn is one closed value:

   ```rust
   #[derive(Default)]
   struct Findings {
       observed: Observed,               // what the page showed, merged at each crossing
       reached: Option<String>,          // the browser that received a command of this frame
       permissions: PermissionTrace,     // every governance check of this frame
       coverage: Option<Coverage>,       // scoped reads and recording, merged
       stale_offer: Option<StaleOffer>,  // taken by the same frame's refusal
   }
   ```

   It sits in a `RefCell` on the frame's stack. A frame runs on one thread, so the compiler refuses
   to share it, and each write is one short method that holds no borrow across a call. Stages write
   it through their `InvocationContext`. The frame's own completion takes it by value. It is never
   keyed by an id and never stored on the executor, so the five tables go. The live coverage notice
   still reaches the window as it does today (`work/documents.rs:202-209`).

10. **One constructor per frame.** `InvocationContext::of` builds the top level and
    `InvocationContext::step` a flow step, each from its own operation: capability requirements,
    requested browser, tab and target, repeat tool, gesture and settlement. A step shares its
    flow's id, workspace, caller, deadline, `deadline_adjustable`, cancellation, authority snapshot
    and lease. It owns its operation's facts, its findings and its own audit record. Overrides
    inside one frame keep `..*context` (`work/recording.rs:236-242, 352-355`) and so keep that
    frame's findings. `Completion::of` builds what the completion path records for either.

11. **A flow folds one fact.** When a step completes, the flow takes only the browser it reached,
    so the flow's record names the last browser a step reached, as today. The flow keeps its own
    admission check in its own findings. Composition still reads a limited step's coverage from the
    step's result (`work/composition.rs:213-224`), so a flow's summary is qualified once.

12. **A record states only what was learned.** A request that never decoded records no
    capabilities, as a flow step that never started already does (`work/receipt.rs:176-198` against
    `work/mod.rs:358-366`). Lifecycle refusals keep their `Decision` as today; separating lifecycle
    outcomes from governance outcomes is a different question.

13. **What changes for agents inside flows** (owner delegation of 2026-10-14). A step's `browser` is
    honored, so a step can meet the same `BrowserPinned` or `AmbiguousBrowser` refusal a direct call
    meets. A step whose target the page removed marks it removed and offers the replacement. A step
    refused for another session's tab names that session. Each is what the same call says outside
    a flow.

### Up to the surface

14. **One agent, named where the words are written.** `language::provenance` names the agent and
    composes the explanation in one function, from the channel, the claim and the source, through a
    closed relation it keeps to itself: the source is the claimed agent's own app; the claimed agent
    runs inside the source's app; the two stand apart; or there is no source.
    - The claim names the agent. Without a claim, as after a restart or from a client that sent no
      name, the source's catalog agent names it. Without either, the agent is Unknown agent. The
      command line is Command line. This returns restored history to ADR-0161's own rule: compact
      attribution from the retained executable.
    - "Claude Code, in Claude Desktop." is said only when the catalog knows the claimed agent runs
      inside that app. Each roster row gains `runs_inside`, the catalog ids of the apps it is known
      to run inside: Claude Code in Claude Desktop, and the Cline rows in VS Code, Cursor and
      Windsurf. Otherwise both names are given without judging, as b3 already does for a name the
      catalog does not know. A missing entry falls back to those neutral words, never to a wrong
      "in".
    - `ConnectionDetails` carries the agent in a field that is not serialized. The `agent` fields
      that `OperationSummary`, `HistoryItem` and `SessionSummary` already serialize copy it. No
      part gains a key, so the amendment's promise to watchers holds. `ConnectionDetails` gains no
      typed source; the sentence stays the one place a person reads it (question 2).
    - `Agent::of_evidence` and the session card's own derivations go.

14a. **History keeps the same name as the live row.** When a connection's claim matches a catalog
    agent, its `Attribution` keeps that catalog id as `claimed_agent`, labelled a claim: a closed,
    bounded value, never the client's free text, which stays memory-only. A restored row is named
    by the same function as a live one, from the claimed agent and the source, so Claude Code work
    in Claude Desktop reads "Claude Code, in Claude Desktop." live and after a restart. Without a
    catalog claim, the source's catalog agent names it; without either, Unknown agent. This amends
    ADR-0161's durable list by one bounded field.

15. **A workspace is named for the connection that opened it.** `WorkspaceState` keeps one
    `opener: Arc<ConnectionEvidence>` in place of its `client_label` and `channel` copies, and
    `WorkspaceSummary` carries `opener: ConnectionDetails`. The session card, connection counts,
    Integrations in use, the tab-group title and released-tab reuse read the opener. That is
    today's behavior stated as a rule: a command-line workspace outlives its connections
    (ADR-0106), so it cannot be named from live ones, and a group is named for the client that
    opened it (ADR-0066). A later connection is listed in `connections` and renames nothing.
    `ApplicationExecutor::execute` and `WorkspaceStore::single_connection` become test helpers, so
    no production path assumes one connection.

16. **The single completion path stays the only joiner.** `complete_terminal` takes the frame's
    findings by value: coverage qualifies the summary, facts and audit projection; `observed` merges
    with the terminal's; `reached` becomes the record's browser; `permissions` become the record's.
    The record carries the connection's `Attribution`. The recorder receives the evidence beside the
    record, so the claim reaches only the live view (`audit.rs:356-388`).

17. **Live rows tell the same time and the same browser as history.** `WorkWaiting` and
    `WorkStarted` carry the connection's details (required) and the request's intake time, and a
    live row counts from it. Today each event restamps the row (`workbench/mod.rs:299`), so a call
    that waited shows a shorter time live than its record. A live row names its session's browser
    through the page's session fallback (`ui/lib/view.js:227-231`), which is true because a
    workspace works in one browser (ADR-0084 Decision 4). The dead read at `ui/lib/entries.js:21`
    goes, and no event carries the browser reached.

18. **Fields nobody reads go.** `SessionSummary.leased` and `held_tab_count` are read by no page,
    so they leave the window's JSON, and the watch view's removal of them
    (`workbench/parts.rs:679-686`) goes. `AuditRecord.channel` stays for SIEM readers; its doc and
    `HistoryItem.channel`'s say what is true: they are the provenance's channel, absent only on a
    record without provenance (`audit.rs:116-119`, `workbench/mod.rs:1913`).

19. **Diagnostics join a request to its connection.** `OPERATION_COMPLETED` and `OPERATION_FAILED`
    name the connection id (`work/receipt.rs:76-94`), and `HARNESS_ATTACHED` names the source's file
    name (`service/mod.rs:1521-1534`). Never the claim (ADR-0161).

20. **The model is not told where it came from.** The MCP result stays provenance-free
    (`work/result.rs:67-70`, `deny_unknown_fields`). Its `invocation` is the join key.

21. **A pause from a watching page says what is true.** `PAUSED_FROM_WATCHER` becomes "Paused from
    a watching page or a local program." The tray and the toolbar keep their own words.

### The channel switches return

22a. **A policy can turn a channel off, and every request answers to it.** Schema 4 regains
    `channels.mcp.enabled` and `channels.cli.enabled` (ADR-0105's keys, a closed vocabulary of
    two), unset meaning on. The governance facade reads the request's channel (Decision 7: every
    request carries its connection's evidence) and refuses a request whose channel is off before
    any browser work, through the one completion path, with a refusal composed in `language/`
    that says which switch is off and where ("Ghostlight's policy turns off work from MCP agents.").
    The connection still opens, so the agent hears why. The window's Policy page shows both
    switches with the layer that set them and lets the authored user layer change them. The
    schema-3 migration carries both keys instead of dropping a `true` and refusing a `false`.
    Honest limit, stated wherever the switch is described (docs/trust included): the channel is
    declared by Ghostlight's own peer, and any program running as the same user could declare
    another, so a switch is a person's or an organization's guardrail for well-behaved agents, not
    a barrier against hostile software on the same account. This amends ADR-0197 Decision 19, whose
    reason (no promise behind the switch) the request-carried channel removes.

### Seams

22. **Seams that change, each for a separation-of-concerns reason.**
    - `relay::protocol` gains the two `mcp` channel words, and the edge and the command line use
      them. The wire bytes are identical, and `Opening.channel` stays an optional string because the
      desktop role reuses it. A word that crosses a process boundary gets one home, and the only
      crate the edge and the service share is `relay`. Relay 4 is unpublished, so the connectors
      guard (`tests/guards/connectors.test.mjs:210-224`) has no published source to compare.
    - The session vocabulary and the executor's door, both in-process: `ServiceRequest` and
      `ServiceResponse` carry `InvocationId`; `execute_prepared(&prepared, admission)`.
    - `DomainEvent::WorkWaiting` and `WorkStarted`: provenance is required, and the intake time is
      added. The set stays small and closed, and every other variant still joins by id.
    - The workspace aggregate: one opener in place of two copies.
    - The window's JSON for its own page: the agent is named once, and two unread fields go.
    - The resume token, which the service writes and the edge holds opaque: `channel`,
      `session_key` and `recording` go. The read stays tolerant, so older tokens still resume.

23. **Seams that stay.** Relay protocol 4's frames, with nothing per request; edge hold time and
    replay stay edge-local. The edge, verb-free and stamping nothing. The relay endpoint: b3's
    `peer_process_id` and `os::ancestor_images` are enough. Audit line v2 gains one optional field, the claimed agent's
    catalog id (Decision 14a); readers that do not know it ignore it. The
    governance facade, which still receives only a capability set and a destination. The browser
    port and `BrowserDispatch`. Adapter protocol 4 and the extension. The MCP result and the
    catalog. The presentation reactor's and the window's own invocation-keyed state, which are
    projections of events, not request facts. The command line's client-side owner walk. No bus,
    no generic map, no new crate or process.

## Decided under the owner's delegation

The panel left six questions for the owner. The owner delegated them on 2026-10-18.

1. **Restored history names:** keep the claimed agent's catalog id, labelled a claim (Decision
   14a). Names then match live and after a restart; free text never becomes durable.
2. **An observed-program row:** none. The connection sentence already names the app.
3. **Who pressed Hold on a watching page:** Decision 21's true words now; the loopback-owner
   observer is not built. It would add platform code for one sentence.
4. **Who owns a command-line session:** unchanged. The marker and `GHOSTLIGHT_SESSION` keep
   ADR-0106's continuity; observing the owner in the service is not needed for any surface now.
5. **Client-written tab bindings** (the unsigned resume token, released-tab reuse by the opener's
   claimed label): accepted as ADR-0199's same-user risk for now, and listed for the
   architecture sweep that follows this rebuild, where it is decided on its own.
6. **Timing:** every slice lands in 2.0, the flow fixes (S8) and the cancel cause (S9) included.
   The owner asked for this release to streamline architecture.

## Considered and not taken

- **One live record handed to the window through events.** The window would read the executor's
  mutable request under its own lock: a lock order to defend, projection lifecycle moved into the
  request, and events that are no longer values.
- **Trust classes as types everywhere, typed audiences, and a source on every stream.** About twice
  the scope. It reverses the amendment's rule that browser, control and desktop streams decide no
  source, and its key rename and audience rewrite churn surfaces that work.
- **The channel enum inside `relay`.** A service vocabulary in the relay crate; the two words are
  enough.
- **The browser reached, sent live to running rows.** The session already names it truthfully
  (Decision 17), so a new event or side path would carry a fact the page has.
- **A process-id reuse guard against the accept time.** A peer that exits closes its stream, so no
  request can be attributed to a reused id, and the walk already stops at a parent that started
  after its child (`crates/relay/src/os/ancestry.rs:36-57`). The guard would need a clock
  conversion in the relay for a case that attributes no work.
- **A cache for the app lookup.** The owner's rule is a fresh look every time. S1 measures first.
- **A closed enum for every way a source can be missing.** Every surface says the same words for
  each case today, so one `None` is enough.

## Consequences

For people:

- After a restart, a recorded MCP action with a source keeps an agent name, the app that started
  it, instead of "Unknown agent" (question 1).
- "X, in Y." appears only when the catalog knows X runs inside Y. Cline started by Claude Desktop
  gives both names without judging.
- A running row's time counts from when Ghostlight received the call, the span history records.
- A pause from a watching page no longer claims a screen did it.
- A flow's row lists its own admission check, and each step lists only its own checks, coverage
  and browser.
- A slow app lookup can no longer fail an edge's opening and restart the service.

For agents:

- Every result's `invocation` is the id of its audit line.
- Inside a flow, a step's `browser`, removed target and foreign tab behave as they do in a direct
  call (Decision 13).
- With S9, a cancelled call says whether the agent cancelled it.
- The catalog, the result shape and the lean projection are unchanged.

For audit readers: the line's shape is unchanged. Three values change because they become true:
`observed_at_ms` is the accept time for fresh sessions, an invalid request records no capabilities,
and a flow's own record lists its admission check. The SIEM guide says so in the slices that change
them.

What is deleted: the service-local `Opening`, `ConnectionEvidence::started_by`, the `mcp_` id and
its prefix, `deadline_ms` and the caller-deadline branch, `Job`, `Refused::Duplicate` and its
literal, three dead resume-token fields, `McpSession`'s unread channel, key and getters, the edge
literal and the intake channel constants, the workspace's label and channel copies,
`WorkspaceStore::channel`, `Agent::of_evidence` and its re-derivations, the five executor tables
with their types and drains, the permission discard at a step's start, two of three hand-built
completions, `single_connection` and the public `execute` from production, two unread window fields
and their redaction, and the read of a field that never existed. One small type is added for the
id, one for the findings and one enum for admission.

Risks:

- The source is read on every connection, including every `ghostlight call`: a process snapshot and
  an uncached app lookup. It now delays only the first request. S1 measures it on the owner's
  machine. A cache would need its own decision.
- Restored names can differ from live ones (question 1).
- S8 changes what agents see inside flows. These are fixes, but they are behavior changes. The
  browser journeys that use flows run against a fresh build.
- Findings are local to a frame. Code that wrote a fact in one frame and read it in another would
  lose it. Every producer and consumer found is in one frame (the stale offer at `work/mod.rs:881`
  and `2128`), and the unchanged documents, rebound and replacement suites guard it.
- Churn: S5 edits 326 test call sites mechanically, and S6 and S7 touch every `InvocationContext`
  literal in tests. They land as their own commits when `work/tests` is quiet.
- The opener rule keeps a shared `GHOSTLIGHT_SESSION` key named for its first launcher.
- Dropping token fields relies on the token's tolerant read (no `deny_unknown_fields` on
  `ResumeState`); S2's test pins it.
- Deleting `Refused::Duplicate` trusts one minter and `McpSession`'s duplicate-id refusal; the
  queue stops checking it again.
- `runs_inside` is a kept list. A missing entry costs the neutral words, never a false "in".

## What this supersedes and amends

- **ADR-0161, amendment of 2026-10-18 (b3):**
  - "decided once when an `mcp` stream opens, before anything the client sends" becomes: decided
    once per connection from the operating system's facts, after the service has answered the
    opening and, for a fresh session, `initialize`. Nothing the client sends takes part.
  - "The claimed agent runs inside another agent's app: both, 'Claude Code, in Claude Desktop.'"
    applies only when the catalog knows the claimed agent runs inside that app (Decision 14).
  - "History keeps no claim": the record still keeps none, and a restored row is now named for its
    source's catalog agent (Decision 14).
  - "The watcher audience sees the same connection explanation it already saw; no part gains a
    key" stands, and Decision 14 keeps it.
- **ADR-0161, "Capture evidence once for each connection":** stands. Its "local observation time"
  is the accept time for every session, and a request's evidence is required, no longer optional.
- **ADR-0197 Decision 12:** the resume token no longer holds the channel or a session key; the
  opening names the channel.
- **ADR-0066 and ADR-0137:** unchanged. Decision 15 states their per-client group as the opener
  rule.
- **ADR-0198:** gains Decision 13's three flow behaviors under the owner's delegation.
- **ADR-0199, web apps amendment:** unchanged; Decision 21 matches its consequence that any
  program on this computer can watch.
- **`docs/1.0/ARCHITECTURE.md`:** the provenance paragraph (`366-378`) and the Work section
  (`486-488`) change in the commits that make them true.

## Rebuild plan

Each slice is one signed conventional commit, or two where marked, and leaves a green tree. Each
runs the checks AGENTS.md names for what it touches: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, then `cargo test -p ghostlight-service`,
or `cargo test --workspace` and `node --test tests/guards/connectors.test.mjs` when `relay` or
`edge` change; `npm test` in `extension/` is not needed, because no extension file changes. Docs run
`pwsh -NoProfile -File scripts/check-repository-integrity.ps1`. Each slice ends with something the
owner can see on the dev-loop deploy where it has a surface.

Lanes: **docs** (S0), **connection** (S1 to S4), **request** (S5 to S9), **window** (S10) and
**governance** (S11, after S6). S1
and S2 go first, because both rewrite `serve_mcp` and S2 and S5 both edit `McpSession`. After S2,
the connection and request lanes run in parallel. They share `service/mod.rs` and
`workbench/mod.rs` in different functions, so each rebases on the other's landed commits. S10 runs
any time after S0. Progress goes in the rc-1 LEDGER rows.

**S0 -- docs(adr): what a request carries.** Lane docs.
- Changes: this ADR, its README row, and a pointer line in ADR-0161's and ADR-0197's headers.
- Deletes: nothing.
- Proves: the integrity script.

**S1 -- fix(provenance): a connection's source is read after its opening is answered, and its
time is the accept time.** Lane connection. Depends on S0. rc-1.
- Changes: `serve_mcp` mints the `Attribution` after the accept reply into
  `Accepted { attribution, peer, marker }`. `LiveSession::open` reads the source once. A fresh
  session writes its `initialize` answer before the session opens. `ConnectionEvidence::new` takes
  the attribution and the claim. `HARNESS_ATTACHED` names the source's file name. The ADR-0161
  amendment's wording, the doc comment at `service/mod.rs:1214-1218`, `ARCHITECTURE.md` and the
  SIEM guide's `observed_at_ms` (the accept time) follow.
- Deletes: the local `Opening` (`service/mod.rs:1329-1335`), `ConnectionEvidence::started_by`
  (`provenance.rs:142-147`), the production use of `ConnectionEvidence::new(label, channel)`
  (tests keep a helper), and the read before the accept (`service/mod.rs:1245`).
- Proves: `an_opening_is_answered_before_the_source_is_read` and
  `initialize_is_answered_before_the_source_is_read`, with a blocking source reader injected in
  the test intake; `a_fresh_session_is_dated_when_it_was_accepted` (a pause before `initialize`);
  `a_resumed_session_is_dated_when_it_was_accepted`;
  `harness_attached_names_the_source_file_and_never_the_claim`; `work/tests/provenance.rs`
  unchanged and green; `GHOSTLIGHT_BIN_DIR=<fresh build> node tests/provenance-journey.mjs`. Live:
  the edges under the owner's Store Claude Desktop connect at once, and the observation's time on
  that machine is recorded in the ledger.

**S2 -- refactor(intake): one channel word, and the session engine keeps only what it reads.** Lane
connection. Depends on S1. rc-1, before `v2.0.0`.
- Changes: `relay::protocol` gains the `mcp` and `cli` channel words. The edge and the command
  line use them. `IntakeChannel::of_opening` reads them, and an unknown or missing word is still an
  MCP client.
- Deletes: `MCP_CHANNEL`, `CLI_CHANNEL` and `intake_channel` (`intake/mod.rs:22-38`); the edge's
  `CHANNEL` (`crates/edge/src/forward.rs:57-58`); `McpSession`'s `channel` and `session_key` fields,
  `channel()`, `session_key()`, `revision()` and `bind_session()` (`intake/mcp.rs:181-260`) and
  `McpWire::bind_session` (`service/mod.rs:1369-1371`); `ResumeState.channel`, `.session_key` and
  `.recording` (`intake/mcp.rs:597-608`) with `resume_state`'s `recording` parameter; the token's
  channel fold (`service/mod.rs:1246-1255`).
- Proves: `a_resume_token_written_with_the_retired_fields_still_resumes`;
  `an_opening_with_an_unknown_channel_is_an_mcp_client`;
  `an_opening_with_no_channel_is_an_mcp_client`;
  `the_edge_and_the_command_line_send_the_opening_bytes_they_sent_before`; the session tests without
  the dead-getter asserts; `cargo test --workspace`; the connectors guard.

**S3 -- fix(provenance): one function names the agent, and "in" means the catalog knows it.** Lane
connection. Depends on S1. rc-1.
- Changes: `language::provenance` names the agent and composes the explanation from the channel,
  the claim and the source through its closed relation. `ConnectionDetails` carries the agent
  unserialized, and the summaries' `agent` fields copy it. Roster rows gain `runs_inside`. A
  restored MCP row with a source is named for its source's catalog agent. The comment at
  `ui/lib/view.js:212-214` becomes true. The ADR-0161 amendment's surface section follows.
- Deletes: `Agent::of_evidence` (`agent/mod.rs:217-226`) and its calls (`workbench/mod.rs:1822,
  1982`, `workbench/history.rs:55-58`); the guess in `claimed_beside`
  (`language/provenance.rs:110-111`).
- Proves: `a_claim_inside_its_known_host_says_both`;
  `a_known_claim_beside_another_agents_app_does_not_say_in`;
  `a_live_claim_names_the_agent_before_its_source`;
  `restored_mcp_history_is_named_for_the_app_that_started_it`;
  `restored_mcp_history_without_a_source_stays_unknown`;
  `watchers_see_the_keys_they_saw_before` (activity, history and connections parts); b3's
  sentence tests kept; the page tests. Live: a service restart on the owner's machine keeps agent
  names in history.

**S4 -- refactor(workspace): a workspace is named for the connection that opened it.** Lane
connection. Depends on S3.
- Changes: `WorkspaceState.opener` and `WorkspaceSummary.opener`. The session card, counts and
  Integrations in use take the opener's agent. The tab-group title and released-tab reuse read the
  opener's claim, unchanged in behavior. `ARCHITECTURE.md`'s Workspace section states the rule.
- Deletes: `WorkspaceState.client_label` and `.channel` (`workspace/mod.rs:521-522, 726-727`),
  `WorkspaceSummary.client_label` and `.channel`, `WorkspaceStore::channel`
  (`workspace/mod.rs:986-992`), the derivations at `workbench/live.rs:304-307, 492-494`,
  `SessionSummary.leased` and `held_tab_count` and their removal (`workbench/parts.rs:679-686`),
  and `ui/lib/entries.js:21`.
- Proves: `a_command_line_session_keeps_its_name_while_no_call_is_connected`;
  `a_later_connection_neither_renames_the_session_nor_its_tab_group`;
  `a_session_card_names_the_agent_its_operations_name`; the parts tests; `node --test` for the
  page; `node tests/workbench-surface.mjs`.

**S5 -- refactor(work): one request id from intake to audit.** Lane request. Depends on S2. Two
commits: the id, then the test-only `execute` signature.
- Changes: `InvocationId` in `work`, minted by `McpSession::call`, whose maps it keys.
  `ServiceRequest` and `ServiceResponse` carry it. `Job.id` is an `InvocationId` until S6.
  `deliver_result` keys by `result.invocation`.
- Deletes: `SERVICE_ID_PREFIX` and the `mcp_` id (`intake/mcp.rs:53, 536`);
  `ServiceRequest::Invoke.deadline_ms` (`intake/session.rs:163-165`), the `None` at
  `intake/mcp.rs:543`, the caller-deadline branch (`work/mod.rs:121-137`) and its assertion
  (`work/tests/execution.rs:409-419`); `deliver_result`'s id argument (`service/mod.rs:1969`); the
  test-only `execute`'s deadline parameter.
- Proves: `the_result_a_client_receives_names_the_invocation_its_receipt_names` (a service over a
  relay pair: the result's `invocation` equals the audit line's);
  `a_cancel_reaches_the_invocation_the_client_named`;
  the session's batch, duplicate-id and cancel tests, rekeyed; the result goldens unchanged without
  `GHOSTLIGHT_CATALOG_UPDATE`.

**S6 -- refactor(work): the request owns its workspace, caller and cancellation.** Lane request.
Depends on S5.
- Changes: `PreparedInvocation` gains the workspace, required evidence, the cancellation token and
  the intake wall time. The queue holds it. `execute_prepared(&prepared, admission)` and
  `show_waiting(&prepared)`. `WorkWaiting` and `WorkStarted` carry required details and the intake
  time, and the window's row starts from it. `ARCHITECTURE.md`'s Work section says the request
  arrives as one value.
- Deletes: `Job` (`service/admission.rs:74-79`); `Refused::Duplicate` and its check
  (`service/admission.rs:98, 131-134`) and `"duplicate_request"` (`service/mod.rs:1659-1665`);
  `execute_prepared`'s workspace, cancellation and bool parameters; `show_waiting`'s workspace
  parameter; the restamp at `workbench/mod.rs:299`; `ApplicationExecutor::execute` and
  `WorkspaceStore::single_connection` from production.
- Proves: the admission tests over `PreparedInvocation`; `work/tests/provenance.rs` unchanged and
  green; `a_full_queue_refuses_through_the_completion_path`;
  `a_disconnect_cancels_queued_work_and_records_it_once`;
  `a_waiting_call_keeps_its_start_time_when_it_runs`.

**S7 -- refactor(work): what a frame learns lives with the frame.** Lane request. Depends on S6.
- Changes: `Findings`; `InvocationContext::of` and `::step`, with a step still inheriting its
  flow's requested facts until S8; `Completion::of`; the flow folds `reached` and keeps its own
  admission check; an undecoded request records no capabilities; the operation lines name the
  connection. `ARCHITECTURE.md`'s Work section: what a frame learns lives with the frame and is
  read once by its completion. The SIEM guide notes the two values that change.
- Deletes: the five executor fields (`work/mod.rs:168-172`), their types (`189-228`) and
  initializers (`252-256`); `observe`, `take_observation`, `observations`, `take_stale_offer`
  (`work/mod.rs:1584-1613`); `reached` and `take_reached` (`work/mod.rs:1386-1407`);
  `retain_permission`'s table and `take_permissions` (`work/receipt.rs:201-220`); the discard at
  `work/receipt.rs:142`; `take_coverage` and the table half of `retain_coverage`
  (`work/documents.rs:179-200`); two of the three hand-built `Completion`s.
- Proves: `a_flow_record_keeps_its_own_admission_check`;
  `each_flow_step_record_holds_only_its_own_checks_and_coverage`;
  `a_flow_record_names_the_browser_its_last_step_reached`;
  `two_calls_in_one_workspace_keep_their_own_findings` (the independent lane beside a leased read);
  `an_invalid_request_records_no_capabilities`; `operation_lines_name_the_connection`; the
  documents, rebound, replacement, tabs, execution, golden and composition suites unchanged and
  green; the audit goldens unchanged, since they are built by hand (`audit/record_tests.rs`).

**S8 -- fix(work): a flow step names its own browser, tab and target.** Lane request. Depends on S7.
- Changes: `InvocationContext::step` derives the requested browser, tab, target and repeat tool
  from the step's own operation. ADR-0202 Decision 13 records the effect for agents.
- Deletes: the inheritance of those facts in `run_child` (`work/receipt.rs:144-152`).
- Proves, each failing before the change: `a_flow_step_opens_in_the_browser_it_names` (two fake
  browsers); `a_flow_step_whose_target_the_page_removed_is_marked_and_offered_its_replacement`;
  `a_flow_step_refused_for_another_sessions_tab_names_that_session`. The live browser journeys that
  run flows, against a fresh `GHOSTLIGHT_BIN_DIR`.

**S9 (optional) -- feat(work): a cancelled call says who cancelled it.** Lane request. Depends on
S6.
- Changes: the cancellation token's cause, set once beside its flag; the queue's cancel sets
  `Client`, and `LiveSession::close` sets `Quit` or `SessionEnded`; `language::outcome` words each.
- Deletes: nothing.
- Proves: `a_client_cancel_is_recorded_as_cancelled_by_the_agent`;
  `work_waiting_when_its_session_ends_says_the_session_ended`;
  `work_waiting_at_quit_says_ghostlight_quit`; the result goldens unchanged unless the client
  cancel's words change.

**S10 -- fix(workbench): a pause from a watching page says what is true.** Lane window. Depends on
S0.
- Changes: `PAUSED_FROM_WATCHER` (`language/workbench.rs:87-88`).
- Deletes: nothing.
- Proves: `a_pause_from_a_watching_page_names_no_screen`; the page tests that render a hold.

**S11 -- feat(governance): a policy can turn a channel off, and every request answers to it.** Lane
governance. Depends on S6 (every request carries its evidence).
- Changes: schema 4's two channel keys (Decision 22a); the facade's channel check before any
  browser work; the refusal and its words; the Policy page's two switches with their layer; the
  migration carries the keys; `docs/guides/governance-configuration.md`, LANGUAGE, ARCHITECTURE and
  docs/trust state the switch and its honest limit.
- Deletes: the migration's drop-or-refuse of `channels.*.enabled` and its refusal sentence.
- Proves: `an_mcp_request_is_refused_when_its_channel_is_off_and_nothing_reaches_the_browser`;
  `a_command_line_request_still_runs_when_only_mcp_is_off`; `a_switch_left_unset_is_on`;
  `the_schema_3_switches_migrate`; the Policy page renders and edits the switches; the refusal
  golden regenerated on purpose.

S3 also carries Decision 14a: `Attribution.claimed_agent`, the audit field and its golden, and
restored rows named by the same function as live ones.

Not built (decided above): the loopback-owner observer, the service-observed command-line owner.
Listed for the architecture sweep: client-written tab bindings.
