use super::*;
use crate::browser::adapter::chromium::protocol4::wire::FrameFact;

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

/// The extension's frame inventory decodes to one tree or to a refusal: documents in depth-first
/// order, with each out-of-process subtree under its parent though the extension lists it last,
/// so `nth` and form numbers count in one stable order; a frame the extension cannot see as a
/// placeholder under the top that is never addressable; only a real web page addressable, never
/// `about:blank` or a frame that did not load; and an inventory that is not one tree, or holds
/// more documents than the protocol allows, refused, so a malformed answer never gives a frame the
/// wrong parent or another's authority.
#[test]
fn an_inventory_becomes_one_tree_in_document_order_with_only_web_pages_addressable() {
    let tree = FrameTree::from_inventory(&inventory(
        nested_inventory().frames,
        &["https://hidden.test/", "https://other.test/"],
    ))
    .unwrap();
    assert_eq!(
        ids(&tree),
        ["T.t1", "A.a1", "A1.x1", "A2.x2", "B.b1", "unseen.0", "unseen.1"]
    );
    assert_eq!(tree.top(), &doc("T", "t1"));
    let hidden = &tree.documents()[5];
    assert_eq!(
        (hidden.parent.as_deref(), hidden.supported),
        (Some("T.t1"), false)
    );
    assert_eq!(tree.supported_ref("unseen.0"), None);
    assert_eq!(tree.real_ref("unseen.0"), None);
    assert!(
        !tree.contains(&doc("A", "a2")),
        "a replaced document is gone"
    );

    let mut blocked = frame("B", "b1", Some("T"), "https://b.test/");
    blocked.unreachable = true;
    let pages = FrameTree::from_inventory(&inventory(
        vec![
            frame("T", "t1", None, "HTTPS://top.test/"),
            frame("A", "a1", Some("T"), "about:blank"),
            blocked,
            frame("C", "c1", Some("T"), "http://c.test/"),
        ],
        &[],
    ))
    .unwrap();
    for (id, addressable) in [
        ("T.t1", true),
        ("A.a1", false),
        ("B.b1", false),
        ("C.c1", true),
    ] {
        assert_eq!(pages.supported_ref(id).is_some(), addressable, "{id}");
        assert!(pages.real_ref(id).is_some(), "{id} is still in the tree");
    }

    an_inventory_that_is_not_one_tree_is_refused();
}

/// An inventory that is not one tree, or that holds more documents than the protocol allows, is
/// refused, so a malformed answer never gives a frame the wrong parent or the authority of
/// another.
fn an_inventory_that_is_not_one_tree_is_refused() {
    let top = frame("T", "t1", None, "https://top.test/");
    let child = |id: &str, parent: &str| frame(id, "l1", Some(parent), "https://c.test/");
    let mut full = vec![top.clone()];
    for index in 1..DOCUMENT_LIMIT {
        full.push(child(&format!("F{index}"), "T"));
    }
    assert!(FrameTree::from_inventory(&inventory(full.clone(), &[])).is_ok());
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
        inventory(full, &["https://h.test/"]),
    ];
    for inventory in refused {
        assert_eq!(
            FrameTree::from_inventory(&inventory),
            Err(BrowserError::DocumentUnavailable),
            "{} frames",
            inventory.frames.len()
        );
    }
}
