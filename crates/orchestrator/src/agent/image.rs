//! The one image the window serves for each catalog agent, and the version its address carries.
//!
//! Every catalog agent has one address, [`ICON_DIRECTORY`]`/<agent id>`, and the service decides
//! what it answers there, in the owner's order: the installed app's own icon when the cache
//! ([`crate::install::icons`]) holds one, else the agent's packaged monogram tile. A curated vendor
//! mark, once one is packaged, takes its place between the two; none is packaged today. The agent's
//! symbol names the address with the served image's version ([`AgentImage::version`]), so the
//! address changes exactly when the image does, and the window serves the bytes ([`served`]). The
//! page asks for the address and draws what comes back; it never chooses between images.
//!
//! The image is the agent's image and nothing else: whether the agent is connected or working is
//! the page's dot and words, never part of the image.
//!
//! [`ICON_DIRECTORY`]: super::ICON_DIRECTORY

use crate::install::icons::{sha256_hex, version_of, CachedIcon, FoundIcon, IconCache};
use crate::language::agent_icon::icon_from;

/// The media type of a packaged monogram tile.
const MONOGRAM_MEDIA_TYPE: &str = "image/svg+xml";

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
    /// The installed app's own icon, as the cache found it.
    AppIcon(FoundIcon),
    /// The agent's packaged monogram tile.
    Monogram(Monogram),
}

impl AgentImage {
    /// The best image for the catalog agent `agent_id`, whose packaged tile is `monogram`: its
    /// app's own icon when `cache` holds one for it, else the monogram. This is the one resolution
    /// order; the symbol and the window both read it.
    #[must_use]
    pub fn best(agent_id: &str, monogram: Monogram, cache: Option<&IconCache>) -> Self {
        cache
            .and_then(|cache| cache.found(agent_id))
            .map_or(Self::Monogram(monogram), Self::AppIcon)
    }

    /// The image's version: the first characters of the SHA-256 of the bytes the window serves.
    #[must_use]
    pub fn version(&self) -> String {
        match self {
            Self::AppIcon(found) => found.version.clone(),
            Self::Monogram(monogram) => version_of(&sha256_hex(monogram.bytes())),
        }
    }

    /// The sentence that says which installed app the image came from; none for a monogram.
    #[must_use]
    pub fn about(&self) -> Option<String> {
        match self {
            Self::AppIcon(found) => Some(icon_from(found.app_name)),
            Self::Monogram(_) => None,
        }
    }
}

/// An agent's image as the window serves it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServedImage {
    /// Its media type: `image/png` or `image/x-icon` for an app's icon, `image/svg+xml` for a
    /// monogram.
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

    fn app_icon(icon: CachedIcon) -> Self {
        Self {
            media_type: icon.kind.media_type(),
            version: version_of(&icon.sha256),
            bytes: icon.bytes,
        }
    }
}

/// The image the window serves for the catalog agent `agent_id` from `cache` now
/// ([`AgentImage::best`]); none for an id outside the catalog. A cached copy that can no longer be
/// read gives the monogram, the best image left, until the next look drops it.
#[must_use]
pub fn served(agent_id: &str, cache: Option<&IconCache>) -> Option<ServedImage> {
    let monogram = crate::install::monogram_of(agent_id)?;
    let icon = match AgentImage::best(agent_id, monogram, cache) {
        AgentImage::AppIcon(_) => cache.and_then(|cache| cache.icon(agent_id)),
        AgentImage::Monogram(_) => None,
    };
    Some(icon.map_or_else(|| ServedImage::monogram(monogram), ServedImage::app_icon))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::icons::IconPlaces;
    use std::path::PathBuf;

    /// A cache in a fresh temp directory that has found Zed's icon on a Linux computer, and the
    /// icon's bytes.
    fn zed_cache(name: &str) -> (PathBuf, IconCache, Vec<u8>) {
        let root = std::env::temp_dir().join(format!(
            "ghostlight-agent-image-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let share = root.join("share");
        let entry = share.join("applications").join("dev.zed.Zed.desktop");
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, "[Desktop Entry]\nName=Zed\nIcon=zed\n").unwrap();
        let png = share.join("icons/hicolor/64x64/apps/zed.png");
        std::fs::create_dir_all(png.parent().unwrap()).unwrap();
        let bytes = b"\x89PNG\r\n\x1a\nzed".to_vec();
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
    fn an_agent_with_a_cached_icon_is_served_that_icon_and_its_version_moves_with_it() {
        let (root, cache, bytes) = zed_cache("served");
        let before = served("zed", None).unwrap();
        let after = served("zed", Some(&cache)).unwrap();
        assert_eq!(after.media_type, "image/png");
        assert_eq!(after.bytes, bytes);
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
        std::fs::remove_dir_all(root).unwrap();
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
