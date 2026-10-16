//! Guards over the workbench page: what its markup, styles and scripts must keep.
//!
//! The page is plain files under `ui/`, served by the loopback server and drawn by the window and
//! by browser tabs alike. These tests read those files as text beside the Rust vocabularies they
//! must agree with (tools, readiness, capabilities, destinations), so a change on either side
//! fails here instead of in a person's window.

#[test]
fn workbench_uses_the_original_ghostlight_artwork_byte_for_byte() {
    assert_eq!(
        include_bytes!("../../ui/ghostlight.png"),
        include_bytes!("../../../../extension/icons/icon128.png")
    );
}

#[test]
fn the_surface_uses_the_published_ghostlight_palette() {
    let styles = include_str!("../../ui/styles.css");
    // Ghostlight's accent and the night-garden ground, as published on sylin.org.
    for value in ["#5eead4", "94, 234, 212", "#0f0e12"] {
        assert!(
            styles.contains(value),
            "the workbench dropped the published palette value {value}"
        );
    }
}

#[test]
fn the_surface_and_the_page_share_one_motion_curve() {
    let styles = include_str!("../../ui/styles.css");
    let renderer = include_str!("../browser/adapter/chromium/protocol4/assets/presentation.js");
    assert!(styles.contains("cubic-bezier(.22, 1, .36, 1)"));
    assert!(renderer.contains("cubic-bezier(.22,1,.36,1)"));
}

