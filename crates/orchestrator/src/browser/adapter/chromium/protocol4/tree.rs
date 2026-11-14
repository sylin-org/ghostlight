//! The frame tree of one tab, in the order the service reads it.
//!
//! The tree comes from two places that must agree: a live `frames` inventory the extension
//! reports, and the document list the authority admitted a request against. Both reduce to the
//! same shape: one top document, every other document under its parent, out-of-process frames
//! included, in depth-first order with the top first. Frames the extension knows exist but cannot
//! describe stay in the tree as placeholders. They are never targetable, and they make the tree
//! incomplete for any check that needs all of it.
//!
//! This module owns tree shape. An inventory is whatever a published extension sends, so reading
//! one never fails on its shape: what reaches the top is placed, the rest is left out, and the
//! result says it is not whole. The authority's list is this module's own output handed back, so
//! a list that is not one tree is a protocol failure.
//!
//! A document is named to the authority by `{frame_id}.{loader_id}` and a placeholder by
//! `unseen.{n}`. Frame ids are not unique over time, so the loader id is part of every name.

use std::collections::{HashMap, HashSet};

use super::locator::is_browser_id;
use super::scope::PlannedCall;
use super::wire::{FrameFact, FrameInventory, FrameRef, GuardDoc};
use super::world;
use crate::browser::adapter::documents::{PhysicalDocument, DOCUMENT_LIMIT};
use crate::browser::BrowserError;
use crate::language::document_refusal::DocumentCause;

/// The separator between the parts of a document name.
const ID_SEPARATOR: char = '.';
/// The first part of a placeholder's name.
const UNSEEN: &str = "unseen";
/// The protocol failure of a document list or a document that is not of the tree the adapter
/// described.
const NOT_THE_DESCRIBED_TREE: &str = "documents are not the tree the adapter described";
/// The schemes whose documents the adapter can address.
const ADDRESSABLE_SCHEMES: [&str; 2] = ["http:", "https:"];

/// One real document of the tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeDocument {
    /// The frame and the loader that produced its current content.
    pub doc: FrameRef,
    /// The URL the document was admitted at.
    pub url: String,
    /// Whether the adapter can address this document.
    pub supported: bool,
}

impl From<&TreeDocument> for GuardDoc {
    fn from(document: &TreeDocument) -> Self {
        Self {
            frame_id: document.doc.frame_id.clone(),
            loader_id: document.doc.loader_id.clone(),
            url: document.url.clone(),
        }
    }
}

/// What a node of the tree stands for.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Slot {
    /// A document the extension described.
    Real(TreeDocument),
    /// A frame the extension knows exists and could not describe.
    Unseen { ordinal: usize, url: String },
}

/// One node: what it stands for, and the index of its parent.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Node {
    slot: Slot,
    parent: Option<usize>,
}

/// A name read from the authority's document list.
enum DocumentId {
    Real(FrameRef),
    Unseen(usize),
}

/// The name the authority knows a document by.
#[must_use]
pub fn document_id(doc: &FrameRef) -> String {
    format!("{}{ID_SEPARATOR}{}", doc.frame_id, doc.loader_id)
}

fn unseen_id(ordinal: usize) -> String {
    format!("{UNSEEN}{ID_SEPARATOR}{ordinal}")
}

fn parse_document_id(id: &str) -> Option<DocumentId> {
    let (head, tail) = id.split_once(ID_SEPARATOR)?;
    if head == UNSEEN {
        return if tail.bytes().all(|byte| byte.is_ascii_digit()) {
            tail.parse().ok().map(DocumentId::Unseen)
        } else {
            None
        };
    }
    (is_browser_id(head) && is_browser_id(tail)).then(|| {
        DocumentId::Real(FrameRef {
            frame_id: head.to_owned(),
            loader_id: tail.to_owned(),
        })
    })
}

fn not_the_described_tree() -> BrowserError {
    BrowserError::Protocol(NOT_THE_DESCRIBED_TREE.into())
}

/// Whether the adapter can address a document at this URL.
fn addressable(url: &str) -> bool {
    ADDRESSABLE_SCHEMES.iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
    })
}

/// The documents of one tab: the top, and every other document under its parent, in depth-first
/// order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameTree {
    top: FrameRef,
    nodes: Vec<Node>,
}

