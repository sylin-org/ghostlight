//! The page areas an observed item can belong to (ADR-0198 Decision 19).
//!
//! The page tells which landmark an item sits in, and this module is the one closed list of
//! those words. A word the page sends that is not on the list is dropped, so a changed or
//! hostile page cannot put a new word in front of the model. An item with no area is page
//! content: the main region or unmarked markup.

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
}
