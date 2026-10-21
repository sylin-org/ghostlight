//! The tile the window serves for an installed app's own icon: the icon on a ground the service
//! chose for it, so every agent image the page draws is whole, the way a monogram is, and the page
//! decides nothing about grounds.
//!
//! The icon is decoded (a PNG, or the largest image of a Windows icon file: an embedded PNG or a
//! 32-bit bitmap) and set in one of three ways, the closed [`Ground`]:
//!
//! - An icon that is its own plate is served exactly as cached. It is judged at the edges of its
//!   drawn area, the box around its pixels at least [`DRAWN_ALPHA`] opaque, because most app
//!   icons set their plate inside a transparent margin: the area must span at least
//!   [`PLATE_SPAN`] of the icon's side both ways, and the middle half of each of its edges, in a
//!   band [`RING_SHARE`] of its side deep, must be at least [`PLATE_OPACITY`] opaque. Rounded
//!   corners do not count against a plate, a faint drop shadow does not move its edges, and a
//!   small solid mark is a glyph, not a plate.
//! - Otherwise each pixel's color is compared with the dark and the light ground by its WCAG
//!   contrast, and the ground under which more of the icon's ink, weighted by opacity, reaches
//!   [`READABLE_CONTRAST`] is chosen. A tie goes to the light ground.
//!
//! The tile is a square of the ground [`PADDING_SHARE`] of the icon's side wider on each edge, with
//! the icon's pixels copied at their own size into its middle and blended over the ground. It is
//! encoded as an opaque PNG with fixed settings, so one icon always gives the same bytes; the
//! browser scales it as it scales a monogram. An icon that cannot be decoded, or is larger than
//! [`MAX_SIDE`] on a side, is served as cached.

use std::io::Cursor;

use crate::install::icons::IconKind;

/// The dark ground: a dark gray, which reads as a tile on the window's near-black.
const DARK_GROUND: [u8; 3] = [0x3a, 0x3a, 0x41];

/// The light ground: a slightly warm off-white, softer than paper white on the dark window.
const LIGHT_GROUND: [u8; 3] = [0xe7, 0xe5, 0xe4];

/// The largest side, in pixels, of an icon the service sets on a tile.
const MAX_SIDE: u32 = 512;

/// The depth of the band along each edge of an icon's drawn area that says whether it is its own
/// plate, as a share of the area's longer side (at least one pixel).
const RING_SHARE: f64 = 0.04;

/// How opaque the middle of the drawn area's edges must be, on average, for the icon to be its
/// own plate.
const PLATE_OPACITY: f64 = 0.9;

/// The opacity at which a pixel counts as drawn when the service finds an icon's edges: at least
/// half, so a faint drop shadow is not taken for the plate's edge.
const DRAWN_ALPHA: u8 = 0x80;

/// How much of the icon's side, both ways, a plate's drawn area spans at least. App icons set
/// their plate on about four fifths of the canvas.
const PLATE_SPAN: f64 = 0.75;

/// The share of each edge of the drawn area, around its middle, that is measured: the middle
/// half, clear of rounded corners.
const EDGE_MIDDLE_SHARE: f64 = 0.5;

/// The WCAG contrast at which a pixel of the icon reads on a ground: the level for graphics.
const READABLE_CONTRAST: f64 = 3.0;

/// The room around the icon on each edge of its tile, as a share of its longer side (at least one
/// pixel).
const PADDING_SHARE: f64 = 0.10;

/// The most memory the PNG decoder may take for one icon.
const DECODE_LIMIT_BYTES: usize = 8 * 1024 * 1024;

/// The first bytes of every PNG file.
const PNG_SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";

/// The size of an icon file's header.
const ICO_HEADER_SIZE: usize = 6;

/// The size of each entry of an icon file's directory.
const ICO_ENTRY_SIZE: usize = 16;

/// The resource type an icon file's header names.
const ICO_TYPE_ICON: u16 = 1;

/// The smallest bitmap header an icon image may carry (`BITMAPINFOHEADER`).
const BITMAP_HEADER_SIZE: usize = 40;

/// The bitmap compression value of plain, uncompressed pixels (`BI_RGB`).
const BITMAP_UNCOMPRESSED: u32 = 0;

/// The one bitmap depth the service reads: 32-bit blue, green, red and alpha.
const BITMAP_DEPTH: u16 = 32;

