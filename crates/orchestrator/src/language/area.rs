//! The page areas an observed item can belong to, and the tiers a capped listing is cut by
//! (ADR-0198 Decisions 19 and 20).
//!
//! The page tells which landmark an item sits in, and this module is the one closed list of
//! those words. A word the page sends that is not on the list is dropped, so a changed or
//! hostile page cannot put a new word in front of the model. An item with no area is page
//! content: the main region or unmarked markup.
//!
//! When a listing has more items than its cap, it keeps the tiers that matter most and counts
//! what it left out. The page, the fold across documents and the facts the model reads all use
//! the one closed list of tier words defined here.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

/// A landmark region of a page, as the page reports it for one item.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageArea {
    /// The page banner: a header that belongs to the page rather than to one section or dialog.
    Header,
    /// A navigation region.
    Nav,
    /// A complementary region, such as a sidebar.
    Aside,
    /// The page footer: a footer that belongs to the page rather than to one section or dialog.
    Footer,
    /// A dialog, and the header and footer inside it.
    Dialog,
    /// A search region.
    Search,
}

impl PageArea {
    /// Every area, in the order the model-facing documents list them.
    pub const ALL: [Self; 6] = [
        Self::Header,
        Self::Nav,
        Self::Aside,
        Self::Footer,
        Self::Dialog,
        Self::Search,
    ];

    /// The area a word from the page names, or `None` for any other word.
    #[must_use]
    pub fn classify(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|area| area.word() == word)
    }

    /// The word the page sends and the model sees for this area.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Nav => "nav",
            Self::Aside => "aside",
            Self::Footer => "footer",
            Self::Dialog => "dialog",
            Self::Search => "search",
        }
    }
}

/// The state word the page gives an item it does not show.
pub const HIDDEN_STATE: &str = "hidden";

/// Where an item falls when a capped listing is cut, or why it was dropped before the cut.
///
/// The first four are tiers, from the one cut first to the one kept longest. `BehindDialog` is
/// not a tier: those items are dropped whatever the cap, because the page cannot act on them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LeftOutKey {
    /// An item the page does not show. Cut first.
    Hidden,
    /// An item in the page header, navigation or footer.
    Chrome,
    /// An item in a complementary region, such as a sidebar.
    Aside,
    /// Everything else: dialogs, forms, the main region and search. Cut last.
    Content,
    /// A control under an inert ancestor or outside an open modal dialog.
    BehindDialog,
}

impl LeftOutKey {
    /// The tiers a cap keeps, from the one it keeps first to the one it cuts first.
    pub const KEPT_FIRST: [Self; 4] = [Self::Content, Self::Aside, Self::Chrome, Self::Hidden];

    /// Every key, in the order the facts list them.
    pub const ALL: [Self; 5] = [
        Self::Hidden,
        Self::Chrome,
        Self::Aside,
        Self::Content,
        Self::BehindDialog,
    ];

    /// The key a word from the page names, or `None` for any other word.
    #[must_use]
    pub fn classify(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.word() == word)
    }

    /// The word the page sends and the model sees for this key.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Chrome => "chrome",
            Self::Aside => "aside",
            Self::Content => "content",
            Self::BehindDialog => "behind_dialog",
        }
    }

    /// The tier an item belongs to, from what the page reported about it: its state words and the
    /// landmark that holds it. A word off the area list counts as page content.
    #[must_use]
    pub fn tier_of(state: &[String], area: Option<&str>) -> Self {
        if state.iter().any(|word| word == HIDDEN_STATE) {
            return Self::Hidden;
        }
        match area.and_then(PageArea::classify) {
            Some(PageArea::Header | PageArea::Nav | PageArea::Footer) => Self::Chrome,
            Some(PageArea::Aside) => Self::Aside,
            _ => Self::Content,
        }
    }
}

/// How many items a listing left out, by key. Only keys with a count are rendered.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeftOut {
    counts: BTreeMap<LeftOutKey, usize>,
}

impl LeftOut {
    /// Count `amount` more items under `key`.
    pub fn add(&mut self, key: LeftOutKey, amount: usize) {
        if amount > 0 {
            *self.counts.entry(key).or_default() += amount;
        }
    }

    /// Fold another listing's counts into this one.
    pub fn merge(&mut self, other: &Self) {
        for (key, amount) in &other.counts {
            self.add(*key, *amount);
        }
    }

    /// How many items were left out under `key`.
    #[must_use]
    pub fn count(&self, key: LeftOutKey) -> usize {
        self.counts.get(&key).copied().unwrap_or(0)
    }

    /// Whether nothing was left out.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// The counts as a facts object keyed by word, or `None` when nothing was left out.
    #[must_use]
    pub fn to_value(&self) -> Option<Value> {
        if self.is_empty() {
            return None;
        }
        let mut object = Map::new();
        for key in LeftOutKey::ALL {
            let amount = self.count(key);
            if amount > 0 {
                object.insert(key.word().into(), Value::from(amount));
            }
        }
        Some(Value::Object(object))
    }
}

/// The page sends an object of counts keyed by word. A word off the list and a count that is not
/// a whole number are dropped, so a changed page cannot add a key to the facts.
impl<'de> Deserialize<'de> for LeftOut {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let words = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let mut left_out = Self::default();
        for (word, amount) in words {
            if let (Some(key), Some(amount)) = (LeftOutKey::classify(&word), amount.as_u64()) {
                left_out.add(key, usize::try_from(amount).unwrap_or(usize::MAX));
            }
        }
        Ok(left_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a page sends as `left_out` reaches the model only as the closed tier words with a
    /// count (LANGUAGE.md; ADR-0198 Decisions 19 and 20): a word off the list, a count that is not
    /// a whole number and a zero are dropped, so a changed or hostile page cannot put a new key in
    /// front of the model.
    #[test]
    fn the_left_out_facts_carry_only_listed_words_with_a_count() {
        let decoded: LeftOut = serde_json::from_value(serde_json::json!({
            "chrome": 4, "secret": 9, "page": 2, "content": "many", "aside": -1, "hidden": 0,
            "behind_dialog": 3
        }))
        .expect("an object decodes");
        let mut expected = Map::new();
        expected.insert(LeftOutKey::Chrome.word().into(), Value::from(4));
        expected.insert(LeftOutKey::BehindDialog.word().into(), Value::from(3));
        assert_eq!(decoded.to_value(), Some(Value::Object(expected)));
        assert_eq!(LeftOut::default().to_value(), None);
    }
}
