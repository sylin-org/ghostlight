//! The one image the window serves for each catalog agent, and the version its address carries.
//!
//! Every catalog agent has one address, [`ICON_DIRECTORY`]`/<agent id>`, and the service decides
//! what it answers there, in the owner's order: the installed app's own icon when the cache
//! ([`crate::install::icons`]) holds one, else the agent's packaged monogram tile. A curated vendor
//! mark, once one is packaged, takes its place between the two; none is packaged today.
//!
//! An app's icon is served as a finished tile ([`tile`]): the icon on the ground the service chose
//! for it, or exactly as cached when it has its own opaque plate or cannot be read. So every image
//! the page draws is whole, as a monogram is, and the page decides nothing about grounds. The
//! image made from a cached copy is made once, keyed by the copy's SHA-256, and kept in memory;
//! the copy is still read and checked against its hash each time, so a copy that is gone or
//! changed gives the monogram even after its tile was served.
//! The agent's symbol names the address with the served image's version
//! ([`AgentImage::version`]), so the address changes exactly when the image does, and the window
//! serves those same bytes ([`served`]); both read [`AgentImage::best`], so they cannot disagree.
//! The page asks for the address and draws what comes back; it never chooses between images.
//!
//! The image is the agent's image and nothing else: whether the agent is connected or working is
//! the page's dot and words, never part of the image.
//!
//! [`ICON_DIRECTORY`]: super::ICON_DIRECTORY

mod tile;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::install::icons::{sha256_hex, version_of, CachedIcon, FoundIcon, IconCache, IconKind};
use crate::language::agent_icon::icon_from;

/// The media type of a packaged monogram tile.
const MONOGRAM_MEDIA_TYPE: &str = "image/svg+xml";

/// The most images made from app icons that are kept in memory. The catalog has about twenty
/// agents, so the memory is emptied only when icons change many times in one run.
const APP_IMAGE_LIMIT: usize = 64;

/// The image made from each cached app icon, by the cached copy's SHA-256.
static APP_IMAGES: Mutex<BTreeMap<String, Arc<ServedImage>>> = Mutex::new(BTreeMap::new());

/// Embeds the packaged monogram tile `$file` of `src/agent/monograms/` as a [`Monogram`].
macro_rules! monogram {
    ($file:literal) => {
        $crate::agent::image::Monogram::packaged(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/agent/monograms/",
            $file
        )))
    };
}
pub(crate) use monogram;

/// A packaged monogram tile: Ghostlight's own SVG of an agent's initials on a colored tile, which
/// draws no script and loads nothing (`monograms/PROVENANCE.md`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Monogram(&'static [u8]);

impl Monogram {
    /// The packaged tile whose SVG is `svg`; [`monogram!`] embeds one by its file name.
    #[must_use]
    pub const fn packaged(svg: &'static [u8]) -> Self {
        Self(svg)
    }

    /// The tile's SVG.
    #[must_use]
    pub const fn bytes(self) -> &'static [u8] {
        self.0
    }
}

#[cfg(test)]
impl Monogram {
    /// A tile for test fixtures that need an agent and do not look at its image.
    pub(crate) const FIXTURE: Self = Self::packaged(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
}

/// Which image the window serves for one catalog agent now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentImage {
    /// The installed app's own icon, as the cache found it, and the image the window serves for
    /// it: its tile, or the icon as cached.
    AppIcon {
        /// What the cache holds: which copy, and which app it came from.
        found: FoundIcon,
        /// The image made from that copy.
        image: Arc<ServedImage>,
    },
    /// The agent's packaged monogram tile.
    Monogram(Monogram),
}

impl AgentImage {
    /// The best image for the catalog agent `agent_id`, whose packaged tile is `monogram`: its
    /// app's own icon when `cache` holds one for it that can still be read, else the monogram.
    /// This is the one resolution order; the symbol and the window both read it.
    #[must_use]
    pub fn best(agent_id: &str, monogram: Monogram, cache: Option<&IconCache>) -> Self {
        cache
            .and_then(|cache| {
                let found = cache.found(agent_id)?;
                let image = app_image(cache, agent_id, &found.sha256)?;
                Some(Self::AppIcon { found, image })
            })
            .unwrap_or(Self::Monogram(monogram))
    }

