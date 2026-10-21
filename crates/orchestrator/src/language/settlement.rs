//! One bounded, optional page-preparation contract shared by ordinary page tools.

use super::Operation;

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
        let attachment = crate::scratch::Scratch::file("settlement-attachment.txt");
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
                json!({"target":"target_one","paths":[attachment.path()]}),
            ),
            ("browser_execute", json!({"script":"return 1"})),
        ];
        let settling = crate::language::Tool::ALL
            .into_iter()
            .filter(|tool| tool.settles())
            .count();
        assert_eq!(cases.len(), settling);
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
