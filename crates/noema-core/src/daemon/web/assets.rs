use std::borrow::Cow;

pub(super) struct EmbeddedAsset {
    pub(super) content_type: &'static str,
    pub(super) body: Cow<'static, [u8]>,
}

pub(super) fn embedded_asset(path: &str) -> Option<EmbeddedAsset> {
    let (content_type, name) = match path {
        "/assets/app.js" => ("application/javascript; charset=utf-8", "app.js"),
        "/assets/styles.css" => ("text/css; charset=utf-8", "styles.css"),
        "/assets/noema-mark.svg" => ("image/svg+xml; charset=utf-8", "noema-mark.svg"),
        path if is_spa_entry_path(path) => ("text/html; charset=utf-8", "index.html"),
        _ => return None,
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

/// Resolve a web asset's bytes for release builds: embed them into the binary.
#[cfg(not(debug_assertions))]
pub(super) fn asset_body(name: &str) -> Option<Cow<'static, [u8]>> {
    let body: &'static [u8] = match name {
        "index.html" => include_bytes!("../../../target/web-assets/index.html"),
        "app.js" => include_bytes!("../../../target/web-assets/app.js"),
        "styles.css" => include_bytes!("../../../target/web-assets/styles.css"),
        "noema-mark.svg" => include_bytes!("../../../target/web-assets/noema-mark.svg"),
        _ => return None,
    };
    Some(Cow::Borrowed(body))
}

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
