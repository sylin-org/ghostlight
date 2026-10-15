// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Guards the paste-ready Chrome Web Store permission justifications.
//!
//! The Chrome dashboard needs one justification per manifest permission and truncates each
//! field at 1000 characters. On 2026-08-22 the `downloads` and `offscreen` permissions shipped
//! in the manifest without a justification, and a hand-written section list let that pass. The
//! sections are therefore derived from the document itself, and the bare-identifier headings
//! must equal the manifest permission set. Every section, including the host-permission and
//! page-context ones, must carry a fenced paste block that fits the field limit.

use std::collections::BTreeSet;

const PERMISSION_COPY: &str = include_str!("../../../docs/legal/PERMISSION_JUSTIFICATIONS.md");
const EXTENSION_MANIFEST: &str = include_str!("../../../extension/manifest.json");
const MAX_DASHBOARD_JUSTIFICATION_CHARS: usize = 1_000;
const SECTION_PREFIX: &str = "## ";
const MANIFEST_PERMISSION_KEYS: &[&str] = &["permissions", "optional_permissions"];

/// Splits the document into `(heading, body)` pairs, one per `## ` heading.
fn sections(document: &str) -> Vec<(&str, &str)> {
    let mut found: Vec<(&str, usize, usize)> = Vec::new();
    let mut offset = 0;
    for line in document.split_inclusive('\n') {
        if let Some(heading) = line.strip_prefix(SECTION_PREFIX) {
            if let Some(last) = found.last_mut() {
                last.2 = offset;
            }
            found.push((heading.trim_end(), offset + line.len(), document.len()));
        }
        offset += line.len();
    }
    found
        .into_iter()
        .map(|(heading, start, end)| (heading, &document[start..end]))
        .collect()
}

fn paste_block<'a>(section: &str, body: &'a str) -> &'a str {
    body.split_once("```text\n")
        .unwrap_or_else(|| panic!("section {section} must contain a fenced text block"))
        .1
        .split_once("\n```")
        .unwrap_or_else(|| panic!("section {section} has an unclosed fenced text block"))
        .0
        .trim()
}

fn is_bare_identifier(heading: &str) -> bool {
    let mut characters = heading.chars();
    characters.next().is_some_and(|c| c.is_ascii_alphabetic())
        && characters.all(|c| c.is_ascii_alphanumeric())
}

fn manifest_permissions() -> Vec<String> {
    let manifest: serde_json::Value =
        serde_json::from_str(EXTENSION_MANIFEST).expect("extension manifest is valid JSON");
    let mut permissions = Vec::new();
    for key in MANIFEST_PERMISSION_KEYS {
        let Some(value) = manifest.get(key) else {
            continue;
        };
        let entries = value
            .as_array()
            .unwrap_or_else(|| panic!("manifest {key} must be an array"));
        for entry in entries {
            let name = entry
                .as_str()
                .unwrap_or_else(|| panic!("manifest {key} must hold only strings"));
            permissions.push(name.to_owned());
        }
    }
    permissions
}

#[test]
fn chrome_permission_justifications_fit_dashboard_limit() {
    let normalized = PERMISSION_COPY.replace("\r\n", "\n");
    let found = sections(&normalized);
    assert!(!found.is_empty(), "no permission-justification sections");
    for (section, body) in found {
        let copy = paste_block(section, body);
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

#[test]
fn every_manifest_permission_has_exactly_one_justification() {
    let permissions = manifest_permissions();
    let unique: BTreeSet<&str> = permissions.iter().map(String::as_str).collect();
    assert_eq!(
        unique.len(),
        permissions.len(),
        "manifest lists a permission twice"
    );
    let normalized = PERMISSION_COPY.replace("\r\n", "\n");
    let headings: Vec<&str> = sections(&normalized)
        .into_iter()
        .map(|(heading, _)| heading)
        .filter(|heading| is_bare_identifier(heading))
        .collect();
    let documented: BTreeSet<&str> = headings.iter().copied().collect();
    assert_eq!(
        documented.len(),
        headings.len(),
        "a permission has two justification sections"
    );
    assert_eq!(
        documented, unique,
        "manifest permissions and Chrome Web Store justifications differ"
    );
}
