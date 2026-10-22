//! Properties of the workbench page that no behavioral test can show, read from its files.
//!
//! The page is plain files under `ui/`. Its behavior is tested by running its real modules in
//! `tests/workbench-store.test.mjs`, `tests/workbench-transport.test.mjs` and
//! `tests/workbench-settings.test.mjs`. What stays here is what running a module cannot reveal:
//! the product's artwork byte for byte, the palette and motion curve the page shares with
//! sylin.org and the page assets, and the closed names the page and the service exchange, which
//! each side keeps a copy of.

use std::collections::{BTreeMap, BTreeSet};

/// The page's words, where it keeps its closed tables and its list of addresses.
const PAGE_WORDS: &str = include_str!("../../ui/lib/words.js");

/// The page's transport, which names the snapshot parts it asks for.
const PAGE_TRANSPORT: &str = include_str!("../../ui/lib/transport.js");

/// The Policy page's view, which composes the settings a draft authors.
const PAGE_POLICY_VIEW: &str = include_str!("../../ui/lib/policy-view.js");

/// The spring curve the workbench and the page assets move on, written without spaces.
const SPRING: &str = "cubic-bezier(.22,1,.36,1)";

/// Protects desktop gate 12: the workbench shows the original Ghostlight icon bytes and the
/// published palette, and moves on the same spring curve as the page assets.
#[test]
fn the_workbench_wears_the_original_icon_palette_and_spring_curve() {
    assert_eq!(
        include_bytes!("../../ui/ghostlight.png"),
        include_bytes!("../../../../extension/icons/icon128.png")
    );
    let styles = include_str!("../../ui/styles.css");
    // Ghostlight's accent and the night-garden ground, as published on sylin.org.
    for value in ["#5eead4", "94, 234, 212", "#0f0e12"] {
        assert!(
            styles.contains(value),
            "the workbench dropped the published palette value {value}"
        );
    }
    // Each colour's channels, which washes and edges read as rgba(var(--x-rgb), alpha), are the
    // channels of the colour itself (arch-1 SURF-11).
    let declared = |token: &str| -> &str {
        let opening = format!("  {token}: ");
        let start = styles
            .find(&opening)
            .expect("the stylesheet declares the token")
            + opening.len();
        styles[start..]
            .split(';')
            .next()
            .expect("the declaration ends")
    };
    for (colour, channels) in [
        ("--a", "--argb"),
        ("--held", "--held-rgb"),
        ("--danger", "--danger-rgb"),
        ("--ok", "--ok-rgb"),
    ] {
        let hex = declared(colour).trim_start_matches('#');
        let parsed: Vec<String> = (0..3)
            .map(|at| {
                u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16)
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            declared(channels),
            parsed.join(", "),
            "{channels} is not {colour}"
        );
    }
    let renderer = include_str!("../browser/adapter/chromium/protocol4/assets/presentation.js");
    for (name, text) in [("the workbench", styles), ("the page assets", renderer)] {
        assert!(
            text.replace(' ', "").contains(SPRING),
            "{name} left the spring curve"
        );
    }
}

/// The entries of the page's frozen object `name` in `words.js`, as (key, value) pairs in order,
/// with the quotes taken off.
fn page_table<'a>(words: &'a str, name: &str) -> Vec<(&'a str, &'a str)> {
    let opening = format!("const {name} = Object.freeze({{");
    let start = words.find(&opening).expect("the page has the table") + opening.len();
    let block = &words[start..];
    let block = &block[..block.find("});").expect("the table ends")];
    block
        .lines()
        .filter_map(|line| line.trim().trim_end_matches(',').split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim().trim_matches('"')))
        .collect()
}

/// The members of the page's frozen array `name` in `source`, in order, with the quotes taken
/// off.
fn page_list<'a>(source: &'a str, name: &str) -> Vec<&'a str> {
    let opening = format!("const {name} = Object.freeze([");
    let start = source.find(&opening).expect("the page has the list") + opening.len();
    let list = &source[start..];
    list[..list.find("])").expect("the list ends")]
        .split(',')
        .map(|entry| entry.trim().trim_matches('"'))
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// Every `key: "..."` value written between `opening` and the next `closing` in `source`, in
/// order, with the quotes taken off.
fn page_keys<'a>(source: &'a str, opening: &str, closing: &str) -> Vec<&'a str> {
    const KEY: &str = "key: \"";
    let start = source.find(opening).expect("the page has the block") + opening.len();
    let mut block = &source[start..];
    block = &block[..block.find(closing).expect("the block ends")];
    let mut keys = Vec::new();
    while let Some(at) = block.find(KEY) {
        block = &block[at + KEY.len()..];
        let end = block.find('"').expect("the key ends");
        keys.push(&block[..end]);
        block = &block[end..];
    }
    keys
}