    /// The image's version: the first characters of the SHA-256 of the bytes the window serves.
    #[must_use]
    pub fn version(&self) -> String {
        match self {
            Self::AppIcon { image, .. } => image.version.clone(),
            Self::Monogram(monogram) => version_of(&sha256_hex(monogram.bytes())),
        }
    }

    /// The sentence that says which installed app the image came from; none for a monogram.
    #[must_use]
    pub fn about(&self) -> Option<String> {
        match self {
            Self::AppIcon { found, .. } => Some(icon_from(found.app_name)),
            Self::Monogram(_) => None,
        }
    }
}

/// The image made from `agent_id`'s cached copy, whose SHA-256 is `sha256`. The copy is read and
/// hashed on every call, so a copy that can no longer be read or is no longer the one the cache
/// vouches for gives none, also after its image was made; memory spares only the decode and the
/// composite of a copy whose image was made before.
fn app_image(cache: &IconCache, agent_id: &str, sha256: &str) -> Option<Arc<ServedImage>> {
    let icon = cache.icon(agent_id)?;
    if sha256_hex(&icon.bytes) != sha256 {
        return None;
    }
    if let Some(image) = app_images().get(sha256) {
        return Some(Arc::clone(image));
    }
    let image = Arc::new(ServedImage::app_icon(icon));
    let mut images = app_images();
    if images.len() >= APP_IMAGE_LIMIT {
        images.clear();
    }
    images.insert(sha256.to_owned(), Arc::clone(&image));
    Some(image)
}

fn app_images() -> MutexGuard<'static, BTreeMap<String, Arc<ServedImage>>> {
    crate::sync::lock(&APP_IMAGES)
}

/// An agent's image as the window serves it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServedImage {
    /// Its media type: `image/png` for an app's icon on its tile, the cached copy's type
    /// (`image/png` or `image/x-icon`) for one served as cached, `image/svg+xml` for a monogram.
    pub media_type: &'static str,
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// Its version, the one its address carries ([`AgentImage::version`]).
    pub version: String,
}

impl ServedImage {
    fn monogram(monogram: Monogram) -> Self {
        Self {
            media_type: MONOGRAM_MEDIA_TYPE,
            bytes: monogram.bytes().to_vec(),
            version: AgentImage::Monogram(monogram).version(),
        }
    }

    /// The image for the cached app icon `icon`: its tile, or the icon as cached when it is its
    /// own plate or cannot be read ([`tile::tile`]).
    fn app_icon(icon: CachedIcon) -> Self {
        match tile::tile(icon.kind, &icon.bytes) {
            Some(bytes) => Self {
                media_type: IconKind::Png.media_type(),
                version: version_of(&sha256_hex(&bytes)),
                bytes,
            },
            None => Self {
                media_type: icon.kind.media_type(),
                version: version_of(&icon.sha256),
                bytes: icon.bytes,
            },
        }
    }
}

