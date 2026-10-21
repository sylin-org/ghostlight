use super::*;
use crate::browser::adapter::TreeNodeKind;
use crate::language::area::{PageArea, HIDDEN_STATE};

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
        ..DocumentTargets::default()
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

fn node(label: &str, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: TreeNodeKind::Container,
        label: label.into(),
        children,
    }
}

fn value(tree: TreeNode, truncated: bool) -> TreeValue {
    TreeValue { tree, truncated }
}

/// A target in `area`, hidden when `hidden`.
fn placed(locator: &str, area: Option<&str>, hidden: bool) -> ObservedTarget {
    ObservedTarget {
        area: area.map(str::to_owned),
        state: if hidden {
            vec![HIDDEN_STATE.to_owned()]
        } else {
            Vec::new()
        },
        ..target(locator)
    }
}

fn document(targets: Vec<ObservedTarget>) -> DocumentTargets {
    DocumentTargets {
        targets,
        ..DocumentTargets::default()
    }
}

/// A capped list of targets keeps one ceiling across documents in document order, cuts page
/// chrome and hidden items before page content, so a child frame's form survives a top document
/// full of navigation links (finding a19-inspect-tiers, ADR-0198 Decision 20), and says exactly
/// what it left out and that it was cut.
#[test]
fn a_capped_list_keeps_page_content_first_and_counts_what_it_left_out() {
    let nav = PageArea::Nav.word();
    let links: Vec<ObservedTarget> = (0..6)
        .map(|index| placed(&format!("link-{index}"), Some(nav), false))
        .collect();
    let fields: Vec<ObservedTarget> = (0..3)
        .map(|index| placed(&format!("field-{index}"), None, false))
        .collect();
    let mut own = LeftOut::default();
    own.add(LeftOutKey::Hidden, 7);
    own.add(LeftOutKey::BehindDialog, 2);
    let mut second = LeftOut::default();
    second.add(LeftOutKey::Chrome, 5);
    type Case = (
        Vec<DocumentTargets>,
        usize,
        &'static [&'static str],
        bool,
        &'static [(LeftOutKey, usize)],
    );
    let cases: Vec<Case> = vec![
        (
            vec![
                found(&["0:locator_top", "0:locator_top2"], false),
                found(&["5:locator_a"], false),
                found(&["12:locator_deep"], false),
            ],
            10,
            &[
                "0:locator_top",
                "0:locator_top2",
                "5:locator_a",
                "12:locator_deep",
            ],
            false,
            &[],
        ),
        (
            vec![
                found(&["0:locator_a"], false),
                found(&["1:locator_b", "1:locator_c", "1:locator_d"], false),
            ],
            3,
            &["0:locator_a", "1:locator_b", "1:locator_c"],
            true,
            &[],
        ),
        (vec![], 3, &[], false, &[]),
        (
            vec![found(&["a", "b"], false), found(&["c"], false)],
            3,
            &["a", "b", "c"],
            false,
            &[],
        ),
        (vec![found(&["a", "b"], true)], 2, &["a", "b"], true, &[]),
        (
            vec![
                document(vec![
                    placed("nav-1", Some(nav), false),
                    placed("ghost", None, true),
                    placed("nav-2", Some(PageArea::Footer.word()), false),
                ]),
                document(vec![
                    placed("field-1", None, false),
                    placed("side", Some(PageArea::Aside.word()), false),
                    placed("field-2", Some(PageArea::Dialog.word()), false),
                ]),
            ],
            4,
            &["nav-1", "field-1", "side", "field-2"],
            true,
            &[
                (LeftOutKey::Hidden, 1),
                (LeftOutKey::Chrome, 1),
                (LeftOutKey::Aside, 0),
            ],
        ),
        (
            vec![document(links), document(fields)],
            5,
            &["link-0", "link-1", "field-0", "field-1", "field-2"],
            true,
            &[(LeftOutKey::Chrome, 4)],
        ),
        (
            vec![
                DocumentTargets {
                    targets: vec![placed("a", None, false), placed("b", Some(nav), false)],
                    truncated: true,
                    left_out: own,
                    ..DocumentTargets::default()
                },
                DocumentTargets {
                    targets: vec![placed("c", Some(nav), false)],
                    left_out: second,
                    modal_open: true,
                    ..DocumentTargets::default()
                },
            ],
            2,
            &["a", "b"],
            true,
            &[
                (LeftOutKey::Hidden, 7),
                (LeftOutKey::BehindDialog, 2),
                (LeftOutKey::Chrome, 6),
            ],
        ),
    ];
    for (documents, cap, kept, over, left_out) in cases {
        let merged = targets(documents, cap);
        assert_eq!(locators(&merged), kept);
        assert_eq!(merged.over, over, "{kept:?}");
        for (key, count) in left_out {
            assert_eq!(merged.left_out.count(*key), *count, "{kept:?} {key:?}");
        }
    }

    let everything = targets([document(vec![placed("a", Some(nav), true)])], 1);
    assert!(everything.left_out.is_empty());
    assert!(!everything.over && !everything.modal_open);
}

