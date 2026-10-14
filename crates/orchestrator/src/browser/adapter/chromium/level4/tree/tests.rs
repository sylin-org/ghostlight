use super::*;
use crate::browser::adapter::chromium::level4::wire::FrameFact;

fn frame(id: &str, loader: &str, parent: Option<&str>, url: &str) -> FrameFact {
    FrameFact {
        frame_id: id.into(),
        loader_id: loader.into(),
        parent_frame_id: parent.map(Into::into),
        url: url.into(),
        unreachable: false,
    }
}

fn inventory(frames: Vec<FrameFact>, unseen: &[&str]) -> FrameInventory {
    FrameInventory {
        frames,
        complete: true,
        unseen: unseen.iter().map(|url| (*url).into()).collect(),
    }
}

fn doc(frame: &str, loader: &str) -> FrameRef {
    FrameRef {
        frame_id: frame.into(),
        loader_id: loader.into(),
    }
}

fn physical(id: &str, parent: Option<&str>, url: &str, supported: bool) -> PhysicalDocument {
    PhysicalDocument {
        id: id.into(),
        url: url.into(),
        parent: parent.map(Into::into),
        supported,
    }
}

fn ids(tree: &FrameTree) -> Vec<String> {
    tree.documents().into_iter().map(|d| d.id).collect()
}

/// The extension walks the root session first and appends each out-of-process subtree after it,
/// so the child session of `A` is listed after `B`.
fn nested_inventory() -> FrameInventory {
    inventory(
        vec![
            frame("T", "t1", None, "https://top.test/"),
            frame("A", "a1", Some("T"), "https://a.test/"),
            frame("B", "b1", Some("T"), "https://b.test/"),
            frame("A1", "x1", Some("A"), "https://a1.test/"),
            frame("A2", "x2", Some("A"), "https://a2.test/"),
        ],
        &[],
    )
}

#[test]
fn depth_first_order_keeps_out_of_process_children_under_their_parent() {
    let tree = FrameTree::from_inventory(&nested_inventory()).unwrap();
    assert_eq!(ids(&tree), ["T.t1", "A.a1", "A1.x1", "A2.x2", "B.b1"]);
    assert_eq!(tree.top(), &doc("T", "t1"));
}

#[test]
fn an_unseen_frame_is_a_placeholder_under_the_top_after_its_real_children() {
    let tree = FrameTree::from_inventory(&inventory(
        nested_inventory().frames,
        &["https://hidden.test/", "https://other.test/"],
    ))
    .unwrap();
    assert_eq!(
        ids(&tree),
        ["T.t1", "A.a1", "A1.x1", "A2.x2", "B.b1", "unseen.0", "unseen.1"]
    );
    assert_eq!(tree.placeholder_count(), 2);
    assert_eq!(tree.real_documents().count(), 5);
    let documents = tree.documents();
    let hidden = &documents[5];
    assert_eq!(hidden.parent.as_deref(), Some("T.t1"));
    assert_eq!(hidden.url, "https://hidden.test/");
    assert!(!hidden.supported);
}

#[test]
fn a_placeholder_is_never_addressable() {
    let tree =
        FrameTree::from_inventory(&inventory(nested_inventory().frames, &["https://h.test/"]))
            .unwrap();
    assert_eq!(tree.supported_ref("unseen.0"), None);
    assert_eq!(tree.real_ref("unseen.0"), None);
    assert_eq!(tree.supported_ref("A.a1"), Some(doc("A", "a1")));
    assert!(!tree.contains(&doc("unseen", "0")));
}

#[test]
fn a_replaced_document_is_not_the_document_it_replaced() {
    let tree = FrameTree::from_inventory(&nested_inventory()).unwrap();
    assert!(tree.contains(&doc("A", "a1")));
    assert!(!tree.contains(&doc("A", "a2")));
    assert_eq!(tree.real_ref("A.a2"), None);
    assert_eq!(tree.real_ref("A"), None);
    assert_eq!(tree.real_ref("A.a1.extra"), None);
}

