use super::*;

fn target(locator: &str) -> ObservedTarget {
    ObservedTarget {
        locator: locator.into(),
        role: "button".into(),
        name: String::new(),
        state: Vec::new(),
        credential_class: false,
        ..Default::default()
    }
}

fn found(locators: &[&str], truncated: bool) -> DocumentTargets {
    DocumentTargets {
        targets: locators.iter().map(|locator| target(locator)).collect(),
        truncated,
    }
}

fn locators(merged: &MergedTargets) -> Vec<&str> {
    merged
        .targets
        .iter()
        .map(|target| target.locator.as_str())
        .collect()
}

fn section(text: &str, truncated: bool) -> Text {
    Text {
        text: text.into(),
        truncated,
        title: String::new(),
        url: String::new(),
    }
}

fn some(sections: &[Text]) -> Vec<Option<&Text>> {
    sections.iter().map(Some).collect()
}

fn node(label: &str, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: "container".into(),
        label: label.into(),
        children,
    }
}

fn value(tree: TreeNode, truncated: bool) -> TreeValue {
    let nodes = count_nodes(&tree);
    TreeValue {
        tree,
        nodes,
        truncated,
    }
}

fn labels(tree: &TreeNode) -> Vec<&str> {
    tree.children
        .iter()
        .map(|child| child.label.as_str())
        .collect()
}

#[test]
fn targets_keep_document_order_under_one_ceiling() {
    let merged = targets(
        [
            found(&["0:locator_top", "0:locator_top2"], false),
            found(&["5:locator_a"], false),
            found(&["12:locator_deep"], false),
        ],
        10,
    );
    assert_eq!(
        locators(&merged),
        [
            "0:locator_top",
            "0:locator_top2",
            "5:locator_a",
            "12:locator_deep"
        ]
    );
    assert!(!merged.over);
}

#[test]
fn targets_cap_the_total_with_the_top_document_winning_first() {
    let merged = targets(
        [
            found(&["0:locator_a"], false),
            found(&["1:locator_b", "1:locator_c", "1:locator_d"], false),
        ],
        3,
    );
    assert_eq!(
        locators(&merged),
        ["0:locator_a", "1:locator_b", "1:locator_c"]
    );
    assert!(merged.over);
    let empty = targets([], 3);
    assert!(empty.targets.is_empty());
    assert!(!empty.over);
}

#[test]
fn targets_exactly_at_the_ceiling_are_not_over() {
    let merged = targets([found(&["a", "b"], false), found(&["c"], false)], 3);
    assert_eq!(locators(&merged), ["a", "b", "c"]);
    assert!(!merged.over);
}

#[test]
fn a_document_that_found_more_than_it_returned_makes_the_merge_over() {
    let merged = targets([found(&["a", "b"], true)], 2);
    assert_eq!(locators(&merged), ["a", "b"]);
    assert!(merged.over);
}

fn in_forms(forms: &[Option<u32>]) -> DocumentTargets {
    DocumentTargets {
        targets: forms
            .iter()
            .enumerate()
            .map(|(index, form)| ObservedTarget {
                form: *form,
                ..target(&format!("t{index}"))
            })
            .collect(),
        truncated: false,
    }
}

fn form_ordinals(merged: &MergedTargets) -> Vec<Option<u32>> {
    merged.targets.iter().map(|target| target.form).collect()
}

#[test]
fn form_ordinals_are_renumbered_across_documents() {
    let merged = targets(
        [
            in_forms(&[Some(1), None, Some(2), Some(1)]),
            in_forms(&[Some(1), Some(2)]),
            in_forms(&[None, Some(1)]),
        ],
        20,
    );
    assert_eq!(
        form_ordinals(&merged),
        [
            Some(1),
            None,
            Some(2),
            Some(1),
            Some(3),
            Some(4),
            None,
            Some(5)
        ]
    );
}

#[test]
fn a_cut_target_still_counts_its_form_for_the_documents_after_it() {
    let merged = targets([in_forms(&[Some(1), Some(2)]), in_forms(&[Some(1)])], 1);
    assert_eq!(form_ordinals(&merged), [Some(1)]);
    assert!(merged.over);
    let wide = targets([in_forms(&[Some(1), Some(2)]), in_forms(&[Some(1)])], 3);
    assert_eq!(form_ordinals(&wide), [Some(1), Some(2), Some(3)]);
}

