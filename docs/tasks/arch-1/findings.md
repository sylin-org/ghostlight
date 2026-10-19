# arch-1: findings

The architecture sweep of 2026-10-19 read the live tree through eleven lenses: seven in the first
round (E, W, T, G, A, X, V) and four in the gap round (G1 to G4). Skeptics then checked each finding
against the tree; the run recorded two verdicts for most findings and one for the rest.
Every finding held: 74 lens findings and 28 gap findings, 102 in all.

The verdict is `confirmed` when every skeptic confirmed the finding as written, and `partial` when
a skeptic confirmed its core and corrected part of it (a stale line, a narrower scope, or a part
that does not hold). [ADR-0203](../../adr/0203-one-owner-for-each-fact.md) takes the corrections.

"Where it went" names the ADR-0203 decision and the slices that fix the finding, or its entry under
the ADR's "Considered and not taken". Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md).

## Lens findings

| Id | Finding | Severity | Verdict | Where it went |
| --- | --- | --- | --- | --- |
| E-01 | The resume token's tab list is treated as truth on resume: no URL check like the released pool's, so a stale or forged token binds an unrelated tab (decides ADR-0202 delegated item 5) | high | partial | Decision 1 and the tab-bindings decision; C4, C21, B1 |
| E-02 | The client's raw claim is used as a name in three places that bypass the one naming function, and 'MCP client' is a magic string for 'no claim' | medium | partial | Decisions 9 and 5.3; C13, C7, C22 |
| E-03 | A call that arrives during a quit is answered outside the one completion path, as a JSON-RPC error with no audit line | medium | partial | Decision 4.2; C11. Considered and not taken: "A new `Refusal::ShuttingDown`" |
| E-04 | Three opening fields are carried but never read, and the relay goldens show them as working parts of the contract | low | partial | Decision 13 (`Opening.reason` and `Opening.origin` stay, and the relay design says so); D1. Considered and not taken: "Removing `Opening.reason` from relay 4" |
| E-05 | Diagnostics spell the edge's components twice and keep 16 event names no 2.0 process writes; attach and detach lines name different ids | low | confirmed | Decision 8 (Diagnostics) and Decision 13 (`HARNESS_DETACHED`); I12, C19c |
| E-06 | A resume token that fails to decode silently turns a live client into a fresh session it will never initialize | medium | partial | Decision 1 (The token: a token that does not decode); C11 |
| E-07 | In-process leftovers on the session path: unread catalog-change fields, an uncalled workspace query, an error message sent to a null stderr | low | partial | Decision 13 (dead names and paths) and Decision 2.5; C19c, C2 |
| W-01 | A blocked job's page and desktop notice is re-derived from the tool name and a fact string, so flows and the toast say the wrong thing | medium | partial | Decision 5.6; C12b |
| W-02 | Handlers re-derive the capabilities the operation was already classified with, so the governance decision and the audit line can disagree | low | partial | Decision 7.7; C10b |
| W-03 | The wait-condition vocabulary, and other decoded choices, travel as String and are spelled in about ten modules; duration waits get generic words | medium | partial | Decision 8 (Wait conditions); C17. The other decoded choices: Considered and not taken: "Closed enums for `on_error`, `Record.action`, `HandleDialog.action` and Diagnose's fields" |
| W-04 | Activity, capability and group-title words are composed outside language/, in several copies, and other layers parse them back | medium | confirmed | Decision 8 (Activity and denial words) and Decision 9 (the group title); C18, C13, S5 |
| W-05 | Released-tab reuse is keyed on the client's free-text name, for a reason ADR-0201 removed (ADR-0202 item 5) | low | partial | Decision 9 (Reuse key) and the tab-bindings decision, item 3; C13. Considered and not taken: "The observed source in the reuse key, and dropping the claim from reuse" |
| W-06 | A flow step that never started is written by a second, hand-built audit path with its own status words | low | partial | Decision 4.3; C19a |
| W-07 | A recording save's deciding host check bypasses the frame's permission trace and is evaluated twice | high | confirmed | Decision 7.7; C10b |
| W-08 | A step's coverage round-trips through its JSON facts, and 'limited' is defined three ways | low | partial | Decision 6.8; C17. Considered and not taken: "A typed coverage field on `Terminal`" |
| W-09 | A duration wait that outlives its deadline says a command was sent and may have acted | high | partial | Decision 5.7; C9 |
| W-10 | A recovered tab's outcome is chosen after its result is finished, by overwriting fields | medium | confirmed | Decision 4.5; C12b |
| W-11 | The completion path still carries ceremony, dead event fields and optional evidence that the 2.0 rebuild left behind | low | confirmed | Decision 4.4; C19c |
| T-01 | The unsigned resume token writes the workspace's tab bindings, so an edge -- confused or hostile -- can bind any open tab, including the person's own | high | partial | Decision 1 and the tab-bindings decision, item 2; C4, C21, B1. Considered and not taken: "Signing the resume token" |
| T-02 | The service cannot tell a browser restart from a worker restart: bound tabs keep recycled ids, while the released pool is wiped on every reconnect | high | partial | Decision 1; C4, C21, B1. Considered and not taken: "A managed flag on every tab fact" and "A browser run minted at `chrome.runtime.onStartup`" |
| T-03 | Service end and service start decide a session's tabs by two rules: close releases them in a race, resume re-binds them outside their group, and owned sessions are never released | high | partial | Decision 3 (service half); C5, with C4 |
| T-04 | Listing tabs takes the one-browser binding, and can start a browser, for a session that holds no tab, so the browser the catalog tells the model to choose is then refused | high | partial | Decision 2.2; C3a |
| T-05 | Released-tab reuse and the tab-group title key on the free-text claim, whose rationale ADR-0201 removed, so unrelated clients share and reuse each other's tabs | medium | partial | Decision 9; C13. Considered and not taken: "The observed source in the reuse key, and dropping the claim from reuse" |
| T-06 | Distinct workspace refusals collapse into false sentences: a target from another of this session's tabs "belongs to a different Ghostlight session" | high | partial | Decision 5.3; C7. Considered and not taken: "Tab tombstones" |
| T-07 | Which recording a workspace holds is a workspace fact kept on the browser connection, so the port carries a workspace id and a reconnect forgets it | medium | partial | Decision 6.7; C16 |
| T-08 | Two close paths for one tab diverge: a tab the person closes leaves its snapshot and screenshot behind, and a recovered tab under the same handle can reuse them | high | partial | Decision 2.1; C2 |
| T-09 | "Is a browser returning?" is one global slot, so a pinned session waits on evidence from other browsers before starting its own | medium | partial | Decision 2.3; C3b |
| T-10 | Leftovers in the tab and browser seams: dead singleton accessors, a dead owner list, and comments and docs that state superseded designs | low | partial | Decision 2.5; C2 (accessors and owner list), C13 and C10a (stale comments), D2 (docs) |
| G-01 | A form fill that leaves a dialog open writes field names and values into audit | high | partial | Decision 4.1; C1 |
| G-02 | Client-written tab bindings (ADR-0202 item 5): the unsigned resume token decides which physical tabs a session may act on | high | partial | The tab-bindings decision and Decision 1; C4, C21, B1, with D2 for docs/trust |
| G-03 | Closing and listing tabs is decided host-blind, and one tab-close check serves two jobs | high | partial | Decision 7.6; C10a |
| G-04 | Three evaluators answer "can this capability be used anywhere", and they disagree | high | confirmed | Decision 7.1; P1, with C10c (screenshots off) |
| G-05 | A never-touch refusal loses the layer that authored it | high | partial | Decision 7.2; P2 |
| G-06 | Two replays of recorded work, and the window's ignores the channel switches | high | partial | Decision 7.3; P3, S4a |
| G-07 | The organization's name and contacts have two carriers, and the page merges them | high | partial | Decision 7.4; P4, S4a |
| G-08 | One publication's views are read piece by piece and derived twice | medium | partial | Decision 7.5; P5, C10c, S3 |
| G-09 | "Which layer decided" is spelled as three enums and two strings, mapped by hand | medium | partial | Considered and not taken: "One tier vocabulary and one table of home file names in this batch" |
| G-10 | The policy settings registry is spelled in five places, and the required `level` decides nothing | medium | partial | Decision 7.8 (one settings enum; `level` stays and the guide says both levels set a ceiling); P7, S4b, D2. Considered and not taken: "Dropping or loosening schema 4's `level`" |
| G-11 | Lifecycle refusals ride the governance decision, and inconsistently | medium | partial | Decision 4.7 (the doc comments say what `allowed` means); C19c, D2. Considered and not taken: "Flipping invalid requests to `allowed: true`" |
| G-12 | An unsigned managed-policy file path survives only for tests | low | confirmed | Decision 7.8 (the unsigned managed file path goes); P6 |
| G-13 | Person-facing policy sentences are authored outside language/ | low | partial | Decision 7.8 and Decision 8 (policy sentences to `language/policy.rs`); P8, S4b |
| A-01 | The tray offers Resume after End session, and the click does nothing; the state-to-act table is spelled in four places | high | partial | Decisions 12.1 and 7.9; P10, S1 |
| A-02 | A same-version install never restores a missing or damaged shell, while the product tells the person to install again | high | partial | Decision 10.5; I7 |
| A-03 | The agent count counts sessions, not connections, and is computed again in JavaScript | medium | partial | Decision 12.3; S2 |
| A-04 | Setup starts browsers through its own launcher, outside recovery's launch guard, and the service-scoped recovery is built twice | medium | partial | Decision 12.5; C3c, S6 |
| A-05 | Decision for ADR-0202 item 5: client-written tab bindings in the unsigned resume token | high | partial | The tab-bindings decision; C4, C13, C21, B1 |
| A-06 | The npm and MCPB launchers re-implement the install rule in JavaScript, twice, and disagree with it | medium | partial | Decision 10.6; I8. Considered and not taken: "The MCPB launcher delegating to its bundled service, and npm comparing versions for equality only" |
| A-07 | Setup re-derives which browser is connected from display names; the port already learned it | low | partial | Decision 12.5; S6 |
| A-08 | The flat WorkbenchSnapshot is a leftover: built whole for every request, cut into parts Live also builds, with dead fields on both sides | medium | partial | Decision 12.4; S3 |
| A-09 | Words a person reads live outside language/ on the app side, and one fact is worded by both the shell and the service | medium | partial | Decision 8 (words moved into `language/`) and Decision 12.7 (the tray tooltip); I10, S5, S8 |
| A-10 | The icon cache is a process-global singleton beside the facade's own handle | low | partial | Considered and not taken: "One holder for the agent-icon cache" |
| A-11 | Shims for pre-relay-4 development builds and leftovers of the in-process desktop ship in the release | low | partial | Decision 13 (`desktop_ready=true` and the null-stderr `eprintln!`); C19c, with I10 for `pre_edge_layout`'s words. The development shims are considered and not taken: "Removing the development-layout shims" |
| X-01 | A physical tab id has no browser-session dimension, so a restored or long-lived binding can take the person's own tab after a browser restart | high | partial | Decision 1; C4, C21, B1 |
| X-02 | Free text from receipts, page assets and Chrome becomes the agent's sentence, bypassing the closed codes and language/ | medium | partial | Decision 5.2; C12b |
| X-03 | The extension's custody set outlives the service that granted it: tabs stay managed, debugged and glowing after a service restart | high | partial | Decision 3 (The extension half, package item 2); B2 |
| X-04 | Released-tab reuse is gated on the opener's free-text claim, for a reason ADR-0201 removed | low | partial | Decision 9; C13. Considered and not taken: "The observed source in the reuse key, and dropping the claim from reuse" |
| X-05 | The service's adapter_incompatible code is dropped, so the extension tells the person to update the wrong side | high | partial | Decision 3 (The extension half, package item 3); B2 |
| X-06 | Whether the page refused a replay is decided by matching an English sentence across the adapter boundary | medium | partial | Decision 6.9: package item 6 is declined under the owner's delegation, so the sentence stays pinned by `wire::PAGE_REFUSED_REPLAY`; no slice |
| X-07 | The tab group and the popup name the agent from the raw claim, and the popup re-derives it by parsing Chrome's group title | high | partial | Decision 9; C13. The popup keeps its title parse, which then reads the right name (package item 5 declined) |
| X-08 | The activity labels and denial words live in three tables outside language/, and the page ignores the label the service sends | medium | partial | Decision 8 (Activity and denial words); C18, S5 |
| X-09 | Vocabularies that cross the adapter boundary are outside the shared golden, and extension modules re-spell protocol.js members as private literals | medium | partial | Decision 8 (Crossing names pinned); C18. Considered and not taken: "The extension's private copies of `protocol.js` members" |
| X-10 | Dead dimensions, hooks and outputs from earlier designs remain in the package about to be frozen | low | partial | Decision 13 (package item 7); B2 |
| V-01 | The model's refusal `reason` code has no home: four mechanisms in work/ and intake spell it, and it differs from the audit's code for the same refusal | medium | partial | Decision 5.1; C12b |
| V-02 | Another session's claimed client name becomes durable in the audit, and is the agent's name in ownership refusals | medium | confirmed | Decisions 4.1 and 5.3; C1, C7 |
| V-03 | Free text rides in the model's `reason` fact, and an extension sentence decides a refusal across the adapter protocol | medium | partial | Decision 5.2; C12b. The replay sentence: Decision 6.9, package item 6 declined |
| V-04 | Client-written tab bindings (ADR-0202 delegated item 5): an unsigned resume token can bind any tab in the browser to a session | high | partial | The tab-bindings decision; C4, C21, B1 |
| V-05 | The window re-derives each row's outcome from `allowed` and `status` strings, and calls a credential refusal Kept | high | partial | Decision 12.2; C12a, S2 |
| V-06 | Activity words are written three times, and the in-page caption ignores the word the service sends | medium | partial | Decision 8 (Activity and denial words); C18, S5 |
| V-07 | The extension learns who is browsing by parsing the tab-group title, which is built from the raw claim outside language/ | high | partial | Decision 9; C13. The popup keeps its title parse (package item 5 declined) |
| V-08 | Wait conditions stay a string from the catalog to the page, with their rules spelled in six modules | medium | confirmed | Decision 8 (Wait conditions); C17 |
| V-09 | Install-home file names are declared by each consumer and again by the migration that places them | medium | confirmed | Considered and not taken: "One tier vocabulary and one table of home file names in this batch" |
| V-10 | The desktop stream's frames are written twice, in the shell and in the service, held together only by goldens | low | partial | Decision 12.6; S7. Considered and not taken: "Moving the desktop frames into `relay`" |
| V-11 | ghostlight-bridge is a one-consumer crate carrying the 1.x connector's diagnostics vocabulary | low | partial | Decision 8 (Diagnostics); I12. Considered and not taken: "Folding the bridge crate" |
| V-12 | The runtime-control words are authored in two language modules, the window and the popup, and already disagree | medium | confirmed | Decision 8 (Runtime words); S5 |

