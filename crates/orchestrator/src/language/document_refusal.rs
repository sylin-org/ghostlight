//! Why a document the work needs could not be used, behind the one code `document_unavailable`
//! (ADR-0158).
//!
//! The model code never changes: every cause here is `document_unavailable`. What changes is the
//! one fact beside it, the sentence and the next steps, so the model learns which fact stopped
//! the call and what can work instead. The vocabulary is closed and has no default: the site that
//! knows the fact names the cause when it raises
//! [`BrowserError::DocumentUnavailable`](crate::browser::BrowserError::DocumentUnavailable), and
//! the language carries it to the model unchanged. The audit keeps the code and the sentence,
//! never the word.
//!
//! When several facts hold at once the document gate answers in this order, and the first that
//! holds is the cause: a document the work needs that the policy refused (a policy refusal, not a
//! cause here), then [`DocumentCause::InventoryIncomplete`], then a tab whose every addressable
//! document the policy refused (the policy refusal again), then
//! [`DocumentCause::NoWebDocument`], then [`DocumentCause::UnboundedWork`]. [`DocumentCause::SubjectUnreachable`] is found later, when
//! the input is about to be sent. [`DocumentCause::DialogDocumentUnknown`] and
//! [`DocumentCause::RecordingSourcesIncomplete`] are raised on their own paths and meet no other
//! cause.
//!
//! No sentence quotes page text or names a host, an address or a frame. Every step is a call
//! that can work: it either never crosses the document gate, or crosses it past the rule that
//! refused this call.

/// The fact that kept a document from being used.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentCause {
    /// The tab holds no document Ghostlight can address: nothing with a web address is listed,
    /// or the browser listed no top document at all.
    NoWebDocument,
    /// The browser could not list every document of the page, and the policy leaves documents
    /// out, so one it would refuse may be hidden.
    InventoryIncomplete,
    /// The work can reach every document of the page, the policy leaves documents out, and the
    /// page holds a document nobody can put to the policy.
    UnboundedWork,
    /// A point, the keyboard focus or what lies over a target could not be placed in a document
    /// Ghostlight can work in, so no input was sent to it.
    SubjectUnreachable,
    /// A JavaScript dialog is showing and the browser did not name the document that raised it.
    DialogDocumentUnknown,
    /// A recording cannot name every document it captured, and the policy leaves documents out.
    RecordingSourcesIncomplete,
}

impl DocumentCause {
    /// Every cause, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::NoWebDocument,
        Self::InventoryIncomplete,
        Self::UnboundedWork,
        Self::SubjectUnreachable,
        Self::DialogDocumentUnknown,
        Self::RecordingSourcesIncomplete,
    ];

    /// The word that names this cause in the result's facts.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::NoWebDocument => "no_web_document",
            Self::InventoryIncomplete => "inventory_incomplete",
            Self::UnboundedWork => "unbounded_work",
            Self::SubjectUnreachable => "subject_unreachable",
            Self::DialogDocumentUnknown => "dialog_document_unknown",
            Self::RecordingSourcesIncomplete => "recording_sources_incomplete",
        }
    }

    /// What is true, and what did not happen.
    const fn sentence(self) -> &'static str {
        match self {
            Self::NoWebDocument => {
                "This tab shows no web page Ghostlight can work on: its page has no web address, is a browser error page, or has not loaded yet. Nothing was sent."
            }
            Self::InventoryIncomplete => {
                "The browser could not list every document of this page, and this session's policy must judge each document before Ghostlight works on the page. Nothing was sent."
            }
            Self::UnboundedWork => {
                "This call can reach every document of the page, and the page embeds one this session's policy cannot judge: it has no web address or the browser could not describe it. Nothing was sent."
            }
            Self::SubjectUnreachable => {
                "Ghostlight could not place the point, the keyboard focus or what covers the target in a document it can work in: it may be in an embedded document that has no web address or that the browser did not describe. No click, key or text was sent."
            }
            Self::DialogDocumentUnknown => {
                "A JavaScript dialog is open on this page and the browser did not say which document raised it, so this session's access to it could not be checked. The dialog was not read or answered."
            }
            Self::RecordingSourcesIncomplete => {
                "The browser could not list every document this recording captured, and this session's policy must check each one before a recording is saved. The recording is stopped and held; nothing was saved."
            }
        }
    }

    /// At most two steps, each a call that can work.
    const fn steps(self) -> &'static [&'static str] {
        match self {
            // Neither crosses the document gate.
            Self::NoWebDocument => &[
                "Call browser_tabs with action list to see what this tab shows; if its page is still loading, repeat the call.",
                "Otherwise call browser_navigate with url set to a web address.",
            ],
            // A repeat can work because a loading page lists its documents once it settles. A
            // new tab is what works when this tab keeps an unreadable document.
            Self::InventoryIncomplete => &[
                "Repeat the call; a page that is still loading lists its documents once it settles.",
                "If this tab keeps refusing, call browser_navigate with new_tab true and the page's url to open it in a new tab.",
            ],
            // Bounded work passes this rule: it reaches only the documents the policy admitted,
            // and the rules before this one already passed for this page.
            Self::UnboundedWork => &[
                "Read with browser_read or browser_find, or act on one target handle; those calls reach only the documents this session's policy admits.",
            ],
            // A screenshot never crosses the document gate. A call with a target binds its
            // subject from the handle, not from a point or the focus.
            Self::SubjectUnreachable => &[
                "browser_screenshot shows what is there now, embedded documents included.",
                "To act inside a document Ghostlight can reach, use a target handle from browser_find or browser_inspect; a call with a target sets the focus itself.",
            ],
            // browser_dialog crosses this same rule, so only the person or closing the tab
            // clears it.
            Self::DialogDocumentUnknown => &[
                "Ask the user to answer the dialog in the browser, then repeat the call.",
                "Or call browser_tabs with action close to close this tab.",
            ],
            // A save crosses this same rule again; a discard does not.
            Self::RecordingSourcesIncomplete => &[
                "Call browser_record with action discard to drop this recording, then record again.",
            ],
        }
    }
}

/// The summary of a document refusal.
#[must_use]
pub const fn summary(cause: DocumentCause) -> &'static str {
    cause.sentence()
}

/// The steps a document refusal offers.
#[must_use]
pub fn next_steps(cause: DocumentCause) -> Vec<String> {
    cause.steps().iter().map(|step| (*step).into()).collect()
}