#[test]
fn text_keeps_document_order_under_one_page_wide_ceiling() {
    let sections = [
        section("top frame", false),
        section("second frame", false),
        section("third frame", false),
    ];
    assert_eq!(
        text_sections(some(&sections), 26, false),
        MergedText {
            text: "top frame\n\nsecond frame\n\nt".into(),
            truncated: true
        }
    );
}

#[test]
fn text_preserves_local_and_skipped_truncation() {
    let local = [section("complete", true)];
    assert_eq!(
        text_sections(some(&local), 20, false),
        MergedText {
            text: "complete".into(),
            truncated: true
        }
    );
    let whole = [section("complete", false)];
    assert_eq!(
        text_sections(some(&whole), 20, true),
        MergedText {
            text: "complete".into(),
            truncated: true
        }
    );
}

#[test]
fn an_empty_section_does_not_truncate_a_full_bound() {
    let sections = [section("12345", false), section("", false)];
    assert_eq!(
        text_sections(some(&sections), 5, false),
        MergedText {
            text: "12345".into(),
            truncated: false
        }
    );
}

#[test]
fn a_section_that_has_no_room_truncates_a_full_bound() {
    let sections = [section("12345", false), section("x", false)];
    assert_eq!(
        text_sections(some(&sections), 5, false),
        MergedText {
            text: "12345".into(),
            truncated: true
        }
    );
}

#[test]
fn text_trims_sections_and_ignores_documents_that_gave_none() {
    let top = section("  top  \n", false);
    let child = section("child", false);
    let merged = text_sections([Some(&top), None, Some(&child)], 100, false);
    assert_eq!(merged.text, "top\n\nchild");
    assert!(!merged.truncated);
}

#[test]
fn text_counts_characters_not_bytes() {
    let sections = [
        section("ab", false),
        section("\u{e9}\u{e9}\u{e9}\u{e9}", false),
    ];
    let merged = text_sections(some(&sections), 6, false);
    assert_eq!(merged.text, "ab\n\n\u{e9}\u{e9}");
    assert!(merged.truncated);
}

#[test]
fn the_next_document_asks_for_what_is_left_after_the_separator() {
    let top = [section("1234567890", false)];
    assert_eq!(remaining_after(some(&top), 15), 3);
}

#[test]
fn nothing_is_asked_after_a_cut_section() {
    let sections = [section("1234567890", false), section("abc", true)];
    assert_eq!(remaining_after(some(&sections), 15), 0);
    assert_eq!(
        text_sections(some(&sections), 15, false),
        MergedText {
            text: "1234567890\n\nabc".into(),
            truncated: true
        }
    );
}

#[test]
fn an_empty_page_asks_for_the_whole_bound() {
    let top = [section("", false)];
    assert_eq!(remaining_after(some(&top), 40), 40);
    assert_eq!(remaining_after([None], 40), 40);
}

#[test]
fn a_full_bound_still_asks_for_one_character() {
    let top = [section("12345", false)];
    assert_eq!(remaining_after(some(&top), 5), 1);
}

#[test]
fn subtrees_share_one_node_bound() {
    let root = value(node("frame 0", vec![node("top", vec![])]), false);
    let plan = subtree_plan(Some(&root), 5, 5);
    assert_eq!(
        plan,
        Some(SubtreePlan {
            max_depth: 4,
            max_nodes: 3
        })
    );
    let merged = append_and_trim(
        Some(root),
        vec![
            value(node("frame 3", vec![node("a", vec![])]), false),
            value(node("frame 8", vec![]), true),
        ],
        false,
        5,
    );
    let tree = merged.tree.expect("a tree");
    assert_eq!(labels(&tree), ["top", "frame 3", "frame 8"]);
    assert_eq!(count_nodes(&tree), 5);
    assert!(merged.truncated);
}