/// A document that holds `forms` forms and kept targets in the given ones.
fn in_forms(forms: u32, kept: &[Option<u32>]) -> DocumentTargets {
    DocumentTargets {
        targets: kept
            .iter()
            .enumerate()
            .map(|(index, form)| ObservedTarget {
                form: *form,
                ..target(&format!("t{index}"))
            })
            .collect(),
        forms,
        ..DocumentTargets::default()
    }
}

fn form_ordinals(merged: &MergedTargets) -> Vec<Option<u32>> {
    merged.targets.iter().map(|target| target.form).collect()
}

/// A form's number is page-wide: documents number their forms on, and a form the answer did not
/// keep, or whose target was cut, still counts for the documents after it, so the same form keeps
/// one number however much of the page fits (commit f7d430f4).
#[test]
fn a_form_keeps_one_page_wide_number_however_much_of_the_page_fits() {
    let one = Some(1);
    let cases: [(Vec<DocumentTargets>, usize, Vec<Option<u32>>); 5] = [
        (
            vec![
                in_forms(2, &[one, None, Some(2), one]),
                in_forms(2, &[one, Some(2)]),
                in_forms(1, &[None, one]),
            ],
            20,
            vec![one, None, Some(2), one, Some(3), Some(4), None, Some(5)],
        ),
        (
            vec![in_forms(3, &[one]), in_forms(1, &[one])],
            5,
            vec![one, Some(4)],
        ),
        (
            vec![in_forms(3, &[one, Some(2), Some(3)]), in_forms(1, &[one])],
            5,
            vec![one, Some(2), Some(3), Some(4)],
        ),
        (
            vec![in_forms(2, &[one, Some(2)]), in_forms(1, &[one])],
            1,
            vec![one],
        ),
        (
            vec![in_forms(2, &[one, Some(2)]), in_forms(1, &[one])],
            3,
            vec![one, Some(2), Some(3)],
        ),
    ];
    for (documents, cap, numbers) in cases {
        assert_eq!(form_ordinals(&targets(documents, cap)), numbers, "{cap}");
    }
}

/// A button named `name` in the row `context`, numbered `nth` by its own document.
fn repeated(locator: &str, name: &str, context: Option<&str>, nth: Option<u32>) -> ObservedTarget {
    ObservedTarget {
        name: name.into(),
        context: context.map(str::to_owned),
        nth,
        ..target(locator)
    }
}

fn keys(merged: &MergedTargets) -> Vec<(Option<&str>, Option<u32>)> {
    merged
        .targets
        .iter()
        .map(|target| (target.context.as_deref(), target.nth))
        .collect()
}