/// How the service sets an app's icon: on one of its two grounds, or on its own plate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ground {
    /// The dark gray, for an icon that is mostly light or white.
    Dark,
    /// The warm off-white, for an icon that is mostly dark or colored.
    Light,
    /// The icon's own opaque plate: it is served as cached, on no tile.
    Plate,
}

impl Ground {
    /// The ground's color as sRGB bytes; none for an icon's own plate.
    const fn color(self) -> Option<[u8; 3]> {
        match self {
            Self::Dark => Some(DARK_GROUND),
            Self::Light => Some(LIGHT_GROUND),
            Self::Plate => None,
        }
    }
}

/// The tile for the cached icon `bytes` of `kind`: a PNG of the icon on the ground chosen for it.
/// None when the icon is served as cached: it is its own plate, it cannot be decoded, or it is
/// larger than [`MAX_SIDE`].
#[must_use]
pub fn tile(kind: IconKind, bytes: &[u8]) -> Option<Vec<u8>> {
    let pixels = decode(kind, bytes)?;
    composite(&pixels, ground_for(&pixels).color()?)
}

/// An icon's pixels: 8-bit red, green, blue and alpha, row by row from the top.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Pixels {
    /// The pixels `rgba` of an image `width` by `height`, when they fill it and it is within
    /// [`MAX_SIDE`].
    fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let expected = usize::try_from(u64::from(width) * u64::from(height) * 4).ok()?;
        (fits(width, height) && rgba.len() == expected).then_some(Self {
            width,
            height,
            rgba,
        })
    }

    fn side(&self) -> u32 {
        self.width.max(self.height)
    }

    /// The alpha of the pixel at column `x` and row `y`.
    fn alpha(&self, x: u32, y: u32) -> u8 {
        self.rgba[index(x, y, self.width) * 4 + 3]
    }
}

/// Whether an image `width` by `height` is one the service sets on a tile.
fn fits(width: u32, height: u32) -> bool {
    (1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height)
}

fn index(x: u32, y: u32, width: u32) -> usize {
    // Every coordinate is at most a tile's side, a few hundred pixels, so this fits any usize.
    (y as usize) * (width as usize) + (x as usize)
}

/// `side` times `share`, rounded, and at least one pixel.
fn share_of(side: u32, share: f64) -> u32 {
    // The side is within MAX_SIDE, so the rounded product is a small positive number.
    ((f64::from(side) * share).round() as u32).max(1)
}

fn decode(kind: IconKind, bytes: &[u8]) -> Option<Pixels> {
    match kind {
        IconKind::Png => decode_png(bytes),
        IconKind::Ico => decode_ico(bytes),
    }
}

fn decode_png(bytes: &[u8]) -> Option<Pixels> {
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: DECODE_LIMIT_BYTES,
        },
    );
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if !fits(info.width, info.height) {
        return None;
    }
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    if frame.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let samples = buffer.get(..frame.buffer_size())?;
    let rgba = match frame.color_type {
        png::ColorType::Rgba => samples.to_vec(),
        png::ColorType::Rgb => samples
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], u8::MAX])
            .collect(),
        png::ColorType::GrayscaleAlpha => samples
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Grayscale => samples
            .iter()
            .flat_map(|&gray| [gray, gray, gray, u8::MAX])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Pixels::new(frame.width, frame.height, rgba)
}

/// The largest image of a Windows icon file, by area and then depth.
fn decode_ico(bytes: &[u8]) -> Option<Pixels> {
    if u16_at(bytes, 0)? != 0 || u16_at(bytes, 2)? != ICO_TYPE_ICON {
        return None;
    }
    let count = usize::from(u16_at(bytes, 4)?);
    let largest = (0..count)
        .filter_map(|entry| IcoEntry::read(bytes, ICO_HEADER_SIZE + entry * ICO_ENTRY_SIZE))
        .max_by_key(|entry| (entry.area, entry.depth))?;
    let image = bytes.get(largest.offset..largest.offset.checked_add(largest.size)?)?;
    if image.starts_with(&PNG_SIGNATURE) {
        decode_png(image)
    } else {
        decode_bitmap(image)
    }
}

/// One entry of an icon file's directory: how large its image is and where its bytes are.
struct IcoEntry {
    area: u32,
    depth: u16,
    size: usize,
    offset: usize,
}