#[test]
fn subtrees_asked_together_are_trimmed_in_pre_order() {
    let root = value(node("frame 0", vec![node("top", vec![])]), false);
    let merged = append_and_trim(
        Some(root),
        vec![
            value(node("frame 3", vec![node("a", vec![])]), false),
            value(node("frame 8", vec![node("b", vec![])]), false),
        ],
        false,
        5,
    );
    let tree = merged.tree.expect("a tree");
    assert_eq!(count_nodes(&tree), 5);
    assert_eq!(labels(&tree), ["top", "frame 3", "frame 8"]);
    assert!(tree.children[2].children.is_empty());
    assert_eq!(tree.children[1].children.len(), 1);
    assert!(merged.truncated);
}

#[test]
fn a_trim_keeps_the_front_of_a_deep_tree() {
    let root = value(
        node(
            "root",
            vec![
                node("one", vec![node("one.a", vec![]), node("one.b", vec![])]),
                node("two", vec![node("two.a", vec![])]),
            ],
        ),
        false,
    );
    let merged = append_and_trim(
        Some(root),
        vec![value(node("child", vec![]), false)],
        false,
        4,
    );
    let tree = merged.tree.expect("a tree");
    assert_eq!(labels(&tree), ["one"]);
    assert_eq!(count_nodes(&tree), 4);
    assert_eq!(tree.children[0].children.len(), 2);
    assert!(merged.truncated);
}

#[test]
fn a_tree_inside_the_bound_is_not_truncated() {
    let root = value(node("frame 0", vec![]), false);
    let merged = append_and_trim(
        Some(root),
        vec![value(node("frame 2", vec![]), false)],
        false,
        400,
    );
    assert!(!merged.truncated);
    assert_eq!(labels(&merged.tree.expect("a tree")), ["frame 2"]);
}

#[test]
fn a_depth_of_one_keeps_the_top_document_alone() {
    let root = value(node("top", vec![]), false);
    assert_eq!(subtree_plan(Some(&root), 400, 1), None);
    let merged = append_and_trim(Some(root), Vec::new(), true, 400);
    assert!(merged.truncated);
    assert!(merged.tree.expect("a tree").children.is_empty());
}

#[test]
fn a_spent_or_cut_top_tree_asks_for_no_subtrees() {
    let spent = value(node("top", vec![node("a", vec![])]), false);
    assert_eq!(subtree_plan(Some(&spent), 2, 5), None);
    let cut = value(node("top", vec![]), true);
    assert_eq!(subtree_plan(Some(&cut), 400, 5), None);
}

#[test]
fn a_cut_tree_drops_every_later_subtree() {
    let root = value(node("top", vec![]), true);
    let merged = append_and_trim(
        Some(root),
        vec![value(node("frame 2", vec![]), false)],
        false,
        400,
    );
    assert!(merged.tree.expect("a tree").children.is_empty());
    assert!(merged.truncated);
}

#[test]
fn without_a_top_tree_the_first_embedded_tree_stands_in() {
    assert_eq!(
        subtree_plan(None, 400, 3),
        Some(SubtreePlan {
            max_depth: 2,
            max_nodes: 400
        })
    );
    let merged = append_and_trim(
        None,
        vec![
            value(node("frame 2", vec![]), false),
            value(node("frame 5", vec![]), false),
        ],
        false,
        400,
    );
    let tree = merged.tree.expect("a tree");
    assert_eq!(tree.label, "frame 2");
    assert_eq!(labels(&tree), ["frame 5"]);
    assert!(!merged.truncated);
}

#[test]
fn without_a_top_tree_and_no_depth_for_children_nothing_is_read() {
    assert_eq!(subtree_plan(None, 400, 1), None);
    let merged = append_and_trim(None, Vec::new(), true, 400);
    assert_eq!(merged.tree, None);
    assert!(merged.truncated);
}

#[test]
fn a_lone_embedded_tree_with_unread_siblings_is_reported_truncated() {
    let merged = append_and_trim(None, vec![value(node("frame 2", vec![]), false)], true, 400);
    let tree = merged.tree.expect("a tree");
    assert_eq!(tree.label, "frame 2");
    assert!(tree.children.is_empty());
    assert!(merged.truncated);
}

#[test]
fn no_tree_at_all_is_none() {
    let merged = append_and_trim(None, Vec::new(), false, 400);
    assert_eq!(merged.tree, None);
    assert!(!merged.truncated);
}
