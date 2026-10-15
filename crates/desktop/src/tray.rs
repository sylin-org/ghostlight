//! The tray, built from a model (ADR-0199 Decision 7).
//!
//! The service sends a [`TrayModel`]: a tooltip and an ordered list of menu items. The shell
//! renders it and reports clicks as [`TrayEvent`]s. The shell owns no item of its own: which
//! entries exist, what they say and whether they are enabled are all the service's decisions.
//!
//! Thread rule: the tray is created and updated on the event-loop thread, after the loop has
//! started (on Linux `tao` has then initialized GTK). [`install_events`] may run on any thread
//! but only once per process.
//!
//! Linux reports no tray clicks (the indicator shows its menu itself), so on Linux the way to
//! open the window must be a menu item of the model, never the icon click alone.
//!
//! [`TrayModel::validate`], [`icon_image`] and [`decode_rgba`] are pure and unit tested. [`Tray`]
//! and [`install_events`] are thin glue over `tray-icon` and are checked live.

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// The most items a tray model may hold.
pub const MAX_ITEMS: usize = 16;

/// The longest item id, in characters.
pub const MAX_ID_LEN: usize = 32;

/// The longest item label, in characters.
pub const MAX_LABEL_LEN: usize = 64;

/// The longest tooltip, in characters.
pub const MAX_TOOLTIP_LEN: usize = 64;

/// The identity of the one tray icon.
const TRAY_ID: &str = "ghostlight";

/// The product icon, the same file the extension uses.
const ICON_PNG: &[u8] = include_bytes!("../../../extension/icons/icon32.png");

/// Why a tray model or the tray itself was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TrayError {
    /// The model breaks a rule of [`TrayModel::validate`].
    #[error("tray model is invalid: {reason}")]
    Model {
        /// Which rule failed.
        reason: String,
    },
    /// The icon bytes are not an 8-bit RGBA-expandable PNG.
    #[error("tray icon is not a usable PNG")]
    Icon,
    /// The operating system refused the tray operation.
    #[error("tray failed: {detail}")]
    Native {
        /// What the toolkit reported.
        detail: String,
    },
}

/// One menu entry of the tray.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayItem {
    /// The stable id reported back on a click: 1 to 32 characters of `a-z`, `0-9` and `_`.
    pub id: String,
    /// The text shown, 1 to 64 characters without control characters.
    pub label: String,
    /// Whether the entry can be chosen.
    pub enabled: bool,
}

/// The whole tray as the service describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayModel {
    /// The hover text, at most 64 characters.
    pub tooltip: String,
    /// The menu entries in order, 1 to 16 of them.
    pub items: Vec<TrayItem>,
}

fn model_error(reason: impl Into<String>) -> TrayError {
    TrayError::Model {
        reason: reason.into(),
    }
}

fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl TrayModel {
    /// Checks the model against the limits above. The shell renders nothing that fails.
    pub fn validate(&self) -> Result<(), TrayError> {
        if self.tooltip.chars().count() > MAX_TOOLTIP_LEN {
            return Err(model_error(format!(
                "tooltip is longer than {MAX_TOOLTIP_LEN} characters"
            )));
        }
        if self.items.is_empty() || self.items.len() > MAX_ITEMS {
            return Err(model_error(format!(
                "a tray holds 1 to {MAX_ITEMS} items, not {}",
                self.items.len()
            )));
        }
        let mut seen = HashSet::new();
        for item in &self.items {
            if !valid_id(&item.id) {
                return Err(model_error(format!("item id {:?} is not allowed", item.id)));
            }
            if !seen.insert(item.id.as_str()) {
                return Err(model_error(format!("item id {:?} is repeated", item.id)));
            }
            let length = item.label.chars().count();
            if length == 0 || length > MAX_LABEL_LEN {
                return Err(model_error(format!(
                    "label of item {:?} must be 1 to {MAX_LABEL_LEN} characters",
                    item.id
                )));
            }
            if item.label.chars().any(char::is_control) {
                return Err(model_error(format!(
                    "label of item {:?} holds a control character",
                    item.id
                )));
            }
        }
        Ok(())
    }
}

/// Decodes a PNG into 8-bit RGBA pixels, returning the pixels, the width and the height.
pub fn decode_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), TrayError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|_| TrayError::Icon)?;
    let size = reader.output_buffer_size().ok_or(TrayError::Icon)?;
    let mut buffer = vec![0u8; size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|_| TrayError::Icon)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(TrayError::Icon);
    }
    buffer.truncate(info.buffer_size());
    Ok((buffer, info.width, info.height))
}

/// The product icon as 8-bit RGBA pixels, width and height.
pub fn icon_image() -> Result<(Vec<u8>, u32, u32), TrayError> {
    decode_rgba(ICON_PNG)
}

fn native(error: impl std::fmt::Display) -> TrayError {
    TrayError::Native {
        detail: error.to_string(),
    }
}

