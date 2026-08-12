//! Generates release-time web asset includes for the daemon.

use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct ManifestChunk {
    file: String,
    #[serde(default, rename = "isEntry")]
    is_entry: bool,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    assets: Vec<String>,
}

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let asset_dir = manifest_dir.join("target/web-assets");
    println!("cargo:rerun-if-changed={}", asset_dir.display());
    if env::var("PROFILE").as_deref() == Ok("release") {
        validate_release_assets(&asset_dir)?;
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    fs::write(
        out_dir.join("web_assets.rs"),
        release_asset_table(&asset_dir, &out_dir)?,
    )?;
    Ok(())
}

fn validate_release_assets(asset_dir: &Path) -> io::Result<()> {
    for name in [
        "index.html",
        "manifest.webmanifest",
        "sw.js",
        "apple-touch-icon.png",
        "pwa-192x192.png",
        "pwa-512x512.png",
    ] {
        require_asset(asset_dir, name)?;
    }
    let manifest_path = require_file(asset_dir, ".vite/manifest.json")?;
    let manifest: std::collections::HashMap<String, ManifestChunk> =
        serde_json::from_slice(&fs::read(manifest_path)?)
            .map_err(|error| invalid_assets(format!("invalid Vite manifest: {error}")))?;
    if !manifest.values().any(|chunk| chunk.is_entry) {
        return Err(invalid_assets("Vite manifest has no entry chunk"));
    }
    for chunk in manifest.values() {
        for path in std::iter::once(&chunk.file)
            .chain(chunk.css.iter())
            .chain(chunk.assets.iter())
        {
            require_asset(asset_dir, path)?;
        }
    }
    validate_precache(asset_dir)?;
    Ok(())
}

fn validate_precache(asset_dir: &Path) -> io::Result<()> {
    let worker = fs::read_to_string(asset_dir.join("sw.js"))?;
    for entry in fs::read_dir(asset_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let worker_reference = name
            .strip_prefix("workbox-")
            .and_then(|_| name.strip_suffix(".js"))
            .unwrap_or(name);
        if name != "sw.js" && !worker.contains(worker_reference) {
            return Err(invalid_assets(format!(
                "service worker does not reference release asset: {name}"
            )));
        }
    }
    Ok(())
}

fn require_asset(asset_dir: &Path, relative_path: &str) -> io::Result<PathBuf> {
    let mut components = Path::new(relative_path).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || relative_path.contains('\\')
    {
        return Err(invalid_assets(format!(
            "web asset path must be one relative filename: {relative_path}"
        )));
    }
    require_file(asset_dir, relative_path)
}

fn require_file(asset_dir: &Path, relative_path: &str) -> io::Result<PathBuf> {
    let path = asset_dir.join(relative_path);
    if path.is_file() {
        Ok(path)
    } else {
        Err(invalid_assets(format!(
            "required web asset is missing: {relative_path}"
        )))
    }
}

fn invalid_assets(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn release_asset_table(asset_dir: &Path, out_dir: &Path) -> io::Result<String> {
    let mut assets = Vec::new();
    let snapshot_dir = out_dir.join("web-assets");
    fs::create_dir_all(&snapshot_dir)?;
    if asset_dir.is_dir() {
        for entry in fs::read_dir(asset_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let name = name.to_owned();
            let snapshot_path = snapshot_dir.join(&name);
            fs::copy(&path, &snapshot_path)?;
            assets.push((name, snapshot_path));
        }
    }
    assets.sort_by(|left, right| left.0.cmp(&right.0));

    let mut output = String::from(
        "pub(super) fn release_asset_body(name: &str) -> Option<Cow<'static, [u8]>> {\n    match name {\n",
    );
    for (name, path) in assets {
        output.push_str("        ");
        output.push_str(&format!("{name:?}"));
        output.push_str(" => Some(Cow::Borrowed(include_bytes!(");
        output.push_str(&format!("{:?}", path.display().to_string()));
        output.push_str("))),\n");
    }
    output.push_str("        _ => None,\n    }\n}\n");
    Ok(output)
}
