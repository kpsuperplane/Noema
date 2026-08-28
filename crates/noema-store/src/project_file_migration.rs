//! One-time creation or adoption of project context documents.

use std::path::{Path, PathBuf};

use rusqlite::Transaction;
use rusqlite_migration::{HookError, HookResult};

pub(crate) fn create_project_documents(transaction: &Transaction<'_>) -> HookResult {
    let database_path: String = transaction.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |row| row.get(0),
    )?;
    let home_root = infer_home_root(Path::new(&database_path));
    let mut statement = transaction.prepare(
        "SELECT workspace_id, project_id, name, description, folder FROM projects ORDER BY project_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        let (workspace_id, project_id, name, description, folder) = row?;
        crate::project_files::ensure_migrated_project_document(
            &home_root,
            &workspace_id,
            &project_id,
            &name,
            &description,
            folder.as_deref(),
        )
        .map_err(|error| HookError::Hook(error.to_string()))?;
    }
    Ok(())
}

fn infer_home_root(database_path: &Path) -> PathBuf {
    let parent = database_path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().is_some_and(|name| name == "db") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}