impl IcoEntry {
    fn read(bytes: &[u8], at: usize) -> Option<Self> {
        let entry = bytes.get(at..at.checked_add(ICO_ENTRY_SIZE)?)?;
        // A width or height of 0 in the directory means 256.
        let side = |byte: u8| if byte == 0 { 256 } else { u32::from(byte) };
        Some(Self {
            area: side(entry[0]) * side(entry[1]),
            depth: u16_at(entry, 6)?,
            size: usize::try_from(u32_at(entry, 8)?).ok()?,
            offset: usize::try_from(u32_at(entry, 12)?).ok()?,
        })
    }
}

/// An icon image stored as a 32-bit bitmap: its header, its pixels bottom row first in blue,
/// green, red and alpha, then its one-bit mask. The bitmap's height counts the pixels and the mask
/// together. A bitmap whose alpha is zero everywhere takes its transparency from the mask.
fn decode_bitmap(image: &[u8]) -> Option<Pixels> {
    let header_size = usize::try_from(u32_at(image, 0)?).ok()?;
    let width = u32::try_from(i32_at(image, 4)?).ok()?;
    let height = u32::try_from(i32_at(image, 8)?).ok()? / 2;
    if header_size < BITMAP_HEADER_SIZE
        || u16_at(image, 14)? != BITMAP_DEPTH
        || u32_at(image, 16)? != BITMAP_UNCOMPRESSED
        || !fits(width, height)
    {
        return None;
    }
    let row = width as usize * 4;
    let rows = height as usize;
    let start = header_size.checked_add(row * rows)?;
    let colors = image.get(header_size..start)?;
    let mut rgba = vec![0; row * rows];
    for (target, source) in rgba
        .chunks_exact_mut(row)
        .zip(colors.chunks_exact(row).rev())
    {
        for (pixel, bgra) in target.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
            pixel.copy_from_slice(&[bgra[2], bgra[1], bgra[0], bgra[3]]);
        }
    }
    if rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        let mask_row = (width as usize).div_ceil(32) * 4;
        let mask = image.get(start..start.checked_add(mask_row * rows)?)?;
        for (target, bits) in rgba
            .chunks_exact_mut(row)
            .zip(mask.chunks_exact(mask_row).rev())
        {
            for (x, pixel) in target.chunks_exact_mut(4).enumerate() {
                let transparent = (bits[x / 8] >> (7 - x % 8)) & 1 == 1;
                pixel[3] = if transparent { 0 } else { u8::MAX };
            }
        }
    }
    Pixels::new(width, height, rgba)
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// How the service sets `pixels`: on its own plate when the edges of its drawn area are opaque,
/// else on the ground under which more of its ink reads.
fn ground_for(pixels: &Pixels) -> Ground {
    if plate_opacity(pixels) >= PLATE_OPACITY {
        return Ground::Plate;
    }
    let dark = luminance(DARK_GROUND);
    let light = luminance(LIGHT_GROUND);
    let (mut on_dark, mut on_light) = (0_u64, 0_u64);
    for pixel in pixels.rgba.chunks_exact(4) {
        let weight = u64::from(pixel[3]);
        if weight == 0 {
            continue;
        }
        let ink = luminance([pixel[0], pixel[1], pixel[2]]);
        if contrast(ink, dark) >= READABLE_CONTRAST {
            on_dark += weight;
        }
        if contrast(ink, light) >= READABLE_CONTRAST {
            on_light += weight;
        }
    }
    if on_dark > on_light {
        Ground::Dark
    } else {
        Ground::Light
    }
}

