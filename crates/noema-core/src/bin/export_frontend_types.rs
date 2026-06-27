//! Export GraphQL schema for frontend code generation.

use std::{env, fs, path::PathBuf};

fn main() -> std::io::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_path = manifest_dir.join("web/src/generated/schema.graphql");
    let schema = noema_core::graphql::build_schema(noema_core::graphql::GraphqlState::for_tests());

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, schema.sdl())?;
    println!("wrote {}", output_path.display());
    Ok(())
}
