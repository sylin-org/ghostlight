//! The frame tree of one tab, in the order the service reads it.
//!
//! The tree comes from two places that must agree: a live `frames` inventory the extension
//! reports, and the document list the authority admitted a request against. Both reduce to the
//! same shape: one top document, every other document under its parent, out-of-process frames
//! included, in depth-first order with the top first. Frames the extension knows exist but cannot
//! describe stay in the tree as placeholders. They are never targetable, and they make the tree
//! incomplete for any check that needs all of it.
//!
//! A document is named to the authority by `{frame_id}.{loader_id}` and a placeholder by
//! `unseen.{n}`. Frame ids are not unique over time, so the loader id is part of every name.

use std::collections::{HashMap, HashSet};

use super::locator::is_browser_id;
use super::scope::PlannedCall;
use super::wire::{FrameInventory, FrameRef, GuardDoc};
use super::world;
use crate::browser::adapter::documents::{PhysicalDocument, DOCUMENT_LIMIT};
use crate::browser::BrowserError;

/// The separator between the parts of a document name.
const ID_SEPARATOR: char = '.';
/// The first part of a placeholder's name.
const UNSEEN: &str = "unseen";
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

impl FrameTree {
    /// Read the tree from the extension's frame inventory.
    ///
    /// Frames come in the order the extension walked its sessions, so an out-of-process child
    /// can follow frames that are not its siblings. Each lands under its parent. Unseen frames
    /// become placeholders under the top.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::DocumentUnavailable`] for an inventory that is not one tree: no
    /// frames, no or several tops, a duplicate frame, a missing parent, a cycle, or more
    /// documents than the document limit.
    pub fn from_inventory(inventory: &FrameInventory) -> Result<Self, BrowserError> {
        let by_frame: HashMap<&str, usize> = inventory
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| (frame.frame_id.as_str(), index))
            .collect();
        let mut slots = Vec::with_capacity(inventory.frames.len() + inventory.unseen.len());
        let mut parents = Vec::with_capacity(slots.capacity());
        for frame in &inventory.frames {
            slots.push(Slot::Real(TreeDocument {
                doc: FrameRef {
                    frame_id: frame.frame_id.clone(),
                    loader_id: frame.loader_id.clone(),
                },
                url: frame.url.clone(),
                supported: !frame.unreachable && addressable(&frame.url),
            }));
            parents.push(match &frame.parent_frame_id {
                None => None,
                Some(parent) => Some(
                    *by_frame
                        .get(parent.as_str())
                        .ok_or(BrowserError::DocumentUnavailable)?,
                ),
            });
        }
        let top = parents
            .iter()
            .position(Option::is_none)
            .ok_or(BrowserError::DocumentUnavailable)?;
        for (ordinal, url) in inventory.unseen.iter().enumerate() {
            slots.push(Slot::Unseen {
                ordinal,
                url: url.clone(),
            });
            parents.push(Some(top));
        }
        Self::assemble(slots, &parents)
    }

    /// Read the tree from the documents the authority admitted a request against.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::DocumentUnavailable`] for a name that is neither
    /// `{frame}.{loader}` nor `unseen.{n}`, and for a list that is not one tree.
    pub fn from_documents(documents: &[PhysicalDocument]) -> Result<Self, BrowserError> {
        let mut by_id = HashMap::with_capacity(documents.len());
        for (index, document) in documents.iter().enumerate() {
            if by_id.insert(document.id.as_str(), index).is_some() {
                return Err(BrowserError::DocumentUnavailable);
            }
        }
        let mut slots = Vec::with_capacity(documents.len());
        let mut parents = Vec::with_capacity(documents.len());
        for document in documents {
            slots.push(
                match parse_document_id(&document.id).ok_or(BrowserError::DocumentUnavailable)? {
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
                        .ok_or(BrowserError::DocumentUnavailable)?,
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
            return Err(BrowserError::DocumentUnavailable);
        }
        let mut frames = HashSet::new();
        let mut ordinals = HashSet::new();
        for slot in &slots {
            let fresh = match slot {
                Slot::Real(document) => frames.insert(document.doc.frame_id.as_str()),
                Slot::Unseen { ordinal, .. } => ordinals.insert(*ordinal),
            };
            if !fresh {
                return Err(BrowserError::DocumentUnavailable);
            }
        }
        let mut children = vec![Vec::new(); count];
        let mut root = None;
        for (index, parent) in parents.iter().enumerate() {
            match parent {
                None if root.is_none() => root = Some(index),
                None => return Err(BrowserError::DocumentUnavailable),
                Some(parent) => {
                    if *parent == index || !matches!(slots[*parent], Slot::Real(_)) {
                        return Err(BrowserError::DocumentUnavailable);
                    }
                    children[*parent].push(index);
                }
            }
        }
        let root = root.ok_or(BrowserError::DocumentUnavailable)?;
        let Slot::Real(top) = &slots[root] else {
            return Err(BrowserError::DocumentUnavailable);
        };
        let top = top.doc.clone();
        let mut order = Vec::with_capacity(count);
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            order.push(index);
            pending.extend(children[index].iter().rev());
        }
        if order.len() != count {
            return Err(BrowserError::DocumentUnavailable);
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
            .ok_or(BrowserError::DocumentUnavailable)?;
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
        self.real_documents()
            .any(|document| document.doc == doc && document.supported)
            .then_some(doc)
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

    /// The document that embeds this one.
    #[must_use]
    pub fn parent(&self, doc: &FrameRef) -> Option<&FrameRef> {
        let parent = self.nodes[self.index_of(doc)?].parent?;
        self.real_at(parent).map(|document| &document.doc)
    }

    /// Every document that embeds this one, nearest first, ending at the top.
    #[must_use]
    pub fn ancestors(&self, doc: &FrameRef) -> Vec<&FrameRef> {
        let mut ancestors = Vec::new();
        let mut current = doc;
        while let Some(parent) = self.parent(current) {
            ancestors.push(parent);
            current = parent;
        }
        ancestors
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
    /// Returns [`BrowserError::DocumentUnavailable`] when the document is not in the tree.
    pub fn chain(&self, doc: &FrameRef) -> Result<Vec<PlannedCall>, BrowserError> {
        let mut index = self
            .index_of(doc)
            .ok_or(BrowserError::DocumentUnavailable)?;
        let mut calls = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            let (Some(frame), Some(owner)) = (self.real_at(index), self.real_at(parent)) else {
                return Err(BrowserError::DocumentUnavailable);
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
