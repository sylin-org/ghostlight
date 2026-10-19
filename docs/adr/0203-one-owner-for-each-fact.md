# ADR-0203: One owner for each fact

Date: 2026-10-19. Status: Accepted. Decided by the coordinator on 2026-10-19 under the owner's
delegation of 2026-10-18 ("you're authorized to evaluate the proposals and decide. I want this
release to be an opportunity to streamline architecture aspects like this one.") and the owner's
leave to proceed autonomously ("You're authorized to proceed autonomously."). The two questions at
the end stay the owner's.

Decides ADR-0202's delegated item 5 (client-written tab bindings). What it amends, restores and
keeps is listed under What this supersedes and amends.

Paths without a crate are under `crates/orchestrator/src`, and `ui/` means `crates/orchestrator/ui`.
Line numbers are the `dev` tree at 5464865e. They drift while other batches land, so find code by
name.

## Context

The owner's objective for 2.0 is "to get the architecture right". The seams are the executables,
relay protocol 4, adapter protocol 4, policy schema 4, audit v2, agent entries, the install home and
the extension package. They stay stable by default and change only where a separation-of-concerns
analysis shows a meaningful change needs it (ADR-0197 amendment of 2026-10-18). The owner asked for
facts that are re-derived, duplicated, or held in a singleton. Eleven lenses read the live tree, and
two skeptics checked every finding against it. 102 findings held in whole or in part. Where the
skeptics corrected a finding, this record takes the correction.
`docs/tasks/arch-1/findings.md` maps every finding to its decision and slice, or to its entry under
Considered and not taken.

**What is sound.** One executor, one workspace aggregate, one governance facade, one browser port
and one completion path hold. ADR-0202's rebuild holds: one id from intake to audit, findings per
frame, required evidence. It has two exceptions, both below: a second audit writer for a step that
never started, and a refusal code spelled at each call site. The edge knows no verb and stamps
nothing. No bus, actor, extra process or crate has crept in. The relay is sound apart from one field
no peer sets.

**What is not**, by class:

1. **A tab id outlives the browser run that issued it.**
   - Chrome hands tab ids out again after a restart. The extension's `browserId` lives in
     `chrome.storage.local` and survives the restart (`extension/worker.js:327-331`).
   - The service knows this (`service/mod.rs:2077-2078`) but applies it only to the released pool,
     which `apply_browser_disconnect` drops at every reconnect, a worker restart included
     (`workspace/mod.rs:905-907`).
   - So a live workspace keeps its bindings through a browser restart, and `restore_tabs` binds a
     resume token's tabs by id alone (`service/mod.rs:1767-1771`). The extension takes custody of
     any id it is sent (`worker.js:268-281`, ADR-0196 Decision 5).
   - After a Chrome restart, an agent's handle can name the person's restored tab. `browser_tabs`
     lists its title and URL under the agent's handle, a read or screenshot lands on it, and the
     debugger attaches to it. No forged token is needed.
   - The worker forwards only top-frame commits (`worker.js:722-733`), so the URL the service last
     knew lags a page that changes its address without a navigation.
2. **A service end decides tabs by two rules, and the browser keeps custody a past service
   granted.** `LiveSession::close` releases tabs on any goodbye, the quit's own included, racing the
   process exit (`service/mod.rs:1699-1710`, `836-843`). The edge then resumes, and the resumed
   session rebinds the released, ungrouped tabs. Owned command-line workspaces are never released. A
   new service epoch leaves the extension's custody map and debugger retention in place, and turns
   focus emulation and the glow back on (`worker.js:625-668`), although ADR-0186 says a new epoch
   detaches custody.
3. **Wrappers skip the rules of what they wrap.** `Outcome::audit` strips a fill's read-backs only
   when `FormFilled` is the top variant (`language/audit.rs:147-168`), so a fill that leaves a dialog
   open, or whose `expect` falls short, writes field values into `audit.jsonl`. An ownership refusal
   makes another session's claimed name durable. A tab held after a refused landing answers
   "Ghostlight is paused on that tab." and points to a Resume the tray does not offer and that would
   not release the tab (`language/outcome.rs:1977`, `2042`). A duration wait cut short by its
   deadline says a command was sent and may have acted.
4. **One question, several answers.** When two layers grant disjoint hosts, the catalog's
   `could_admit`, the Policy page's reach and enforcement disagree. The command line's simulate and
   the window's preview replay recorded work two ways, and the preview ignores the channel switches.
   The organization's name has two carriers, and the page merges them. Tab close and tab listing are
   decided without looking at the tab's host.
5. **The adapter holds facts the authority owns, and the authority re-derives facts the adapter
   learned.** Which recording a workspace holds is kept on a browser connection. Diagnostics are
   bound to the documents' loader ids. Settle budgets and effect classes are written again in the
   adapter. A locator whose iframe reloaded reaches the authority as "Document access could not be
   verified".
6. **Install keeps several writers with different rules.** An update replaces Ghostlight's whole
   JSON entry and drops the client's own keys. The managed-policy signing seed gets no Windows
   protection. Whether a file is owner-only on Windows depends on which helper wrote it. The plugin
   is a third Claude Code registration the installer cannot see.
7. **Vocabularies are spelled in several places,** in Rust and JavaScript, and some words a person
   reads live outside `language/`.
8. **Leftovers the frozen seams would carry:** fifteen event names no 2.0 process writes, a
   completion gate that guards nothing, and an adapter path with no document guard.

**Public claims this tree breaks.** Five are false today, and slices gated candidate make them true:

- Audit holds no typed values or page text (`docs/trust/security-overview.md:71-73`,
  `data-flows.md:116-118`, `faq.md:139-141`). A fill wrapped in a dialog or an unmet expectation
  writes them (C1).
- "The client's own words are never written" (`docs/guides/siem-integration.md:99`). An ownership
  refusal writes another session's claim (C1).
- Every policy denial is attributed to authority, tier, grant and rule (`security-overview.md:56-57`,
  `tiers.md:12`). A never-touch refusal drops its layer (P2).
- Each terminal invocation produces an audit record, with `policy_seq` under managed governance
  (`faq.md:136`). A call during a quit writes none (C11), and a flow step that never started omits
  `policy_seq` (C19a).
- Ghostlight "never takes over a tab you opened" (`README.md:106`). After a browser restart it can
  (C4, with the residual D2 states).

## Decision

### 1. A remembered tab is a hint

A physical tab id names one tab only within one browser run. A binding the service keeps across a
gap in what it knows about a browser is pending until that browser confirms it. A gap begins when
the browser stream ends, when a hello registers that browser id again (whether or not the old
stream's end was seen), and when a resume token brings a binding back after a service restart.

- **The rule.** The workspace aggregate states it once. A pending binding is kept when the live
  browser has a tab with its id and either the browser reports the extension life the binding was
  made in (B1), or the tab shows the URL the workspace last knew. In the same life, the binding
  takes the live URL and title, and its generation moves on when they differ. Otherwise the binding
  is forgotten as closed, and the agent reads "no longer open" and reopens.
- **A pending binding is inert.**
  - It takes no commit, readiness or dialog event. A `TabClosed` forgets it.
  - It does not count as owned: `controlled_elsewhere` and the confirmation check other sessions
    against confirmed bindings only.
  - An `OpenTab` or `AdoptTab` result that carries its id in its browser forgets it, because a
    freshly assigned id proves it stale.
  - It stays in the one tabs map with its state, so no handle is minted twice.
  - It is never released through the browser. Release, quit, session end, reap and a failed
    confirmation forget it without a command. Only confirmed bindings reach `cleanup_released_tabs`,
    because the extension closes any id it is sent when preserve-tabs is off
    (`extension/lib/tabs.js:433-455`).
- **The released pool** follows the same rule instead of being dropped at a disconnect. Its reuse
  check (`work/navigation.rs:257-260`) is the same predicate.
- **Who confirms.**
  - The first call of a workspace that holds pending bindings confirms those of every connected
    browser, with one `ListTabs` each, before any tab selection or browser targeting.
    `browser_tabs list` does the same.
  - A call that selects a pending tab of a browser not yet connected waits for that browser, as a
    pinned session waits, and forgets the binding if the wait ends without it.
  - When the workspace binds another browser, the other browsers' pending bindings are forgotten,
    and the result says so.
- **The token.**
  - It is written from each binding's own browser, not from the workspace pin, so a call that
    completes before that browser returns keeps the tab in the token.
  - Each tab carries a digest of its `TabState.url`, bounded as the adapter bounds a tab's URL
    (`URL_CHARS`), so the edge holds no page text and the digest and the live comparison read one
    string. Each browser carries its life (B1).
  - It is republished when a binding is confirmed or forgotten, or when its URL changes, and it is
    bounded when written.
  - An older token without digests resumes with every tab stale. A token that does not decode still
    resumes an initialized session, at the oldest supported revision with no tabs, and writes a
    diagnostics line. The token is published with the `initialize` answer.
- **Deleted:** `McpWire.unrestored`, `restore_returned_tabs`, and `restore_tabs`' open-time
  `ListTabs` and pin. That was a second pending mechanism for the same fact.
- **The extension life (B1, package item 1).** The hello carries an optional `life`: an opaque
  id the worker mints into `chrome.storage.session` when none is there, beside the custody map that
  already lives there. Chrome keeps that storage across a worker restart and a native-host restart,
  and clears it on a browser restart and on an extension reload, update or disable. The same life
  therefore proves the same browser run, in which tab ids are never reused. A different or missing
  life proves nothing, and the URL decides. Why the seam: only the browser can observe whether its
  life went on (ADR-0109). Without it, the URL check forgets an agent's own tab whenever its page
  changed its address without a navigation the service saw, which a single-page app does on most
  clicks, at every update and every reconnect.
- **Seam:** none for the rule. The token is written by the service and opaque to the edge. Adapter
  protocol 4 gains one optional hello field, and only together with the extension.

### 2. One tab lifecycle

1. **One forget.** `forget_tab` removes a tab's targets, views, snapshots and images. It runs on
   Ghostlight's own close, on the person's close (`apply_browser_close`) and on a failed
   confirmation. Binding a handle at generation 0 clears whatever that handle carried before.
2. **The browser binding comes with the first tab.** A session's browser is bound when its first tab
   binds: an open, an adopt or a confirmed resume. A crossing that binds no tab never binds a browser:
   `browser_tabs list` and `browser_record` status, stop and discard.
   - A list in an empty session answers at once with `tabs: []`, the connected browsers and the
     `browser_navigate` next step. It never waits, starts a browser or binds one, because the answer
     is known without a browser.
   - A bound session is never rebound. In ADR-0114 Decision 2, "the first crossing binds it" becomes
     "the first tab binds it".
3. **A pinned wait reads its own browser.** Departures are kept per browser identity. The wait holds
   only while a connection or a recent departure of the same installed browser is evident, counting
   identities that reported no browser name. Another browser's comings and goings do not delay it.
   The unbound wait keeps the global evidence.
4. **A held tab can be left.** A held tab still refuses reading and acting. `browser_navigate` to an
   allowed URL and `browser_tabs` close may select it. Each is authorized on its own, apart from the
   page the tab shows (Decision 7.6), and `apply_landing` clears the hold. One helper holds a tab and
   emits `HoldEntered`, so a hold taken on the read path shows too.
5. **Tidy-up.** `WorkspaceSummary` carries the browser read under the same lock.
   `WorkspaceStore::owners` and the port's "most recently attended" accessors go.

### 3. One service end

- **When it runs.** After the drain, the quit thread takes over every session and ends it in this
  order, under one shared deadline:
  1. each MCP stream is handed its resume token;
  2. the MCP goodbyes;
  3. the release of unleased, confirmed bindings; a leased workspace is cancelled first, and
     released only if its call finishes inside the deadline;
  4. the browser goodbye.

  Session end and idle exit forget every binding without a browser command: session end has a 2 s
  fallback exit (`crates/relay/src/os/session.rs:46`), and at idle exit no browser stream is open.
- **Owned (command-line) workspaces** are released at every end, because nothing can resume them.
- **On `replace`:** an MCP workspace with a live connection stays bound for its edge's resume; one
  whose client left during the drain is released.
- **On quit, uninstall or an unknown reason:** every MCP workspace is released, and its token carries
  no tabs. A resumed session then reads honest stale handles instead of rebinding ungrouped tabs.
- **No double release.** `LiveSession::close` skips release once the quit has taken over. A flag set
  before the goodbyes decides this, not `is_quitting`.
- **The extension half (B2, package items 2 and 3).** It amends ADR-0186's epoch rule.
  - The extension keeps the service epoch in `chrome.storage.session`. On a new epoch it releases
    every managed tab through `controlledTabs.release`: the debugger is unretained, focus emulation
    and the glow go off, and the entry stays released until its next commit, where ADR-0186 cleared
    it. Custody comes back only through `manage()` on the next tab-scoped call (ADR-0168). Without
    this, a crashed service leaves the debugging bar on orphaned tabs until the browser restarts.
  - An extension the service refuses as outdated (`adapter_incompatible`) says to update Ghostlight
    in Browser, and stops restarting the native host until a hello is accepted.

### 4. One completion path

1. **One retained projection.** This restores ADR-0198's rule 3 (Rules that hold everywhere) and
   ADR-0202 Decision 14a. One `retained()` in `language/audit.rs` strips page-authored text and client
   claims: fill read-backs, dialog messages, the ownership owner, a credential control's name and the
   window's origin. It recurses into `DialogOpened` and `ActionUnconfirmed`. `Outcome::audit` and
   `Refusal::audit` render from it, with no fallback to the agent's sentence.
2. **Calls during a quit.** A call refused because Ghostlight is quitting is answered through
   `execute_prepared` as `CancelledBeforeStart { by: Quit }`, with an audit line. The cause carries
   the quit's reason: after a quit or a replace the next step is "Try the call again in a moment.";
   after an uninstall there is none. The protocol-error side path goes. No audit word is added.
3. **The step that never started.** Its audit line carries the policy sequence. `Effect` gets a stored
   name beside `Status`'s, and the not-started status is spelled once.
4. **Leftovers on the path.** `CompletionGate` goes, because single completion already follows from
   the structure. `WorkPhaseStarted.physical_id`, `TabCreated.tab` and `DocumentCommitted.tab`, which
   nothing reads, go. A `DomainEvent` producer cannot omit provenance. `AuditRecorder::record` becomes
   test-only. Two false doc comments are corrected.
5. **Recovered and already-closed tabs.** A recovered tab's `Outcome` is chosen in
   `settle_opened_tab`. An already-closed tab goes through `succeeded`, so its next step reaches the
   agent.
6. **The browser in the record.** The frame keeps the browser id with its name, and the audit line
   gains an optional `browser_id`, install-local and random. Two profiles of one browser are two
   browsers in the record. This amends ADR-0202 Decision 1: the reached browser's id joins the
   channel as a durable declared fact, bounded and opaque, never authority.
7. **What `allowed` means.** The doc comments on `allowed` and `InvalidRequest` say what is true.
   Separating lifecycle outcomes from governance outcomes stays ADR-0202 Decision 12's open question.

### 5. Refusals say what is true

1. **One model code per refusal.**
   - `Refusal::reason()` gives each variant its closed model code, and one `Terminal` constructor
     writes it.
   - Variants whose code varies carry a closed cause: `FilesUnreadable` gets a `FileProblem`; the
     export failure gets a closed word, with the browser's text moved to `detail`; `AuthorityBlocked`
     keeps its `ReasonCode`.
   - `browser_reason`, `unconfirmed_reason` and every call-site literal go.
   - A table test pins the (model code, audit tag) pair for each variant. Aligning the two spellings
     is a separate, visible choice.
2. **The adapter keeps the receipt's closed codes.**
   - `capture_too_large` reaches the existing `Refusal::CaptureTooLarge`, and the service's 7,000,000
     check reads the budget constant.
   - `debugger_unavailable` and `session_lost` become closed causes of `BrowserPrimitive`, with true
     next steps.
   - The page's `credential_authorization_required` becomes `Refusal::CredentialAuthorization`.
   - The browser's raw text stays only in `facts.detail`, and the audit keeps
     `browser_primitive_failed`. Other asset codes keep their present path; ADR-0198 Decision 29's
     fold stays after 2.0.
3. **Workspace refusals carry their own fact.**
   - A handle on another of this session's tabs names that tab (`handle_on_other_tab`).
   - A fill across two tabs says a fill works in one tab (`fields_on_several_tabs`).
   - A point outside the image gives the image's size (`outside_image`).
   - `TabUnavailable` is worded so it is true for a closed handle and for a handle never issued.
   - A flow does not inject its tab into a step whose handle names its own tab.
   - An ownership refusal names the owner the way `language::provenance` names its opener, and names
     no one for the unknown agent.
4. **Embedded documents.** A locator whose embedded document was replaced or removed is stale
   (navigated), decided where the adapter learns it. A `DocumentChanged` on a call that names one
   locator is treated the same.
5. **Held tabs.** This restores ADR-0198 Decision 29.
   - The sentence names the refused host and the calls that clear the hold (Decision 2.4).
   - The refusal records the denial reason of the landing that held the tab, not `RuntimeHold`.
   - A flow step on a held tab is `PolicyBlocked`, not `Paused`.
   - The person's hold words move to `language/`.
   - The skill's "do not ask the user to resume" stays, because it is true.
   - The catalog's "Pause/Stop" becomes "Pause or End session".
6. **Notices.** The in-page notice and the desktop toast are chosen from the typed `Terminal`, not
   from the tool name and a fact string.
7. **Duration waits.** A duration wait that the deadline cuts short is `DeadlineBeforeDispatch`. One
   that cannot fit in the time left is refused at once.

### 6. The authority owns its facts

The authority owns budgets, scopes, bindings and effect classes. The adapter translates and reports.

1. **Targeted reads.** A targeted read reports the URL of the tab's top document. The landing check,
   the observed host and the agent's `url` fact are therefore the tab's, and an embedded host never
   enters the durable record.
2. **Downloads.** A recording saved to Downloads carries effect, so a lost receipt reads as after
   dispatch.
3. **Diagnostics binding.** A binding is sent only when the authority asked to watch documents. In
   B2 (package item 4), the worker resets the ring only on a top-frame navigation.
4. **Budgets as values.**
   - `query_semantic` carries its maximum, as `find` does.
   - The listing's cut crosses as a flag, so the selector refusal says "at least N".
   - Screenshots get their settle budget from the authority's one rule.
   - `MAX_WAIT_MS` is the only settle bound.
   - The inline-file ceiling is one constant, checked once at intake.
5. **The document tree.** It crosses typed. Its node kind is decoded once, in `world.rs`, and its
   bounds are one constant sent as `max_nodes`.
6. **No unguarded path.** `reach()` refuses a content command outside `InDocuments`. The unguarded
   path and its dead helpers go.
7. **Recordings belong to the workspace.**
   - `WorkspaceState.recording { browser, id, saved }`.
   - Recording commands carry the id, and the held-until-saved rule lives in `work/recording.rs`.
   - `BrowserPort::call` drops its workspace parameter. This amends ADR-0202 Decision 23, which listed
     the port as unchanged. The parameter existed only so the adapter could hold this fact, and its
     callers pass placeholders.
8. **Coverage.** Composition reads a step's coverage from `terminal.audit.coverage()`. One "withholds
   anything" predicate serves composition and the lean view, and the `coverage` key is one constant.
9. **The replay's answer.** Package item 6 is declined (Decided under the owner's delegation), so the
   2.0 extension's sentence stays pinned by `wire::PAGE_REFUSED_REPLAY`. A refused replay that
   carries the page's own closed code, with the sentence compare gone, waits for the next extension
   revision.

### 7. One governance answer

Governance answers from one snapshot with one evaluator, each decision names its layer, and each
check is on the frame.

1. **One reach evaluator.**
   - `AuthoritySnapshot` computes the cross-layer answer for a requirement set through the snapshot's
     full host decision, never-touch hosts and the workbench's own origin included, over a finite
     probe set: every exact pattern, one fresh label under each suffix wildcard (authored or
     never-touch), and one host matching no suffix.
   - `could_admit` is "reach is not unavailable", and the catalog also drops `browser_screenshot` when
     screenshots are off.
   - `effective.rs` and `note()` read the same answer, and the disjoint case gets its own sentence.
2. **Never-touch refusals.** An authored never-touch refusal is a policy decision with its layer,
   `PolicyRule::SacredHost` and a denial id. Only non-HTTP(S) schemes, localhost, loopback and
   link-local stay Ghostlight's own ceiling, credited to Ghostlight with no authored layer. This
   amends ADR-0122 Decision 2, item 4. (Amended 2026-10-19: the ceiling is non-HTTP(S) schemes and
   the workbench's own origin; see the amendment at the end.)
3. **One replay.** `AuthoritySnapshot::replay(channel, requirements, host)` checks the channel first,
   treating an undeclared channel as MCP, then the host. The command line's simulate and the window's
   preview both call it.
4. **One organization identity per publication,** resolved with ADR-0122 Decision 3's precedence: the
   signed presentation wins and the manifest fills what it lacks. The snapshot, the chip,
   `channels_off`, the passport and the status file all read it.
5. **One publication read.** The executor keeps the whole publication beside its snapshot, so
   `policy_explain`'s facts and the audit's authority id come from the same one. The chip is built
   from the compiled authority. The audit requirement is resolved once. `EffectiveAuthority` carries
   the authority id.
6. **Tab close and listing.**
   - Closing a tab checks Action on the tab's host, then the tab-close setting.
   - Closing a held tab, like undoing Ghostlight's own denied landing, undoes Ghostlight's refused
     landing and checks only the setting (Decision 2.4).
   - `browser_tabs list` withholds the title and URL of a tab on a host the policy refuses Read on,
     and names that host, as the held-tab refusal does.
7. **Checks on the frame's own path.** Single-stage handlers authorize `context.requirements`. A
   recording save checks each source URL once, through the frame's `authorize_landing`.
8. **Tidy-up.**
   - The unsigned managed file path goes.
   - The setting keys become one enum, and a test pins the page's copy of them.
   - `level` stays in schema 4, and the guide says both levels set a ceiling.
   - The policy sentences move to `language/policy.rs`.
   - An owned user policy moved to schema 4 at load is written owner-private.
9. **The act table.** `governance::offers(state)`, beside `apply_intent`, names the acts a runtime
   state accepts. The tray, the controls part and `apply_intent` read it.

### 8. One home per vocabulary

A vocabulary that crosses a boundary has one home, and words a person or agent reads live in
`language/`.

- **Wait conditions.**
  - One closed `language::WaitCondition`, decoded by `TryFrom` inside validation so the curated
    refusal words stay. It generates the catalog branches, and the catalog golden stays
    byte-identical.
  - It carries methods for the default settle, absence and whole-document scope.
  - `Outcome::Waited` matches it exhaustively, so a duration wait gets its own sentence.
  - `BrowserCommand::Observe` carries the subset the page evaluates.
- **Activity and denial words.** One table in `language/presentation.rs`. The page caption draws the
  label the service sends. The window keys its glyphs on the closed activity code, not on English
  words.
- **Crossing names pinned.** Presentation kinds and activities, area words, left-out tiers and
  recording stop reasons are pinned by tests that read the JavaScript.
- **Runtime words** are pinned to the window and the popup. ux-2 u6 already aligned them on "Start a
  new session".
- **Diagnostics.** The edge's components are spelled once, by `relay::log`, and fifteen unused event
  names go.
- **Words moved into `language/`:** install words to `language/deployment.rs`, search words to
  `language/workbench.rs`, policy sentences to `language/policy.rs`. Doctor's "authority" line speaks
  of the service. The browser-family list is composed once from the browser table.

### 9. One name per opener

- **The claim.** It is `Option<String>`, bounded once in `provenance.rs`. `DEFAULT_CLIENT_LABEL`
  ("MCP client") and its decode go.
- **One name for one opener.** The opener's agent, as `language::provenance` names it, is read once
  at workspace open (ADR-0202 Decision 15's opener rule). It is the one name for:
  - the tab-group title, composed in `language/`: "Ghostlight - Claude Code", "Ghostlight - Command
    line", and "Ghostlight - Unknown agent" for an opener the catalog cannot name;
  - the ownership refusal;
  - the window;
  - the popup, through its existing title parse, which then reads the right name.
- **Reuse key.** Released-tab reuse is keyed on the claim-derived agent, `Agent::of(claim,
  channel).id`: the command line for every command-line session, the catalog id for a catalog claim,
  and an unknown agent under its claimed name otherwise. A claim that names no agent (`Agent::unknown`) gives no key, so a
  claimless session reuses no kept tab and no one reuses its tabs. The observed source is not in the
  key, because ADR-0202 Decision 1 says the source decides nothing. It may shape the group title,
  because display decides nothing.
- **Comments.** The comments that justify the key with "never regrouped" are rewritten to give
  ADR-0201 Decision 2's reason.
- **For the window.** `ConnectionDetails` carries the source's catalog agent, not serialized, so the
  window can mark the right host row as in use.

### 10. One owner per registration

1. **Who owns which keys.** Three classes:
   - owned keys Ghostlight sets: the command and its arguments, plus `type` where a dialect needs it;
   - required members Ghostlight merges into a list that is otherwise the client's: Codex's
     `env_vars` on Linux;
   - defaults inserted only when absent: `env: {}`, `tools: ["*"]`, `enabled: true`.

   Every other key is the client's and is kept. The JSON writer edits an existing owned entry in
   place. The full entry is written only when there is none. Fix still replaces a foreign entry
   (ADR-0154). One "current" predicate, inspection's, checks the first two classes and decides
   whether install is a no-op.
2. **Owned-entry test.** One predicate (Current, Updatable or Retired) decides whether Ghostlight holds
   an owned entry, for uninstall too.
3. **The npm launcher.** Ghostlight's own npm launcher entry is recognized as `Launcher { pinned }`.
   An unpinned one says that npm chooses the version at each launch and is never shown as current,
   because the service cannot ask npm (ADR-0028). One pinned to 1.x needs attention. Reconcile still
   leaves the entry alone (`install/reconcile.rs:8-9`), and Fix points it at the installed edge.
4. **The roster cache** is written only under the action lock, and one command reads the registry
   once.
5. **The shell.** A same-version install treats a missing or changed shell as damage and puts it back.
6. **Carriers.** Both launchers order numeric pre-release identifiers by number. A test pins their
   restore words to the service's. The get scripts finish through the downloaded service.
7. **The plugin.** The RC notes tell plugin users to disable it until the cutover. The unverified
   precedence claim is withdrawn. The skill keeps only guidance the catalog and the server
   instructions cannot carry, and a test checks it against the catalog. RELEASE.md's plugin step is
   the one place its version is checked and bumped. Whether the plugin keeps a server is question 1.

### 11. One way to write files

1. **One creator.** `install::record`'s `create_private` (a protected DACL for the user and SYSTEM on
   Windows, 0600 on Unix) is the one creator for a file Ghostlight makes that holds a secret or a
   record about the person: the keygen seeds, a new audit log, harness locations and the owned
   policy. Existing files keep the access they have; nothing is re-protected at each start. The 1.x
   migration keeps its rename, because the 1.x data sits under `%LOCALAPPDATA%`, which already closes
   it to other users.
2. **One atomic replace,** with stated options: private or keep access, backup, write through a link
   or refuse one. `atomic_replace` and `atomic_replace_keeping_access` stay its named entry points,
   so no caller outside install changes. It uses one temporary-name pattern. The paths compare by one
   same-location rule.
3. **win-peer** gets one token module for its two callers.

### 12. The window draws decisions

1. **The tray** offers Resume only while paused. The band and Settings read the offered acts from the
   controls part.
2. **History rows** carry one closed outcome class computed in `language/` from the typed record:
   succeeded, refused, kept, needs you, failed, unknown or cancelled. A credential refusal is "needs
   you", not Kept.
3. **Counts.** The connections part carries the counts the service computed, which count connections,
   not sessions.
4. **One gather.** Every part is built from one gather, and the flat `WorkbenchSnapshot` goes.
5. **Setup** starts browsers through recovery's one launch function and records each start in the
   port's launch guard, so recovery never opens a second window. It matches its rows by the port's
   browser-id rule.
6. **The shell's copy.** The shell keeps its own copy of the desktop frames (ADR-0199 Decisions 3 and
   7), and its tests round-trip every golden.
7. **A window that cannot be built.** The shell reports `no_webview` only from preflight, and
   `window_failed` for a build error while the runtime is present. The service words both, the tray
   tooltip included.

### 13. Leftovers go

- **`Opening.reason` stays in relay 4.** No peer sets it and no reader reads it.
  `docs/design/relay-protocol-4.md` says so, and that the start reason travels only as
  `serve --reason`. `Opening.origin` stays too: only the edge learns which extension identity opened
  the port.
- **Dead names and paths go:** the unused diagnostics events; `CatalogChanged`'s unread fields;
  `desktop_ready=true`; the `eprintln!` to a null stderr, which becomes a diagnostics line;
  ARCHITECTURE's "restart-local adapter epoch"; and, in B2, the policy-shaped network filter and
  the recording and queue workspace keys.
- **`HARNESS_DETACHED`** names the connection id that `HARNESS_ATTACHED` named.

This ADR changes no relay or edge source. A later change there bumps `RELAY`, as the connectors
guard records: it locks `crates/relay` and `crates/edge` to the newest `v[0-9]*` tag at relay
revision 4, prereleases included (`tests/guards/connectors.test.mjs:210-224`). That bump is a
compatibility cost weighed on the change's merit, never a deadline.

## The tab-bindings decision (ADR-0202 delegated item 5)

ADR-0202 left three client-written bindings for this sweep: the unsigned resume token's tab list,
released-tab reuse by the opener's claimed label, and the command line's process marker.

**Who can write a binding:** the edge, which is the same user, holds the token in memory and never
reads it; any same-user program that speaks relay 4, since ADR-0199 Decision 5 admits any same-user
peer at medium integrity; and the browser, which hands tab ids out again after a restart.

**What goes wrong:**

1. **Honest staleness.** A genuine token, or a live binding, outlives the browser run that issued its
   ids. This is the common case, and it is the one that hands an agent the person's tab.
2. **Forgery.** A same-user program writes a token that names the person's tab.
3. **Impersonation.** A client that claims another client's name adopts the tabs that client kept.

**Decision:**

1. **The token stays unsigned and opaque,** a hint and never authority. Decision 1 closes case 1;
   see Considered for signing.
2. **Case 2 stays ADR-0199's accepted same-user risk, now narrower.** A crafted token binds only a
   tab whose id shows the URL it names, or, carrying the browser's current life, a tab of the current
   run. The life lives only in the browser, the service and the token in the edge's memory, and a
   program that can read those can already act as the user. A same-user program can also open the
   same site in a new tab with the person's cookies; what a forged token adds is an existing tab's
   in-page state. ARCHITECTURE and docs/trust say so. No trust claim weakens, because none promises
   tab ownership against same-user programs.
3. **Released-tab reuse keyed on the claim-derived agent (Decision 9)** narrows presentation between
   well-behaved clients and is never authority. The adopt loads the page in the kept tab, so the
   origin's sessionStorage and the tab's history cross; cookies are profile-wide anyway. Case 3 is
   the accepted same-user risk, and docs/trust says what crosses. A client that claims nothing
   reuses nothing.
4. **The command-line marker stays.** Its process id plus start time names one live process
   (ADR-0106). A forged marker is the same accepted same-user risk as case 3 (ADR-0199 Decision 5).

## Seams

**Seams that change, each for a separation-of-concerns reason:**

- **Adapter protocol 4 and the extension package,** item by item as decided under the owner's
  delegation. Item 1 adds the hello's optional `life`, additive under the tolerant readers
  (`wire.rs:7-9`). Items 2 to 4 change the extension's own behavior, not the protocol, and item 7
  removes dead code. Items 5 and 6, the notice's optional `agent` and the export failure's page
  answer, are declined. The reason: only the browser observes its own life (ADR-0109).
- **Audit v2:**
  - Additive: an optional `browser_id`; the `policy_rule` word `sacred_host`; the
    `workspace_unusable` cause words `handle_on_other_tab`, `fields_on_several_tabs` and
    `outside_image`.
  - Values that become true: summaries and `refusal_facts` carry no page text and no other client's
    name; a held-tab refusal's reason is the landing's denial; a step that never started carries
    `policy_seq`; a targeted iframe read's host is the tab's.
- **Agent entries:** the writer edits an existing owned JSON entry in place. Bytes change only for
  entries that carry the client's own keys. The npm launcher entry is recognized at inspection, with
  no write.
- **The browser port,** in-process: `call` drops its workspace parameter (ADR-0202 Decision 23,
  amended).
- **The resume token,** written by the service and opaque to the edge: a URL digest per tab, a life
  per browser (B1), no tabs when the session ends at a quit, and a bound on its size. The read stays
  tolerant.
- **The desktop contract,** same-build: the shell reports `window_failed` beside `no_webview`, and S7
  round-trips every golden.
- **Model surface,** which is not a frozen seam: refusal sentences and codes, held-tab words, the
  empty list's answer, and the catalog's "Pause or End session". The catalog and result goldens are
  regenerated on purpose.

**Seams that stay:** the three executables; the edge, which knows no verb and stamps nothing; relay
4, every frame and field; policy schema 4, `level` included; the install home layout, with no new
file; the governance facade's inputs; the MCP result shape.

There is still one executor, one workspace aggregate and one completion, with no bus, no new crate
and no new process.

## Considered and not taken

- **Signing the resume token,** with a key in the home or in memory. The forger can read a home key.
  A memory-only key defeats the token's one purpose, surviving a restart. Neither helps case 1, where
  the token is genuine.
- **A managed flag on every tab fact.** After B2's epoch release no tab would be reported managed
  after a replace, so nothing would restore, and it would need a field on every `ListTabs` entry. One
  life per browser on the hello is smaller.
- **A browser run minted at `chrome.runtime.onStartup` and kept in `chrome.storage.local`.** It would
  also catch a restored tab with the same id at the same URL. But the worker connects at load, before
  `onStartup` is dispatched, so the first hello after a start can carry the old run, and a stored run
  says nothing after a reload. The life cannot race, and the restored-copy case stays the stated
  residual.
- **Forwarding same-document navigations from the extension** to keep the URL current. It adds an
  event for every history change of every managed tab, and still misses a page that routed itself
  while the stream was down.
- **The observed source in the reuse key,** and dropping the claim from reuse. The first lets the
  source decide something, which ADR-0202 Decision 1 forbids, and splits one client's pool. The
  second lets any session adopt any kept tab.
- **Removing `Opening.reason` from relay 4 (E-04).** It changes locked relay source and five fixtures
  for a field that decides nothing. A line in the relay design says the truth.
- **A per-check replay of recorded traces.** An admitted whole-page read records refused frame
  checks, and embedded checks carry no host. So the active policy previewed against its own history
  would report refusals.
- **A new `Refusal::ShuttingDown`.** `CancelledBeforeStart { Quit }` already says this and adds no
  audit word.
- **Moving the desktop frames into `relay` (V-10).** That puts a same-build contract under the relay
  source lock, so every tray or view change would need a relay revision.
- **Merging the relay's token code with win-peer's (G2-03).** Relay depends on no workspace crate
  (ADR-0199 Decision 3), and the merge would change locked source.
- **One shared state vocabulary across the five install registries (G3-05).** They model different
  states, and those states drive behavior.
- **Dropping or loosening schema 4's `level` (G-10).** That is a schema change with no
  separation-of-concerns reason.
- **Flipping invalid requests to `allowed: true` (G-11).** The Policy preview would count typos as
  work that ran. ADR-0202 Decision 12 left this open on purpose.
- **The MCPB launcher delegating to its bundled service, and npm comparing versions for equality
  only (A-06).** The bundled 2.0 service would stay alive as the parent, and its stricter reader could
  downgrade a newer set. npm would download on every launch over a newer install.
- **Get scripts that derive the image set from `SHA256SUMS`, or install only through the carrier rule
  (G4-06).** The set is a named seam, and the carrier rule would skip the payload and development
  refusals.
- **Tab tombstones** to tell a closed handle from a never-issued one (T-06). The recovery step is the
  same, so a true sentence is enough.
- **A typed coverage field on `Terminal` (W-08).** The typed value already travels in the audit
  projection.
- **One holder for the agent-icon cache (A-10).** The global cache and the facade's holder are set
  once, at the same point, before the window server binds, and hold one `Arc`. One holder would
  thread the cache through three lanes and fix nothing a person or agent can hit.
- **Removing the development-layout shims (A-11).** `RetiredDevelopment`, `EARLIER_DEVELOPMENT_RELAY`
  and the development directory names came with the owner's ADR-0199 amendment of 2026-10-15. They
  cost a few file checks per install, and they stay until 2.x.
- **Splitting the roster into an agent table and host rows, and dropping `product_id` (G3-10).**
  Deferred to 2.x. A test keeps the Cline rows equal, and dropping `product_id` would amend ADR-0125
  Decision 1 and ADR-0148 for a field that decides nothing today.
- **Beta, Dev and Canary channels (G3-09).** Each is its own installation and profile, and naming the
  connected one would reach the adapter hello. 2.0 states the stable-channel bound (I10, D2).
- **Folding the bridge crate (V-11).** It touches imports in every lane with no behavior change, and
  it needs an ADR-0145 amendment. I12 removes its dead names.
- **Re-protecting existing files at each start (G2-02).** It would strip grants made on purpose, such
  as a log collector's account, and add an unsafe call.
- **Copying the 1.x data on Windows in the migration.** The rename keeps `%LOCALAPPDATA%`'s access,
  which already closes it to other users. A copy and delete adds partial-copy and disk-space failures
  to a one-time step.
- **One tier vocabulary (G-09) and one table of home file names (V-09) in this batch.** Neither
  changes a byte anyone reads. They are refactors for whoever next touches that code.
- **The extension's private copies of `protocol.js` members (X-09).** C18's pins catch a crossing
  name that drifts. The copies change no behavior, so they wait for the next extension revision.
- **Finding Ghostlight's entry under a key the person renamed (G3-03).** Ownership stays the key plus
  the edge's file name. Reading every key's command would take a person's own wrapper for
  Ghostlight's.
- **An integrity check that fails when the plugin differs from `main` at main's version (G4-05).** It
  would fail every plugin edit on `dev` until an early bump, and make a local docs check depend on
  remote-tracking state. RELEASE.md's plugin step does the job once.
- **Closed enums for `on_error`, `Record.action`, `HandleDialog.action` and Diagnose's fields.** They
  fix no defect a person or agent meets.

## Consequences

**For people:**

- After a browser restart, an agent's handle never reaches a tab showing a different page; it reads
  "no longer open" and the agent reopens. A tab at the same address that received the recycled id is
  the stated residual.
- An update keeps a session's tabs bound and grouped, apart from the cases under Risks.
- A quit releases a session's tabs every time, as preserve-tabs says. Command-line tabs no longer
  keep the debugging bar after Ghostlight quits; through B2, neither do a crashed service's tabs.
- Closing a tab needs Action on its host. Under a read-only policy an agent cannot close its own tab
  on a host it may only read, and the tab stays open for the person. A tab Ghostlight's refused
  landing left held can always be closed under the tab-close setting.
- A form value never reaches the audit through a dialog or an unmet expectation.
- The tab group and the popup name the agent the window names. After the update, each client gets
  one new group, and ADR-0201 deletes the old one once it is idle. A group Chromium already saved
  keeps its old title until the person removes it.
- History says "needs you" for a credential refusal. The tray never offers a Resume that does
  nothing.
- Updating Ghostlight keeps the agent's own settings in its client config: disabled, approvals, tool
  filters and environment.
- The managed-policy seeds are owner-only on Windows.
- The Policy page, the catalog and enforcement agree. The preview honors the channel switches. An
  organization's never-touch refusal names the organization.

**For agents:**

- Refusals say what is true: held tabs, a handle on another tab, a point outside the image, an
  embedded document that changed, a duration wait that sent nothing, a capture too large, a debugger
  refusal.
- `browser_tabs list` in an empty session answers at once and binds no browser, so the browser it
  names can still be chosen.
- A tab on a refused host is listed without its title or URL, and the refused host is named.
- A selector that matches many controls says "at least N".
- The catalog text changes by one phrase. Under a policy that admits a tool nowhere, such as disjoint
  layer grants or screenshots off, that tool is no longer listed. The result shape is unchanged.

**For audit readers:** the line's shape is unchanged. One optional field and four words are added,
the values listed under Seams become true, and the SIEM guide says so.

**What is deleted.** Each slice lists what it deletes. In sum: the second pending mechanism for
resumed tabs, the release that races the goodbye, the quit's protocol-error path, `CompletionGate`,
the host-blind close and listing checks, two replays, two evaluators, two organization carriers, the
connection-held recording index and the port's workspace parameter, four copies of the activity
words, the second wait vocabulary, `DEFAULT_CLIENT_LABEL`, two private file writers, the unsigned
managed file path, fifteen event names and `WorkbenchSnapshot`.

**Risks:**

- **URL confirmation fails closed.** A page that navigated itself while the stream was down, or
  after the last token publish, goes stale and the agent reopens it. With the life (B1) this happens
  only after a browser restart or an extension reload, update or disable.
- **Same URL, same id.** After a browser restart, the URL check cannot tell two tabs at one URL
  apart, for example a restored copy of the agent's page, and that tab is bound. docs/trust states
  it.
- **If item 1 is reversed** at validation, every update and reconnect forgets a tab whose page
  routed itself, and that tab keeps the extension's custody and the debugging bar until B2's epoch
  release or a browser restart.
- **Release on a new epoch.** Through B2, a resumed session's tabs lose the glow and focus emulation
  until its next call. Tabs of a crashed service, or of a session that never resumes, keep
  Ghostlight's group until the person closes them.
- **Agent-entry bytes.** Decision 10.1 changes written bytes for entries that carry the client's keys.
  The writer golden pins every dialect.
- **Behavior changes for agents.** Decisions 5 and 6 change what agents see. The result goldens are
  regenerated on purpose, and the flow and browser journeys run against a fresh build.
- **The candidate waits on its candidate slices,** and core's are the longest path. A slice gated
  none never holds it.

## What this supersedes and amends

- **ADR-0202, delegated item 5:** decided here.
- **ADR-0202 Decision 1:** the reached browser's id joins the channel as a durable declared fact,
  bounded and opaque, never authority.
- **ADR-0202 Decision 15 and S4:** "unchanged in behavior" becomes: the tab-group title reads the
  opener's agent, and reuse reads the claim-derived agent.
- **ADR-0202 Decision 23:** the browser port drops its workspace parameter.
- **ADR-0197 Decision 12:** the token's tabs are pending until the browser confirms them, and a quit
  hands out a token with no tabs.
- **ADR-0196 Decision 12, amendment of 2026-10-17:** a pinned wait reads its own installed browser's
  evidence, not the evidence the unbound path reads.
- **ADR-0114 Decision 2:** the binding is taken when the first tab binds.
- **ADR-0066 Decisions 1 and 2, and ADR-0201 Decision 2:** "the client label" becomes the opener's
  agent name. A client reuses only tabs its own sessions kept, as a courtesy between clients, not for
  a reason about groups.
- **ADR-0122 Decision 2, item 4:** authored never-touch destinations are the authoring layer's
  decision; only non-HTTP(S) schemes and the workbench's own origin (ADR-0199) stay Ghostlight's
  ceiling. Localhost, loopback and link-local follow policy under ADR-0155 (corrected by the
  amendment of 2026-10-19).
- **ADR-0144:** the precedence claim is withdrawn. The skill carries no bounds.
- **ADR-0145 Decision 4:** the unused event names go. Folding the crate is left for 2.x.
- **ADR-0186, through B2:** a new service epoch releases custody and keeps each entry released until its
  next commit, where it cleared the entry.
- **Restores** rules the tree broke: ADR-0198's rule 3 and ADR-0202 Decision 14a (Decision 4.1), and
  ADR-0198 Decision 29 (Decision 5.5).
- **Keeps** ADR-0106, ADR-0109, ADR-0122 Decision 3, ADR-0125 and ADR-0148 (`product_id` stays),
  ADR-0154, ADR-0168, ADR-0185, ADR-0199 (Decision 5's opening and same-user boundary included) and
  ADR-0200 Decision 16.
- **`docs/1.0` contracts, the guides, README and docs/trust:** each changes in D2, after the code that
  makes it true.

## Decided under the owner's delegation

The sweep left three questions for the owner. The owner delegated architecture decisions on
2026-10-18 and gave leave to proceed autonomously on 2026-10-19. The coordinator decided the first
question, whether to reopen the 2.0 extension package before the store submission is staged
(ADR-0200 Decision 16), item by item as this record recommended. The 2.0 package is not staged with
any store, so a packaged change costs one more review and a regenerated package record and no
resubmission (AGENTS.md; ADR-0200 Decision 16). The owner's direction is that a seam changes when a
separation-of-concerns analysis needs it, and only the browser can observe its own life (ADR-0109).
The owner can reverse any item at validation. B1 carries package item 1. B2 carries package items
2, 3, 4 and 7.

1. **The extension life in the hello (B1, with C21): yes.** After an update or a reconnect, an agent
   keeps its tab in a single-page app instead of opening a second one while the first keeps the
   debugging bar. If reversed, the URL check alone confirms (Risks, "If item 1 is reversed").
2. **Custody released on a new service epoch (B2): yes.** After a service crash, the debugging bar
   leaves the orphaned tabs. If reversed, a crash leaves the debugging bar on orphaned tabs until the
   browser restarts.
3. **The outdated-extension message and the native-host restart stop (B2): yes.** An extension the
   service refuses as outdated says to update Ghostlight in Browser, and stops restarting the native
   host twice a second. If reversed, a frozen 2.0 extension that meets a later protocol tells the
   person to update the wrong side.
4. **The diagnostics ring reset only on a top-frame navigation (B2): yes.** An iframe reload no
   longer clears a tab's console and network history. If reversed, an iframe reload still clears
   them.
5. **The notice's agent: no.** Nothing new for a person: once C13 makes the group title right, the
   popup's title parse names the agent the window names. The popup keeps its title parse.
6. **The replay's page code: no.** Nothing in 2.0: both sides ship together and the sentence is
   pinned by `wire::PAGE_REFUSED_REPLAY`. A refused replay is still recognized by its English
   sentence until the next extension revision.
7. **The dead filter, hooks and keys: they ride with B2.** They have no runtime effect, and B2
   already opens the package.

The cost is one more review of packaged files and one regenerated package record, joining the review
ux-2's e1 already needs. The record has been regenerated four times since 49588d96.

## Questions for the owner

Question 1 is asked when D0 lands. If its answer has not come when the other candidate slices are
in, the candidate takes its decline path.

1. **What should the Claude Code plugin register?**
   - Option A: keep its `npx -y ghostlight` server, documented as "the plugin or `ghostlight install`,
     not both".
   - Option B: ship a skill-only plugin that points at `npx -y ghostlight install`, so the installer's
     entry is the only registration.
   - The plugin is a public channel with two directory submissions, one of them vendored by Z.ai.
   - Either way, the RC notes tell plugin users to disable it until the cutover, because npm still
     serves 1.3.14, which would put 1.x back over 2.0.
   - Recommendation: a live check of whether Claude Code then starts both servers, on a dedicated
     test machine with a 2.0 build installed and the plugin enabled, never on the owner's machine.
     If both start, ship skill-only. Without an answer, the candidate keeps the server and the RC
     notes' line, and the double registration after the cutover stays a known issue.
2. **When is the candidate tagged?**
   - Option A: one candidate, tagged once ux-2 and this batch's candidate slices are in. The install
     proofs run once.
   - Option B: `v2.0.0-rc.1` when ux-2 lands, so the install proofs start now, and the cutover from a
     later rc that carries these slices. D1 then makes RELEASE.md and the store disclosure say the
     cutover comes from the last rc, and the pinned install and 1.x upgrade proofs run again on it.
   - Recommendation: A, because the candidate slices fix what the RC's testers would meet. B only if
     a proof needs a published tag sooner.

## Rebuild plan

**Slices.** Each slice is one signed conventional commit, or two where marked, and leaves a green
tree. Each runs the checks AGENTS.md's Building and testing names for what it touches, the
connectors guard for any `tests/golden/` change included, and names the update variable of each
golden it regenerates. A slice that changes `tests/golden/agents/native/` has `linux.txt`
regenerated on the Linux test machine before the push. Line numbers, order and splits are hints: the
coordinator may re-order, split or re-gate a slice in the ledger under the gate rule without amending
this record. Only a changed decision needs an amendment.

**Gates.**

- `candidate`: in the release candidate the cutover uses. RELEASE.md cuts `v2.0.0` and stages the
  store package from that candidate's commit, so nothing lands between the rc tag and `v2.0.0`. A
  slice is gated candidate when it fixes a harm a person or agent meets in 2.0 (every high finding),
  makes a public claim true, changes a seam whose later change costs compatibility (agent-entry
  bytes, the extension package, the adapter hello), or acts once at the 1.x upgrade.
- `none`: lands on `dev` when ready, before or after the candidate, and never holds it. This is every
  slice whose effect is internal or small: 2.0 settles the seams, and 2.x rebuilds internals lean
  behind them (STATUS, Release bar).

**Lanes:** docs, core, adapter (`browser/adapter/**`), policy, surface, install and package.
`docs/tasks/arch-1/BOOTSTRAP.md` (D0) assigns every file a slice edits to one lane, the journeys and
`tests/lib/**` to core, and carries the one-checkout rules: a lane builds in
`.target-lane-arch-<lane>`, stages by path, never stashes, and puts its ledger row in the slice's
commit. Three rules bind the plan:

- **ux-2 first.** A slice that edits a file any ux-2 commit touched starts after ux-2's ledger closes
  (u7, the j-32 tooltip, the integration gate and the push). So core, policy, surface and package
  start then, and adapter, docs (D0's ADR and task files) and the install slices outside
  `install/mod.rs` and `agent/mod.rs` start at once.
- **Cross-lane changes.** A slice that removes a field, changes a signature or trait method, or
  deletes a name that another lane's file uses, first greps the name across `crates/`. It edits those
  uses in the same commit, and lands when that lane has no open slice in those files.
- **Journeys.** Lanes run the cargo and node tiers. A slice writes or extends the journeys it needs.
  The coordinator has every journey that starts its own service (process, installer, live browser)
  run on test-01, test-02, test-03 or leo-desktop-02, or in CI after the push, from a fresh build
  named by `GHOSTLIGHT_BIN_DIR`. On the owner's machine only unit tests, the headless
  workbench-history journey and the in-place deploy through `scripts/dev-loop.ps1` run (MEMORY, One
  live Ghostlight).

**Integration and the candidate.**

- At each lane's close, the coordinator integrates, has the journeys run as above, deploys in place
  on the owner's machine, checks live and pushes. CI goes green on both systems.
- When the candidate slices are in, the owner gets a try-list on the deployed build: restart Chrome
  mid-session, and the agent's handle reads "no longer open"; update, and a client's own keys survive
  in its config, and a single-page app's tab stays bound; quit, and command-line tabs lose the
  debugging bar; End session, and the tray offers no Resume; a credential refusal reads "needs you"
  in history.
- Before the candidate tag, the Build release candidate rehearsal and
  `scripts/check-release-candidate.ps1` run again on the candidate commit, which `v2.0.0` is then
  tagged on.

**Order,** candidate slices first:

- docs: D0, D1, then D2 as slices land.
- core: C1, C2, C3a, C4, C5, C6, C7, C13, C9, C10a, C10b, C11, C12a, C19a, C21 (after B1); then C3b,
  C3c, C12b, C10c (after P1 and P5), C16, C17, C18, C19b, C19c, C20 (after I9b) and C22.
- adapter: A1, A2, then A3 and A4.
- policy: P10, P1, P2, P3, P4, P9, then P5, P6, P7 and P8.
- surface: S1 (after P10), S2 (after C12a), S4a (after P3 and P4), then S8, S6 (after C3c and C13),
  S3 (after P5 and I6), S4b (after P7 and P8), S5 (after C12b and C18) and S7.
- install: I1, I3, I7, I9a, I9b, I11 (after question 1), then I4, I5, I2, I6, I8, I10, I12 and I13.
- package: B2, B1.

### docs

**D0 -- docs(adr): one owner for each fact.** Two commits. Gate candidate.
- Changes:
  - this ADR and its README row;
  - pointer lines in the headers of ADR-0066, 0114, 0122, 0144, 0145, 0196, 0197, 0201 and 0202, and
    ADR-0186's when B2 lands;
  - `docs/tasks/arch-1/BOOTSTRAP.md`, and `findings.md` with one row per confirmed finding (id,
    title, severity, verdict, decision, slice or Considered entry), and one ledger per lane;
  - after ux-2's closing commit, the second commit: the row in `docs/tasks/README.md`, and STATUS
    says the batch started.
- Deletes: nothing.
- Proves: the integrity script.

**D1 -- docs(release): the release candidate names the plugin, and LANGUAGE names where a selector
works.** Depends on D0. Gate candidate.
- Changes:
  - The RC and 2.0 notes tell plugin users to disable it until the cutover.
  - RELEASE.md: the plugin joins the release members, with its version step before cutover step 2.
    Under question 2's option B, RELEASE.md and the store disclosure say the cutover comes from the
    last rc.
  - A marked amendment to ADR-0144 withdraws the precedence claim.
  - `docs/design/relay-protocol-4.md`: no 2.0 peer sets `Opening.reason`, readers ignore it, and the
    start reason travels only as `serve --reason`.
  - `LANGUAGE.md:138-139` lists where a selector is accepted: click, type_text, upload, fill_form
    fields and the `selector_present` wait.
- Deletes: nothing.
- Proves: the integrity script; `scripts/check-public-surfaces.ps1`.

**D2 -- docs(contracts): the contracts state what landed.** The docs lane writes the lines for each
landed slice in its next commit, several slices to a commit, code first and claim after. Gate as the
code it follows.
- Changes:
  - ARCHITECTURE: what the hello carries (`404-406`); the confirmation rule and the inert pending
    binding; the service end; the Work chokepoints without `CompletionGate` (`827-845`); the
    recording binding; the browser binding taken with the first tab; the token's limit; the
    opener-claim lines (`544`, `566-567`, `1081-1083`).
  - LANGUAGE: the refusal, reason and cause tables; the list facts, and the empty list's answer, with
    "refuses without one" (`351-353`) narrowed to sessions that hold tabs; the held-tab entry; the
    ownership refusal (`63`) and the reuse rule (`375-376`).
  - ACCEPTANCE: Browser job journeys items 20, 24 and 25, and Extension product gates item 8. INTENT:
    the group title.
  - The SIEM guide: `browser_id`, install-local and random; `sacred_host`; the three causes;
    `policy_seq`; what `allowed` means; a new audit log is the user's and SYSTEM's on Windows, and how
    an administrator grants a collector account read access.
  - The governance guide: both `level` values set a ceiling; closing a tab needs Action on its host.
  - The installation guide: stable channels only; the group title (`215`).
  - docs/trust and README: the five claims under Context, once their slices land; Ghostlight's own
    ceiling named as Ghostlight's; `README.md:106` with the same-id, same-URL residual; the resume
    token's honest limit, and what a reused tab carries across; on Windows, the files Decision 11.1
    lists are created owner-only, and a file that already existed keeps its access.
  - Every "Pause/Stop" (`security-overview.md:58-59`, `README.md:172`, `compliance-team.md:31`,
    `governance-configuration.md:267`, `LANGUAGE.md:162`, ACCEPTANCE's ADR-0185 evidence item 5)
    renamed with the catalog, or kept with a stated reason.
  - The adapter-protocol-4 design: the dropped `presentationLabel`, and `life` once B1 lands.
  - If C1 finds the same leak in a published release, `data-flows.md` gains a correction paragraph
    like the H1 one.
- Deletes: claims the code no longer makes.
- Proves: the integrity script; each trust claim read against the tree.

### core

**C1 -- fix(audit): the durable record keeps only what may be retained.** Gate candidate.
- Changes: `retained()` in `language/audit.rs`, covering the variants Decision 4.1 lists and
  recursing into `DialogOpened.action` and `ActionUnconfirmed.action`. `Outcome::audit` and
  `Refusal::audit` render from it. The commit message says whether 1.3.14 has the same leak.
- Deletes: the strip arms per variant, and both `_ => self.summary()` fallbacks
  (`language/audit.rs:147-168`, `260-284`).
- Proves:
  - `no_page_text_reaches_the_audit_through_any_wrapper`: a sentinel choice and a reformatted reading,
    tested bare, inside `DialogOpened`, and inside `ActionUnconfirmed` for each check failure;
  - `an_ownership_refusal_writes_no_other_clients_name`;
  - `a_fill_whose_submit_raises_a_confirm_writes_no_field_value`, through the executor with a fake
    browser;
  - the audit goldens change only where a wrapper's summary carried page text or a client claim, and
    the diff is read.

**C2 -- fix(workspace): a tab binding has one lifecycle.** Gate candidate.
- Changes:
  - `forget_tab` is used by `confirm_tab_closed` and `apply_browser_close`.
  - `restore_tab` clears what a handle carried.
  - `navigate_page` on an existing tab and `close_tab` select a held tab; every other selection
    refuses it.
  - One executor helper holds a tab and emits `HoldEntered`, which `work/reading.rs:58` gains.
  - `WorkspaceSummary.browser` is added, with a constructor.
- Deletes:
  - the two hand-written close removals;
  - `WorkspaceStore::owners` (`workspace/mod.rs:808-816`);
  - `RelayBrowserPort::adapter_version` and `browser_id` (`browser/mod.rs:768-784`).
- Proves:
  - `a_tab_the_person_closes_leaves_no_snapshot_or_image`;
  - `a_recreated_handle_never_serves_the_closed_pages_capture_or_diff`;
  - `a_held_tab_can_be_navigated_to_an_allowed_page_and_is_released`;
  - `a_held_tab_can_be_closed`;
  - `a_held_tab_still_refuses_reads_actions_history_and_focus`;
  - `a_read_that_holds_a_tab_announces_the_hold`.

**C3a -- fix(work): a session's browser is bound by its first tab.** Depends on C2. Gate candidate.
- Changes:
  - `target_browser` chooses and reads the pin but never writes it.
  - `add_tab` and a confirmed restore pin the browser.
  - A list in an empty session answers at once with `tabs: []`, the port's browsers and the
    `browser_navigate` next step.
  - The doc comment at `work/mod.rs:1388-1391` becomes true.
- Deletes: the pin in `target_browser` (`work/mod.rs:1410-1419`); the empty list's absence wait and
  refusal.
- Proves:
  - `listing_tabs_in_an_empty_session_binds_no_browser`;
  - `an_empty_session_lists_at_once_without_a_browser`;
  - `an_empty_session_opens_in_the_browser_its_list_named`;
  - `recording_status_in_an_empty_session_binds_no_browser`;
  - `a_pinned_session_is_never_rebound`;
  - `work/tests/availability.rs:224-231` expecting no binding after a list.

**C3b -- fix(browser): a pinned wait reads its own browser.** Depends on C3a. Gate none.
- Changes: departures are kept per browser identity, and `await_returning(installed, ..)` reads only
  that browser's evidence.
- Deletes: the single `departed` slot (`browser/mod.rs:401-406`).
- Proves: `another_browsers_departure_does_not_delay_a_pinned_start`;
  `another_profile_of_the_same_browser_still_holds_the_wait`.

**C3c -- refactor(browser): one browser start, recorded in the launch guard.** Depends on C3b. Gate
none.
- Changes: pending launches move into the port's registry beside `arrivals_of`, with
  `note_start(installed)`. One `start_browser(executable, args)` in `browser/recovery.rs` keeps
  recovery's reaped `Command` and its graphical-session environment, so relay is untouched.
- Deletes: recovery's own `PendingLaunch` map (`browser/recovery.rs:520`).
- Proves: `a_noted_start_is_not_repeated_by_recovery_within_the_handshake_window`.

**C4 -- fix(workspace): a remembered tab is pending until its browser confirms it.** Depends on C3a.
Two commits: the aggregate rule with the disconnect and the registration, then the resume path and
the confirmation. Gate candidate.
- Changes: Decision 1 without the life:
  - pending bindings, inert as Decision 1 says, in the one tabs map;
  - `Disconnected` and every registration of a browser id mark its bindings and its pool pending;
  - the token's tabs enter as pending, each with its own browser;
  - the executor's one `ListTabs` per browser and `lease.confirm`, before any selection or
    targeting; the list confirms every pending tab;
  - one predicate shared with the released pool;
  - `OwnedTab.url_digest`, of `TabState.url` bounded as the adapter bounds it;
  - the token written from each binding's browser, and republished when a binding changes.
- Deletes: `McpWire.unrestored`, `restore_returned_tabs`, and the open-time `ListTabs` and pin in
  `restore_tabs` (`service/mod.rs:1728-1800`); `apply_browser_disconnect`'s pool drop;
  `publish_resume`'s read of the workspace pin (`service/mod.rs:1387-1401`).
- Proves, with a fake browser that restarts and gives a person's tab the agent's old id:
  - `a_browser_restart_never_hands_an_agent_handle_the_persons_tab` (list, read, screenshot,
    navigate);
  - `a_port_restart_keeps_a_tab_whose_page_is_unchanged`;
  - `a_resumed_tab_is_confirmed_by_id_and_url_before_its_first_use`;
  - `a_token_from_before_a_browser_restart_restores_nothing`;
  - `a_token_written_before_digests_resumes_with_stale_tabs`;
  - `a_new_tab_with_a_recycled_id_forgets_the_pending_binding`;
  - `a_commit_never_confirms_a_pending_binding`;
  - `a_release_after_a_browser_restart_never_closes_the_persons_tab`, with preserve-tabs off;
  - `a_call_before_the_tokens_browser_returns_keeps_its_tabs_in_the_token`;
  - `a_registration_that_replaces_a_connection_marks_its_bindings_pending`;
  - the resume tests in `service/mod.rs`, rekeyed;
  - a service-restart process journey, run as the Journeys rule says.

**C5 -- fix(service): a service end decides its sessions' tabs once.** Depends on C4. Gate candidate.
- Changes: Decision 3's service half: the order under one deadline in the quit thread; `session_end`
  and the idle exit forget without a browser command; a flag for when the quit has taken over.
- Deletes: the release that races the goodbye in `LiveSession::close` (`service/mod.rs:1699-1710`).
- Proves:
  - `a_replace_keeps_a_live_sessions_tabs_bound_grouped_and_resumable`;
  - `a_quit_releases_every_session_and_hands_each_stream_a_token_without_tabs`;
  - `a_command_line_session_is_released_at_every_service_end`;
  - `a_session_whose_client_left_during_the_drain_is_released_on_replace`;
  - `a_leased_workspace_is_cancelled_before_its_release`;
  - `a_session_end_says_goodbye_inside_the_fallback_with_many_tabs`;
  - the quit and replace process journeys, run as the Journeys rule says.

**C6 -- fix(language): a held tab is described truthfully.** Depends on C2. Gate candidate.
- Changes:
  - `TabState` keeps the denial reason of the landing that held it, and the refusal records it.
  - `TabHeld` names the host and the calls that clear the hold.
  - The flow cause becomes `PolicyBlocked` (`work/composition.rs:251-258`).
  - The hold words move to `language/presentation.rs`.
  - The catalog says "Pause or End session" (`language/catalog.rs:695`).
- Deletes:
  - "Ghostlight is paused on that tab." and its Resume step;
  - `TabHeld` in the paused group;
  - `RuntimeHold` for a tab hold (`work/mod.rs`, `workspace_refusal`);
  - the hold literals at `presentation/mod.rs:229-236`.
- Proves:
  - `a_held_tab_refusal_names_the_refused_host_and_the_calls_that_clear_it`;
  - `a_held_tab_is_audited_with_its_landing_denial_not_a_pause`;
  - `a_flow_step_on_a_held_tab_is_blocked_by_policy`;
  - the catalog and result goldens regenerated on purpose.

**C7 -- fix(language): a workspace refusal says what is true.** Depends on C6. Gate candidate.
- Changes:
  - the three new `WorkspaceReason` causes, with the tab left out of the audit;
  - `TabUnavailable` reworded;
  - flow tab injection only into steps whose handles name no tab (`language/mod.rs:1663-1678`);
  - the ownership owner named the way `language::provenance` names the opener.
- Deletes: the lossy arms of `From<WorkspaceError>` and its unreachable `BrowserPinned` arm
  (`language/outcome.rs:2062-2087`); `labeled_owner`'s raw claim.
- Proves:
  - `a_target_from_another_of_this_sessions_tabs_names_that_tab`;
  - `a_fill_across_two_tabs_says_a_fill_works_in_one_tab`;
  - `a_point_outside_the_image_gives_the_image_size`;
  - `an_ownership_refusal_names_the_owner_as_the_window_does`;
  - `a_flow_tab_is_not_injected_into_a_step_whose_handle_names_its_tab`;
  - the result goldens regenerated on purpose.

**C13 -- fix(provenance): the opener is named once.** Depends on C7. Gate candidate.
- Changes: Decision 9 inside core. The claim is bounded once. The group title is composed in
  `language/`. Reuse is keyed on `Agent::of(claim, channel).id`, with no key for the unknown agent.
  `ConnectionDetails` carries the source's catalog agent, unserialized. Core stops using
  `DEFAULT_CLIENT_LABEL`, and C22 deletes it.
- Deletes:
  - `claimed_name`'s decode;
  - `McpSession`'s own bound;
  - the `format!` at `work/navigation.rs:36-38`;
  - reuse keyed on the claim;
  - the stale comments at `workspace/mod.rs:562-568` and `869-870`.
- Proves:
  - `a_group_is_titled_for_the_agent_the_window_names`;
  - `an_opener_the_catalog_cannot_name_is_titled_unknown_agent`;
  - `a_client_that_names_nothing_reuses_no_kept_tab`;
  - `a_claimless_client_started_by_claude_code_reuses_no_claude_code_tab`;
  - `a_command_line_session_reuses_a_tab_another_command_line_session_kept`;
  - `a_tab_a_command_line_session_kept_is_not_reused_by_claude_code`;
  - `an_empty_name_and_no_name_stay_the_same_unknown_agent_after_a_resume`;
  - `connection_details_carry_the_sources_catalog_agent_unserialized`.

**C9 -- fix(work): a duration wait says what its deadline did.** Depends on A1 and A2. Gate
candidate.
- Changes: the duration wait refuses at once when it cannot fit, and is before-dispatch when cut
  short (`work/forms.rs:803-834`). It also carries the work-level proofs of A1 and A2.
- Deletes: the after-dispatch deadline for a sleep.
- Proves:
  - `a_duration_wait_past_its_deadline_is_repeat_safe_and_says_nothing_was_sent`;
  - `a_duration_longer_than_the_time_left_is_refused_without_sleeping`;
  - `a_download_save_that_outlives_its_deadline_is_unknown_and_not_repeat_safe`;
  - `no_embedded_host_reaches_the_permission_trace_from_a_targeted_read`.

**C10a -- fix(work): closing and listing a tab look at its host.** Depends on C2. Gate candidate.
- Changes: a model close checks Action on the tab's URL, then the setting; closing a held tab and
  compensation check only the setting; the list asks the snapshot's `authorize_landing(Read, url)`
  for each tab, and withholds the title and URL of a refused one while naming its host.
- Deletes: the host-blind admission of a model close (`work/mod.rs:1271-1287`); the false comment at
  `work/mod.rs:1207-1216`.
- Proves:
  - `closing_a_tab_needs_action_on_its_host`;
  - `a_denied_landing_is_closed_under_a_read_only_policy_when_the_setting_allows`;
  - `a_held_tab_is_closed_under_the_setting_alone`;
  - `listing_withholds_title_and_url_of_a_tab_on_a_refused_host_and_names_the_host`.

**C10b -- fix(work): every authority check of a frame is on the frame's path.** Gate candidate.
- Changes: a recording save makes one pass through `self.authorize_landing`; single-stage handlers use
  `context.requirements`.
- Deletes: the second source pass (`work/recording.rs:305-313`); the capability literals at
  `work/forms.rs:112-119`, `work/interaction.rs:43-47` and their siblings.
- Proves: `a_refused_recording_save_records_its_deciding_check`;
  `every_single_stage_tool_is_admitted_against_its_classified_requirements`.

**C11 -- fix(service): a call during a quit and a token that cannot be read both get an honest
answer.** Gate candidate.
- Changes:
  - A request admitted after a quit began is cancelled with `CancelCause::Quit`, which carries the
    quit's reason, and completes through `finish`.
  - The token-carrying opening that fails to decode is handled as Decision 1 says.
  - The token is published with the `initialize` answer and bounded on write.
- Deletes:
  - `refuse_while_quitting`, `ServiceResponse::Error`, `ProtocolRefusal::Service`,
    `SHUTTING_DOWN_CODE` and `SHUTTING_DOWN_MESSAGE`;
  - decode's revision check;
  - the fresh-session fallback.
- Proves:
  - `a_call_during_a_quit_gets_a_tool_result_and_an_audit_line`;
  - `a_call_during_an_uninstall_gets_no_retry_step`;
  - `an_unreadable_token_resumes_an_initialized_session_without_tabs`;
  - `an_unknown_revision_resumes_at_the_oldest_supported_revision`;
  - `an_answered_initialize_always_has_a_token`;
  - `a_token_never_outgrows_the_opening_cap`.

**C12a -- feat(language): history rows carry one closed outcome class.** Gate candidate.
- Changes: `language/history.rs` gains the closed outcome class, computed from the typed record, and
  composition's `child_cause` derives from it. Re-read `history.rs` after ux-2's 2af1b94a, which
  already reads closed refusal facts.
- Deletes: composition's own cause mapping.
- Proves: `a_credential_refusal_is_classed_needs_you_and_a_kept_tab_kept`.

**C19a -- fix(audit): a step that never started carries the policy sequence.** Gate candidate.
- Changes: Decision 4.3.
- Deletes: the second spelling of the not-started status.
- Proves: `a_not_started_step_carries_policy_seq_under_managed_policy`; the audit golden regenerated on
  purpose.

**C21 -- feat(adapter): the service reads the browser's life.** Lands with B1, in the same
candidate. Depends on C4 and B1. Cross-lane: `protocol4/wire.rs`. Gate candidate.
- Changes:
  - optional `Hello.life`, with its fixtures;
  - the registry keeps the last life for each browser id;
  - a binding records its life, and the token carries each browser's life;
  - a pending binding in the same life confirms by id and takes the live URL and title;
  - not the notice's `agent` or the export failure's page code: package items 5 and 6 are declined.
- Deletes: nothing; `wire::PAGE_REFUSED_REPLAY` and `page_refused` stay, because item 6 is declined.
- Proves:
  - `a_worker_restart_in_the_same_life_keeps_a_tab_whose_page_routed_itself`;
  - `a_service_replace_in_the_same_life_keeps_a_routed_tab_and_takes_its_live_url`;
  - `a_new_life_falls_back_to_the_url_check`;
  - `a_token_or_hello_without_a_life_falls_back_to_the_url_check`.

**C12b -- refactor(language): each refusal is typed where it is learned, and its code is written
once.** Two commits. Gate none.
- Changes: Decision 5.1, 5.2 and 5.6, Decision 4.5, and Decision 6.4's inline-file ceiling: one
  constant for 5,000,000 bytes, checked once at intake, where inline base64 decodes once.
- Deletes:
  - `browser_reason`, `unconfirmed_reason` and the reason literals at call sites;
  - the unreachable let-else at `work/forms.rs:908-912` and the inline-ceiling recheck;
  - the `7_000_000` literal (`work/reading.rs:420`);
  - the recovered-tab overwrite (`work/navigation.rs:451-461`);
  - the hand-built already-closed terminal;
  - the tool-name compare in `denial_presentation`.
- Proves:
  - `every_refusal_has_one_model_code_and_one_audit_tag`;
  - `a_capture_over_the_budget_says_capture_too_large_with_its_step`;
  - `a_debugger_refusal_reads_its_own_sentence_and_keeps_the_browser_text_in_detail`;
  - `a_page_credential_refusal_names_no_control`;
  - `a_flow_close_refused_by_policy_shows_the_kept_tab_notice`;
  - `a_recovered_tab_reports_page_recovered`;
  - `an_already_closed_tab_gets_its_next_step`;
  - the result goldens regenerated where a prose code became closed.

**C10c -- refactor(work): the executor reads one publication.** Depends on P1 and P5. Gate none.
- Changes: the executor keeps the publication, and `policy_explain` reads it; the catalog drops
  `browser_screenshot` when screenshots are off.
- Deletes: `policy_explain`'s own publication read.
- Proves: `policy_explain_and_its_audit_line_name_one_authority`;
  `screenshots_off_removes_browser_screenshot_from_the_catalog`.

**C16 -- refactor(work): which recording a workspace holds is a workspace fact.** Cross-lane: the
three `BrowserPort` fakes in `workbench/tests.rs` and `protocol4/link.rs`. Gate none.
- Changes: Decision 6.7.
- Deletes: `RecordingIndex` (`protocol4/link.rs:274-331`), `Link::workspace`, and the `"reaped"` and
  `"released"` placeholders.
- Proves:
  - `a_relay_restart_keeps_the_workspaces_recording_saveable`;
  - `a_second_start_while_one_is_held_is_refused`;
  - `a_released_workspace_forgets_its_recording`;
  - the link tests without workspaces.

**C17 -- refactor(language): one wait vocabulary and one coverage predicate.** Gate none.
- Changes: Decision 8's wait conditions and Decision 6.8.
- Deletes:
  - the condition literals in `work/forms.rs` and `work/documents.rs:137`, `403`;
  - the second `pub mod condition` (`translate/observe.rs:32-48`) and its unknown arm;
  - `limited_coverage`'s JSON parse;
  - lean's field-name list.
- Proves:
  - `a_duration_wait_has_its_own_sentence`;
  - `the_wait_catalog_is_unchanged`, without the update variable;
  - `every_condition_settles_and_scopes_as_the_catalog_says`;
  - `a_new_limitation_field_reaches_the_lean_view`;
  - the result goldens regenerated for the duration sentence.

**C18 -- refactor(presentation): one table of activity and denial words, with the crossing names
pinned.** Depends on C6. Cross-lane: `protocol4/golden.rs`. Gate none.
- Changes:
  - `presentation/mod.rs` reads `language/presentation.rs`.
  - The page caption draws `signal.label`, and the denial draws `signal.phase` and `signal.detail`.
  - `protocol4/golden.rs` pins the presentation kinds and activities, area words, left-out tiers and
    recording stop reasons against the JavaScript, which it reads but does not edit.
- Deletes:
  - `activity_label` in `presentation/mod.rs`;
  - `activityLabel` and the unused `presentationLabel` in `page-shared.js`;
  - the fallbacks at `presentation.js:514` and `517`.
- Proves:
  - `the_page_caption_is_the_word_the_service_sent`;
  - `every_presentation_name_the_service_sends_is_one_the_page_draws`;
  - `every_area_and_tier_the_page_names_is_one_the_service_decodes`;
  - `every_stop_reason_the_extension_sends_is_one_the_service_decodes`.

**C19b -- feat(audit): the record names the browser.** Depends on I1. Gate none.
- Changes: Decision 4.6. The audit sink creates a log that does not exist yet through
  `create_private`.
- Deletes: nothing.
- Proves: `two_profiles_of_one_browser_are_two_browser_ids_in_the_record`; `a_new_audit_log_is_owner_only`;
  the audit golden regenerated with `GHOSTLIGHT_AUDIT_GOLDEN_UPDATE`, on purpose.

**C19c -- refactor(work): the completion path's leftovers go.** Gate none.
- Changes: Decision 4.4 and 4.7. `HARNESS_DETACHED` names the connection. A session that ends in
  error writes one diagnostics line. `CatalogChanged` becomes a unit variant.
- Deletes:
  - `CompletionGate` (`work/result.rs:150-173`);
  - `WorkPhaseStarted.physical_id`, `TabCreated.tab` and `DocumentCommitted.tab`;
  - `AuditRecorder::record` from production;
  - `desktop_ready=true` (`service/mod.rs:112-118`);
  - the `eprintln!` (`service/mod.rs:969-971`);
  - the stale `AuthorityAlreadyRunning` doc;
  - the watch's counter.
- Proves: `harness_detached_names_the_connection_attached_named`;
  `an_mcp_session_that_ends_in_error_leaves_a_diagnostics_line`.

**C20 -- test(language): the plugin skill names only catalog tools and fields.** Depends on I9b. Gate
none.
- Changes: a test in `language/self_check/hints.rs` reads the skill through the hints reader.
- Deletes: nothing.
- Proves: the test fails on any field the catalog does not advertise.

**C22 -- refactor(intake): the "MCP client" placeholder goes.** Depends on C13. Cross-lane:
`agent/mod.rs:387` and `workbench/tests.rs:1632`, `1749`. Gate none.
- Changes: the last uses read no claim.
- Deletes: `DEFAULT_CLIENT_LABEL` (`intake/mcp.rs:53-56`, `206`, `221`).
- Proves: the tests that used it, unchanged in meaning.

### adapter

**A1 -- fix(adapter): an embedded document's change is stale, and a targeted read reports the tab.**
Gate candidate.
- Changes:
  - `describe_documents` returns `StaleLocator { Navigated }` for a locator whose frame holds another
    loader, or whose frame is absent from a complete inventory.
  - `DocumentChanged` on a single-locator call is treated the same (`protocol4/link.rs:731`).
  - `read_text` below the top document reports the tree's admitted top URL and no title.
- Deletes: the locator branch of `unresolved` (`translate/observe.rs:95-110`).
- Proves:
  - `a_target_in_a_reloaded_iframe_is_refused_as_navigated`;
  - `an_incomplete_inventory_keeps_document_unavailable`;
  - `a_targeted_read_in_an_iframe_reports_the_tabs_url_and_host`;
  - `translate/tests.rs:757-773` rewritten.

**A2 -- fix(adapter): a Downloads save carries effect, and diagnostics bind only when asked.** Gate
candidate.
- Changes: a Downloads export carries effect (`protocol4/link.rs:255-263`); `read_diagnostics` binds
  only when the scope watches documents (`translate/records.rs:69`).
- Deletes: the binding on unrestricted reads.
- Proves: `an_unrestricted_diagnostics_read_sends_no_binding`;
  `a_restricted_read_still_binds_its_documents`.

**A3 -- refactor(adapter): no content command runs without a scope.** Gate none.
- Changes:
  - `reach()` returns a protocol error when there is no scope.
  - `script::evaluate` and the recording translators take a required scope.
  - The six adapter tests that used the open path admit an all-open scope.
- Deletes:
  - `Reach::Open`, `Scope::open`, and `PlannedCall::unscoped` for content;
  - `doc_of` and `doc_or_top`;
  - `FrameTree::parent` and `ancestors`;
  - `geometry::output_scale` and its tests;
  - `Listing.forms`;
  - `EffectClass::Prepare`, unless a rule reads it.
- Proves: `a_content_command_without_a_scope_is_a_protocol_error`; the two tests that pinned unguarded
  behavior are removed.

**A4 -- refactor(adapter): the authority's budgets reach the adapter as values.** Gate none.
- Changes: Decision 6.4, apart from the inline-file ceiling, and Decision 6.5.
- Deletes:
  - the page's `8` (`assets/content.js:1737-1743`) and `QUERY_SEMANTIC_CAP`;
  - `SETTLE_MS` and `SETTLE_CALL_MS` (`translate/capture.rs:25-27`);
  - the `SETTLE_CAP_MS` copies (`scope.rs:23`, `translate/observe.rs:25`);
  - the tree's JSON string round trip;
  - work's `DOCUMENT_TREE_NODE_LIMIT` and the page's fallback;
  - `bounded_tree`'s rebuild.
- Proves:
  - `a_selector_matching_thirty_controls_says_at_least_eight`;
  - `a_screenshot_under_a_short_deadline_keeps_half_for_the_capture`;
  - `a_target_screenshot_under_one_second_still_measures`;
  - `a_tree_over_the_node_budget_is_cut_once_and_says_so`;
  - the page-asset tests, with `assets.sha256` regenerated.

### policy

**P10 -- feat(governance): the runtime state names the acts it accepts.** Gate candidate.
- Changes: `governance::offers(state)` beside `apply_intent`, which reads it.
- Deletes: nothing.
- Proves: `ended_offers_only_start_session`; `stop_is_still_terminal`.

**P1 -- fix(governance): one evaluator says whether a capability can be used anywhere.** Gate
candidate.
- Changes: Decision 7.1. Probe hosts never collide with an authored or never-touch suffix.
- Deletes: `decide_potential_host`, `grant_has_possible_host`, and the universal shortcut in `reach`
  (`effective.rs:603-626`).
- Proves:
  - `disjoint_layer_grants_admit_nothing_and_advertise_nothing`;
  - `a_grant_whose_deny_cancels_its_universal_allow_reads_unavailable`;
  - `a_grant_reaching_only_never_touch_hosts_advertises_nothing`;
  - `the_catalog_the_policy_page_and_enforcement_agree_on_every_fixture`;
  - `no_probe_matches_an_authored_suffix`.

**P2 -- fix(governance): a never-touch refusal names the layer that wrote it.** Gate candidate.
- Changes: Decision 7.2. The first layer whose list matches is credited, with its layer evidence.
- Deletes: the layer-dropping flatten in `assemble` (`governance/mod.rs:1790-1792`).
- Proves:
  - `an_organization_never_touch_refusal_carries_tier_rule_and_denial_id`;
  - `a_non_http_scheme_stays_ghostlights_own_ceiling`;
  - `the_denial_id_reveals_no_listed_host`;
  - the watcher redaction unchanged.

**P3 -- fix(governance): one replay of recorded work.** Gate candidate.
- Changes: `AuthoritySnapshot::replay`; simulate calls it.
- Deletes: `decide_record` (`governance/inspection.rs:229-246`).
- Proves: `simulate_refuses_recorded_mcp_work_under_a_draft_that_turns_mcp_off`.

**P4 -- fix(governance): one organization identity per publication.** Gate candidate.
- Changes: Decision 7.4.
- Deletes: `effective::identity`'s read of the manifest alone; the passport's name taken from the
  presentation alone.
- Proves:
  - `a_bundle_named_only_in_its_presentation_names_the_organization_on_every_surface`;
  - `a_manifest_block_alone_names_the_refusal_row`;
  - `on_conflict_the_signed_presentation_wins`.

**P9 -- fix(governance): an owned user policy moved at load stays owner-only.** Gate candidate, because
the 1.x migration runs once, at the upgrade.
- Changes: `read_policy` writes the move of an owned file through `install::record::atomic_replace`.
- Deletes: nothing.
- Proves: `a_schema_3_user_policy_moved_at_load_keeps_owner_only_access`.

**P5 -- refactor(governance): one publication is read whole.** Gate none.
- Changes: `published()` returns the whole publication, plus the rest of Decision 7.5.
- Deletes: the second situation and headline builders; `AuditPolicy::resolve`'s own precedence; the
  `matches!` that is always true (`governance/audit.rs:68`).
- Proves:
  - `the_chip_is_the_compiled_headline`;
  - `enforcement_and_the_view_name_the_same_audit_layer`;
  - `the_authority_id_tags_the_policy_hint`.

**P6 -- refactor(governance): the unsigned managed file path goes.** Cross-lane: about twenty core
call sites of `GovernanceFacade::new` and two surface tests of `owning_user_policy`. Gate none.
- Changes: `GovernanceFacade::new` takes the user policy only; the layered tests use the signed
  bundle.
- Deletes: `PolicySources.managed` and its branches (`governance/mod.rs:1148-1291`), and the managed
  parameter of `owning_user_policy`.
- Proves: the fail-closed tests over `with_managed_paths`;
  `a_passport_never_reports_an_unsigned_file_as_verified`.

**P7 -- refactor(governance): one settings vocabulary.** Gate none.
- Changes: a closed `PolicySetting` used by `validate_config`, `assemble` and the readers.
- Deletes: the literal keys at `governance/manifest.rs:225`, `528-583` and `governance/mod.rs:1783-1792`.
- Proves: `every_registered_setting_parses_and_round_trips`; schema-4 documents unchanged.

**P8 -- refactor(language): the policy sentences live in language/policy.rs.** Cross-lane: its `pub
mod` line in `language/mod.rs`. Gate none.
- Changes: the move, with one sentence each for "not owned" and "no home".
- Deletes: the sentences in `effective.rs`, `governance/mod.rs:1302-1309` and `1712-1725`, and in
  `inspection.rs`.
- Proves: `the_window_and_the_error_say_one_sentence_for_a_policy_this_window_cannot_change`.

### surface

**S1 -- fix(tray): every surface offers the acts the runtime state accepts.** Depends on P10. Gate
candidate.
- Changes: `TrayModel::of` enables Resume only while paused. The controls part carries the state's
  offers, and Settings and the band read them.
- Deletes: `RUNTIME_OFFERS` (`ui/kit/settings.js:42-46`), the band's ternary (`ui/lib/view.js:820-824`)
  and the false doc at `intake/desktop.rs:383-384`.
- Proves: `an_ended_session_offers_no_resume_in_the_tray`, which replaces the test at
  `intake/desktop.rs:641`; the Settings and band page tests.

**S2 -- fix(workbench): history rows and counts say what the service decided.** Depends on C12a. Gate
candidate.
- Changes: `HistoryItem` carries the outcome class, and the connections part carries the counts; the
  page renders both.
- Deletes: `STATUS`, `isRefusal` and `isKept` (`ui/lib/entries.js:86-113`); the band's and About's
  own counts.
- Proves: `a_credential_refusal_row_says_needs_you`; `a_kept_tab_row_says_kept`;
  `the_band_and_about_count_what_the_tray_counts`.

**S4a -- fix(workbench): the Policy page previews and names what governance decides.** Depends on P3
and P4. Gate candidate.
- Changes: the preview calls `replay` with each item's channel; the card and the refusal row render
  `authority.organization`.
- Deletes: the page merge (`ui/lib/view.js:1589-1591`) and the six "Your organization" literals.
- Proves: `a_draft_that_turns_mcp_off_previews_every_mcp_action_refused`;
  `the_refusal_row_and_the_card_name_one_organization`.

**S8 -- fix(window): a window that cannot be built says so.** Gate none.
- Changes: Decision 12.7. `WindowReason::WindowFailed` is added, with its sentence. The tray tooltip
  comes from the service.
- Deletes: `WINDOW_FAILS_TOOLTIP` (`crates/desktop/src/app.rs:81`).
- Proves: `a_window_build_failure_with_webview2_installed_is_not_reported_missing`; the desktop
  goldens regenerated with `GHOSTLIGHT_DESKTOP_GOLDEN_UPDATE`.

**S6 -- fix(setup): setup starts browsers the way recovery does, and marks the right host in use.**
Depends on C3c and C13. Gate none.
- Changes:
  - `ThisComputer::start` calls `start_browser` and `note_start`;
  - rows match by `native_host::browser_id_named`;
  - a host row is in use when the session's source agent is the app the row runs inside, as today
    otherwise;
  - `language/setup.rs`'s Chrome row comes from the browser table.
- Deletes: setup's own `spawn_detached` start (`workbench/setup.rs:191-205`) and `names()`
  (`workbench/setup.rs:405-407`).
- Proves:
  - `opening_chrome_from_setup_is_never_followed_by_a_second_window`;
  - `an_unnamed_adapter_reads_as_chromium_the_way_the_port_maps_it`;
  - `cline_in_vs_code_marks_only_its_row_in_use`;
  - `claude_code_from_a_terminal_still_marks_its_row`.

**S3 -- refactor(workbench): every part comes from one gather.** Depends on P5, I6 and C2. Gate none.
- Changes: the route gathers once and builds only the parts asked for; search reads the same gather;
  one publication is read; cards take the browser from `WorkspaceSummary`; `icons_changed`
  re-symbols the cached roster.
- Deletes: `WorkbenchSnapshot`, `OverviewSummary`, `ConfigurationSummary` and `generated_at_ms`;
  `ConnectionsPart::of`'s second roster read; the page's `overview`; the `browser_of` read for each
  workspace.
- Proves: `a_setup_read_gathers_once`; `status_connections_and_controls_come_from_one_publication`.

**S4b -- refactor(workbench): the Policy page's words come from governance.** Depends on P7 and P8.
Gate none.
- Changes: the sentences come from `language::policy`; the page's setting keys are pinned to
  `PolicySetting`.
- Deletes: `SACRED_DOMAINS_KEY` (`workbench/parts.rs:42`) and the preview sentences
  (`workbench/mod.rs:1203-1272`).
- Proves: `every_setting_the_page_writes_is_registered`.

**S5 -- refactor(workbench): the window's words come from language/ and are pinned.** Depends on C18
and C12b. Gate none.
- Changes:
  - the search words come from `language::workbench`;
  - `OperationSummary` carries the activity code, and the glyphs are keyed on it;
  - the toast's words are chosen by presentation;
  - pins tie `words.js` and `extension/lib/popup-view.js` (read only) to `language::control`.
- Deletes: the second `activity_label` (`workbench/mod.rs:2339-2359`), `runtime_state_label`, and
  the English keys of `ACTIVITY_GLYPH`.
- Proves:
  - `the_window_and_the_popup_say_the_runtime_words_the_tray_says`;
  - `a_glyph_follows_its_activity_through_a_reworded_label`;
  - `a_kept_tab_toast_says_a_tab_was_kept`.

**S7 -- test(desktop): the shell decodes and re-encodes every desktop golden.** Gate none.
- Changes: the shell's link tests read every file in `tests/golden/desktop`, refuse `Unknown`, and
  compare the re-encoded value.
- Deletes: nothing.
- Proves: the test fails on a field the shell's copy lacks; `cargo test -p ghostlight-desktop`.

### install

**I1 -- fix(install): one owner-private creator for secrets and records.** Gate candidate.
- Changes: Decision 11.1 for the keygen seeds and harness locations. `install::record`'s doc says what
  `create_private` gives on Windows.
- Deletes: `write_private` and its `0o600` (`governance/managed/cli.rs:396-410`).
- Proves: `keygen_seeds_are_readable_only_by_the_user` (the DACL on Windows, 0600 on Unix);
  `harness_locations_are_owner_only`.

**I3 -- fix(install): Ghostlight owns the command, and the client keeps its own keys.** Starts after
ux-2 closes. Gate candidate.
- Changes: Decision 10.1.
- Deletes: the whole-value replace of an owned entry (`install/mod.rs:2302-2303`) and the two
  equality no-ops (`2286`, `2358-2361`).
- Proves:
  - a writer-golden seed "owned with the client's keys" for every dialect, Codex's `env_vars` on Linux
    included;
  - the stale, legacy relay and retired scenarios regenerated on purpose;
  - `the_dry_run_and_the_install_agree_on_a_current_entry`.

**I7 -- fix(deployment): a same-version install restores a missing shell.** Gate candidate.
- Changes: both `Standing::Same` arms send a shell change to `place_in_bin`. The npm and MCPB
  launchers check that the shell is present.
- Deletes: nothing.
- Proves: `a_same_version_install_restores_a_missing_shell`;
  `a_same_version_install_restores_a_changed_shell`; the launcher tests.

**I9a -- docs(plugin): the plugin README withdraws the precedence claim.** Gate candidate.
- Changes: the README withdraws the claim and says npx asks the npm registry at launch.
- Deletes: the precedence claim.
- Proves: the integrity script.

**I9b -- fix(plugin): the skill says only what the catalog cannot.** Gate candidate.
- Changes: the skill keeps the activation text, observe-then-act, view-bound coordinates, honest
  effects, credential acknowledgement, `policy_explain` first, and refusals that create no hold.
  "Pause/Stop" becomes "Pause or End session".
- Deletes: every restated bound, default, enumeration and envelope list in the skill.
- Proves: the integrity script; C20 later.

**I11 -- fix(plugin): the plugin registers what question 1 decides.** Depends on the owner's answer
and the live check. Gate candidate.
- Changes: option A keeps the server, and the RC notes gain "the plugin or `ghostlight install`, not
  both" (docs lane); option B ships the skill only and points at `npx -y ghostlight install`.
- Deletes: under option B, the plugin's server entry.
- Proves: the live check's result in the ledger; the integrity script.

**I4 -- fix(install): one rule says Ghostlight has an entry to remove.** Gate none.
- Changes: `can_uninstall` and `left_private` use `holds_owned_entry`'s reading.
- Deletes: the Installed-only tests (`install/mod.rs:1811`, `cli/setup.rs:974`).
- Proves:
  - `a_whole_uninstall_removes_a_stale_owned_entry_and_names_it`;
  - the stale and retired `offers:` lines gain uninstall;
  - `cli/setup.rs:1508` expects zed.

**I5 -- fix(install): Ghostlight's npm launcher entry is Ghostlight's.** Gate none.
- Changes: Decision 10.3, with its words in `language/deployment.rs`.
- Deletes: the foreign-program words for the launcher entry.
- Proves:
  - `an_unpinned_npx_ghostlight_entry_says_npm_chooses_the_version`;
  - `a_launcher_pinned_to_1x_needs_attention`;
  - the test at `install/reconcile.rs:673` unchanged.

**I2 -- fix(install): one atomic replace and one same-location rule.** Depends on I1. Gate none.
- Changes: Decision 11.2. Keep-access gives a new file default access and creates plain parent
  directories. The stray sweep knows the one temporary name.
- Deletes:
  - `atomic_write` (`governance/managed/cli.rs:412-419`);
  - `write_file_atomic` (`install/native_host.rs:1369-1418`);
  - the second remove-then-rename fallback;
  - `paths_equal` and `comparable`;
  - `restore_override`'s fixed name.
- Proves:
  - `a_failed_native_host_write_leaves_no_temporary`;
  - `register_and_inspect_agree_on_a_respelled_registry_value`;
  - `a_first_policy_sign_creates_its_bundle`;
  - `a_bundle_write_is_synced_before_its_rename`;
  - `a_legacy_relay_entry_spelled_with_forward_slashes_is_owned`.

**I6 -- fix(install): the roster cache follows the acts, and a command reads it once.** Gate none.
- Changes:
  - the public `refresh` takes the action lock, and the acts call a private scan under the lock they
    hold;
  - `apply` finds its definition by id;
  - the commands build the registry once;
  - `Registrations` builds the client registry only for reconcile;
  - a re-symbol call for the window.
- Deletes: the unlocked store (`install/mod.rs:162-168`) and the extra scans (`cli/setup.rs:176`,
  `312`, `318`, `508`, `574`, `cli/doctor.rs:130`).
- Proves: `a_background_scan_never_overwrites_an_acts_result`;
  `one_install_scans_once_plus_once_per_write`.

**I8 -- fix(packaging): the carriers agree with the service's rule and words.** Gate none.
- Changes:
  - numeric pre-release order, with vectors shared with `install/record.rs`;
  - a test in `language/deployment.rs` reads both launchers and compares their restore words with
    `restore_route()`;
  - the get scripts end with the downloaded service's `install` after `deployment install`.
- Deletes: the string compare of pre-release parts; the scripts' installed-path literal.
- Proves: the shared vectors on both sides; `tests/installer-powershell.ps1` and
  `tests/installer-shell.mjs`, run as the Journeys rule says.

**I10 -- refactor(install): install words in language/deployment, and the browser list composed
once.** Cross-lane: the family sentence at `language/outcome.rs:1819`. Gate none.
- Changes:
  - The harness details, apply messages, `setup_message`, `HarnessError`, container sentences and
    doctor lines move to `language/deployment.rs`, and say "the service".
  - `ghostlight native-host` prints `browser_line`.
  - A desktop entry that only lost its icon is not described as "an older installation".
  - A Windows key replaced from another home says "another installation".
  - `language::deployment` composes the family list from the browser table, and the outcome's family
    sentence reads it.
  - BROWSERS' doc states that only stable channels are supported.
  - `pre_edge_layout`'s words describe an older layout with connectors in bin.
- Deletes: the second native-host table (`cli/setup.rs:1093-1099`) and the six family-list literals
  in install and cli.
- Proves: `native_host_check_and_doctor_say_one_word_per_state`;
  `the_family_list_is_the_browser_table`; the agent goldens regenerated with
  `GHOSTLIGHT_AGENTS_GOLDEN_UPDATE`, on purpose.

**I12 -- refactor(diagnostics): one spelling of the edge's logs, and no name nobody writes.** Gate
none.
- Changes: the bridge's `Component` keeps only `Orchestrator`. `ghostlight diagnostics show` loops
  over it and `relay::log::Component`.
- Deletes: the bridge's `EdgeMcp` and `EdgeBrowser` (`crates/bridge/src/diagnostics.rs:81-97`) and the
  fifteen unused event names (`41-75`).
- Proves: the diagnostics tests; `cargo test --workspace`.

**I13 -- refactor(win-peer): one token module for its two callers.** Gate none.
- Changes: Decision 11.3, and win-peer's manifest and lib doc naming its current jobs.
- Deletes: win-peer's duplicate token code; the token-file wording
  (`crates/win-peer/src/private_file.rs:168-169`, `lib.rs:40-41`).
- Proves: `cargo test --workspace`.

### package (after ux-2 closes)

**B2 -- fix(extension): custody follows the service that granted it, and an outdated extension says
so.** Package items 2, 3, 4 and 7, decided under the owner's delegation. Gate candidate.
- Changes:
  - The service epoch is kept in `chrome.storage.session`, and a new epoch releases every managed tab
    before Active.
  - `adapter_incompatible` ends the hello wait and sets a sticky state that only `hello_accepted`
    clears. It takes the delayed retry, and the popup and the setup page say to update Ghostlight in
    Browser. A test pins the extension's constant to the service's code.
  - The diagnostics ring resets only on a top-frame navigation.
  - The package record is regenerated.
- Deletes: `allowNetworkUrl`; the recording and queue workspace keys; `SIGNAL.ATTENTION`; the unused
  helpers in `state.js`; `NATIVE_HELLO_SKIPPED`; the snapshot fields nothing reads; the opener
  computation in `tabs.js`.
- Proves:
  - `a_new_service_epoch_releases_every_managed_tab`;
  - `a_worker_restart_against_the_same_service_keeps_custody`;
  - `an_outdated_extension_says_so_and_stops_restarting_the_host`;
  - `a_subframe_navigation_keeps_the_diagnostics_ring`;
  - `worker.test.js:449-450` updated;
  - the freeze guard.

**B1 -- feat(extension): the browser names its life.** Package item 1, decided under the owner's
delegation; items 5 and 6 are declined. Lands in the same candidate as C21, or neither does. Gate
candidate.
- Changes:
  - `initializeLocalState` mints the life into `chrome.storage.session` when none is there, before
    the hello, and the hello sends it.
  - The package record is regenerated.
- Deletes: nothing; `notice.js` keeps its title parse and `buffers.js:455` its sentence, because
  items 5 and 6 are declined.
- Proves:
  - `a_worker_restart_keeps_the_life`;
  - `an_extension_reload_mints_a_new_life`;
  - `the_hello_carries_the_life`;
  - the freeze guard.

## Amendment (2026-10-19): Ghostlight's own ceiling

Decision 7.2 named non-HTTP(S) schemes, localhost, loopback and link-local as Ghostlight's own
ceiling. That contradicts ADR-0155, which removed the localhost, loopback and link-local refusals
on the owner's direction so that local destinations follow policy, and ADR-0199, which holds
ADR-0155 and adds the workbench's own origin as a product-owned ceiling. This record did not mean
to reverse ADR-0155, and P2 did not restore those refusals.

The ceiling credited to Ghostlight, with no authored layer, is therefore:

- non-HTTP(S) schemes;
- the workbench's own origin (ADR-0199).

Localhost, loopback and link-local addresses follow policy under ADR-0155. A layer that wants them
refused lists them as never-touch, and that refusal is the layer's decision under Decision 7.2's
first sentence. The ADR-0122 line under What this supersedes and amends is corrected to match. The
rest of Decision 7.2 stands.
