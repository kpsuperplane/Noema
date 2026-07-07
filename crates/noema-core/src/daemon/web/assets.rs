use std::borrow::Cow;

pub(super) struct EmbeddedAsset {
    pub(super) content_type: &'static str,
    pub(super) body: Cow<'static, [u8]>,
}

pub(super) fn embedded_asset(path: &str) -> Option<EmbeddedAsset> {
    let (content_type, name) = if let Some(name) = static_asset_name(path) {
        (content_type_for_asset_name(name)?, name)
    } else {
        match path {
            path if is_spa_entry_path(path) => ("text/html; charset=utf-8", "index.html"),
            _ => return None,
        }
    };

    Some(EmbeddedAsset {
        content_type,
        body: asset_body(name)?,
    })
}

pub(super) fn is_spa_entry_path(path: &str) -> bool {
    if !path.starts_with('/') {
        return false;
    }

    if path == "/assets"
        || path.starts_with("/assets/")
        || path == "/api"
        || path.starts_with("/api/")
        || path == "/graphql"
        || path.starts_with("/graphql/")
    {
        return false;
    }

    path.rsplit('/')
        .next()
        .is_some_and(|segment| !segment.contains('.'))
}

fn static_asset_name(path: &str) -> Option<&str> {
    let name = path.strip_prefix("/assets/")?;
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name.contains("..")
    {
        return None;
    }
    Some(name)
}

fn content_type_for_asset_name(name: &str) -> Option<&'static str> {
    if name.ends_with(".js") {
        return Some("application/javascript; charset=utf-8");
    }
    if name.ends_with(".css") {
        return Some("text/css; charset=utf-8");
    }
    if name.ends_with(".svg") {
        return Some("image/svg+xml; charset=utf-8");
    }
    if name.ends_with(".html") {
        return Some("text/html; charset=utf-8");
    }
    None
}

/// Resolve a web asset's bytes for release builds: embed them into the binary.
#[cfg(not(debug_assertions))]
pub(super) fn asset_body(name: &str) -> Option<Cow<'static, [u8]>> {
    release_asset_body(name)
}

#[cfg(not(debug_assertions))]
include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

/// Resolve a web asset's bytes for debug builds: read them from disk at runtime.
///
/// Unlike `include_*!`, `env!` does not register the asset files as build
/// inputs, so the dev daemon can serve freshly rebuilt web assets (e.g. from a
/// running `vite build --watch`) without forcing a recompile of this crate.
#[cfg(debug_assertions)]
pub(super) fn asset_body(name: &str) -> Option<Cow<'static, [u8]>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web-assets");
    std::fs::read(dir.join(name)).ok().map(Cow::Owned)
}

#[cfg(test)]
mod tests {
    use super::{content_type_for_asset_name, embedded_asset, static_asset_name};

    #[test]
    fn static_asset_paths_resolve_to_safe_names() {
        assert_eq!(static_asset_name("/assets/app.js"), Some("app.js"));
        assert_eq!(static_asset_name("/assets/route-chunk.js"), Some("route-chunk.js"));
        assert_eq!(static_asset_name("/assets/styles.css"), Some("styles.css"));
        assert_eq!(static_asset_name("/assets/nested/chunk.js"), None);
        assert_eq!(static_asset_name("/assets/../chunk.js"), None);
        assert_eq!(static_asset_name("/assets"), None);
    }

    #[test]
    fn static_asset_content_types_are_known() {
        assert_eq!(
            content_type_for_asset_name("route-chunk.js"),
            Some("application/javascript; charset=utf-8")
        );
        assert_eq!(
            content_type_for_asset_name("styles.css"),
            Some("text/css; charset=utf-8")
        );
        assert_eq!(
            content_type_for_asset_name("noema-mark.svg"),
            Some("image/svg+xml; charset=utf-8")
        );
        assert_eq!(content_type_for_asset_name("data.bin"), None);
    }

    #[test]
    fn unknown_static_extensions_do_not_fall_back_to_spa_entry() {
        assert!(embedded_asset("/assets/data.bin").is_none());
    }
}
