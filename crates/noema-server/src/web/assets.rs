use std::borrow::Cow;

pub(super) struct EmbeddedAsset {
    pub(super) content_type: &'static str,
    pub(super) cache_control: &'static str,
    pub(super) service_worker_allowed: bool,
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
        cache_control: cache_control_for_asset_name(name),
        service_worker_allowed: name == "sw.js",
        body: asset_body(name)?,
    })
}

pub(super) fn is_spa_entry_path(path: &str) -> bool {
    if !path.starts_with('/') {
        return false;
    }

    if [
        "/assets",
        "/api",
        "/graphql",
        "/auth",
        "/__noema",
        "/mcp/oauth",
        "/provider/oauth",
        "/adapter/oauth",
        "/artifacts",
    ]
    .iter()
    .any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('/'))
    }) {
        return false;
    }

    path.rsplit('/')
        .next()
        .is_some_and(|segment| !segment.contains('.'))
}

fn static_asset_name(path: &str) -> Option<&str> {
    let name = path.strip_prefix("/assets/")?;
    if name.is_empty()
        || name == "graphiql.html"
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return None;
    }
    Some(name)
}

fn content_type_for_asset_name(name: &str) -> Option<&'static str> {
    match name.rsplit_once('.')?.1 {
        "js" => Some("application/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "svg" => Some("image/svg+xml; charset=utf-8"),
        "png" => Some("image/png"),
        "ttf" => Some("font/ttf"),
        "webmanifest" => Some("application/manifest+json; charset=utf-8"),
        "html" => Some("text/html; charset=utf-8"),
        _ => None,
    }
}

fn cache_control_for_asset_name(name: &str) -> &'static str {
    if is_fingerprinted_code(name) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn is_fingerprinted_code(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(extension, "js" | "css" | "ttf") && name != "sw.js"
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
    let dir = std::env::var_os("NOEMA_DEV_ASSET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web-assets")
        });
    std::fs::read(dir.join(name)).ok().map(Cow::Owned)
}

#[cfg(test)]
mod tests {
    use super::{
        cache_control_for_asset_name, content_type_for_asset_name, embedded_asset,
        is_spa_entry_path, static_asset_name,
    };

    #[test]
    fn assets_preserve_safe_paths_content_types_dynamic_chunks_and_spa_boundaries() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web-assets");
        std::fs::create_dir_all(&dir).expect("create web asset dir");
        let asset_path = dir.join("__noema_asset_test_chunk.js");
        std::fs::write(&asset_path, b"export {};").expect("write test chunk");

        let asset = embedded_asset("/assets/__noema_asset_test_chunk.js")
            .expect("test chunk should resolve");
        assert_eq!(asset.content_type, "application/javascript; charset=utf-8");
        assert_eq!(asset.cache_control, "public, max-age=31536000, immutable");
        assert!(!asset.service_worker_allowed);
        assert_eq!(asset.body.as_ref(), b"export {};");

        std::fs::remove_file(asset_path).expect("remove test chunk");

        assert_eq!(static_asset_name("/assets/app.js"), Some("app.js"));
        assert_eq!(
            static_asset_name("/assets/route-chunk.js"),
            Some("route-chunk.js")
        );
        assert_eq!(static_asset_name("/assets/styles.css"), Some("styles.css"));
        assert_eq!(static_asset_name("/assets/nested/chunk.js"), None);
        assert_eq!(static_asset_name("/assets/../chunk.js"), None);
        assert_eq!(static_asset_name("/assets"), None);

        assert_eq!(
            content_type_for_asset_name("route-chunk.js"),
            Some("application/javascript; charset=utf-8")
        );
        assert_eq!(
            content_type_for_asset_name("styles.css"),
            Some("text/css; charset=utf-8")
        );
        assert_eq!(
            content_type_for_asset_name("icon.svg"),
            Some("image/svg+xml; charset=utf-8")
        );
        assert_eq!(content_type_for_asset_name("data.bin"), None);
        assert_eq!(
            content_type_for_asset_name("manifest.webmanifest"),
            Some("application/manifest+json; charset=utf-8")
        );
        assert_eq!(content_type_for_asset_name("icon.png"), Some("image/png"));
        assert_eq!(
            content_type_for_asset_name("noema-font.ttf"),
            Some("font/ttf")
        );

        assert_eq!(
            cache_control_for_asset_name("app-a1B2c3D4.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control_for_asset_name("sw.js"), "no-cache");
        assert_eq!(
            cache_control_for_asset_name("noema-font-a1B2c3D4.ttf"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control_for_asset_name("index.html"), "no-cache");

        assert!(embedded_asset("/assets/data.bin").is_none());
        assert!(is_spa_entry_path("/memory"));
        assert!(!is_spa_entry_path("/graphql/schema.graphql"));
    }
}
