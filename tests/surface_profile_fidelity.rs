// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0101 R2 guards for the edge-owned `ghostlight-legacy/v1` profile.
//!
//! Exact model-facing declarations and rendering belong to the MCP connector. The service core
//! may retain only two bounded compatibility seams: historical audit replay aliases, and the
//! temporary legacy extension-wire serializer that R3 isolates below typed mechanisms.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const EDGE_CATALOG: &str =
    include_str!("../crates/mcp-connector/src/surface/data/ghostlight-legacy-v1.json");
const FROZEN_CATALOG: &str = include_str!("golden/surfaces/ghostlight-legacy-v1.json");
const EDGE_GUIDE: &str =
    include_str!("../crates/mcp-connector/src/surface/data/ghostlight-legacy-v1-agent-guide.txt");
const FROZEN_GUIDE: &str = include_str!("golden/surfaces/ghostlight-legacy-v1-agent-guide.txt");

const HISTORICAL_AUDIT_ALIAS_MODULE: &str = "crates/core/src/operation/audit_compat.rs";
const R3_EXTENSION_ALIAS_MODULE: &str = "crates/core/src/hub/outbound/legacy_mechanism.rs";
const R3_EXTENSION_ALIAS_CONSUMER: &str = "crates/core/src/hub/outbound/browser.rs";

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_path(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn edge_assets_remain_the_exact_frozen_legacy_profile() {
    assert_eq!(EDGE_CATALOG, FROZEN_CATALOG);
    assert_eq!(EDGE_GUIDE, FROZEN_GUIDE);

    let catalog: Value = serde_json::from_str(EDGE_CATALOG).expect("valid edge catalog");
    assert_eq!(
        catalog["tools"].as_array().map(Vec::len),
        Some(25),
        "ghostlight-legacy/v1 must retain its complete ordered declaration set"
    );
}

#[test]
/// Architecture guard only. Exact revision envelopes and terminal transcripts are exercised by
/// the handlers' own unit tests, where their private state machines are directly observable.
fn both_mcp_revisions_delegate_catalog_decode_and_result_rendering_to_the_edge_profile() {
    for revision in [
        "crates/mcp-connector/src/mcp_2025_11_25.rs",
        "crates/mcp-connector/src/mcp_2026_07_28.rs",
    ] {
        let source = read_repo_file(revision);
        for call in [
            "ghostlight_legacy::filtered_declarations",
            "ghostlight_legacy::decode_call",
            "ghostlight_legacy::encode_result",
        ] {
            assert!(
                source.contains(call),
                "{revision} must route through the edge profile's {call}"
            );
        }
        assert!(
            !source.contains("ghostlight_core")
                && !source.contains("browser::directory")
                && !source.contains("tool::tools"),
            "{revision} regained a service-core declaration dependency"
        );
    }
}

#[test]
fn service_core_has_no_model_surface_declaration_or_legacy_decoder_dependency() {
    for removed in [
        "crates/core/src/browser/advertise.rs",
        "crates/core/src/browser/directory.rs",
        "crates/core/src/tool/tools.rs",
    ] {
        assert!(
            !repo_path(removed).exists(),
            "removed model-surface module still exists: {removed}"
        );
    }

    let forbidden = [
        "ToolDescriptor",
        "AgentGuide",
        "advertised_tools_json",
        "agent_guide_text",
        "browser::directory",
        "browser::advertise",
        "tool::tools",
        "ICapability",
        "BrowserCapability",
        "decode_legacy_call",
        "run_tool_call",
    ];
    let core = repo_path("crates/core/src");
    let mut files = Vec::new();
    collect_rust_files(&core, &mut files);
    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("read core source");
        for symbol in forbidden {
            if source.contains(symbol) {
                violations.push(format!("{}: {symbol}", file.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "model-facing declarations or the legacy decoder crossed into core:\n{}",
        violations.join("\n")
    );
}

#[test]
fn remaining_compatibility_seams_are_precisely_bounded() {
    let audit_aliases = read_repo_file(HISTORICAL_AUDIT_ALIAS_MODULE);
    assert!(audit_aliases.contains("const HISTORICAL_ALIASES"));
    assert!(audit_aliases.contains("OperationId::parse(tool)"));
    assert!(
        audit_aliases.find("OperationId::parse(tool)")
            < audit_aliases
                .find("HISTORICAL_ALIASES.iter")
                .or_else(|| { audit_aliases.find("HISTORICAL_ALIASES\n        .iter") }),
        "canonical audit identities must be attempted before historical aliases"
    );

    let serializer = read_repo_file(R3_EXTENSION_ALIAS_MODULE);
    for symbol in [
        "serialize_tool_request",
        "serialize_tab_url_request",
        "fn legacy_tool",
    ] {
        assert!(
            serializer.contains(symbol),
            "R3 extension serializer seam is missing its bounded {symbol} hook"
        );
    }
    assert!(serializer.contains("use MechanismId::*"));

    let mut files = Vec::new();
    collect_rust_files(&repo_path("crates/core/src"), &mut files);
    for file in files {
        let relative = file
            .strip_prefix(repo_path("."))
            .expect("core source under repository")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&file).expect("read core source");
        if source.contains("HISTORICAL_ALIASES") {
            assert_eq!(relative, HISTORICAL_AUDIT_ALIAS_MODULE);
        }
        for removed in [
            ".legacy_dispatch_tool(",
            "fn legacy_dispatch_tool(",
            ".legacy_arguments(",
            "fn legacy_arguments(",
            "encode_legacy_arguments(",
        ] {
            assert!(
                !source.contains(removed),
                "obsolete operation-to-surface serializer remains in {relative}: {removed}"
            );
        }
        if source.contains("serialize_tool_request") || source.contains("serialize_tab_url_request")
        {
            assert!(
                relative == R3_EXTENSION_ALIAS_MODULE || relative == R3_EXTENSION_ALIAS_CONSUMER,
                "extension alias serialization escaped its R3 seam: {relative}"
            );
        }
    }

    let registry = read_repo_file("crates/core/src/operation/registry.rs");
    assert!(registry.contains("Handler::Mechanism"));
    assert!(!registry.contains("Handler::ExtensionForward"));
    let pipeline = read_repo_file("crates/core/src/tool/pipeline.rs");
    assert!(pipeline.contains("compile_operation"));
    assert!(pipeline.contains("execute_mechanism_with_delivery_outcome"));
}