## Gap findings

| Id | Finding | Severity | Verdict | Where it went |
| --- | --- | --- | --- | --- |
| G1-01 | A handle whose embedded document was replaced or removed is refused as "Document access could not be verified", not as stale, and stays live | high | partial | Decision 5.4; A1 |
| G1-02 | A targeted read in an embedded document treats that document's URL as the tab's: a landing check, the agent's url/title facts and the durable audit host | high | partial | Decision 6.1; A1, with C9 for the work-level proof |
| G1-03 | Whether a primitive changes the world is decided twice and disagrees: a recording saved to disk that times out is reported as nothing changed and safe to repeat | high | partial | Decision 6.2; A2, with C9 for the work-level proof |
| G1-04 | The diagnostics binding is keyed on loader identity, so any iframe reload wipes the tab's console and network history, even in all-open mode | high | partial | Decision 6.3; A2, B2 (package item 4) |
| G1-05 | Limits the authority owns are spelled again in the adapter and the page: the agent hears "8 controls matched" when 30 did, and screenshots ignore the shared settle budget | medium | partial | Decision 6.4; A4 |
| G1-06 | The audit names the browser by product name, so two profiles of one browser are one browser in the record and history | medium | partial | Decision 4.6; C19b |
| G1-07 | The document tree crosses the adapter boundary as a JSON string and is bounded three times, with its node vocabulary spelled in three places | medium | partial | Decision 6.5; A4 |
| G1-08 | Leftovers in the adapter: an unguarded unscoped path only tests use, test-only methods on production types, and a dead copy of the extension's capture-scale rule | low | confirmed | Decision 6.6; A3 |
| G2-01 | The managed-policy signing seed, the one secret Ghostlight writes to disk, gets no Windows protection; keygen carries its own third private-write with its own 0o600 | high | confirmed | Decision 11.1; I1 |
| G2-02 | On Windows the write helper decides whether a file is owner-only, not what the file holds: the audit logs inherit whatever the home grants, while empty markers are locked down | medium | partial | Decision 11.1; I1, C19b (a new audit log), P9 (the owned policy). Considered and not taken: "Re-protecting existing files at each start" |
| G2-03 | Token FFI is declared three times for one job: relay os/token.rs, win-peer elevation.rs and win-peer private_file.rs each open the process token, and two different descriptors mean 'only this user' | low | partial | Decision 11.3; I13. Considered and not taken: "Merging the relay's token code with win-peer's" |
| G2-04 | Five atomic file replacements with different rules: only the agent-settings writer follows a symlink, and the policy CLI's skips fsync and leaves its temporary behind | medium | partial | Decision 11.2; I2 |
| G3-01 | An update replaces Ghostlight's whole JSON entry, and whether an entry is current is decided three ways | high | partial | Decision 10.1; I3 |
| G3-02 | Five rules decide whether Ghostlight has an entry to remove: a whole uninstall leaves a stale one, and the window hides a Remove the service offers | medium | partial | Decision 10.2; I4, S9 (the window's Remove) |
| G3-03 | Ownership is one key and the edge's file name: Ghostlight's own npx launcher entry is foreign, and Ghostlight under any other key is invisible | medium | partial | Decision 10.3; I5. Considered and not taken: "Finding Ghostlight's entry under a key the person renamed" |
| G3-04 | Every host row of one agent is marked in use: the observed source that names the host is dropped before the workbench | medium | partial | Decision 9 (For the window); C13, S6 |
| G3-05 | Registration standing is five enums with their own words in install/ and cli/, and each registry says the same fact differently | medium | partial | Decision 8 (install words to `language/deployment.rs`); I10. Considered and not taken: "One shared state vocabulary across the five install registries" |
| G3-06 | The cached roster is one slot written by unordered scans, so a background rescan can overwrite an act's result | low | partial | Decision 10.4; I6 |
| G3-07 | One install reads every client configuration four or more times, and reconciling the Windows entries reads them all for nothing | low | confirmed | Decision 10.4; I6 |
| G3-08 | Install keeps four atomic writers and about seven same-path rules with different guarantees | low | partial | Decision 11.2; I2 |
| G3-09 | The browser family table is the record of supported browsers, but its names are spelled six more times and each row binds one channel | medium | partial | Decision 8 (the browser-family list composed once); I10, S6, D2. Considered and not taken: "Beta, Dev and Canary channels" |
| G3-10 | The roster row carries dead and repeated fields: product_id decides nothing, per-agent facts repeat per host, and the installation's command rides every row | low | confirmed | Considered and not taken: "Splitting the roster into an agent table and host rows, and dropping `product_id`" |
| G4-01 | The plugin skill is a second catalog description that no check reads, and several of its statements are false for 2.0 | medium | partial | Decision 10.7; I9b, C20 |
| G4-02 | The skill and the catalog still teach retired 1.x control concepts: 'session hold', 'Stop', and 'do not ask to resume' | high | partial | Decision 5.5 (the catalog's "Pause or End session"; the skill's "do not ask the user to resume" stays, because it is true); C6, I9b |
| G4-03 | A tab held after a refused landing is described as the person's pause, and its remedy cannot release it (found while checking the skill's hold claims) | high | partial | Decisions 5.5 and 2.4; C6, C2 |
| G4-04 | The plugin's npx entry is a third Claude Code registration the installer cannot see: during the RC it puts 1.x back, and after the cutover it doubles the agent | high | partial | Decision 10.7 and question 1 (the plugin); D1, I11 |
| G4-05 | The plugin's shipped files are tied to no version and no release step, so skill edits ship under an unchanged version | medium | partial | Decision 10.7 (RELEASE.md's plugin step); D1. Considered and not taken: "An integrity check that fails when the plugin differs from `main` at main's version" |
| G4-06 | get.ps1 and get.sh re-derive the install home and the component set instead of taking them from the service and the release | low | partial | Decision 10.6 (the get scripts finish through the downloaded service); I8. Considered and not taken: "Get scripts that derive the image set from `SHA256SUMS`, or install only through the carrier rule" |

## Unplaced

None. Every finding has a decision, a slice, or a "Considered and not taken" entry. One decision
builds nothing: package item 6 was declined, so X-06's replay sentence across the adapter boundary
stays pinned for 2.0, and V-03's replay part points here.
