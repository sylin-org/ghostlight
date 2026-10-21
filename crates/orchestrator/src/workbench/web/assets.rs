//! The workbench's own files, embedded in the service.
//!
//! The table is closed: a path that is not in it is not served. Types come from a closed table of
//! four kinds, and each file's tag is a hash of its bytes. Almost every file is under the `ui`
//! folder. The two the setup view shares with the browser extension (its stylesheet and the
//! mascot) are the extension's own files, embedded from where they live, so the window and the
//! extension's pages draw from the same bytes. The agents' images are not page files: the window
//! serves each agent's one image address from the catalog ([`crate::agent::image`]).

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

/// The kinds of file the workbench serves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetType {
    /// A page.
    Html,
    /// A stylesheet.
    Css,
    /// A script.
    Js,
    /// A raster image.
    Png,
}

impl AssetType {
    /// The media type sent with a file of this kind.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",
            Self::Css => "text/css; charset=utf-8",
            Self::Js => "text/javascript; charset=utf-8",
            Self::Png => "image/png",
        }
    }
}

/// One embedded file.
#[derive(Clone, Copy, Debug)]
pub struct Asset {
    /// The path the file is served at.
    pub path: &'static str,
    /// Its kind.
    pub kind: AssetType,
    /// Its bytes.
    pub bytes: &'static [u8],
}

/// Embeds a file of the `ui` folder, or with `extension` a file of the browser extension.
macro_rules! asset {
    ($path:literal, $kind:ident, $file:literal) => {
        Asset {
            path: $path,
            kind: AssetType::$kind,
            bytes: include_bytes!(concat!("../../../ui/", $file)),
        }
    };
    ($path:literal, $kind:ident, extension $file:literal) => {
        Asset {
            path: $path,
            kind: AssetType::$kind,
            bytes: include_bytes!(concat!("../../../../../extension/", $file)),
        }
    };
}

/// Every file the workbench serves: the `ui` folder, and the extension files the setup view
/// shares.
pub const ASSETS: &[Asset] = &[
    asset!("/", Html, "index.html"),
    asset!("/styles.css", Css, "styles.css"),
    asset!("/app.js", Js, "app.js"),
    asset!("/ghostlight.png", Png, "ghostlight.png"),
    asset!("/kit/settings.js", Js, "kit/settings.js"),
    asset!("/kit/kit.css", Css, "kit/kit.css"),
    asset!("/lib/entries.js", Js, "lib/entries.js"),
    asset!("/lib/icons.js", Js, "lib/icons.js"),
    asset!("/lib/setup-view.js", Js, "lib/setup-view.js"),
    asset!("/lib/store.js", Js, "lib/store.js"),
    asset!("/lib/transport.js", Js, "lib/transport.js"),
    asset!("/lib/view.js", Js, "lib/view.js"),
    asset!("/lib/words.js", Js, "lib/words.js"),
    asset!("/extension/kit.css", Css, extension "kit.css"),
    asset!(
        "/extension/ghostlight-mascot.png",
        Png,
        extension "icons/ghostlight-mascot.png"
    ),
];

/// The file served at `path`, if the table holds one.
#[must_use]
pub fn lookup(path: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|asset| asset.path == path)
}

/// The tag of a served file, quoted as a header value: a hash of its bytes.
#[must_use]
pub fn etag(asset: &'static Asset) -> &'static str {
    static TAGS: OnceLock<Vec<String>> = OnceLock::new();
    let tags = TAGS.get_or_init(|| {
        ASSETS
            .iter()
            .map(|asset| {
                let digest = Sha256::digest(asset.bytes);
                let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
                format!("\"{hex}\"")
            })
            .collect()
    });
    let index = ASSETS
        .iter()
        .position(|candidate| candidate.path == asset.path)
        .expect("a served asset is in the table");
    &tags[index]
}
