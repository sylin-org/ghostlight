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

    /// Whether the cap cuts this key, as opposed to a control the page cannot act on.
    #[must_use]
    pub const fn is_size_cut(self) -> bool {
        !matches!(self, Self::BehindDialog)
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

    /// How many items the cap cut, which excludes the controls dropped for being unusable.
    #[must_use]
    pub fn size_cut(&self) -> usize {
        self.counts
            .iter()
            .filter(|(key, _)| key.is_size_cut())
            .map(|(_, amount)| amount)
            .sum()
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

    #[test]
    fn every_area_round_trips_through_its_word() {
        for area in PageArea::ALL {
            assert_eq!(PageArea::classify(area.word()), Some(area));
        }
    }

    #[test]
    fn a_word_off_the_list_is_not_an_area() {
        for word in ["", "main", "Header", "nav ", "banner", "form"] {
            assert_eq!(PageArea::classify(word), None, "{word:?}");
        }
    }

    #[test]
    fn every_left_out_key_round_trips_through_its_word() {
        for key in LeftOutKey::ALL {
            assert_eq!(LeftOutKey::classify(key.word()), Some(key));
        }
        assert_eq!(LeftOutKey::classify("page"), None);
    }

    #[test]
    fn tiers_follow_visibility_then_landmark() {
        let shown: Vec<String> = vec!["required".into()];
        let hidden: Vec<String> = vec!["disabled".into(), HIDDEN_STATE.into()];
        let tier = LeftOutKey::tier_of;
        assert_eq!(tier(&hidden, Some("nav")), LeftOutKey::Hidden);
        assert_eq!(tier(&hidden, None), LeftOutKey::Hidden);
        for chrome in ["header", "nav", "footer"] {
            assert_eq!(tier(&shown, Some(chrome)), LeftOutKey::Chrome);
        }
        assert_eq!(tier(&shown, Some("aside")), LeftOutKey::Aside);
        for content in [Some("dialog"), Some("search"), Some("banner"), None] {
            assert_eq!(tier(&shown, content), LeftOutKey::Content, "{content:?}");
        }
    }

    #[test]
    fn cut_order_runs_from_hidden_to_content() {
        assert!(LeftOutKey::Hidden < LeftOutKey::Chrome);
        assert!(LeftOutKey::Chrome < LeftOutKey::Aside);
        assert!(LeftOutKey::Aside < LeftOutKey::Content);
    }

    #[test]
    fn left_out_renders_only_the_keys_with_a_count() {
        let mut left_out = LeftOut::default();
        assert_eq!(left_out.to_value(), None);
        left_out.add(LeftOutKey::Content, 0);
        assert!(left_out.is_empty());
        left_out.add(LeftOutKey::Chrome, 96);
        left_out.add(LeftOutKey::BehindDialog, 3);
        assert_eq!(
            left_out.to_value(),
            Some(serde_json::json!({"chrome": 96, "behind_dialog": 3}))
        );
        assert_eq!(left_out.size_cut(), 96);
    }

    #[test]
    fn left_out_counts_add_across_documents() {
        let mut total = LeftOut::default();
        let mut one = LeftOut::default();
        one.add(LeftOutKey::Chrome, 2);
        let mut two = LeftOut::default();
        two.add(LeftOutKey::Chrome, 5);
        two.add(LeftOutKey::Hidden, 1);
        total.merge(&one);
        total.merge(&two);
        assert_eq!(total.count(LeftOutKey::Chrome), 7);
        assert_eq!(total.count(LeftOutKey::Hidden), 1);
        assert_eq!(total.count(LeftOutKey::Aside), 0);
    }

    #[test]
    fn the_page_cannot_add_a_left_out_key() {
        let decoded: LeftOut = serde_json::from_value(serde_json::json!({
            "chrome": 4, "secret": 9, "content": "many", "aside": -1, "hidden": 0
        }))
        .expect("an object decodes");
        let mut expected = LeftOut::default();
        expected.add(LeftOutKey::Chrome, 4);
        assert_eq!(decoded, expected);
    }
}