/// The tree read from one frame inventory, and whether it is the whole tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Described {
    /// Every listed frame that reaches the top, with the frames the extension could not describe
    /// as placeholders.
    pub tree: FrameTree,
    /// Whether the tree holds every document of the tab: the extension called its inventory
    /// complete and nothing it listed was left out.
    pub whole: bool,
}

impl FrameTree {
    /// Read the tree from the extension's frame inventory. This is the one owner of tree shape:
    /// whatever the extension lists, the result is one tree.
    ///
    /// Frames come in the order the extension walked its sessions, so an out-of-process child
    /// can follow frames that are not its siblings. Each lands under its parent. The top is the
    /// first frame with no parent, and a frame is placed when its parents reach the top. A
    /// frame that does not (a parent that is not listed, a second frame with no parent, a
    /// repeated frame id, a loop) is left out, as is one whose ids cannot make a document name
    /// and anything past the document limit. Unseen
    /// frames become placeholders under the top while room is left. A tree with anything left
    /// out is not whole, and neither is one from an inventory the extension called incomplete.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::DocumentUnavailable`] when no listed frame is a top document.
    pub fn describe(inventory: &FrameInventory) -> Result<Described, BrowserError> {
        let frames = &inventory.frames;
        // A frame is named to the authority by its two ids, and the name must read back. One
        // whose ids cannot make a name is not listed as far as the tree goes.
        let nameable =
            |frame: &FrameFact| is_browser_id(&frame.frame_id) && is_browser_id(&frame.loader_id);
        let mut by_frame: HashMap<&str, usize> = HashMap::with_capacity(frames.len());
        for (index, frame) in frames.iter().enumerate() {
            if nameable(frame) {
                by_frame.entry(frame.frame_id.as_str()).or_insert(index);
            }
        }
        // The fact this site knows: the inventory lists no frame without a parent, so the tab
        // shows no top document to place anything under.
        let top = frames
            .iter()
            .position(|frame| frame.parent_frame_id.is_none() && nameable(frame))
            .ok_or(BrowserError::DocumentUnavailable(
                DocumentCause::NoWebDocument,
            ))?;
        let mut children = vec![Vec::new(); frames.len()];
        for (index, frame) in frames.iter().enumerate() {
            let first = by_frame.get(frame.frame_id.as_str()) == Some(&index);
            let parent = frame
                .parent_frame_id
                .as_deref()
                .and_then(|parent| by_frame.get(parent));
            if let (true, Some(parent)) = (first, parent) {
                children[*parent].push(index);
            }
        }
        let mut nodes = Vec::with_capacity(frames.len().min(DOCUMENT_LIMIT));
        let mut pending = vec![(top, None)];
        while let Some((index, parent)) = pending.pop() {
            if nodes.len() == DOCUMENT_LIMIT {
                break;
            }
            let frame = &frames[index];
            let placed = nodes.len();
            nodes.push(Node {
                slot: Slot::Real(TreeDocument {
                    doc: FrameRef {
                        frame_id: frame.frame_id.clone(),
                        loader_id: frame.loader_id.clone(),
                    },
                    url: frame.url.clone(),
                    supported: !frame.unreachable && addressable(&frame.url),
                }),
                parent,
            });
            pending.extend(
                children[index]
                    .iter()
                    .rev()
                    .map(|child| (*child, Some(placed))),
            );
        }
        let placed_frames = nodes.len();
        let room = DOCUMENT_LIMIT - placed_frames;
        nodes.extend(
            inventory
                .unseen
                .iter()
                .enumerate()
                .take(room)
                .map(|(ordinal, url)| Node {
                    slot: Slot::Unseen {
                        ordinal,
                        url: url.clone(),
                    },
                    parent: Some(0),
                }),
        );
        let whole = inventory.complete
            && placed_frames == frames.len()
            && nodes.len() == frames.len() + inventory.unseen.len();
        let top = FrameRef {
            frame_id: frames[top].frame_id.clone(),
            loader_id: frames[top].loader_id.clone(),
        };
        Ok(Described {
            tree: Self { top, nodes },
            whole,
        })
    }

