//! One bounded, optional page-preparation contract shared by ordinary page tools.

use super::Operation;

/// Ordinary page tools that accept the same settlement preference.
pub(crate) const PAGE_TOOLS: &[&str] = &[
    "browser_read",
    "browser_inspect",
    "browser_find",
    "browser_screenshot",
    "browser_click",
    "browser_scroll",
    "browser_hover",
    "browser_fill_form",
    "browser_type_text",
    "browser_press_key",
    "browser_drag",
    "browser_upload",
    "browser_execute",
];

/// Maximum optional preparation wait, independent of the overall invocation deadline.
pub(crate) const MAX_WAIT_MS: u64 = 1_000;

/// Whether this operation uses bounded settlement; explicit waits own their conditions.
pub(crate) fn enabled(operation: &Operation) -> bool {
    match operation {
        Operation::ReadPage(value) => value.visual_settle != Some(false),
        Operation::InspectPage(value) => value.visual_settle != Some(false),
        Operation::Find(value) => value.visual_settle != Some(false),
        Operation::Click(value) => value.visual_settle != Some(false),
        Operation::ScrollPage(value) => value.visual_settle != Some(false),
        Operation::Hover(value) => value.visual_settle != Some(false),
        Operation::FillForm(value) => value.visual_settle != Some(false),
        Operation::TypeText(value) => value.visual_settle != Some(false),
        Operation::PressKey(value) => value.visual_settle != Some(false),
        Operation::Drag(value) => value.visual_settle != Some(false),
        Operation::UploadFiles(value) => value.visual_settle != Some(false),
        Operation::RunScript(value) => value.visual_settle != Some(false),
        Operation::TakeScreenshot(value) => value.visual_settle != Some(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_similar_page_tool_defaults_to_bounded_settlement_and_accepts_opt_out() {
        let attachment = std::env::temp_dir().join("settlement-attachment.txt");
        let cases = [
            ("browser_read", json!({})),
            ("browser_inspect", json!({})),
            ("browser_find", json!({"text":"Save"})),
            ("browser_screenshot", json!({})),
            ("browser_click", json!({"selector":{"name":"Save"}})),
            ("browser_scroll", json!({})),
            ("browser_hover", json!({"target":"target_one"})),
            (
                "browser_fill_form",
                json!({"fields":[{"target":"target_one","value":"draft"}]}),
            ),
            (
                "browser_type_text",
                json!({"target":"target_one","text":"draft"}),
            ),
            ("browser_press_key", json!({"key":"Enter"})),
            (
                "browser_drag",
                json!({"source_target":"target_one","destination_target":"target_two"}),
            ),
            (
                "browser_upload",
                json!({"target":"target_one","paths":[attachment]}),
            ),
            ("browser_execute", json!({"script":"return 1"})),
        ];
        assert_eq!(cases.len(), PAGE_TOOLS.len());
        for (name, mut input) in cases {
            assert!(
                enabled(&super::super::decode(name, input.clone()).unwrap()),
                "{name}"
            );
            input["visual_settle"] = json!(false);
            assert!(
                !enabled(&super::super::decode(name, input.clone()).unwrap()),
                "{name}"
            );
            input["visual_settle"] = json!("false");
            assert!(super::super::decode(name, input).is_err(), "{name}");
            let tool = super::super::catalog()
                .into_iter()
                .find(|tool| tool.name == name)
                .unwrap();
            assert_eq!(
                tool.input_schema["properties"]["visual_settle"]["default"],
                true
            );
        }
        assert!(!enabled(
            &super::super::decode("browser_wait", json!({"condition":"visual_settle"})).unwrap()
        ));
    }
}