/// The image the window serves for the catalog agent `agent_id` from `cache` now
/// ([`AgentImage::best`]); none for an id outside the catalog. A cached copy that can no longer be
/// read, or no longer matches its hash, gives the monogram, the best image left, also when its
/// tile was served before, until the next look drops it.
#[must_use]
pub fn served(agent_id: &str, cache: Option<&IconCache>) -> Option<ServedImage> {
    let monogram = crate::install::monogram_of(agent_id)?;
    Some(match AgentImage::best(agent_id, monogram, cache) {
        AgentImage::AppIcon { image, .. } => ServedImage::clone(&image),
        AgentImage::Monogram(monogram) => ServedImage::monogram(monogram),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::icons::IconPlaces;
    use crate::scratch::Scratch;

    /// The bytes of a cached icon that starts like a PNG and cannot be decoded.
    const UNREADABLE_PNG: &[u8] = b"\x89PNG\r\n\x1a\nzed";

    /// A cache in a fresh temp directory that has found Zed's icon, `bytes`, on a Linux computer,
    /// and those bytes.
    fn zed_cache(name: &str, bytes: &[u8]) -> (Scratch, IconCache, Vec<u8>) {
        let root = Scratch::named(&format!("agent-image-{name}"));
        let share = root.join("share");
        let entry = share.join("applications").join("dev.zed.Zed.desktop");
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, "[Desktop Entry]\nName=Zed\nIcon=zed\n").unwrap();
        let png = share.join("icons/hicolor/64x64/apps/zed.png");
        std::fs::create_dir_all(png.parent().unwrap()).unwrap();
        let bytes = bytes.to_vec();
        std::fs::write(&png, &bytes).unwrap();
        let cache = IconCache::open(root.join("icons"), IconPlaces::linux(vec![share]), None);
        assert!(cache.refresh());
        (root, cache, bytes)
    }

    #[test]
    fn an_agent_with_no_cached_icon_is_served_its_monogram() {
        let served = served("zed", None).unwrap();
        assert_eq!(served.media_type, "image/svg+xml");
        assert_eq!(served.bytes, monogram!("zed.svg").bytes());
        assert_eq!(served.version, version_of(&sha256_hex(&served.bytes)));
        assert_eq!(served.version.len(), 12);
        assert!(served.version.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn an_agent_with_a_cached_icon_that_cannot_be_decoded_is_served_that_icon_as_cached() {
        let (_root, cache, bytes) = zed_cache("served", UNREADABLE_PNG);
        let before = served("zed", None).unwrap();
        let after = served("zed", Some(&cache)).unwrap();
        assert_eq!(after.media_type, "image/png");
        assert_eq!(
            after.bytes, bytes,
            "an icon that cannot be decoded is served as cached"
        );
        assert_eq!(after.version, version_of(&sha256_hex(&bytes)));
        assert_ne!(
            after.version, before.version,
            "a new image is a new address"
        );

        let best = AgentImage::best("zed", monogram!("zed.svg"), Some(&cache));
        assert_eq!(
            best.version(),
            after.version,
            "the symbol names what is served"
        );
        assert_eq!(
            best.about().as_deref(),
            Some("Icon from the installed Zed app.")
        );

        let codex = served("codex", Some(&cache)).unwrap();
        assert_eq!(
            codex.bytes,
            monogram!("codex.svg").bytes(),
            "an agent with no icon of its own keeps its monogram beside one that has"
        );
        assert_eq!(
            AgentImage::best("codex", monogram!("codex.svg"), Some(&cache)).about(),
            None
        );
    }

    #[test]
    fn an_app_icon_is_served_as_its_tile_and_the_symbol_names_the_tile() {
        let source = tile::tests::white_glyph_png();
        let (_root, cache, _) = zed_cache("tile", &source);
        let served = served("zed", Some(&cache)).unwrap();
        assert_eq!(served.media_type, "image/png");
        assert_ne!(served.bytes, source, "the icon is served on its ground");
        assert_eq!(
            tile::tests::ground_of(&served.bytes),
            Some(tile::Ground::Dark),
            "a white glyph sits on the dark ground"
        );
        assert_eq!(served.version, version_of(&sha256_hex(&served.bytes)));
        assert_ne!(
            served.version,
            version_of(&cache.found("zed").unwrap().sha256),
            "the version follows the tile, not the cached copy"
        );
        let best = AgentImage::best("zed", monogram!("zed.svg"), Some(&cache));
        assert_eq!(best.version(), served.version, "the symbol names the tile");
        assert_eq!(
            super::served("zed", Some(&cache)).unwrap(),
            served,
            "the same copy gives the same image"
        );
    }

    /// The symbol and the window both give `zed`'s monogram from `cache`.
    fn both_give_the_monogram(cache: &IconCache, case: &str) {
        let served = served("zed", Some(cache)).unwrap();
        assert_eq!(served.media_type, "image/svg+xml", "{case}");
        assert_eq!(served.bytes, monogram!("zed.svg").bytes(), "{case}");
        let best = AgentImage::best("zed", monogram!("zed.svg"), Some(cache));
        assert_eq!(best, AgentImage::Monogram(monogram!("zed.svg")), "{case}");
        assert_eq!(
            best.version(),
            served.version,
            "{case}: the symbol and the window agree on the monogram"
        );
    }

    #[test]
    fn a_copy_the_cache_no_longer_vouches_for_gives_the_monogram() {
        let source = tile::tests::png_of(8, &[0xff; 8 * 8 * 4]);
        let (root, cache, _) = zed_cache("changed", &source);
        let copy = root.join("icons").join("zed.png");
        std::fs::write(&copy, b"changed underneath").unwrap();
        both_give_the_monogram(&cache, "changed before any tile was made");
    }

    #[test]
    fn a_tile_already_served_gives_way_to_the_monogram_when_its_copy_goes() {
        let (root, cache, _) = zed_cache("gone", &tile::tests::white_glyph_png());
        let copy = root.join("icons").join("zed.png");
        let tiled = served("zed", Some(&cache)).unwrap();
        assert_eq!(
            tile::tests::ground_of(&tiled.bytes),
            Some(tile::Ground::Dark)
        );

        std::fs::write(&copy, b"replaced underneath").unwrap();
        both_give_the_monogram(&cache, "replaced after its tile was served");

        std::fs::remove_file(&copy).unwrap();
        both_give_the_monogram(&cache, "deleted after its tile was served");
    }

    /// The variable that rewrites the preview's app icons from the service's tiles.
    const FIXTURE_UPDATE: &str = "GHOSTLIGHT_TILE_FIXTURES_UPDATE";

    /// The preview's synthetic all-white app icon, which the tests draw.
    const WHITE_ICON: &str = "agent-icon-white.png";

    /// Each app icon the preview server serves, a synthetic source, and the tile it serves for it.
    const PREVIEW_TILES: [(&str, &str); 2] = [
        ("agent-icon.png", "agent-icon-tile.png"),
        (WHITE_ICON, "agent-icon-white-tile.png"),
    ];

    /// `path` holds `bytes`, or is rewritten to hold them when [`FIXTURE_UPDATE`] is set.
    fn fixture_holds(path: &std::path::Path, bytes: &[u8]) {
        if std::env::var_os(FIXTURE_UPDATE).is_some_and(|value| value == "1") {
            std::fs::write(path, bytes).unwrap();
        }
        assert!(
            std::fs::read(path).is_ok_and(|kept| kept == bytes),
            "{} is not what the service makes; run with {FIXTURE_UPDATE}=1 and look at it",
            path.display()
        );
    }

    #[test]
    fn the_preview_serves_the_tiles_the_service_makes_one_on_each_ground() {
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/workbench");
        fixture_holds(&folder.join(WHITE_ICON), &tile::tests::white_glyph_png());
        let mut grounds = Vec::new();
        for (source, tiled) in PREVIEW_TILES {
            let bytes = std::fs::read(folder.join(source)).unwrap();
            let served = ServedImage::app_icon(CachedIcon {
                kind: IconKind::Png,
                sha256: sha256_hex(&bytes),
                bytes,
            });
            assert_eq!(served.media_type, "image/png", "{source}");
            fixture_holds(&folder.join(tiled), &served.bytes);
            grounds.push(tile::tests::ground_of(&served.bytes));
        }
        assert_eq!(
            grounds,
            [Some(tile::Ground::Light), Some(tile::Ground::Dark)],
            "the colored icon sits on the light ground and the white one on the dark"
        );
    }

    #[test]
    fn an_id_outside_the_catalog_has_no_image() {
        for id in ["not-an-agent", "", "command_line", "unknown", "zed.json"] {
            assert_eq!(served(id, None), None, "{id}");
        }
    }

    #[test]
    fn every_packaged_tile_is_an_agents_and_draws_no_script_and_loads_nothing() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent/monograms");
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("svg") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            assert!(
                crate::install::monograms()
                    .iter()
                    .any(|monogram| monogram.bytes() == bytes.as_slice()),
                "no agent wears {}",
                path.display()
            );
            let text = String::from_utf8(bytes).unwrap().to_ascii_lowercase();
            for part in ["<script", "href", "url(", "<foreignobject", "<style"] {
                assert!(!text.contains(part), "{} holds {part}", path.display());
            }
            assert!(
                !text
                    .split(|c: char| c.is_whitespace() || c == '<')
                    .any(|word| word.starts_with("on") && word.contains('=')),
                "{} holds an event handler",
                path.display()
            );
        }
    }
}