#[test]
fn the_surface_handles_every_change_the_orchestrator_can_publish() {
    use crate::browser::adapter::RuntimeControlState;

    use crate::governance::{Capability, CapabilitySet};
    use crate::workbench::parts::{ConnectionsPart, HintedPart, StatusPart};
    use crate::workbench::{
        HistoryItem, OperationPhase, OperationSummary, WorkbenchChange, WorkbenchFacade,
        WorkbenchProjection,
    };

    let app = &surface_source();
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        crate::governance::GovernanceFacade::new(None, None),
        std::sync::Arc::new(crate::browser::RelayBrowserPort::new(
            "guard".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let snapshot = facade.snapshot();
    let operation = OperationSummary {
        provenance: None,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        activity: "Reading page".into(),
        capability: Capability::Read.as_str().into(),
        started_at_ms: Some(0),
        phase: OperationPhase::Running,
    };
    let record = HistoryItem {
        provenance: None,
        storage: crate::language::audit_health::Storage::Saved,
        storage_detail: String::new(),
        complete: true,
        composition: None,
        steps: vec![],
        permissions: Default::default(),
        permission_explanations: vec![],
        timestamp_ms: 0,
        invocation: "invocation_1".into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        capability: "read".into(),
        requirements: CapabilitySet::READ,
        allowed: true,
        reason: "permitted".into(),
        policy_tier: None,
        grant_id: None,
        denial_id: None,
        status: "succeeded".into(),
        effect: "none".into(),
        summary: "Read 1,240 words from example.com.".into(),
        duration_ms: 1200,
        observed: sample_observation(),
        channel: Some(crate::intake::session::IntakeChannel::Mcp),
        browser: None,
    };

    for change in [
        WorkbenchChange::OperationStarted {
            operation: operation.clone(),
        },
        WorkbenchChange::OperationChanged { operation },
        WorkbenchChange::OperationSettled {
            record: Box::new(record),
        },
        WorkbenchChange::RuntimeChanged {
            runtime_state: RuntimeControlState::Active,
        },
        WorkbenchChange::StatusChanged {
            status: StatusPart::of(&snapshot),
        },
        WorkbenchChange::ConnectionsChanged {
            connections: ConnectionsPart::of(&snapshot),
        },
        WorkbenchChange::ControlsChanged {
            controls: Box::new(snapshot.controls.clone()),
        },
        WorkbenchChange::PartChanged {
            part: HintedPart::Integrations,
            etag: String::new(),
        },
    ] {
        let encoded = serde_json::to_value(&change).expect("changes serialize");
        let kind = encoded["kind"].as_str().expect("every change is tagged");
        assert!(
            app.contains(&format!("case \"{kind}\":")),
            "the workbench does not handle the {kind} change"
        );
    }
}

/// One observation with every field populated, so a guard sees the whole vocabulary.
fn sample_observation() -> crate::language::outcome::Observed {
    crate::language::outcome::Observed {
        host: Some("example.com".into()),
        readiness: Some("complete".into()),
        count: Some(1240),
        width: Some(1280),
        height: Some(720),
        rebound: true,
    }
}

#[test]
fn the_band_latches_working_instead_of_tracking_each_operation() {
    let app = &surface_source();
    // Most calls settle in well under a second. A word tied to them strobes, so it latches
    // and every new action pushes the deadline back.
    assert!(app.contains("const WORKING_LATCH_MS = 10_000;"));
    assert!(
        app.contains("now() - state.interactionAt < WORKING_LATCH_MS"),
        "the band no longer latches on recent interaction"
    );
    assert!(
        !app.contains(r#"state.feed.some(isRunning) ? "runtime-working""#),
        "the band still reads its word straight off the live operations"
    );
    // Nothing else wakes the band once the last operation settles, so the latch has to
    // schedule its own expiry or the word stays lit until an unrelated repaint.
    assert!(
        app.contains("state.latchTimer = setTimer(() => emit(CHANGE.Band), WORKING_LATCH_MS"),
        "the latch never expires on its own"
    );
    // The negative control: every operation event still refreshes it, so the latch is fed by
    // real work rather than by one arbitrary moment at startup.
    for event in [
        "case \"operation_started\": touch();",
        "case \"operation_changed\": touch();",
        "case \"operation_settled\": touch();",
    ] {
        assert!(
            app.contains(event),
            "an operation event does not refresh the latch: {event}"
        );
    }
}

#[test]
fn about_wears_the_published_card_and_the_products_own_artwork() {
    let markup = include_str!("../../ui/index.html");
    let styles = include_str!("../../ui/styles.css");

    assert!(
        markup.contains(r#"data-view="about""#),
        "About is not reachable from the band"
    );
    assert!(markup.contains(r#"id="view-about""#), "About has no view");

    // The card is the sylin.org card: same anatomy, same holo layers.
    for part in [
        "card-art",
        "card-divider",
        "card-disc",
        "card-notch",
        "card-pane",
        "card-title",
        "card-rules",
        "card-foot",
        "holo2",
    ] {
        assert!(markup.contains(part), "the About card is missing {part}");
    }
    assert!(
        styles.contains("mix-blend-mode: color-dodge"),
        "the foil layer lost its blend"
    );
    // The surface already owns .card for its diagnostic panels. The ported card keeps its own
    // root so restyling one never restyles the other.
    assert!(
        markup.contains(r#"<button class="tcg" id="about-card""#),
        "the About card is not scoped away from the diagnostic cards"
    );
    // One artwork. The card wears the same bytes the extension ships as its icon, so the
    // character cannot drift between the two places a person meets it.
    assert!(
        markup.contains(r#"class="px mascot" src="ghostlight.png""#),
        "the card does not wear the product's own artwork"
    );
}

/// Every script the window loads, in the order the page loads it.
///
/// A guard should care what the surface does, not which module a line happens to live in.
/// Reading one file made every assertion quietly depend on the file layout, so splitting the
/// surface broke guards that were still perfectly true.
fn surface_source() -> String {
    [
        include_str!("../../ui/lib/words.js"),
        include_str!("../../ui/lib/entries.js"),
        include_str!("../../ui/lib/transport.js"),
        include_str!("../../ui/lib/store.js"),
        include_str!("../../ui/lib/view.js"),
        include_str!("../../ui/kit/settings.js"),
        include_str!("../../ui/app.js"),
    ]
    .join(
        "
",
    )
}

#[test]
fn the_pure_layers_of_the_surface_hold_no_state_and_no_document() {
    let words = include_str!("../../ui/lib/words.js");
    let entries = include_str!("../../ui/lib/entries.js");
    let store = include_str!("../../ui/lib/store.js");
    let transport = include_str!("../../ui/lib/transport.js");
    // These two exist to be readable and testable without a browser. The moment either
    // reaches for the document or keeps state of its own, that stops being true and the
    // seam has quietly moved back.
    // The store keeps the cache and the transport talks to the orchestrator. Neither may
    // touch the document: that is what stops a rendering fault from corrupting what the
    // window believes, and what lets both be exercised with no browser present.
    for (name, module) in [
        ("words", words),
        ("entries", entries),
        ("store", store),
        ("transport", transport),
    ] {
        for forbidden in ["document.", "window.", "el["] {
            assert!(
                !module.contains(forbidden),
                "{name} reaches for {forbidden}, so it is no longer a pure layer"
            );
        }
    }

    // The negative control: the composition root is where the document belongs, so a rule
    // that held everywhere would be measuring nothing.
    let view = include_str!("../../ui/lib/view.js");
    assert!(
        view.contains("document.querySelectorAll"),
        "the view is where the document belongs, so a rule that held everywhere would be              measuring nothing"
    );
}

#[test]
fn every_element_the_surface_reaches_for_exists_in_the_markup() {
    let app = &surface_source();
    let markup = include_str!("../../ui/index.html");

    let ids: Vec<&str> = markup
        .match_indices("id=\"")
        .filter_map(|(at, _)| {
            let rest = &markup[at + 4..];
            rest.find('"').map(|end| &rest[..end])
        })
        .collect();
    // The negative control: a scrape that finds nothing would let every assertion below pass
    // while proving nothing at all.
    assert!(ids.len() > 20, "the id scrape found only {} ids", ids.len());

    let mut referenced = Vec::new();
    for (at, _) in app.match_indices("el[\"") {
        let rest = &app[at + 4..];
        if let Some(end) = rest.find('"') {
            referenced.push(&rest[..end]);
        }
    }
    assert!(
        referenced.len() > 10,
        "the reference scrape found only {} lookups",
        referenced.len()
    );

    // A node the surface reaches for but the markup never defines reads as undefined and
    // throws on first use. One of those took the whole window down: the boot sequence
    // abandoned the snapshot, the change subscription, and the heartbeat behind it.
    for name in referenced {
        assert!(
            ids.contains(&name),
            "the surface reaches for #{name}, which the markup does not define"
        );
    }
}

#[test]
fn a_surface_that_cannot_draw_is_not_reported_as_a_lost_connection() {
    let app = &surface_source();
    // One catch around both the fetch and the render said "Not connected" for either, which
    // sends whoever is reading it to look at the orchestrator when the fault is in here.
    assert!(
        app.contains(r#"attempt("rendering the snapshot", () =>"#),
        "a render failure is still indistinguishable from a lost connection"
    );
    // Every wiring statement was at module scope ahead of boot, where one failure to attach
    // took the snapshot and the heartbeat with it.
    assert!(app.contains("function wire() {"));
    assert!(
        !app.contains("document.getElementById(\"refresh-status\")"),
        "a listener still reaches around the derived table at module scope"
    );
}

#[test]
fn a_failed_paint_is_retried_rather_than_remembered_as_done() {
    let app = &surface_source();
    // The signature must be recorded only after the paint succeeded. Recording first, with a
    // memo that is never cleared, remembers a panel that threw as finished and leaves it
    // blank for the life of the window.
    assert!(
        app.contains("if (attempt(`painting ${key}`, paint)) painted[key] = signature;"),
        "a failed paint is still memoised as a completed one"
    );
    // And nothing may fail where only a console would notice.
    assert!(app.contains(r#"window.addEventListener("error""#));
    assert!(app.contains(r#"window.addEventListener("unhandledrejection""#));
    assert!(
        app.contains("if (view?.el.toast) view.toast(detail, true);"),
        "failures never reach the person using the window"
    );
}

#[test]
fn the_about_page_can_reach_only_the_destinations_the_product_chose() {
    use crate::workbench::WorkbenchDestination;

    let markup = include_str!("../../ui/index.html");
    let app = &surface_source();

    // The surface holds no addresses at all. Without this, "closed vocabulary" would be a
    // convention rather than a property, and one hand-written anchor would quietly undo it.
    assert!(
        !markup.contains("href=\"http"),
        "the About markup still carries a raw URL"
    );
    assert!(
        !app.contains("https://"),
        "the surface script still carries a raw URL"
    );
    assert!(app.contains(r#"call("open_destination", { destination })"#));

    // Every destination the orchestrator will open is offered, and every name the surface
    // offers is one the orchestrator knows. Either half alone lets the two drift. The setup view
    // reaches Get Chrome, which is Google's page and not one of the product's own.
    for destination in WorkbenchDestination::all()
        .into_iter()
        .filter(|destination| *destination != WorkbenchDestination::GetChrome)
    {
        let key = destination.key();
        assert!(
            app.contains(&format!("[\"{key}\", \""))
                || markup.contains(&format!(r#"data-destination="{key}""#)),
            "{key} is a destination nobody can reach"
        );
        let url = destination.url();
        assert!(
            url.starts_with("https://sylin.org/")
                || url.starts_with("https://github.com/sylin-org/"),
            "{key} points outside the product's own surfaces: {url}"
        );
    }

    // Documentation deliberately points at dev: main still carries the 0.8 line, so a main
    // link would answer questions about a product this window is not running.
    for destination in WorkbenchDestination::all() {
        let url = destination.url();
        assert!(
            !url.contains("/blob/main/") && !url.contains("/tree/main/"),
            "{} points at main, which is still the 0.8 line: {url}",
            destination.key()
        );
    }
}

#[test]
fn the_card_mascot_renders_at_a_whole_multiple_of_its_sprite() {
    // The expectation is read off the artwork, not written down beside it. Swap the sprite
    // for one of a different size and this fails until the card is resized to match.
    let sprite = include_bytes!("../../ui/ghostlight.png");
    assert_eq!(&sprite[1..4], b"PNG", "the mascot is not a PNG");
    let native = u32::from_be_bytes([sprite[16], sprite[17], sprite[18], sprite[19]]);
    let tall = u32::from_be_bytes([sprite[20], sprite[21], sprite[22], sprite[23]]);
    assert_eq!(native, tall, "a square sprite is assumed by the card art");

    let styles = include_str!("../../ui/styles.css");
    let rule = styles
        .split_once(".tcg .card-art .mascot {")
        .expect("the card has no mascot rule")
        .1
        .split_once('}')
        .expect("the mascot rule never closes")
        .0;
    let rendered: u32 = rule
        .split_once("width:")
        .expect("the mascot has no width")
        .1
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("the mascot width is not a plain pixel count");

    assert!(
        rendered >= native,
        "the sprite is rendered below its native size"
    );
    assert_eq!(
        rendered % native,
        0,
        "the mascot renders at {rendered}px from a {native}px sprite, so source pixels split              unevenly across screen pixels and the art aliases however it is filtered"
    );
    let sprite_rule = styles
        .split_once(".tcg img.px {")
        .expect("the card sprite has no pixel rule")
        .1
        .split_once('}')
        .expect("the pixel rule never closes")
        .0;
    assert!(
        sprite_rule.contains("image-rendering: pixelated"),
        "the sprite is being smoothed"
    );
}

#[test]
fn the_connections_bar_groups_by_client_rather_than_by_connection() {
    let app = &surface_source();
    // One chip per client. A client that opens a session per request otherwise fills the bar
    // with identical names that tell the user nothing.
    assert!(
        app.contains("function connectionGroups(sessions)"),
        "the connections bar does not group its sessions"
    );
    assert!(
        app.contains("connectionGroups(snapshot.sessions).map"),
        "the connections bar is not painted from the grouped sessions"
    );
    assert!(
        !app.contains("snapshot.sessions.map(session =>"),
        "the connections bar still paints one chip per connection"
    );
    // The negative control: grouping is a rendering choice for one bar, not a change to the
    // data. History attribution still resolves a single workspace to the client that owns it,
    // and that lookup breaks the moment someone collapses the sessions array itself.
    assert!(
        app.contains("state.snapshot?.sessions.find((item) => item.id === workspace)"),
        "per-session attribution was lost when the bar was grouped"
    );
}

#[test]
fn surface_renders_seam_facts_and_trusts_outcome_language_for_measurements() {
    let app = &surface_source();
    // Readiness is the only observed fact no sentence states, so it is the only one the
    // surface must read structurally. The host is guarded where it is collected instead: an
    // assertion that the surface renders it separately would only pin a second rendering of
    // what the sentence already says.
    assert!(
        app.contains("observed.readiness"),
        "the workbench never renders seam-owned readiness evidence"
    );
    assert!(app.contains("const body = sentence(entry);"));
    assert!(
        !app.contains("measured("),
        "the surface is still guessing which outcome register to render"
    );
    assert!(
        !app.contains("observed.host"),
        "the hero says the host twice: the sentence already names it"
    );
}

#[test]
fn managed_policy_passport_surfaces_every_signed_provenance_fact() {
    // Optional chaining is ordinary in the surface, so the guard compares the reach itself
    // rather than the punctuation around it. What it protects is unchanged: every signed
    // provenance fact is rendered somewhere a person can see it.
    let app = &surface_source().replace("passport?.", "passport.");
    for field in [
        "organization",
        "rationale",
        "contacts",
        "sequence",
        "freshness",
        "source_class",
        "last_success_ms",
    ] {
        assert!(
            app.contains(&format!("passport.{field}")),
            "the Policy Passport does not render {field}"
        );
    }
}

#[test]
fn every_observed_fact_is_documented_where_it_is_collected() {
    // The audit record's consumer is a person configuring a collector, so the guide is where
    // the vocabulary has to stay honest. A new field that nobody documents fails here.
    let guide = include_str!("../../../../docs/guides/siem-integration.md");
    let encoded = serde_json::to_value(sample_observation()).expect("observations serialize");
    let fields = encoded.as_object().expect("an observation is an object");
    for field in fields.keys() {
        assert!(
            guide.contains(&format!("`{field}`")),
            "siem-integration.md documents no {field} field, so collectors cannot expect it"
        );
    }
}

#[test]
fn every_row_cell_has_a_grid_track_at_every_width() {
    // A row is a CSS grid, so adding a cell without adding a track silently shifts every
    // column after it. Both numbers are derived here rather than pinned, and the narrow
    // layouts are checked against what they hide.
    let app = &surface_source();
    let styles = include_str!("../../ui/styles.css");

    let markup = app
        .split_once("function rowMarkup(entry) {")
        .and_then(|(_, rest)| rest.split_once("\n    }"))
        .map(|(body, _)| body)
        .expect("the surface still builds rows");
    let spanning = markup.matches("<div class=\"row-history\"").count();
    if spanning > 0 {
        let span = styles
            .split_once(".row-history {")
            .unwrap()
            .1
            .split_once('}')
            .unwrap()
            .0;
        assert!(
            span.contains("grid-column: 2 / -1"),
            "history detail must span the row below its cells"
        );
    }
    let cells = markup.matches("<div class=\"").count() - spanning;
    assert!(cells >= 6, "expected a row of cells, saw {cells}");

    fn tracks(block: &str) -> usize {
        let value = block
            .split_once("grid-template-columns:")
            .and_then(|(_, rest)| rest.split_once(';'))
            .map(|(value, _)| value)
            .expect("a row grid declares its columns");
        // minmax(0, 1fr) is one track that contains a space; collapse it before counting.
        let mut collapsed = String::new();
        let mut depth = 0_usize;
        for character in value.chars() {
            match character {
                '(' => depth += 1,
                ')' => depth -= 1,
                c if c.is_whitespace() && depth > 0 => continue,
                _ => {}
            }
            collapsed.push(character);
        }
        collapsed.split_whitespace().count()
    }

    // Narrower media queries stack on wider ones, so what a width hides is everything hidden
    // up to and including its own section.
    let mut hidden = 0_usize;
    for (index, section) in styles.split("@media").enumerate() {
        hidden += section
            .lines()
            .filter(|line| line.contains("display: none"))
            .flat_map(|line| line.split(','))
            .filter(|selector| {
                selector.trim_start().starts_with(".row-")
                    && !selector.split('{').next().unwrap_or("").contains(':')
            })
            .count();
        let Some((_, rest)) = section.split_once(".row {") else {
            continue;
        };
        let Some((block, _)) = rest.split_once('}') else {
            continue;
        };
        if !block.contains("grid-template-columns") {
            continue;
        }
        assert_eq!(
            tracks(block) + hidden,
            cells,
            "row layout {index} declares {} tracks and hides {hidden} of {cells} cells",
            tracks(block)
        );
    }
}

#[test]
fn every_readiness_the_surface_can_receive_has_a_note() {
    use crate::work::result::Readiness;

    let app = &surface_source();
    // Read the table itself rather than the whole file: several of these words appear in the
    // effect story too, and a guard that matches anywhere would pass without the table.
    let notes = app
        .split_once("const READINESS_NOTE = {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(block, _)| block)
        .expect("the surface keeps a readiness note table");
    // Exhaustive on purpose: a new readiness must not compile until the surface decides what
    // it says about a settled row.
    for readiness in [
        Readiness::NotApplicable,
        Readiness::Loading,
        Readiness::Interactive,
        Readiness::Complete,
        Readiness::Unknown,
    ] {
        let name = serde_json::to_value(readiness).expect("readiness serializes");
        let name = name.as_str().expect("readiness encodes as a string");
        assert!(
            notes.contains(&format!("{name}:")),
            "the surface has no note for {name} readiness"
        );
    }
}

#[test]
fn every_catalog_tool_has_a_medallion() {
    let app = &surface_source();
    let tools = crate::language::catalog();
    assert_eq!(
        tools.len(),
        crate::language::CATALOG_TOOL_COUNT,
        "expected the complete catalog"
    );
    for tool in tools {
        assert!(
            app.contains(&format!("{}: \"", tool.name)),
            "{} has no medallion, so its row would fall back to a generic glyph",
            tool.name
        );
    }
}

#[test]
fn every_runtime_intent_stays_reachable_from_the_surface() {
    use crate::workbench::WorkbenchRuntimeIntent;

    let surface = format!(
        "{}{}",
        include_str!("../../ui/index.html"),
        surface_source()
    );
    // This match is exhaustive on purpose: a new runtime intent must not compile until
    // someone decides where the workbench offers it.
    let names = [
        WorkbenchRuntimeIntent::Hold,
        WorkbenchRuntimeIntent::Resume,
        WorkbenchRuntimeIntent::EndSession,
        WorkbenchRuntimeIntent::StartSession,
    ]
    .map(|intent| match intent {
        WorkbenchRuntimeIntent::Hold => "hold",
        WorkbenchRuntimeIntent::Resume => "resume",
        WorkbenchRuntimeIntent::EndSession => "end_session",
        WorkbenchRuntimeIntent::StartSession => "start_session",
    });

    for name in names {
        serde_json::from_value::<WorkbenchRuntimeIntent>(serde_json::Value::String(name.into()))
            .unwrap_or_else(|_| panic!("the adapter no longer accepts the {name} intent"));
        assert!(
            surface.contains(&format!("\"{name}\"")),
            "the workbench can no longer request the {name} intent"
        );
    }
}

#[test]
fn every_capability_class_the_surface_can_receive_has_a_visual_treatment() {
    use crate::governance::Capability;

    let app = &surface_source();
    let styles = include_str!("../../ui/styles.css");
    for capability in [
        Capability::Read,
        Capability::Action,
        Capability::Write,
        Capability::Execute,
    ] {
        let name = serde_json::to_value(capability).expect("capabilities serialize");
        let name = name.as_str().expect("capabilities encode as strings");
        assert!(
            app.contains(&format!("{name}: \"cap-{name}\"")),
            "the workbench cannot classify the {name} capability"
        );
        assert!(
            styles.contains(&format!(".cap-{name} {{")),
            "the {name} capability has no visual treatment"
        );
    }
}

#[test]
fn the_stylesheets_carry_a_forced_colors_block_for_the_band_the_tabs_and_the_rail() {
    // Windows High Contrast replaces the page's colours with the system's. Washes and tints
    // vanish, so a selection must be told by a border or a system colour, never by a wash alone.
    let styles = [
        include_str!("../../ui/styles.css"),
        include_str!("../../ui/kit/kit.css"),
    ]
    .join("\n");
    let covered: String = styles
        .split("@media (forced-colors: active)")
        .skip(1)
        .map(|rest| rest.split("@media").next().unwrap_or_default())
        .collect();
    assert!(
        !covered.is_empty(),
        "the stylesheets have no forced-colors rules"
    );
    for selector in [
        ".lampband",
        ".state-word",
        ".tab",
        ".tab.active",
        ".rail-entry",
        ".rail-entry.active",
        ".rail-state",
    ] {
        assert!(
            covered.contains(selector),
            "forced colours leave {selector} unstyled"
        );
    }
    for system in ["ButtonBorder", "Highlight", "HighlightText"] {
        assert!(covered.contains(system), "no rule uses the {system} colour");
    }
}