/// The average opacity, from 0 to 1, of the middle half of each edge of the icon's drawn area, in
/// a band [`RING_SHARE`] of the area's longer side deep; zero when nothing is drawn or the area
/// spans less than [`PLATE_SPAN`] of the icon either way.
fn plate_opacity(pixels: &Pixels) -> f64 {
    let Some((left, top, right, bottom)) = drawn_area(pixels) else {
        return 0.0;
    };
    let (width, height) = (right - left, bottom - top);
    let spans = |drawn: u32, side: u32| f64::from(drawn) >= f64::from(side) * PLATE_SPAN;
    if !spans(width, pixels.width) || !spans(height, pixels.height) {
        return 0.0;
    }
    let band = share_of(width.max(height), RING_SHARE);
    // Each edge loses the same share at both ends; a side of a few pixels keeps all of it.
    let inset = |length: u32| (f64::from(length) * (1.0 - EDGE_MIDDLE_SHARE) / 2.0) as u32;
    let (middle_left, middle_right) = (left + inset(width), right - inset(width));
    let (middle_top, middle_bottom) = (top + inset(height), bottom - inset(height));
    let on_edge = |x: u32, y: u32| {
        let across = (middle_left..middle_right).contains(&x);
        let down = (middle_top..middle_bottom).contains(&y);
        (across && (y < top + band || y >= bottom.saturating_sub(band)))
            || (down && (x < left + band || x >= right.saturating_sub(band)))
    };
    let (mut total, mut count) = (0_u64, 0_u64);
    for y in top..bottom {
        for x in (left..right).filter(|&x| on_edge(x, y)) {
            total += u64::from(pixels.alpha(x, y));
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    total as f64 / (count as f64 * f64::from(u8::MAX))
}

/// The box around the pixels at least [`DRAWN_ALPHA`] opaque, as left, top, right and bottom,
/// the right and bottom one past the last drawn pixel; none when no pixel is drawn.
fn drawn_area(pixels: &Pixels) -> Option<(u32, u32, u32, u32)> {
    let mut area: Option<(u32, u32, u32, u32)> = None;
    for y in 0..pixels.height {
        for x in 0..pixels.width {
            if pixels.alpha(x, y) < DRAWN_ALPHA {
                continue;
            }
            area = Some(
                area.map_or((x, y, x + 1, y + 1), |(left, top, right, bottom)| {
                    (left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1))
                }),
            );
        }
    }
    area
}

/// The WCAG relative luminance of an sRGB color.
fn luminance(color: [u8; 3]) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / f64::from(u8::MAX);
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
}

/// The WCAG contrast ratio of two relative luminances.
fn contrast(one: f64, other: f64) -> f64 {
    (one.max(other) + 0.05) / (one.min(other) + 0.05)
}

/// `pixels` at their own size in the middle of a square of `ground`, as an opaque PNG.
fn composite(pixels: &Pixels, ground: [u8; 3]) -> Option<Vec<u8>> {
    let side = pixels.side() + 2 * share_of(pixels.side(), PADDING_SHARE);
    let left = (side - pixels.width) / 2;
    let top = (side - pixels.height) / 2;
    let mut rgb: Vec<u8> = ground
        .iter()
        .copied()
        .cycle()
        .take(side as usize * side as usize * 3)
        .collect();
    for y in 0..pixels.height {
        for x in 0..pixels.width {
            let source = &pixels.rgba[index(x, y, pixels.width) * 4..][..4];
            let target = &mut rgb[index(left + x, top + y, side) * 3..][..3];
            let alpha = u32::from(source[3]);
            for ((out, &ink), &under) in target.iter_mut().zip(&source[..3]).zip(&ground) {
                let blended = (u32::from(ink) * alpha
                    + u32::from(under) * (u32::from(u8::MAX) - alpha)
                    + u32::from(u8::MAX) / 2)
                    / u32::from(u8::MAX);
                *out = u8::try_from(blended).ok()?;
            }
        }
    }
    encode(side, &rgb)
}

