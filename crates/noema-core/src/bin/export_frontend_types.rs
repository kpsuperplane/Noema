//! Export Rust-owned TypeScript definitions for the web frontend.

use std::{env, path::PathBuf};

fn main() -> std::io::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_path = manifest_dir.join("web/src/generated/noema.ts");

    noema_core::frontend_protocol::write_frontend_typescript(&output_path)?;
    println!("wrote {}", output_path.display());
    Ok(())
}
