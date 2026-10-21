//! Every bound and default the model-facing language enforces and advertises, in one place: the
//! decoder checks them, the catalog advertises them, and the sentences that name a bound read it
//! from here, so the three can never disagree.

use super::choices::{
    DiagnosticDetail, DiagnosticSource, FindScope, InspectScope, OnError, PointerButton,
};

/// An inclusive range a whole-number field accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Range {
    /// The smallest accepted value.
    pub min: usize,
    /// The largest accepted value.
    pub max: usize,
}

/// The call time a tool gets when the caller names none.
pub(crate) const DEFAULT_TIMEOUT_MS: u64 = 8_000;
/// The shortest `timeout_ms` any tool accepts.
pub(crate) const MIN_TIMEOUT_MS: u64 = 100;
/// The longest `timeout_ms` any tool accepts.
pub(crate) const MAX_TIMEOUT_MS: u64 = 30_000;
/// The call time a form fill gets when the caller names none.
pub(crate) const DEFAULT_FILL_TIMEOUT_MS: u64 = MAX_TIMEOUT_MS;
/// The call time a recording call gets when the caller names none.
pub(crate) const DEFAULT_RECORD_TIMEOUT_MS: u64 = MAX_TIMEOUT_MS;

/// The longest page text or URL fragment a wait or an `expect` compares.
pub(crate) const MAX_POSTCONDITION_VALUE_CHARS: usize = 2_000;
/// The longest answer to a JavaScript prompt.
pub(crate) const DIALOG_TEXT_CHARS: usize = 2_000;
/// How long a duration wait may last, in milliseconds.
pub(crate) const DURATION_MS: Range = Range {
    min: 0,
    max: 10_000,
};

/// How many levels a document-scope inspection reaches when the call names no `max_depth`.
pub const DEFAULT_DOCUMENT_DEPTH: usize = 6;
/// How many levels a document-scope inspection may reach.
pub(crate) const DOCUMENT_DEPTH: Range = Range { min: 1, max: 12 };
/// The most items one `browser_inspect` call may ask for.
pub const MAX_INSPECT_ITEMS: usize = 200;
/// How many items an inspection may list.
pub(crate) const INSPECT_ITEMS: Range = Range {
    min: 1,
    max: MAX_INSPECT_ITEMS,
};
/// How many items an inspection lists when the call names no `max_items`.
pub(crate) const DEFAULT_INSPECT_ITEMS: usize = 80;

/// How many characters a page read may return.
pub(crate) const READ_CHARS: Range = Range {
    min: 500,
    max: 50_000,
};
/// How many characters a page read returns when the call names no `max_chars`.
pub(crate) const DEFAULT_READ_CHARS: usize = 8_000;

/// The longest text a find, a semantic selector or a diagnostics match pattern names.
pub(crate) const SEARCH_TEXT_CHARS: usize = 500;
/// How many matches a find may return.
pub(crate) const FIND_RESULTS: Range = Range { min: 1, max: 50 };
/// How many matches a find returns when the call names no `max_results`.
pub(crate) const DEFAULT_FIND_RESULTS: usize = 20;

/// The most characters a `browser_execute` script may have, as the model sends it. The adapter
/// sends nothing longer, so it also bounds a script it wraps.
pub const SCRIPT_MAX_CHARS: usize = 20_000;
/// How many characters of a script's result may come back.
pub(crate) const SCRIPT_RESULT_CHARS: Range = Range {
    min: 100,
    max: 20_000,
};
/// How many characters of a script's result come back when the call names no `max_result_chars`.
pub(crate) const DEFAULT_SCRIPT_RESULT_CHARS: usize = 8_000;

/// The page zoom a call may set, in percent.
pub(crate) const ZOOM_PERCENT: Range = Range { min: 25, max: 500 };
/// The outer window width a resize may set, in pixels.
pub(crate) const WINDOW_WIDTH: Range = Range {
    min: 320,
    max: 7_680,
};
/// The outer window height a resize may set, in pixels.
pub(crate) const WINDOW_HEIGHT: Range = Range {
    min: 240,
    max: 4_320,
};

/// How many clicks one click call may make.
pub(crate) const CLICK_COUNT: Range = Range { min: 1, max: 3 };
/// How many clicks a click makes when the call names no `click_count`.
pub(crate) const DEFAULT_CLICK_COUNT: u8 = 1;
/// How far a wheel scroll at a view point may go, in wheel ticks.
pub(crate) const WHEEL_TICKS: Range = Range { min: 1, max: 10 };
/// The largest coordinate a point or region in a view may name.
pub(crate) const COORDINATE_MAX: u32 = 1_000_000;

/// How many fields one form fill may set.
pub(crate) const FILL_FIELDS: Range = Range { min: 1, max: 30 };
/// The longest text a form field or a typing call may take.
pub(crate) const FIELD_VALUE_CHARS: usize = 8_000;
/// How many options one multi-select value may choose.
pub(crate) const FIELD_CHOICES: usize = 100;

/// How many keystrokes one key sequence may hold.
pub(crate) const KEY_STROKES: Range = Range { min: 1, max: 20 };
/// How many times one key call may repeat its sequence.
pub(crate) const KEY_REPEAT: Range = Range { min: 1, max: 100 };
/// How many times a key call plays its sequence when the call names no `repeat`.
pub(crate) const DEFAULT_KEY_REPEAT: u16 = 1;

/// How many local paths one upload may name.
pub(crate) const UPLOAD_PATHS: Range = Range { min: 1, max: 5 };
/// How many inline files one upload may carry.
pub(crate) const INLINE_FILE_LIMIT: usize = 5;
/// The longest local path an upload may name.
pub(crate) const UPLOAD_PATH_CHARS: usize = 4_096;
/// The longest file name an inline file may have.
pub(crate) const FILE_NAME_CHARS: usize = 255;
/// The longest media type an inline file may name.
pub(crate) const MEDIA_TYPE_CHARS: usize = 100;

/// The longest URL a navigation takes. The decoder counts bytes and the schema characters; an
/// ASCII URL, which every encoded URL is, has as many of one as the other.
pub(crate) const URL_MAX: usize = 4_096;
/// The longest handle any call may name.
pub(crate) const HANDLE_CHARS: usize = 160;
/// The longest id a flow step may give itself.
pub(crate) const FLOW_STEP_ID_CHARS: usize = 64;

/// How many diagnostics entries one read may return.
pub(crate) const DIAGNOSTIC_LIMIT: Range = Range { min: 1, max: 200 };
/// How many diagnostics entries a read returns when the call names no `limit`.
pub(crate) const DEFAULT_DIAGNOSTIC_LIMIT: usize = 50;

/// The scope an inspection reads when the call names none.
pub(crate) const DEFAULT_INSPECT_SCOPE: InspectScope = InspectScope::Controls;
/// The scope a find searches when the call names none.
pub(crate) const DEFAULT_FIND_SCOPE: FindScope = FindScope::Any;
/// The button a click presses when the call names none.
pub(crate) const DEFAULT_BUTTON: PointerButton = PointerButton::Primary;
/// The diagnostics a read returns when the call names no `source`.
pub(crate) const DEFAULT_DIAGNOSTIC_SOURCE: DiagnosticSource = DiagnosticSource::Both;
/// The detail a diagnostics read returns when the call names none.
pub(crate) const DEFAULT_DIAGNOSTIC_DETAIL: DiagnosticDetail = DiagnosticDetail::Problems;
/// What a flow does at a failed step when the call names no `on_error`.
pub(crate) const DEFAULT_ON_ERROR: OnError = OnError::Stop;
