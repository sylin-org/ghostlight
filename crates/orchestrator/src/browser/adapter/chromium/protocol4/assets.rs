//! The protocol-4 page assets: one source, hashed once, shipped in every `hello_accepted`.
//!
//! `assets/world.js` is the body of the function the extension runs in the `ghostlight` isolated
//! world. It holds one include marker that lists the other page sources. The marker is replaced
//! by each listed source followed by one newline, in the listed order, which is exactly what the
//! page-asset tests' `bundle.js` does, so the JavaScript tests and the service run the same bytes.
//! The asset id is the lowercase hex SHA-256 of the expanded source, with every line ending
//! written as LF. A checkout that converts text files to CRLF therefore ships the same source and
//! the same id as a clean one.

use std::sync::{Arc, OnceLock};

use crate::install::icons::sha256_hex;

use super::wire::Assets;

/// The world body that holds the include marker.
const WORLD: &str = include_str!("assets/world.js");

/// Every page source the include marker may name, by file name.
const SOURCES: [(&str, &str); 5] = [
    ("page-shared.js", include_str!("assets/page-shared.js")),
    ("sensor.js", include_str!("assets/sensor.js")),
    (
        "presentation-css.js",
        include_str!("assets/presentation-css.js"),
    ),
    ("presentation.js", include_str!("assets/presentation.js")),
    ("content.js", include_str!("assets/content.js")),
];

/// Opening of the include marker; the file names follow, separated by whitespace.
const INCLUDE_OPEN: &str = "/*@include ";
/// Closing of the include marker.
const INCLUDE_CLOSE: &str = "*/";

/// The page assets every protocol-4 connection receives, built once per process.
///
/// # Panics
///
/// Panics when `world.js` does not hold exactly one include marker naming only known sources.
/// The sources are compiled in, so the asset tests catch this before any build ships.
#[must_use]
pub fn page_assets() -> Arc<Assets> {
    static ASSETS: OnceLock<Arc<Assets>> = OnceLock::new();
    Arc::clone(ASSETS.get_or_init(|| {
        let source = expand(WORLD, &SOURCES)
            .expect("world.js holds one include marker naming compiled-in page sources")
            .replace("\r\n", "\n");
        Arc::new(Assets {
            id: sha256_hex(source.as_bytes()),
            source,
        })
    }))
}

/// Replace the one include marker in `world` with each source it names, each followed by a
/// newline. Returns `None` when there is not exactly one marker or it names an unknown source.
fn expand(world: &str, sources: &[(&str, &str)]) -> Option<String> {
    let start = world.find(INCLUDE_OPEN)?;
    let names_start = start + INCLUDE_OPEN.len();
    let names_end = names_start + world[names_start..].find(INCLUDE_CLOSE)?;
    let rest = &world[names_end + INCLUDE_CLOSE.len()..];
    if rest.contains(INCLUDE_OPEN) {
        return None;
    }
    let mut body = String::new();
    for name in world[names_start..names_end].split_whitespace() {
        let (_, text) = sources.iter().find(|(known, _)| *known == name)?;
        body.push_str(text);
        body.push('\n');
    }
    Some(format!("{}{body}{rest}", &world[..start]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::adapter::chromium::protocol4::wire::{
        Outbound, RuntimeControlState, DIRECT_FRAME_BYTES,
    };

    /// The pinned asset id both test suites compare against.
    const PINNED_ID: &str =
        include_str!("../../../../../../../tests/golden/protocol4/assets.sha256");

    /// The service ships the page source whose hash the page-asset tests pin, with every include
    /// expanded and LF line endings, so a checkout's line-ending conversion cannot change the id
    /// (commit 95faf5ee).
    #[test]
    fn the_asset_id_is_the_pinned_hash_of_the_expanded_source() {
        let assets = page_assets();
        assert_eq!(assets.id, PINNED_ID.trim());
        assert_eq!(assets.id, sha256_hex(assets.source.as_bytes()));
        assert!(!assets.source.contains(INCLUDE_OPEN));
        assert!(!assets.source.contains('\r'));
    }

    /// The page assets travel whole in `hello_accepted`, which is never chunked, so they must fit
    /// one native message the extension can receive.
    #[test]
    fn the_hello_accepted_frame_fits_one_native_message() {
        let frame = serde_json::to_vec(&Outbound::HelloAccepted {
            service_epoch: format!("service_{}", "0".repeat(32)),
            service_version: env!("CARGO_PKG_VERSION").into(),
            control_state: RuntimeControlState::Active,
            assets: (*page_assets()).clone(),
        })
        .expect("hello_accepted serializes");
        assert!(frame.len() < DIRECT_FRAME_BYTES, "{}", frame.len());
    }
}