    /// Read the tree from the documents the authority admitted a request against.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::Protocol`] for a name that is neither `{frame}.{loader}` nor
    /// `unseen.{n}`, and for a list that is not one tree. The authority hands back the documents
    /// [`Self::describe`] reported, so either is a breach between the two, not a page state.
    pub fn from_documents(documents: &[PhysicalDocument]) -> Result<Self, BrowserError> {
        let mut by_id = HashMap::with_capacity(documents.len());
        for (index, document) in documents.iter().enumerate() {
            if by_id.insert(document.id.as_str(), index).is_some() {
                return Err(not_the_described_tree());
            }
        }
        let mut slots = Vec::with_capacity(documents.len());
        let mut parents = Vec::with_capacity(documents.len());
        for document in documents {
            slots.push(
                match parse_document_id(&document.id).ok_or_else(not_the_described_tree)? {
                    DocumentId::Real(doc) => Slot::Real(TreeDocument {
                        doc,
                        url: document.url.clone(),
                        supported: document.supported,
                    }),
                    DocumentId::Unseen(ordinal) => Slot::Unseen {
                        ordinal,
                        url: document.url.clone(),
                    },
                },
            );
            parents.push(match &document.parent {
                None => None,
                Some(parent) => Some(
                    *by_id
                        .get(parent.as_str())
                        .ok_or_else(not_the_described_tree)?,
                ),
            });
        }
        Self::assemble(slots, &parents)
    }