/// A repeated control's `nth` and row `context` are decided once, after the fold, across the whole
/// page: a cut renumbers from one or drops a lone repeat's key, two documents' repeats number on,
/// and a credential-class target never carries either (commit 3fbe9b18).
#[test]
fn a_repeated_control_is_numbered_once_across_the_page() {
    let hidden = |target: ObservedTarget| ObservedTarget {
        state: vec![HIDDEN_STATE.to_owned()],
        ..target
    };
    let secret = |locator: &str| ObservedTarget {
        credential_class: true,
        role: "textbox".into(),
        name: "Password".into(),
        ..target(locator)
    };
    let more = "More options";
    let (alpha, beta) = (Some("Alpha"), Some("Beta"));
    type Case<'a> = (
        Vec<DocumentTargets>,
        usize,
        Vec<(Option<&'a str>, Option<u32>)>,
    );
    let cases: [Case; 5] = [
        (
            vec![document(vec![
                repeated("a", more, alpha, Some(1)),
                hidden(repeated("b", more, alpha, Some(2))),
            ])],
            1,
            vec![(None, None)],
        ),
        (
            vec![document(vec![
                hidden(repeated("a", more, alpha, Some(1))),
                repeated("b", more, alpha, Some(2)),
                repeated("c", more, alpha, Some(3)),
                repeated("d", more, beta, None),
            ])],
            3,
            vec![(alpha, Some(1)), (alpha, Some(2)), (beta, None)],
        ),
        (
            vec![
                document(vec![
                    repeated("a", more, alpha, None),
                    repeated("b", more, beta, None),
                ]),
                document(vec![
                    repeated("c", more, alpha, None),
                    repeated("d", more, beta, None),
                ]),
            ],
            10,
            vec![
                (alpha, Some(1)),
                (beta, Some(1)),
                (alpha, Some(2)),
                (beta, Some(2)),
            ],
        ),
        (
            vec![
                document(vec![repeated("a", "Close", None, None)]),
                document(vec![repeated("b", "Close", None, None)]),
            ],
            10,
            vec![(None, Some(1)), (None, Some(2))],
        ),
        (
            vec![document(vec![secret("a")]), document(vec![secret("b")])],
            10,
            vec![(None, None), (None, None)],
        ),
    ];
    for (documents, cap, expected) in cases {
        assert_eq!(keys(&targets(documents, cap)), expected, "{cap}");
    }
}

/// A read across documents is one text in document order under one page-wide ceiling, counted
/// in characters as the page asset counts them, and says it was cut when any document was cut,
/// skipped or left without room; the next document is asked for what is left after the
/// separator, and for one character more when the bound is full, which is how a cut is noticed
/// (ACCEPTANCE, executor and truth gates 7).
#[test]
fn a_read_across_documents_is_one_text_under_one_ceiling() {
    type Case = (Vec<Option<Text>>, usize, bool, String, bool);
    let s = SECTION_SEPARATOR;
    let cases: [Case; 8] = [
        (
            vec![
                Some(section("top frame", false)),
                Some(section("second frame", false)),
                Some(section("third frame", false)),
            ],
            26,
            false,
            format!("top frame{s}second frame{s}t"),
            true,
        ),
        (
            vec![Some(section("complete", true))],
            20,
            false,
            "complete".into(),
            true,
        ),
        (
            vec![Some(section("complete", false))],
            20,
            true,
            "complete".into(),
            true,
        ),
        (
            vec![Some(section("12345", false)), Some(section("", false))],
            5,
            false,
            "12345".into(),
            false,
        ),
        (
            vec![Some(section("12345", false)), Some(section("x", false))],
            5,
            false,
            "12345".into(),
            true,
        ),
        (
            vec![
                Some(section("  top  \n", false)),
                None,
                Some(section("child", false)),
            ],
            100,
            false,
            format!("top{s}child"),
            false,
        ),
        (
            vec![
                Some(section("ab", false)),
                Some(section("\u{e9}\u{e9}\u{e9}\u{e9}", false)),
            ],
            6,
            false,
            format!("ab{s}\u{e9}\u{e9}"),
            true,
        ),
        (
            vec![
                Some(section("1234567890", false)),
                Some(section("abc", true)),
            ],
            15,
            false,
            format!("1234567890{s}abc"),
            true,
        ),
    ];
    for (sections, bound, skipped, text, truncated) in cases {
        assert_eq!(
            text_sections(sections.iter().map(Option::as_ref), bound, skipped),
            MergedText { text, truncated }
        );
    }

    let asks = |sections: &[Option<Text>], bound: usize| {
        remaining_after(sections.iter().map(Option::as_ref), bound)
    };
    let ten = Some(section("1234567890", false));
    assert_eq!(
        asks(std::slice::from_ref(&ten), 15),
        15 - 10 - s.chars().count()
    );
    assert_eq!(asks(&[ten, Some(section("abc", true))], 15), 0);
    assert_eq!(asks(&[Some(section("", false))], 40), 40);
    assert_eq!(asks(&[None], 40), 40);
    assert_eq!(asks(&[Some(section("12345", false))], 5), 1);
}