/// The opaque PNG of the `side` by `side` image `rgb`, with fixed encoder settings.
fn encode(side: u32, rgb: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, side, side);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Balanced);
    encoder.set_filter(png::Filter::Adaptive);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(rgb).ok()?;
    writer.finish().ok()?;
    Some(bytes)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A `side` px square, transparent but for the pixels `ink` paints with `color`.
    fn glyph(side: u32, color: [u8; 4], ink: impl Fn(u32, u32) -> bool) -> Pixels {
        let mut rgba = Vec::new();
        for y in 0..side {
            for x in 0..side {
                rgba.extend(if ink(x, y) { color } else { [0; 4] });
            }
        }
        Pixels::new(side, side, rgba).unwrap()
    }

    /// A ring and a bar in the middle of a square, well inside its edges.
    fn mark(side: u32) -> impl Fn(u32, u32) -> bool {
        move |x, y| {
            let (dx, dy) = (
                f64::from(x) - f64::from(side) / 2.0,
                f64::from(y) - f64::from(side) / 2.0,
            );
            let distance = (dx * dx + dy * dy).sqrt();
            let radius = f64::from(side) * 0.35;
            (radius * 0.7..=radius).contains(&distance)
                || (dx.abs() < f64::from(side) * 0.06 && dy.abs() < radius)
        }
    }

    /// The PNG of the `side` px square `rgba`, as an app would ship it.
    pub(in crate::agent::image) fn png_of(side: u32, rgba: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, side, side);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Balanced);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
        writer.finish().unwrap();
        bytes
    }

    /// The synthetic all-white glyph on a transparent ground the preview's second app icon wears:
    /// a ring and a bar, drawn here, no app's artwork.
    pub(in crate::agent::image) fn white_glyph_png() -> Vec<u8> {
        png_of(48, &glyph(48, [0xff; 4], mark(48)).rgba)
    }

    /// The ground a served tile shows at its corner; none for a color that is neither ground.
    pub(in crate::agent::image) fn ground_of(tile: &[u8]) -> Option<Ground> {
        let corner: [u8; 3] = decode_png(tile)?.rgba.get(..3)?.try_into().ok()?;
        [Ground::Dark, Ground::Light]
            .into_iter()
            .find(|ground| ground.color() == Some(corner))
    }

    /// An icon file holding `images`, each its bytes and its directory side.
    fn ico_of(images: &[(u8, u16, Vec<u8>)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(0_u16.to_le_bytes());
        bytes.extend(ICO_TYPE_ICON.to_le_bytes());
        bytes.extend(u16::try_from(images.len()).unwrap().to_le_bytes());
        let mut offset = ICO_HEADER_SIZE + images.len() * ICO_ENTRY_SIZE;
        for (side, depth, image) in images {
            bytes.extend([*side, *side, 0, 0]);
            bytes.extend(1_u16.to_le_bytes());
            bytes.extend(depth.to_le_bytes());
            bytes.extend(u32::try_from(image.len()).unwrap().to_le_bytes());
            bytes.extend(u32::try_from(offset).unwrap().to_le_bytes());
            offset += image.len();
        }
        for (_, _, image) in images {
            bytes.extend(image);
        }
        bytes
    }

    /// A 32-bit bitmap of `pixels` as an icon file stores it: bottom row first, blue first, with
    /// an all-clear mask, or with alpha zero everywhere and `mask` saying what is transparent.
    fn bitmap_of(pixels: &Pixels, mask: Option<&dyn Fn(u32, u32) -> bool>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(u32::try_from(BITMAP_HEADER_SIZE).unwrap().to_le_bytes());
        bytes.extend(i32::try_from(pixels.width).unwrap().to_le_bytes());
        bytes.extend(i32::try_from(pixels.height * 2).unwrap().to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(BITMAP_DEPTH.to_le_bytes());
        bytes.extend(BITMAP_UNCOMPRESSED.to_le_bytes());
        bytes.extend([0; 20]);
        for y in (0..pixels.height).rev() {
            for x in 0..pixels.width {
                let p = &pixels.rgba[index(x, y, pixels.width) * 4..][..4];
                let alpha = if mask.is_some() { 0 } else { p[3] };
                bytes.extend([p[2], p[1], p[0], alpha]);
            }
        }
        let mask_row = (pixels.width as usize).div_ceil(32) * 4;
        for y in (0..pixels.height).rev() {
            let mut row = vec![0_u8; mask_row];
            for x in 0..pixels.width {
                if mask.is_some_and(|transparent| transparent(x, y)) {
                    row[x as usize / 8] |= 0x80 >> (x % 8);
                }
            }
            bytes.extend(row);
        }
        bytes
    }

    /// Protects defect 5107be98 and the icon-file contract: an icon with an opaque outer ring, or a
    /// plate set in a transparent margin with a faint shadow, is its own plate and is served as cached,
    /// as is an icon that cannot be read or is too large; and an icon file is read from its largest
    /// image, a PNG or a 32-bit bitmap.
    #[test]
    fn an_icon_that_is_its_own_plate_or_cannot_be_tiled_is_served_as_cached() {
        {
            let square = glyph(48, [0x1d, 0x4e, 0xd8, 0xff], |_, _| true);
            assert_eq!(ground_for(&square), Ground::Plate);
            let bytes = png_of(48, &square.rgba);
            assert_eq!(tile(IconKind::Png, &bytes), None);

            let rounded = glyph(48, [0x1d, 0x4e, 0xd8, 0xff], |x, y| {
                let corner = |v: u32| v.min(47 - v);
                corner(x) + corner(y) >= 3
            });
            assert_eq!(
                ground_for(&rounded),
                Ground::Plate,
                "slightly rounded corners keep a plate"
            );
            let disc = glyph(48, [0x1d, 0x4e, 0xd8, 0xff], |x, y| {
                let (dx, dy) = (f64::from(x) - 23.5, f64::from(y) - 23.5);
                (dx * dx + dy * dy).sqrt() <= 24.0
            });
            assert_eq!(
                ground_for(&disc),
                Ground::Light,
                "a disc leaves its corners open"
            );
        }
        {
            // A 64 px icon, as most app icons are drawn: a rounded plate inset by 6 px, and a faint
            // shadow under it.
            let inset = glyph(64, [0x1d, 0x1d, 0x22, 0xff], |x, y| {
                let inside = |v: u32| (6..58).contains(&v);
                let corner = |v: u32| (v - 6).min(57 - v);
                inside(x) && inside(y) && corner(x) + corner(y) >= 8
            });
            let mut rgba = inset.rgba.clone();
            for y in 58..61 {
                for x in 10..54 {
                    rgba[index(x, y, 64) * 4 + 3] = 0x28;
                }
            }
            let shadowed = Pixels::new(64, 64, rgba).unwrap();
            assert_eq!(ground_for(&inset), Ground::Plate);
            assert_eq!(ground_for(&shadowed), Ground::Plate);
            assert_eq!(tile(IconKind::Png, &png_of(64, &shadowed.rgba)), None);

            let inset_disc = glyph(64, [0x1d, 0x4e, 0xd8, 0xff], |x, y| {
                let (dx, dy) = (f64::from(x) - 31.5, f64::from(y) - 31.5);
                (dx * dx + dy * dy).sqrt() <= 26.0
            });
            assert_eq!(
                ground_for(&inset_disc),
                Ground::Light,
                "an inset disc is no plate"
            );
        }
        {
            assert_eq!(tile(IconKind::Png, b"\x89PNG\r\n\x1a\nzed"), None);
            assert_eq!(tile(IconKind::Png, b""), None);
            assert_eq!(tile(IconKind::Ico, b"not an icon file"), None);
            let side = MAX_SIDE + 1;
            let large = png_of(side, &vec![0xff; side as usize * side as usize * 4]);
            assert_eq!(decode_png(&large), None);
            assert_eq!(tile(IconKind::Png, &large), None);
            let largest = glyph(MAX_SIDE, [0xff; 4], mark(MAX_SIDE));
            assert!(tile(IconKind::Png, &png_of(MAX_SIDE, &largest.rgba)).is_some());
        }
        {
            let small = glyph(16, [0, 0, 0, 0xff], mark(16));
            let large = glyph(32, [0xff; 4], mark(32));
            let with_png = ico_of(&[
                (16, 32, png_of(16, &small.rgba)),
                (32, 32, png_of(32, &large.rgba)),
            ]);
            assert_eq!(
                decode_ico(&with_png),
                Some(large.clone()),
                "the largest image, a PNG"
            );

            let with_bitmap = ico_of(&[
                (32, 32, bitmap_of(&large, None)),
                (16, 32, png_of(16, &small.rgba)),
            ]);
            assert_eq!(
                decode_ico(&with_bitmap),
                Some(large.clone()),
                "the largest image, a bitmap"
            );
            assert_eq!(ground_for(&decode_ico(&with_bitmap).unwrap()), Ground::Dark);
            assert!(tile(IconKind::Ico, &with_bitmap).is_some());

            let ink = mark(32);
            let masked = ico_of(&[(32, 32, bitmap_of(&large, Some(&|x, y| !ink(x, y))))]);
            assert_eq!(
                decode_ico(&masked),
                Some(large),
                "an old bitmap's mask gives its transparency"
            );

            let shallow = {
                let mut bitmap = bitmap_of(&small, None);
                bitmap[14..16].copy_from_slice(&8_u16.to_le_bytes());
                ico_of(&[(16, 8, bitmap)])
            };
            assert_eq!(
                tile(IconKind::Ico, &shallow),
                None,
                "a palette bitmap is served as cached"
            );
            let truncated = &with_png[..with_png.len() - 10];
            assert_eq!(tile(IconKind::Ico, truncated), None);
        }
    }
}