    /// Check that nodes and parents make one tree, and put them in depth-first order. Children
    /// keep the order they came in.
    fn assemble(slots: Vec<Slot>, parents: &[Option<usize>]) -> Result<Self, BrowserError> {
        let count = slots.len();
        if count == 0 || count > DOCUMENT_LIMIT {
            return Err(not_the_described_tree());
        }
        let mut frames = HashSet::new();
        let mut ordinals = HashSet::new();
        for slot in &slots {
            let fresh = match slot {
                Slot::Real(document) => frames.insert(document.doc.frame_id.as_str()),
                Slot::Unseen { ordinal, .. } => ordinals.insert(*ordinal),
            };
            if !fresh {
                return Err(not_the_described_tree());
            }
        }
        let mut children = vec![Vec::new(); count];
        let mut root = None;
        for (index, parent) in parents.iter().enumerate() {
            match parent {
                None if root.is_none() => root = Some(index),
                None => return Err(not_the_described_tree()),
                Some(parent) => {
                    if *parent == index || !matches!(slots[*parent], Slot::Real(_)) {
                        return Err(not_the_described_tree());
                    }
                    children[*parent].push(index);
                }
            }
        }
        let root = root.ok_or_else(not_the_described_tree)?;
        let Slot::Real(top) = &slots[root] else {
            return Err(not_the_described_tree());
        };
        let top = top.doc.clone();
        let mut order = Vec::with_capacity(count);
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            order.push(index);
            pending.extend(children[index].iter().rev());
        }
        if order.len() != count {
            return Err(not_the_described_tree());
        }
        let mut position = vec![0; count];
        for (new, old) in order.iter().enumerate() {
            position[*old] = new;
        }
        let mut slots: Vec<Option<Slot>> = slots.into_iter().map(Some).collect();
        let nodes = order
            .iter()
            .map(|old| {
                slots[*old].take().map(|slot| Node {
                    slot,
                    parent: parents[*old].map(|parent| position[parent]),
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(not_the_described_tree)?;
        Ok(Self { top, nodes })
    }

    /// The documents as the authority names them, in depth-first order.
    #[must_use]
    pub fn documents(&self) -> Vec<PhysicalDocument> {
        let id_at = |index: usize| match &self.nodes[index].slot {
            Slot::Real(document) => document_id(&document.doc),
            Slot::Unseen { ordinal, .. } => unseen_id(*ordinal),
        };
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let (url, supported) = match &node.slot {
                    Slot::Real(document) => (document.url.clone(), document.supported),
                    Slot::Unseen { url, .. } => (url.clone(), false),
                };
                PhysicalDocument {
                    id: id_at(index),
                    url,
                    parent: node.parent.map(id_at),
                    supported,
                }
            })
            .collect()
    }

    /// The top document.
    #[must_use]
    pub const fn top(&self) -> &FrameRef {
        &self.top
    }

    /// Whether this exact document, loader included, is in the tree.
    #[must_use]
    pub fn contains(&self, doc: &FrameRef) -> bool {
        self.index_of(doc).is_some()
    }

    /// Whether this exact document is in the tree and the adapter can address it.
    #[must_use]
    pub fn addresses(&self, doc: &FrameRef) -> bool {
        self.real_documents()
            .any(|document| document.doc == *doc && document.supported)
    }

    /// The URL of the top document, as the inventory reported it or the authority admitted it.
    #[must_use]
    pub fn top_url(&self) -> &str {
        self.real_documents()
            .find(|document| document.doc == self.top)
            .map_or("", |document| document.url.as_str())
    }

    /// The real document the authority names, whether or not the adapter can address it.
    #[must_use]
    pub fn real_ref(&self, id: &str) -> Option<FrameRef> {
        let DocumentId::Real(doc) = parse_document_id(id)? else {
            return None;
        };
        self.contains(&doc).then_some(doc)
    }

    /// The real document the authority names when the adapter can address it. A placeholder is
    /// never addressable.
    #[must_use]
    pub fn supported_ref(&self, id: &str) -> Option<FrameRef> {
        let doc = self.real_ref(id)?;
        self.addresses(&doc).then_some(doc)
    }

    /// The real documents, in depth-first order.
    pub fn real_documents(&self) -> impl Iterator<Item = &TreeDocument> {
        self.nodes.iter().filter_map(|node| match &node.slot {
            Slot::Real(document) => Some(document),
            Slot::Unseen { .. } => None,
        })
    }

    /// How many frames the extension knew of and could not describe.
    #[must_use]
    pub fn placeholder_count(&self) -> u16 {
        let count = self
            .nodes
            .iter()
            .filter(|node| matches!(node.slot, Slot::Unseen { .. }))
            .count();
        u16::try_from(count).unwrap_or(u16::MAX)
    }

    /// The real documents this one embeds directly, in tree order.
    #[must_use]
    pub fn children(&self, doc: &FrameRef) -> Vec<&FrameRef> {
        let Some(index) = self.index_of(doc) else {
            return Vec::new();
        };
        self.nodes
            .iter()
            .filter(|node| node.parent == Some(index))
            .filter_map(|node| match &node.slot {
                Slot::Real(document) => Some(&document.doc),
                Slot::Unseen { .. } => None,
            })
            .collect()
    }

    /// The owner-box calls that place a document in the top viewport: one per frame from the
    /// document up to the top, exclusive, each made in the frame's parent and naming the frame.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::Protocol`] when the document is not in the tree: a caller binds
    /// its document to the tree before it asks where the document sits.
    pub fn chain(&self, doc: &FrameRef) -> Result<Vec<PlannedCall>, BrowserError> {
        let mut index = self.index_of(doc).ok_or_else(not_the_described_tree)?;
        let mut calls = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            let (Some(frame), Some(owner)) = (self.real_at(index), self.real_at(parent)) else {
                return Err(not_the_described_tree());
            };
            calls.push(PlannedCall::owner(
                &owner.doc,
                &frame.doc.frame_id,
                world::owner_box(),
            ));
            index = parent;
        }
        Ok(calls)
    }

    /// The required documents and every document above them, in depth-first order. Documents
    /// the tree does not hold are ignored.
    #[must_use]
    pub fn closed_under_ancestors<'a>(
        &self,
        required: impl IntoIterator<Item = &'a FrameRef>,
    ) -> Vec<&TreeDocument> {
        let mut kept = vec![false; self.nodes.len()];
        for doc in required {
            let mut next = self.index_of(doc);
            while let Some(index) = next {
                if kept[index] {
                    break;
                }
                kept[index] = true;
                next = self.nodes[index].parent;
            }
        }
        self.nodes
            .iter()
            .zip(kept)
            .filter(|(_, kept)| *kept)
            .filter_map(|(node, _)| match &node.slot {
                Slot::Real(document) => Some(document),
                Slot::Unseen { .. } => None,
            })
            .collect()
    }

    fn index_of(&self, doc: &FrameRef) -> Option<usize> {
        self.nodes
            .iter()
            .position(|node| matches!(&node.slot, Slot::Real(document) if document.doc == *doc))
    }

    fn real_at(&self, index: usize) -> Option<&TreeDocument> {
        match &self.nodes.get(index)?.slot {
            Slot::Real(document) => Some(document),
            Slot::Unseen { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
