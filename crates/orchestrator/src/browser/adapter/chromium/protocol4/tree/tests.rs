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

/// The extension's frame inventory always decodes to one tree: documents in depth-first order,
/// with each out-of-process subtree under its parent though the extension lists it last, so
/// `nth` and form numbers count in one stable order; a frame the extension cannot see as a
/// placeholder under the top that is never addressable; only a real web page addressable, never
/// `about:blank` or a frame that did not load. A list that is not one tree, or holds more
/// documents than the protocol allows, places what reaches the top, leaves out the rest and says
/// it is not whole, so a malformed answer from a published extension never gives a frame the
/// wrong parent or another's authority and never refuses the page by its shape. Only an
/// inventory with no top is refused.
#[test]
fn an_inventory_becomes_one_tree_in_document_order_with_only_web_pages_addressable() {
    let described = FrameTree::describe(&inventory(
        nested_inventory().frames,
        &["https://hidden.test/", "https://other.test/"],
    ))
    .unwrap();
    assert!(described.whole, "unseen frames are placed, not left out");
    let tree = described.tree;
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
    assert_eq!(
        FrameTree::from_documents(&tree.documents()),
        Ok(tree),
        "the authority hands the described tree back unchanged"
    );

    let mut blocked = frame("B", "b1", Some("T"), "https://b.test/");
    blocked.unreachable = true;
    let pages = FrameTree::describe(&inventory(
        vec![
            frame("T", "t1", None, "HTTPS://top.test/"),
            frame("A", "a1", Some("T"), "about:blank"),
            blocked,
            frame("C", "c1", Some("T"), "http://c.test/"),
        ],
        &[],
    ))
    .unwrap()
    .tree;
    for (id, addressable) in [
        ("T.t1", true),
        ("A.a1", false),
        ("B.b1", false),
        ("C.c1", true),
    ] {
        assert_eq!(pages.supported_ref(id).is_some(), addressable, "{id}");
        assert!(pages.real_ref(id).is_some(), "{id} is still in the tree");
    }

    an_inventory_that_is_not_one_tree_places_what_reaches_the_top();
}

/// What a published extension can send for the life of a tab: a list that is not one tree. Each
/// row is the frames listed and the documents placed. Nothing but a missing top is an error, and
/// every row with a frame left out is not whole.
fn an_inventory_that_is_not_one_tree_places_what_reaches_the_top() {
    let top = frame("T", "t1", None, "https://top.test/");
    let child = |id: &str, parent: &str| frame(id, "l1", Some(parent), "https://c.test/");
    let mut full = vec![top.clone()];
    for index in 1..DOCUMENT_LIMIT {
        full.push(child(&format!("F{index}"), "T"));
    }
    let whole = FrameTree::describe(&inventory(full.clone(), &[])).unwrap();
    assert!(whole.whole);
    assert_eq!(whole.tree.documents().len(), DOCUMENT_LIMIT);

    let mut over = full.clone();
    over.push(child("Over", "T"));
    for (listed, placed) in [
        (
            vec![top.clone(), frame("U", "u1", None, "https://u.test/")],
            vec!["T.t1"],
        ),
        (
            vec![top.clone(), child("A", "T"), child("B", "Missing")],
            vec!["T.t1", "A.l1"],
        ),
        (vec![top.clone(), child("A", "A")], vec!["T.t1"]),
        (
            vec![top.clone(), child("A", "B"), child("B", "A")],
            vec!["T.t1"],
        ),
        (
            vec![
                top.clone(),
                child("A", "T"),
                frame("A", "l2", Some("T"), "https://again.test/"),
                child("B", "A"),
            ],
            vec!["T.t1", "A.l1", "B.l1"],
        ),
        // Ids that cannot make a document name: the frame and what it embeds are left out.
        (
            vec![
                top.clone(),
                frame("A", "", Some("T"), "https://a.test/"),
                child("B", "A"),
                child("C.c", "T"),
                child("D", "T"),
            ],
            vec!["T.t1", "D.l1"],
        ),
    ] {
        let described = FrameTree::describe(&inventory(listed, &[])).unwrap();
        assert_eq!(ids(&described.tree), placed);
        assert!(!described.whole, "{placed:?} leaves a listed frame out");
        assert_eq!(
            FrameTree::from_documents(&described.tree.documents()).as_ref(),
            Ok(&described.tree),
            "whatever was listed, the authority can hand the described tree back"
        );
    }
    for (inventory, kept) in [
        (inventory(over, &[]), DOCUMENT_LIMIT),
        (inventory(full, &["https://h.test/"]), DOCUMENT_LIMIT),
    ] {
        let described = FrameTree::describe(&inventory).unwrap();
        assert_eq!(described.tree.documents().len(), kept);
        assert!(!described.whole, "a document past the limit is left out");
    }
    let mut partial = inventory(vec![top.clone(), child("A", "T")], &[]);
    partial.complete = false;
    let described = FrameTree::describe(&partial).unwrap();
    assert_eq!(ids(&described.tree), ["T.t1", "A.l1"]);
    assert!(!described.whole, "the extension's own word stands");

    for no_top in [
        inventory(vec![], &[]),
        inventory(vec![child("A", "T")], &[]),
        inventory(vec![frame("T", "", None, "https://top.test/")], &[]),
    ] {
        assert_eq!(
            FrameTree::describe(&no_top),
            Err(BrowserError::DocumentUnavailable(
                DocumentCause::NoWebDocument
            )),
            "{} frames",
            no_top.frames.len()
        );
    }
    let mut twice = whole.tree.documents();
    twice.push(twice[1].clone());
    assert!(
        matches!(
            FrameTree::from_documents(&twice),
            Err(BrowserError::Protocol(_))
        ),
        "a list the authority hands back that is not the described tree is a breach"
    );
}