fn menu_for(model: &TrayModel) -> Result<Menu, TrayError> {
    let menu = Menu::new();
    for item in &model.items {
        menu.append(&MenuItem::with_id(
            item.id.clone(),
            &item.label,
            item.enabled,
            None,
        ))
        .map_err(native)?;
    }
    Ok(menu)
}

/// The tray icon in the notification area.
pub struct Tray {
    icon: TrayIcon,
}

impl Tray {
    /// Validates the model and creates the tray icon with its menu. Call on the event-loop
    /// thread after the loop has started.
    pub fn build(model: &TrayModel) -> Result<Tray, TrayError> {
        model.validate()?;
        let (rgba, width, height) = icon_image()?;
        let icon = Icon::from_rgba(rgba, width, height).map_err(|_| TrayError::Icon)?;
        let icon = TrayIconBuilder::new()
            .with_id(TRAY_ID)
            .with_tooltip(&model.tooltip)
            .with_icon(icon)
            .with_menu(Box::new(menu_for(model)?))
            .with_menu_on_left_click(false)
            .build()
            .map_err(native)?;
        Ok(Tray { icon })
    }

    /// Validates the model and replaces the menu and tooltip with it.
    pub fn show(&self, model: &TrayModel) -> Result<(), TrayError> {
        model.validate()?;
        self.icon.set_menu(Some(Box::new(menu_for(model)?)));
        self.icon.set_tooltip(Some(&model.tooltip)).map_err(native)
    }
}

/// What the user did to the tray.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// The icon was clicked with the left button. Never reported on Linux.
    Activate,
    /// The menu item with this id was chosen.
    Item(String),
}

/// Routes tray clicks and menu choices to `sink`. Call once per process; a later call is
/// ignored by the toolkit.
pub fn install_events(sink: impl Fn(TrayEvent) + Send + Sync + 'static) {
    let sink = Arc::new(sink);
    let clicks = Arc::clone(&sink);
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            clicks(TrayEvent::Activate);
        }
    }));
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        sink(TrayEvent::Item(event.id.0));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, label: &str) -> TrayItem {
        TrayItem {
            id: id.to_string(),
            label: label.to_string(),
            enabled: true,
        }
    }

    fn model() -> TrayModel {
        TrayModel {
            tooltip: "Ghostlight".to_string(),
            items: vec![
                item("open", "Open Ghostlight"),
                item("hold", "Pause browser work"),
                item("resume", "Resume browser work"),
                item("quit", "Quit Ghostlight"),
            ],
        }
    }

    fn refused(model: &TrayModel) -> bool {
        matches!(model.validate(), Err(TrayError::Model { .. }))
    }

    #[test]
    fn a_model_with_the_four_familiar_items_passes() {
        assert_eq!(model().validate(), Ok(()));
    }

    #[test]
    fn the_item_count_is_bounded() {
        let mut m = model();
        m.items.clear();
        assert!(refused(&m));
        m.items = (0..=MAX_ITEMS)
            .map(|n| item(&format!("i{n}"), "x"))
            .collect();
        assert_eq!(m.items.len(), 17);
        assert!(refused(&m));
        m.items.pop();
        assert_eq!(m.validate(), Ok(()));
    }

    #[test]
    fn ids_must_be_unique_and_plain() {
        let mut m = model();
        m.items.push(item("open", "Again"));
        assert!(refused(&m));
        for bad in ["Open", "a b", "", "a-b", &"a".repeat(33)] {
            let mut m = model();
            m.items[0].id = bad.to_string();
            assert!(refused(&m), "{bad:?}");
        }
    }

    #[test]
    fn labels_are_bounded_and_printable() {
        for bad in ["".to_string(), "x".repeat(65), "a\tb".to_string()] {
            let mut m = model();
            m.items[0].label = bad.clone();
            assert!(refused(&m), "{bad:?}");
        }
        let mut m = model();
        m.items[0].label = "x".repeat(64);
        assert_eq!(m.validate(), Ok(()));
    }

    #[test]
    fn the_tooltip_is_bounded() {
        let mut m = model();
        m.tooltip = "t".repeat(65);
        assert!(refused(&m));
        m.tooltip = "t".repeat(64);
        assert_eq!(m.validate(), Ok(()));
    }

    #[test]
    fn the_model_keeps_its_wire_field_names() {
        let value = serde_json::to_value(model()).unwrap();
        assert_eq!(value["tooltip"], "Ghostlight");
        let first = &value["items"][0];
        assert_eq!(first["id"], "open");
        assert_eq!(first["label"], "Open Ghostlight");
        assert_eq!(first["enabled"], true);
        let back: TrayModel = serde_json::from_value(value).unwrap();
        assert_eq!(back, model());
    }

    #[test]
    fn the_product_icon_decodes_to_32_by_32_rgba() {
        let (rgba, width, height) = icon_image().unwrap();
        assert_eq!((width, height), (32, 32));
        assert_eq!(rgba.len(), 4096);
        assert!(rgba.chunks(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn bytes_that_are_not_a_png_are_refused() {
        assert_eq!(decode_rgba(b"not a png"), Err(TrayError::Icon));
    }
}