/// The page tree, embedded documents included, stays under one node bound: the embedded trees
/// are asked for what the top tree left and the depth below it, appended in document order and
/// trimmed in pre-order, and the tree says it was cut whenever a document was trimmed, cut or left
/// unread; with no top tree the first embedded tree stands in (ACCEPTANCE, executor and truth
/// gates 7).
#[test]
fn the_page_tree_stays_under_one_node_bound() {
    let top = || value(node("frame 0", vec![node("top", vec![])]), false);
    let leaf = |label: &str| value(node(label, vec![]), false);
    for (root, limit, depth, plan) in [
        (Some(top()), 5, 5, Some((4, 3))),
        (Some(leaf("top")), 400, 1, None),
        (
            Some(value(node("top", vec![node("a", vec![])]), false)),
            2,
            5,
            None,
        ),
        (Some(value(node("top", vec![]), true)), 400, 5, None),
        (None, 400, 3, Some((2, 400))),
        (None, 400, 1, None),
    ] {
        assert_eq!(
            subtree_plan(root.as_ref(), limit, depth),
            plan.map(|(max_depth, max_nodes)| SubtreePlan {
                max_depth,
                max_nodes
            })
        );
    }

    type Shape = Option<(&'static str, Vec<(&'static str, usize)>)>;
    let deep = value(
        node(
            "root",
            vec![
                node("one", vec![node("one.a", vec![]), node("one.b", vec![])]),
                node("two", vec![node("two.a", vec![])]),
            ],
        ),
        false,
    );
    type Case = (Option<TreeValue>, Vec<TreeValue>, bool, usize, Shape, bool);
    let cases: [Case; 8] = [
        (
            Some(top()),
            vec![
                value(node("frame 3", vec![node("a", vec![])]), false),
                value(node("frame 8", vec![]), true),
            ],
            false,
            5,
            Some(("frame 0", vec![("top", 0), ("frame 3", 1), ("frame 8", 0)])),
            true,
        ),
        (
            Some(top()),
            vec![
                value(node("frame 3", vec![node("a", vec![])]), false),
                value(node("frame 8", vec![node("b", vec![])]), false),
            ],
            false,
            5,
            Some(("frame 0", vec![("top", 0), ("frame 3", 1), ("frame 8", 0)])),
            true,
        ),
        (
            Some(deep),
            vec![leaf("child")],
            false,
            4,
            Some(("root", vec![("one", 2)])),
            true,
        ),
        (
            Some(value(node("frame 0", vec![]), false)),
            vec![leaf("frame 2")],
            false,
            400,
            Some(("frame 0", vec![("frame 2", 0)])),
            false,
        ),
        (
            Some(leaf("top")),
            vec![],
            true,
            400,
            Some(("top", vec![])),
            true,
        ),
        (
            Some(value(node("top", vec![]), true)),
            vec![leaf("frame 2")],
            false,
            400,
            Some(("top", vec![])),
            true,
        ),
        (
            None,
            vec![leaf("frame 2"), leaf("frame 5")],
            false,
            400,
            Some(("frame 2", vec![("frame 5", 0)])),
            false,
        ),
        (
            None,
            vec![leaf("frame 2")],
            true,
            400,
            Some(("frame 2", vec![])),
            true,
        ),
    ];
    for (root, subtrees, unread, limit, expected, truncated) in cases {
        let merged = append_and_trim(root, subtrees, unread, limit);
        let shape = merged.tree.as_ref().map(|tree| {
            assert!(tree.nodes() <= limit);
            let children: Vec<_> = tree
                .children
                .iter()
                .map(|child| (child.label.as_str(), child.children.len()))
                .collect();
            (tree.label.as_str(), children)
        });
        assert_eq!((shape, merged.truncated), (expected, truncated));
    }
    let nothing = append_and_trim(None, Vec::new(), true, 400);
    assert_eq!((nothing.tree, nothing.truncated), (None, true));
}
