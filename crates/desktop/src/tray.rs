//! The tray, built from a model (ADR-0199 Decision 7 and its amendment of 2026-10-15).
//!
//! The service sends a [`TrayModel`]: a state, a tooltip and an ordered list of menu items, and
//! sends a new one whenever it changes. The shell renders it and reports clicks as
//! [`TrayEvent`]s. The shell owns no item of its own: which entries exist, what they say and
//! whether they are enabled are all the service's decisions.
//!
//! The icon is the product icon, drawn at run time. A ready service shows the plain ghost; each
//! other state adds a small filled dot in the lower right in the colour the window's band uses for
//! that state ([`state_icon`]). No other artwork is shipped.
//!
//! Thread rule: the tray is created and updated on the event-loop thread, after the loop has
//! started (on Linux `tao` has then initialized GTK). [`install_events`] may run on any thread
//! but only once per process.
//!
//! Linux reports no tray clicks (the indicator shows its menu itself), so on Linux the way to
//! open the window must be a menu item of the model, never the icon click alone.
//!
//! [`TrayModel::validate`], [`icon_image`], [`state_icon`] and [`decode_rgba`] are pure and unit
//! tested. [`Tray`] and [`install_events`] are thin glue over `tray-icon` and are checked live.

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::link::TrayItemId;

/// The most items a tray model may hold.
pub const MAX_ITEMS: usize = 16;

/// The longest item id, in characters.
pub const MAX_ID_LEN: usize = 32;

/// The longest item label, in characters.
pub const MAX_LABEL_LEN: usize = 64;

/// The longest tooltip, in characters: the Windows limit.
pub const MAX_TOOLTIP_LEN: usize = 127;

/// The colour of the dot for a working service, the band's accent `--a`.
pub const WORKING_COLOUR: [u8; 3] = [0x5e, 0xea, 0xd4];

/// The colour of the dot for a paused service, the band's `--held`.
pub const PAUSED_COLOUR: [u8; 3] = [0xfb, 0xbf, 0x24];

/// The colour of the dot for a service that needs attention, the band's `--danger`.
pub const ATTENTION_COLOUR: [u8; 3] = [0xf8, 0x71, 0x71];

/// The colour of the thin ring that keeps the dot apart from the ghost, the window's night ground
/// `--bg`.
const RING_COLOUR: [u8; 3] = [0x0f, 0x0e, 0x12];

/// The centre of the dot in pixels of the 32 pixel icon, counted from the top left.
const DOT_CENTRE: (f32, f32) = (24.5, 24.5);

/// The radius of the dot, in pixels of the 32 pixel icon.
const DOT_RADIUS: f32 = 5.5;

/// How much wider than the dot the ring is, in pixels.
const RING_WIDTH: f32 = 1.5;

/// The icon size the dot is drawn for. A larger icon scales the dot with it.
const REFERENCE_SIZE: f32 = 32.0;

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

/// The state the icon shows, the band's four states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayState {
    /// Ready: the plain ghost.
    Ready,
    /// An agent is working: the ghost with a teal dot.
    Working,
    /// Browser work is paused or ended: the ghost with an amber dot.
    Paused,
    /// Something needs attention: the ghost with a red dot.
    Attention,
}

impl TrayState {
    /// The colour of the dot this state adds to the ghost, or `None` for the plain ghost.
    pub const fn dot(self) -> Option<[u8; 3]> {
        match self {
            TrayState::Ready => None,
            TrayState::Working => Some(WORKING_COLOUR),
            TrayState::Paused => Some(PAUSED_COLOUR),
            TrayState::Attention => Some(ATTENTION_COLOUR),
        }
    }
}

/// The whole tray as the service describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayModel {
    /// The hover text, at most 127 characters.
    pub tooltip: String,
    /// The menu entries in order, 1 to 16 of them.
    pub items: Vec<TrayItem>,
    /// The state the icon shows.
    pub state: TrayState,
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

impl TrayModel {
    /// The tray while the shell has no workbench to act for: the tooltip says `words`, the icon
    /// is the plain ghost, and every entry but Open is disabled, since the service cannot hear a
    /// click.
    pub fn while_without_service(&self, words: &str) -> TrayModel {
        TrayModel {
            tooltip: words.to_owned(),
            items: self
                .items
                .iter()
                .map(|item| TrayItem {
                    enabled: item.enabled && item.id == TrayItemId::Open.as_str(),
                    ..item.clone()
                })
                .collect(),
            state: TrayState::Ready,
        }
    }

