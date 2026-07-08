//! Generates release-time web asset includes for the daemon.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let asset_dir = manifest_dir.join("target/web-assets");
    println!("cargo:rerun-if-changed={}", asset_dir.display());
    let mnemosyne_sidecar_dir = manifest_dir.join("mnemosyne-sidecar");
    println!("cargo:rerun-if-changed={}", mnemosyne_sidecar_dir.display());
    println!(
        "cargo:rustc-env=NOEMA_MNEMOSYNE_SIDECAR_DIR={}",
        mnemosyne_sidecar_dir.display()
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    fs::write(
        out_dir.join("web_assets.rs"),
        release_asset_table(&asset_dir)?,
    )?;
    Ok(())
}

fn release_asset_table(asset_dir: &Path) -> io::Result<String> {
    let mut assets = Vec::new();
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
            assets.push((name.to_owned(), path));
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