/// The value of the page's string constant `name` in `source`, with the quotes taken off.
fn page_constant<'a>(source: &'a str, name: &str) -> &'a str {
    let opening = format!("const {name} = \"");
    let start = source.find(&opening).expect("the page has the constant") + opening.len();
    let value = &source[start..];
    &value[..value.find('"').expect("the constant ends")]
}

/// The text a closed value has on the wire.
fn wire(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .expect("a closed value is a word")
        .to_owned()
}

/// Guards the contract between the service and the page (`ui/`): every closed vocabulary that
/// crosses between them has the same members on both sides, so a rename on one side fails here
/// or in the page's tests, never silently in the window. Service to page: each roster client
/// state (and the page's own `by_hand`) and each outcome class has the page's word, and the
/// outcome word is the word search finds a row by (3f27d340); each change kind the projection
/// publishes is one the page's store folds in (8788805f; PUBLISHED in
/// tests/workbench-store.test.mjs); each page address the service sends is in the page's
/// `ADDRESSES`, in order, and reads back as itself. Page to service: each runtime intent and
/// destination key the page sends decodes (tests/workbench-settings.test.mjs pins the page's
/// copy); every `SNAPSHOT_PARTS` token is a part the server knows, and an unknown token is not.
/// Both ways, the page's `PHASE`, `RUNTIME` and `CONTEXT` hold exactly the service's operation
/// phases, runtime states and window contexts, and its setup part, Chrome destination and connect
/// act are the service's tokens (arch-1 SURF-6). Every setting key the Policy page writes into a
/// draft (its `SETTING_GROUPS`, its `SACRED_KEY` and the literals in `policy-view.js` `settingEntries`) is one
/// governance registers in `PolicySetting` (ADR-0203 S4b,
/// `every_setting_the_page_writes_is_registered`). A new value fails to compile here until it is
/// named.
#[test]
fn every_closed_name_the_page_and_the_service_exchange_is_one_both_know() {
    use super::parts::{HintedPart, Part};
    use super::{
        HistoryItem, OperationPhase, OperationSummary, PageAddress, WorkbenchChange,
        WorkbenchDestination, WorkbenchRuntimeIntent,
    };
    use crate::browser::adapter::RuntimeControlState;
    use crate::governance::manifest::PolicySetting;
    use crate::governance::{Capability, Decision, GovernanceFacade};
    use crate::install::{HarnessAction, HarnessState};
    use crate::language::history::OutcomeClass;
    use crate::language::workbench::search_outcome;
    use crate::workbench::context::WebContext;

    // The roster's client states, and the one standing the page adds: a copy-only agent found
    // here whose file it never reads.
    let states = [
        HarnessState::NotDetected,
        HarnessState::Available,
        HarnessState::Installed,
        HarnessState::Updatable,
        HarnessState::NeedsAttention,
    ];
    for state in states {
        match state {
            HarnessState::NotDetected
            | HarnessState::Available
            | HarnessState::Installed
            | HarnessState::Updatable
            | HarnessState::NeedsAttention => {}
        }
    }
    let mut sent: BTreeSet<String> = states.into_iter().map(wire).collect();
    sent.insert("by_hand".to_owned());
    let worded: BTreeSet<String> = page_table(PAGE_WORDS, "AGENT_STATE_WORDS")
        .into_iter()
        .map(|(key, _)| key.to_owned())
        .collect();
    assert_eq!(worded, sent, "the page names exactly the client states");

    let classes = [
        OutcomeClass::Succeeded,
        OutcomeClass::Refused,
        OutcomeClass::Kept,
        OutcomeClass::NeedsYou,
        OutcomeClass::Failed,
        OutcomeClass::Unknown,
        OutcomeClass::Cancelled,
    ];
    for class in classes {
        match class {
            OutcomeClass::Succeeded
            | OutcomeClass::Refused
            | OutcomeClass::Kept
            | OutcomeClass::NeedsYou
            | OutcomeClass::Failed
            | OutcomeClass::Unknown
            | OutcomeClass::Cancelled => {}
        }
    }
    let names: BTreeMap<&str, &str> = page_table(PAGE_WORDS, "OUTCOME").into_iter().collect();
    assert_eq!(
        names
            .values()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        classes.into_iter().map(wire).collect::<BTreeSet<_>>(),
        "the page names exactly the classes the service sends"
    );
    // The word a row wears is the word search finds it by, and a row with no word is found by none.
    let shown: BTreeMap<String, String> = page_table(PAGE_WORDS, "OUTCOME_WORD")
        .into_iter()
        .map(|(key, word)| {
            let name = key.trim_start_matches("[OUTCOME.").trim_end_matches(']');
            (names[name].to_owned(), word.to_lowercase())
        })
        .collect();
    for class in classes {
        assert_eq!(
            shown.get(&wire(class)).map(String::as_str),
            search_outcome(class),
            "{class:?}"
        );
    }

    // The change kinds the window folds in; PUBLISHED in tests/workbench-store.test.mjs is the
    // page's copy, so a kind renamed here and not there reads as ignored and draws nothing.
    let named = |change: &WorkbenchChange| match change {
        WorkbenchChange::AuditHealthChanged { .. } => "audit_health_changed",
        WorkbenchChange::DocumentCoverageChanged { .. } => "document_coverage_changed",
        WorkbenchChange::OperationStarted { .. } => "operation_started",
        WorkbenchChange::OperationChanged { .. } => "operation_changed",
        WorkbenchChange::OperationSettled { .. } => "operation_settled",
        WorkbenchChange::CompositionChanged { .. } => "composition_changed",
        WorkbenchChange::RuntimeChanged { .. } => "runtime_changed",
        WorkbenchChange::StatusChanged { .. } => "status_changed",
        WorkbenchChange::ConnectionsChanged { .. } => "connections_changed",
        WorkbenchChange::ControlsChanged { .. } => "controls_changed",
        WorkbenchChange::PartChanged { .. } => "part_changed",
    };
    let facade = super::WorkbenchFacade::new(
        super::WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None),
        std::sync::Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let look = facade.look();
    let operation = |phase| OperationSummary {
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: "reading".into(),
        activity_code: crate::browser::adapter::PresentationActivity::Read,
        capability: "read".into(),
        started_at_ms: Some(1),
        phase,
        provenance: None,
        agent: crate::agent::Agent::unknown(),
    };
    let record = || {
        Box::new(HistoryItem::from(crate::audit::AuditRecord::now(
            "invocation_1",
            "workspace_1",
            "browser_read",
            Capability::Read,
            "authority_1",
            Decision::permitted(),
            "succeeded",
            "read",
            &crate::language::outcome::Outcome::TextRead {
                words: 3,
                host: None,
            }
            .audit(),
            10,
        )))
    };
    let samples = [
        WorkbenchChange::AuditHealthChanged {
            health: crate::language::audit_health::AuditHealth::default(),
        },
        WorkbenchChange::DocumentCoverageChanged {
            details: crate::language::coverage::HumanCoverage {
                summary: String::new(),
                explanation: String::new(),
                invocation: "invocation_1".into(),
                coverage: crate::language::coverage::Coverage::default(),
                excluded_hosts: Vec::new(),
                proactive: false,
            },
        },
        WorkbenchChange::OperationStarted {
            operation: operation(OperationPhase::Running),
        },
        WorkbenchChange::OperationChanged {
            operation: operation(OperationPhase::Waiting),
        },
        WorkbenchChange::OperationSettled { record: record() },
        WorkbenchChange::CompositionChanged { record: record() },
        WorkbenchChange::RuntimeChanged {
            runtime_state: crate::browser::adapter::RuntimeControlState::Held,
        },
        WorkbenchChange::StatusChanged {
            status: look.status(),
        },
        WorkbenchChange::ConnectionsChanged {
            connections: look.connections(),
        },
        WorkbenchChange::ControlsChanged {
            controls: Box::new(look.controls()),
        },
        WorkbenchChange::PartChanged {
            part: HintedPart::Integrations,
            etag: "e1".into(),
        },
    ];
    let kinds = [
        "audit_health_changed",
        "document_coverage_changed",
        "operation_started",
        "operation_changed",
        "operation_settled",
        "composition_changed",
        "runtime_changed",
        "status_changed",
        "connections_changed",
        "controls_changed",
        "part_changed",
    ];
    assert_eq!(samples.each_ref().map(named), kinds);
    assert_eq!(
        samples
            .each_ref()
            .map(|change| serde_json::to_value(change).unwrap()["kind"].clone()),
        kinds.map(serde_json::Value::from)
    );

    // Every address the service sends is the page's, in the page's order, and reads back.
    assert_eq!(
        PageAddress::ALL.map(PageAddress::as_str).to_vec(),
        page_list(PAGE_WORDS, "ADDRESSES")
    );
    for address in PageAddress::ALL {
        assert_eq!(PageAddress::parse(address.as_str()), Some(address));
        assert_eq!(
            serde_json::from_value::<PageAddress>(address.as_str().into()).unwrap(),
            address
        );
    }

    // The intents the page's buttons carry; tests/workbench-settings.test.mjs reads them from the
    // markup.
    let intent = |intent: WorkbenchRuntimeIntent| match intent {
        WorkbenchRuntimeIntent::Hold => "hold",
        WorkbenchRuntimeIntent::Resume => "resume",
        WorkbenchRuntimeIntent::EndSession => "end_session",
        WorkbenchRuntimeIntent::StartSession => "start_session",
    };
    for sent in [
        WorkbenchRuntimeIntent::Hold,
        WorkbenchRuntimeIntent::Resume,
        WorkbenchRuntimeIntent::EndSession,
        WorkbenchRuntimeIntent::StartSession,
    ] {
        assert_eq!(
            serde_json::from_value::<WorkbenchRuntimeIntent>(intent(sent).into()).ok(),
            Some(sent)
        );
    }
    // The destinations the page names; PRODUCT_DESTINATIONS in tests/workbench-settings.test.mjs
    // is the page's copy (About offers every one but get_chrome, which only the setup view offers).
    assert_eq!(
        WorkbenchDestination::all().map(WorkbenchDestination::key),
        [
            "home",
            "demo",
            "decision_aid",
            "source",
            "install_guide",
            "governance_guide",
            "scripting_guide",
            "audit_guide",
            "trust_center",
            "decision_records",
            "licensing_guide",
            "sylin_tools",
            "get_chrome",
        ]
    );
    for destination in WorkbenchDestination::all() {
        assert_eq!(
            serde_json::from_value::<WorkbenchDestination>(destination.key().into()).ok(),
            Some(destination),
            "{} is not the name the service reads",
            destination.key()
        );
    }

    // The page's copies of the operation phases, the runtime states and the window contexts.
    let table = |source: &str, name: &str| -> BTreeSet<String> {
        page_table(source, name)
            .into_iter()
            .map(|(key, value)| {
                assert_eq!(key, value, "{name}.{key} is its own wire word");
                value.to_owned()
            })
            .collect()
    };
    let phases = [
        OperationPhase::Waiting,
        OperationPhase::Running,
        OperationPhase::Held,
        OperationPhase::Blocked,
        OperationPhase::Completed,
    ];
    for phase in phases {
        match phase {
            OperationPhase::Waiting
            | OperationPhase::Running
            | OperationPhase::Held
            | OperationPhase::Blocked
            | OperationPhase::Completed => {}
        }
    }
    assert_eq!(
        table(PAGE_WORDS, "PHASE"),
        phases.into_iter().map(wire).collect::<BTreeSet<_>>()
    );
    let runtime = [
        RuntimeControlState::Active,
        RuntimeControlState::Held,
        RuntimeControlState::Ended,
    ];
    for state in runtime {
        match state {
            RuntimeControlState::Active
            | RuntimeControlState::Held
            | RuntimeControlState::Ended => {}
        }
    }
    assert_eq!(
        table(PAGE_WORDS, "RUNTIME"),
        runtime.into_iter().map(wire).collect::<BTreeSet<_>>()
    );
    let contexts = [WebContext::Control, WebContext::Watch, WebContext::Closed];
    for context in contexts {
        match context {
            WebContext::Control | WebContext::Watch | WebContext::Closed => {}
        }
    }
    assert_eq!(
        table(PAGE_TRANSPORT, "CONTEXT"),
        contexts
            .into_iter()
            .map(|context| context.token().to_owned())
            .collect::<BTreeSet<_>>()
    );
    // The setup part the page listens for, the destination its Chrome button opens and the act
    // its connect buttons ask for.
    assert_eq!(
        page_constant(PAGE_TRANSPORT, "SETUP_PART"),
        Part::Setup.token()
    );
    assert_eq!(
        page_constant(PAGE_TRANSPORT, "GET_CHROME"),
        WorkbenchDestination::GetChrome.key()
    );
    assert_eq!(
        serde_json::from_value::<HarnessAction>(
            page_constant(PAGE_TRANSPORT, "CONNECT_ACT").into()
        )
        .ok(),
        Some(HarnessAction::Install)
    );

    // Every setting the page writes is registered: a key governance does not know would make the
    // page's own draft one the validator refuses.
    let mut written = page_keys(PAGE_WORDS, "const SETTING_GROUPS = [", "\n  ];");
    written.extend(page_keys(
        PAGE_POLICY_VIEW,
        "function settingEntries(",
        "return entries;",
    ));
    written.push(page_constant(PAGE_WORDS, "SACRED_KEY"));
    assert!(written.contains(&PolicySetting::NeverTouch.key()));
    for key in written {
        assert!(
            PolicySetting::from_key(key).is_some(),
            "the Policy page writes {key}, which governance does not register"
        );
    }

    // The parts the page asks a snapshot for.
    for token in page_list(PAGE_TRANSPORT, "SNAPSHOT_PARTS") {
        assert!(
            Part::from_token(token).is_ok(),
            "the server knows no part {token}"
        );
    }
    assert!(Part::from_token("nowhere").is_err());
}