    /// The tray while the window cannot be built: the tooltip says `words` and the rest is as the
    /// service described it.
    pub fn while_window_fails(&self, words: &str) -> TrayModel {
        TrayModel {
            tooltip: words.to_owned(),
            ..self.clone()
        }
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

/// The icon for `state`: the product icon, with a filled dot in the lower right for every state
/// but ready. The ready icon is the product icon's own pixels.
pub fn state_icon(state: TrayState) -> Result<(Vec<u8>, u32, u32), TrayError> {
    let (mut rgba, width, height) = icon_image()?;
    if let Some(colour) = state.dot() {
        draw_dot(&mut rgba, width, height, colour);
    }
    Ok((rgba, width, height))
}

/// Lay a ring and a dot over the lower right of `rgba`, with a soft edge one pixel wide.
fn draw_dot(rgba: &mut [u8], width: u32, height: u32, colour: [u8; 3]) {
    let scale = width as f32 / REFERENCE_SIZE;
    let (centre_x, centre_y) = (DOT_CENTRE.0 * scale, DOT_CENTRE.1 * scale);
    let dot = DOT_RADIUS * scale;
    let ring = (DOT_RADIUS + RING_WIDTH) * scale;
    for y in 0..height {
        for x in 0..width {
            let distance =
                ((x as f32 + 0.5 - centre_x).powi(2) + (y as f32 + 0.5 - centre_y).powi(2)).sqrt();
            let index = ((y * width + x) * 4) as usize;
            let pixel = &mut rgba[index..index + 4];
            lay(pixel, RING_COLOUR, coverage(ring, distance));
            lay(pixel, colour, coverage(dot, distance));
        }
    }
}

/// How much of a pixel at `distance` from the centre lies inside a circle of `radius`.
fn coverage(radius: f32, distance: f32) -> f32 {
    (radius + 0.5 - distance).clamp(0.0, 1.0)
}

/// Lay `colour` over `pixel` with `coverage` of opacity, keeping the pixel's alpha channel
/// straight.
fn lay(pixel: &mut [u8], colour: [u8; 3], coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let below = f32::from(pixel[3]) / 255.0;
    let alpha = coverage + below * (1.0 - coverage);
    for channel in 0..3 {
        let mixed = f32::from(colour[channel]) * coverage
            + f32::from(pixel[channel]) * below * (1.0 - coverage);
        pixel[channel] = (mixed / alpha).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
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

fn icon_for(state: TrayState) -> Result<Icon, TrayError> {
    let (rgba, width, height) = state_icon(state)?;
    Icon::from_rgba(rgba, width, height).map_err(|_| TrayError::Icon)
}

/// The tray icon in the notification area.
pub struct Tray {
    icon: TrayIcon,
    /// What the icon shows now, so an unchanged model touches nothing.
    shown: TrayModel,
}

impl Tray {
    /// Validates the model and creates the tray icon with its menu. Call on the event-loop
    /// thread after the loop has started.
    pub fn build(model: &TrayModel) -> Result<Tray, TrayError> {
        model.validate()?;
        let icon = TrayIconBuilder::new()
            .with_id(TRAY_ID)
            .with_tooltip(&model.tooltip)
            .with_icon(icon_for(model.state)?)
            .with_menu(Box::new(menu_for(model)?))
            .with_menu_on_left_click(false)
            .build()
            .map_err(native)?;
        Ok(Tray {
            icon,
            shown: model.clone(),
        })
    }

    /// Validates the model and replaces the icon, the menu and the tooltip with it. A model the
    /// tray already shows changes nothing, so a menu that is open stays open.
    pub fn show(&mut self, model: &TrayModel) -> Result<(), TrayError> {
        model.validate()?;
        if *model == self.shown {
            return Ok(());
        }
        if model.state != self.shown.state {
            self.icon
                .set_icon(Some(icon_for(model.state)?))
                .map_err(native)?;
        }
        self.icon.set_menu(Some(Box::new(menu_for(model)?)));
        self.icon
            .set_tooltip(Some(&model.tooltip))
            .map_err(native)?;
        self.shown = model.clone();
        Ok(())
    }
}

/// The well-known name of the StatusNotifier host that draws tray icons on a Linux desktop.
#[cfg(target_os = "linux")]
const STATUS_NOTIFIER_WATCHER: &str = "org.kde.StatusNotifierWatcher";

/// How long the session bus may take to say whether the watcher has an owner.
#[cfg(target_os = "linux")]
const BUS_PATIENCE: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether the desktop can show a tray icon: always on Windows, and on Linux when a
/// StatusNotifier host owns `org.kde.StatusNotifierWatcher` on the session bus. A desktop without
/// one, such as GNOME without its AppIndicator extension, draws no icon.
pub fn host_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(connection) = dbus::blocking::Connection::new_session() else {
            return false;
        };
        let bus = connection.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            BUS_PATIENCE,
        );
        let owned: Result<(bool,), dbus::Error> = bus.method_call(
            "org.freedesktop.DBus",
            "NameHasOwner",
            (STATUS_NOTIFIER_WATCHER,),
        );
        matches!(owned, Ok((true,)))
    }
    #[cfg(not(target_os = "linux"))]
    true
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
            state: TrayState::Ready,
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
    fn the_tooltip_is_bounded_by_the_windows_limit() {
        assert_eq!(MAX_TOOLTIP_LEN, 127);
        let mut m = model();
        m.tooltip = "t".repeat(128);
        assert!(refused(&m));
        m.tooltip = "t".repeat(127);
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
        assert_eq!(value["state"], "ready");
        let back: TrayModel = serde_json::from_value(value).unwrap();
        assert_eq!(back, model());
    }

    #[test]
    fn without_a_service_the_tooltip_is_the_state_and_only_open_can_be_chosen() {
        let shown = model().while_without_service("Ghostlight is restarting...");
        assert_eq!(shown.tooltip, "Ghostlight is restarting...");
        assert_eq!(shown.validate(), Ok(()));
        let enabled: Vec<&str> = shown
            .items
            .iter()
            .filter(|item| item.enabled)
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(enabled, ["open"]);
        assert_eq!(shown.state, TrayState::Ready);
        assert_eq!(shown.items.len(), model().items.len());
        assert_eq!(
            shown.items.iter().map(|i| &i.label).collect::<Vec<_>>(),
            model().items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_product_icon_decodes_to_32_by_32_rgba() {
        let (rgba, width, height) = icon_image().unwrap();
        assert_eq!((width, height), (32, 32));
        assert_eq!(rgba.len(), 4096);
        assert!(rgba.chunks(4).any(|pixel| pixel[3] != 0));
    }

    /// The pixel at (`x`, `y`) of a 32 pixel wide RGBA image.
    fn pixel(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
        let at = (y * 32 + x) * 4;
        [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
    }

    #[test]
    fn each_state_has_its_own_icon_and_ready_is_the_plain_ghost() {
        let plain = icon_image().unwrap().0;
        let icons: Vec<Vec<u8>> = [
            TrayState::Ready,
            TrayState::Working,
            TrayState::Paused,
            TrayState::Attention,
        ]
        .into_iter()
        .map(|state| state_icon(state).unwrap().0)
        .collect();
        assert_eq!(icons[0], plain, "ready is the original icon bytes");
        for (index, icon) in icons.iter().enumerate() {
            assert_eq!(icon.len(), plain.len());
            for other in &icons[index + 1..] {
                assert_ne!(icon, other, "four distinct images");
            }
        }
    }

    #[test]
    fn the_dot_sits_in_the_lower_right_in_the_colour_of_the_state() {
        let plain = icon_image().unwrap().0;
        for (state, colour) in [
            (TrayState::Working, WORKING_COLOUR),
            (TrayState::Paused, PAUSED_COLOUR),
            (TrayState::Attention, ATTENTION_COLOUR),
        ] {
            let icon = state_icon(state).unwrap().0;
            assert_eq!(
                pixel(&icon, 24, 24),
                [colour[0], colour[1], colour[2], 255],
                "{state:?}: the middle of the dot is the colour, fully opaque"
            );
            let changed: Vec<(usize, usize)> = (0..32)
                .flat_map(|y| (0..32).map(move |x| (x, y)))
                .filter(|(x, y)| pixel(&icon, *x, *y) != pixel(&plain, *x, *y))
                .collect();
            assert!(!changed.is_empty());
            assert!(
                changed.iter().all(|(x, y)| *x >= 14 && *y >= 14),
                "{state:?}: nothing changes outside the lower right"
            );
        }
    }

    #[test]
    fn the_dot_colours_are_the_tokens_of_the_windows_stylesheet() {
        let styles = include_str!("../../orchestrator/ui/styles.css");
        for (token, colour) in [
            ("--a", WORKING_COLOUR),
            ("--held", PAUSED_COLOUR),
            ("--danger", ATTENTION_COLOUR),
            ("--bg", RING_COLOUR),
        ] {
            let declared = format!(
                "{token}: #{:02x}{:02x}{:02x};",
                colour[0], colour[1], colour[2]
            );
            assert!(
                styles.contains(&declared),
                "{declared} is not in the stylesheet"
            );
        }
    }

    #[test]
    fn bytes_that_are_not_a_png_are_refused() {
        assert_eq!(decode_rgba(b"not a png"), Err(TrayError::Icon));
    }
}