#[test]
fn only_addressable_documents_are_supported() {
    let mut blocked = frame("B", "b1", Some("T"), "https://b.test/");
    blocked.unreachable = true;
    let tree = FrameTree::from_inventory(&inventory(
        vec![
            frame("T", "t1", None, "HTTPS://top.test/"),
            frame("A", "a1", Some("T"), "about:blank"),
            blocked,
            frame("C", "c1", Some("T"), "http://c.test/"),
        ],
        &[],
    ))
    .unwrap();
    assert!(tree.supported_ref("T.t1").is_some());
    assert!(tree.supported_ref("A.a1").is_none());
    assert!(tree.real_ref("A.a1").is_some());
    assert!(tree.supported_ref("B.b1").is_none());
    assert!(tree.supported_ref("C.c1").is_some());
}

#[test]
fn the_tree_read_back_from_its_documents_is_the_same_tree() {
    for unseen in [&[][..], &["https://h.test/"][..]] {
        let tree =
            FrameTree::from_inventory(&inventory(nested_inventory().frames, unseen)).unwrap();
        let again = FrameTree::from_documents(&tree.documents()).unwrap();
        assert_eq!(again, tree);
    }
}

#[test]
fn documents_may_arrive_in_any_order() {
    let tree = FrameTree::from_documents(&[
        physical("A1.x1", Some("A.a1"), "https://a1.test/", true),
        physical("B.b1", Some("T.t1"), "https://b.test/", true),
        physical("A.a1", Some("T.t1"), "https://a.test/", true),
        physical("T.t1", None, "https://top.test/", true),
    ])
    .unwrap();
    assert_eq!(ids(&tree), ["T.t1", "B.b1", "A.a1", "A1.x1"]);
}

#[test]
fn a_placeholder_document_is_unsupported_whatever_the_list_says() {
    let tree = FrameTree::from_documents(&[
        physical("T.t1", None, "https://top.test/", true),
        physical("unseen.3", Some("T.t1"), "https://h.test/", true),
    ])
    .unwrap();
    assert_eq!(tree.placeholder_count(), 1);
    assert!(!tree.documents()[1].supported);
}

#[test]
fn ancestors_run_from_the_nearest_to_the_top() {
    let tree = FrameTree::from_inventory(&nested_inventory()).unwrap();
    assert_eq!(
        tree.ancestors(&doc("A1", "x1")),
        [&doc("A", "a1"), &doc("T", "t1")]
    );
    assert!(tree.ancestors(&doc("T", "t1")).is_empty());
    assert!(tree.ancestors(&doc("A1", "stale")).is_empty());
    assert_eq!(tree.parent(&doc("B", "b1")), Some(&doc("T", "t1")));
    assert_eq!(tree.parent(&doc("T", "t1")), None);
}

#[test]
fn children_are_the_real_documents_one_level_down() {
    let tree =
        FrameTree::from_inventory(&inventory(nested_inventory().frames, &["https://h.test/"]))
            .unwrap();
    assert_eq!(
        tree.children(&doc("T", "t1")),
        [&doc("A", "a1"), &doc("B", "b1")]
    );
    assert_eq!(
        tree.children(&doc("A", "a1")),
        [&doc("A1", "x1"), &doc("A2", "x2")]
    );
    assert!(tree.children(&doc("B", "b1")).is_empty());
    assert!(tree.children(&doc("Z", "z")).is_empty());
}

