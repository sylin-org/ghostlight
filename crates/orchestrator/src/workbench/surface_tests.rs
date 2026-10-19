//! Properties of the workbench page that no behavioral test can show, read from its files.
//!
//! The page is plain files under `ui/`. Its behavior is tested by running its real modules in
//! `tests/workbench-store.test.mjs`, `tests/workbench-transport.test.mjs` and
//! `tests/workbench-settings.test.mjs`. What stays here is what running a module cannot reveal:
//! the product's artwork byte for byte, the palette and motion curve the page shares with
//! sylin.org and the page assets, the layers that must never reach for the document, a surface
//! that carries no address of its own, and words the page keys by the service's wire values.

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

/// Every script the window loads, in the order the page loads it, as one text.
fn surface_source() -> String {
    [
        include_str!("../../ui/lib/words.js"),
        include_str!("../../ui/lib/entries.js"),
        include_str!("../../ui/lib/transport.js"),
        include_str!("../../ui/lib/icons.js"),
        include_str!("../../ui/lib/setup-view.js"),
        include_str!("../../ui/lib/store.js"),
        include_str!("../../ui/lib/view.js"),
        include_str!("../../ui/kit/settings.js"),
        include_str!("../../ui/app.js"),
    ]
    .join("\n")
}

/// Whether `module` uses `reach`: a member of `document` or `window` (a sentence that ends on
/// the word "window." uses neither), or an element looked up by `el[`.
fn reaches_for(module: &str, reach: &str) -> bool {
    if !reach.ends_with('.') {
        return module.contains(reach);
    }
    module.match_indices(reach).any(|(at, _)| {
        module[at + reach.len()..]
            .chars()
            .next()
            .is_some_and(|next| next.is_ascii_alphabetic() || next == '_' || next == '$')
    })
}

#[test]
fn the_pure_layers_of_the_surface_hold_no_state_and_no_document() {
    let words = include_str!("../../ui/lib/words.js");
    let entries = include_str!("../../ui/lib/entries.js");
    let store = include_str!("../../ui/lib/store.js");
    let transport = include_str!("../../ui/lib/transport.js");
    // Words and entries exist to be readable and testable without a browser. The store keeps the
    // cache and the transport talks to the orchestrator. None of them may touch the document:
    // that is what stops a rendering fault from corrupting what the window believes, and what
    // lets each be exercised with no browser present.
    for (name, module) in [
        ("words", words),
        ("entries", entries),
        ("store", store),
        ("transport", transport),
    ] {
        for forbidden in ["document.", "window.", "el["] {
            assert!(
                !reaches_for(module, forbidden),
                "{name} reaches for {forbidden}, so it is no longer a pure layer"
            );
        }
    }

    // The negative control: the view is where the document belongs, so a rule that held
    // everywhere would be measuring nothing.
    assert!(!reaches_for(
        r#""Changes are made in the Ghostlight window.""#,
        "window."
    ));
    let view = include_str!("../../ui/lib/view.js");
    assert!(
        reaches_for(view, "document."),
        "the view is where the document belongs, so a rule that held everywhere would be \
         measuring nothing"
    );
}

#[test]
fn the_surface_carries_no_address_of_its_own() {
    // The page names a destination and the orchestrator owns the address, so the page cannot
    // send anyone anywhere the product did not choose. One hand-written link would undo that.
    assert!(
        !include_str!("../../ui/index.html").contains("href=\"http"),
        "the markup carries a raw URL"
    );
    assert!(
        !surface_source().contains("https://"),
        "the surface script carries a raw URL"
    );
}

#[test]
fn the_page_has_a_word_for_every_state_the_roster_sends_and_no_other() {
    use std::collections::BTreeSet;

    use crate::install::HarnessState;

    let words = include_str!("../../ui/lib/words.js");
    let start = words
        .find("const AGENT_STATE_WORDS = Object.freeze({")
        .expect("the page names the agent states");
    let block = &words[start..];
    let block = &block[block.find('{').unwrap() + 1..block.find("});").unwrap()];
    let keys: BTreeSet<&str> = block
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, _)| key.trim())
        .collect();
    let states = [
        HarnessState::NotDetected,
        HarnessState::Available,
        HarnessState::Installed,
        HarnessState::Updatable,
        HarnessState::NeedsAttention,
    ];
    // A new state fails to compile here, so it cannot reach the page without a word.
    for state in states {
        match state {
            HarnessState::NotDetected
            | HarnessState::Available
            | HarnessState::Installed
            | HarnessState::Updatable
            | HarnessState::NeedsAttention => {}
        }
    }
    let mut expected: BTreeSet<String> = states
        .iter()
        .map(|state| {
            serde_json::to_value(state)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    // The one standing the page adds: a copy-only agent found here whose file it never reads.
    expected.insert("by_hand".to_owned());
    assert_eq!(
        keys,
        expected.iter().map(String::as_str).collect::<BTreeSet<_>>()
    );
}
