// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Guards the paste-ready Chrome Web Store permission justifications.
//!
//! Each section of `docs/legal/PERMISSION_JUSTIFICATIONS.md` must carry one fenced
//! paste block that fits Chrome's dashboard field limit, so a store submission never
//! discovers a truncated justification by hand.

const PERMISSION_COPY: &str = include_str!("../../../docs/legal/PERMISSION_JUSTIFICATIONS.md");
const MAX_DASHBOARD_JUSTIFICATION_CHARS: usize = 1_000;
const REQUIRED_SECTIONS: &[&str] = &[
    "tabs",
    "debugger",
    "downloads",
    "nativeMessaging",
    "offscreen",
    "storage",
    "tabGroups",
    "tabs",
    "webNavigation",
    "windows",
    "Host permissions: `http://*/*` and `https://*/*`",
    "Page-context JavaScript",
];

fn paste_block<'a>(document: &'a str, section: &str) -> &'a str {
    let marker = format!("## {section}\n");
    let section_body = document
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing permission-justification section: {section}"))
        .1
        .split("\n## ")
        .next()
        .expect("section body");
    section_body
        .split_once("```text\n")
        .unwrap_or_else(|| panic!("section {section} must contain a fenced text block"))
        .1
        .split_once("\n```")
        .unwrap_or_else(|| panic!("section {section} has an unclosed fenced text block"))
        .0
        .trim()
}

#[test]
fn chrome_permission_justifications_fit_dashboard_limit() {
    let normalized = PERMISSION_COPY.replace("\r\n", "\n");
    for section in REQUIRED_SECTIONS {
        let copy = paste_block(&normalized, section);
        let character_count = copy.chars().count();
        assert!(
            !copy.is_empty(),
            "section {section} has empty dashboard copy"
        );
        assert!(
            character_count <= MAX_DASHBOARD_JUSTIFICATION_CHARS,
            "section {section} is {character_count} characters; Chrome allows at most \
             {MAX_DASHBOARD_JUSTIFICATION_CHARS}"
        );
    }
}
