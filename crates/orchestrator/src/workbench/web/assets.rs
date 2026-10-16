//! The workbench's own files, embedded in the service.
//!
//! The table is closed: a path that is not in it is not served. Types come from a closed table of
//! five kinds, and each file's tag is a hash of its bytes.

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
    /// A vector image.
    Svg,
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
            Self::Svg => "image/svg+xml",
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

macro_rules! asset {
    ($path:literal, $kind:ident, $file:literal) => {
        Asset {
            path: $path,
            kind: AssetType::$kind,
            bytes: include_bytes!(concat!("../../../ui/", $file)),
        }
    };
}

/// Every file under the `ui` folder that the workbench serves.
pub const ASSETS: &[Asset] = &[
    asset!("/", Html, "index.html"),
    asset!("/styles.css", Css, "styles.css"),
    asset!("/app.js", Js, "app.js"),
    asset!("/ghostlight.png", Png, "ghostlight.png"),
    asset!("/kit/settings.js", Js, "kit/settings.js"),
    asset!("/kit/kit.css", Css, "kit/kit.css"),
    asset!("/lib/entries.js", Js, "lib/entries.js"),
    asset!("/lib/store.js", Js, "lib/store.js"),
    asset!("/lib/transport.js", Js, "lib/transport.js"),
    asset!("/lib/view.js", Js, "lib/view.js"),
    asset!("/lib/words.js", Js, "lib/words.js"),
    asset!(
        "/integrations/antigravity.svg",
        Svg,
        "integrations/antigravity.svg"
    ),
    asset!(
        "/integrations/claude-code.svg",
        Svg,
        "integrations/claude-code.svg"
    ),
    asset!(
        "/integrations/claude-desktop.svg",
        Svg,
        "integrations/claude-desktop.svg"
    ),
    asset!("/integrations/cline.svg", Svg, "integrations/cline.svg"),
    asset!("/integrations/codex.svg", Svg, "integrations/codex.svg"),
    asset!(
        "/integrations/continue.svg",
        Svg,
        "integrations/continue.svg"
    ),
    asset!(
        "/integrations/copilot-cli.svg",
        Svg,
        "integrations/copilot-cli.svg"
    ),
    asset!("/integrations/crush.svg", Svg, "integrations/crush.svg"),
    asset!("/integrations/cursor.svg", Svg, "integrations/cursor.svg"),
    asset!("/integrations/generic.svg", Svg, "integrations/generic.svg"),
    asset!("/integrations/goose.svg", Svg, "integrations/goose.svg"),
    asset!("/integrations/junie.svg", Svg, "integrations/junie.svg"),
    asset!(
        "/integrations/kilo-code.svg",
        Svg,
        "integrations/kilo-code.svg"
    ),
    asset!("/integrations/kiro.svg", Svg, "integrations/kiro.svg"),
    asset!(
        "/integrations/opencode.svg",
        Svg,
        "integrations/opencode.svg"
    ),
    asset!(
        "/integrations/qwen-code.svg",
        Svg,
        "integrations/qwen-code.svg"
    ),
    asset!("/integrations/vscode.svg", Svg, "integrations/vscode.svg"),
    asset!(
        "/integrations/windsurf.svg",
        Svg,
        "integrations/windsurf.svg"
    ),
    asset!("/integrations/zcode.svg", Svg, "integrations/zcode.svg"),
    asset!("/integrations/zed.svg", Svg, "integrations/zed.svg"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn files_below(directory: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files_below(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    #[test]
    fn every_file_the_page_ships_is_served_and_every_served_path_exists() {
        let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
        let mut files = Vec::new();
        files_below(&ui, &mut files);
        let mut on_disk: Vec<String> = files
            .iter()
            .map(|file| {
                let relative = file.strip_prefix(&ui).unwrap();
                let parts: Vec<_> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                format!("/{}", parts.join("/"))
            })
            .filter(|path| path != "/integrations/PROVENANCE.md")
            .map(|path| {
                if path == "/index.html" {
                    "/".to_owned()
                } else {
                    path
                }
            })
            .collect();
        on_disk.sort();
        let mut served: Vec<String> = ASSETS.iter().map(|asset| asset.path.to_owned()).collect();
        served.sort();
        assert_eq!(served, on_disk);
    }

    #[test]
    fn the_types_follow_the_extensions() {
        for asset in ASSETS {
            let expected = match asset.path.rsplit('.').next().unwrap() {
                "css" => AssetType::Css,
                "js" => AssetType::Js,
                "png" => AssetType::Png,
                "svg" => AssetType::Svg,
                _ => AssetType::Html,
            };
            assert_eq!(asset.kind, expected, "{}", asset.path);
            assert!(!asset.bytes.is_empty(), "{}", asset.path);
        }
    }

    #[test]
    fn tags_differ_between_files_and_repeat_for_one() {
        let one = etag(lookup("/app.js").unwrap());
        let two = etag(lookup("/styles.css").unwrap());
        assert_ne!(one, two);
        assert_eq!(one, etag(lookup("/app.js").unwrap()));
    }
}