#[test]
fn the_chain_asks_each_parent_for_the_box_of_the_frame_below_it() {
    let tree = FrameTree::from_inventory(&nested_inventory()).unwrap();
    let chain = tree.chain(&doc("A1", "x1")).unwrap();
    let asked: Vec<_> = chain
        .iter()
        .map(|planned| {
            (
                planned.call.frame.clone().unwrap(),
                planned.call.owner_of.clone().unwrap(),
                planned.call.message["kind"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        asked,
        [
            (doc("A", "a1"), "A1".to_owned(), "owner_box".to_owned()),
            (doc("T", "t1"), "A".to_owned(), "owner_box".to_owned()),
        ]
    );
    assert!(chain
        .iter()
        .all(|planned| planned.scope == world::ScopeClass::Metadata));
    assert!(tree.chain(&doc("T", "t1")).unwrap().is_empty());
    assert_eq!(
        tree.chain(&doc("A1", "stale")),
        Err(BrowserError::DocumentUnavailable)
    );
}

#[test]
fn closing_under_ancestors_adds_every_document_above_without_siblings() {
    let tree = FrameTree::from_inventory(&nested_inventory()).unwrap();
    let urls = |required: &[FrameRef]| -> Vec<String> {
        tree.closed_under_ancestors(required)
            .into_iter()
            .map(|document| document_id(&document.doc))
            .collect()
    };
    assert_eq!(urls(&[doc("A2", "x2")]), ["T.t1", "A.a1", "A2.x2"]);
    assert_eq!(
        urls(&[doc("B", "b1"), doc("A1", "x1")]),
        ["T.t1", "A.a1", "A1.x1", "B.b1"]
    );
    assert_eq!(urls(&[doc("A2", "stale")]), Vec::<String>::new());
    assert_eq!(urls(&[]), Vec::<String>::new());
}

#[test]
fn a_guard_document_carries_the_admitted_identity_and_url() {
    let tree = FrameTree::from_inventory(&nested_inventory()).unwrap();
    let guard: Vec<GuardDoc> = tree.real_documents().map(GuardDoc::from).collect();
    assert_eq!(guard[1].frame_id, "A");
    assert_eq!(guard[1].loader_id, "a1");
    assert_eq!(guard[1].url, "https://a.test/");
}

#[test]
fn an_inventory_that_is_not_one_tree_is_refused() {
    let top = frame("T", "t1", None, "https://top.test/");
    let child = |id: &str, parent: &str| frame(id, "l1", Some(parent), "https://c.test/");
    let refused = [
        inventory(vec![], &[]),
        inventory(vec![child("A", "T")], &[]),
        inventory(
            vec![top.clone(), frame("U", "u1", None, "https://u.test/")],
            &[],
        ),
        inventory(vec![top.clone(), child("A", "Missing")], &[]),
        inventory(vec![top.clone(), child("A", "A")], &[]),
        inventory(vec![top.clone(), child("A", "B"), child("B", "A")], &[]),
        inventory(vec![top.clone(), child("A", "T"), child("A", "T")], &[]),
    ];
    for inventory in refused {
        assert_eq!(
            FrameTree::from_inventory(&inventory),
            Err(BrowserError::DocumentUnavailable),
            "{inventory:?}"
        );
    }
}

#[test]
fn a_tree_over_the_document_limit_is_refused() {
    let mut frames = vec![frame("T", "t1", None, "https://top.test/")];
    for index in 1..DOCUMENT_LIMIT {
        frames.push(frame(
            &format!("F{index}"),
            "l",
            Some("T"),
            "https://c.test/",
        ));
    }
    assert!(FrameTree::from_inventory(&inventory(frames.clone(), &[])).is_ok());
    assert_eq!(
        FrameTree::from_inventory(&inventory(frames, &["https://h.test/"])),
        Err(BrowserError::DocumentUnavailable)
    );
}

#[test]
fn a_document_list_that_is_not_one_tree_is_refused() {
    let top = physical("T.t1", None, "https://top.test/", true);
    let under = |id: &str, parent: &str| physical(id, Some(parent), "https://c.test/", true);
    let refused = [
        vec![],
        vec![under("A.a1", "T.t1")],
        vec![top.clone(), physical("U.u1", None, "https://u.test/", true)],
        vec![top.clone(), top.clone()],
        vec![top.clone(), under("A.a1", "Missing.m1")],
        vec![top.clone(), under("A.a1", "B.b1"), under("B.b1", "A.a1")],
        vec![top.clone(), under("A.a1", "T.t1"), under("A.a2", "T.t1")],
        vec![
            top.clone(),
            under("unseen.0", "T.t1"),
            under("A.a1", "unseen.0"),
        ],
        vec![physical("unseen.0", None, "https://h.test/", false)],
        vec![top.clone(), under("A", "T.t1")],
        vec![top.clone(), under("A.a1.extra", "T.t1")],
        vec![top.clone(), under(".a1", "T.t1")],
        vec![top.clone(), under("unseen.x", "T.t1")],
        vec![top.clone(), under("unseen.", "T.t1")],
        vec![top, under("A B.a1", "T.t1")],
    ];
    for documents in refused {
        assert_eq!(
            FrameTree::from_documents(&documents),
            Err(BrowserError::DocumentUnavailable),
            "{documents:?}"
        );
    }
}
