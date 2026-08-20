//! One-time conversion from stored Task content into mutable Task files.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::Transaction;
use rusqlite_migration::{HookError, HookResult};

use crate::TASK_FILE_TEXT_LIMIT;

pub(crate) fn move_task_prose_to_files(transaction: &Transaction<'_>) -> HookResult {
    let database_path: String = transaction.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |row| row.get(0),
    )?;
    let home_root = infer_home_root(Path::new(&database_path));
    validate_current_task_documents(transaction, &home_root)
        .map_err(|error| HookError::Hook(error.to_string()))?;
    create_recurrence_templates(transaction, &home_root)
        .map_err(|error| HookError::Hook(error.to_string()))?;
    rename_manual_task_body_field(transaction)?;
    transaction.execute_batch(
        "ALTER TABLE tasks DROP COLUMN description_markdown;
         ALTER TABLE task_recurrences DROP COLUMN description_markdown;",
    )?;
    Ok(())
}

fn validate_current_task_documents(
    transaction: &Transaction<'_>,
    home_root: &Path,
) -> std::io::Result<()> {
    let mut statement = transaction
        .prepare(
            "SELECT t.task_id, t.cwd_override, t.task_directory, p.folder
             FROM tasks t LEFT JOIN projects p ON p.project_id = t.project_id
             ORDER BY t.task_id",
        )
        .map_err(sql_io)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(sql_io)?;
    for row in rows {
        let (task_id, cwd, directory, project) = row.map_err(sql_io)?;
        let root = cwd
            .map(|root| PathBuf::from(root).join(&directory))
            .or_else(|| project.map(|root| PathBuf::from(root).join(&directory)))
            .unwrap_or_else(|| home_root.join("tasks").join(&directory));
        validate_document(&root.join(crate::TASK_DOCUMENT), &task_id)?;
    }
    Ok(())
}

fn create_recurrence_templates(
    transaction: &Transaction<'_>,
    home_root: &Path,
) -> std::io::Result<()> {
    let recurrence_root = home_root.join("recurrences");
    fs::create_dir_all(&recurrence_root)?;
    reject_symlink_components(&recurrence_root)?;
    let mut statement = transaction
        .prepare(
            "SELECT recurrence_id, description_markdown
             FROM task_recurrences ORDER BY recurrence_id",
        )
        .map_err(sql_io)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_io)?;
    for row in rows {
        let (recurrence_id, content) = row.map_err(sql_io)?;
        if content.len() > TASK_FILE_TEXT_LIMIT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("recurrence template exceeds 64 KiB for {recurrence_id}"),
            ));
        }
        let suffix = recurrence_id.trim_start_matches("recurrence:");
        let directory = Path::new(suffix);
        if directory.components().count() != 1
            || !matches!(
                directory.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("recurrence template path is unsafe for {recurrence_id}"),
            ));
        }
        let root = recurrence_root.join(directory);
        fs::create_dir_all(&root)?;
        reject_symlink_components(&root)?;
        let path = root.join(crate::TASK_DOCUMENT);
        if entry_exists(&path)? {
            validate_document(&path, &recurrence_id)?;
        } else {
            atomic_write(&path, content.as_bytes())?;
        }
    }
    Ok(())
}

fn validate_document(path: &Path, owner: &str) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("required TASK.md is unsafe for {owner}"),
        ));
    }
    if metadata.len() > TASK_FILE_TEXT_LIMIT as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("required TASK.md exceeds 64 KiB for {owner}"),
        ));
    }
    let bytes = fs::read(path)?;
    std::str::from_utf8(&bytes).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("required TASK.md is not UTF-8 for {owner}"),
        )
    })?;
    Ok(())
}

fn rename_manual_task_body_field(transaction: &Transaction<'_>) -> HookResult {
    for table in ["tasks", "task_recurrences"] {
        let sql = format!("SELECT rowid, authorization_context_json FROM {table}");
        let mut statement = transaction.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut updates = Vec::new();
        for row in rows {
            let (rowid, json) = row?;
            let mut value: serde_json::Value =
                serde_json::from_str(&json).map_err(|error| HookError::Hook(error.to_string()))?;
            let Some(object) = value.as_object_mut() else {
                continue;
            };
            if object.get("kind").and_then(serde_json::Value::as_str) == Some("manual_task_body")
                && let Some(document) = object.remove("description_markdown")
            {
                object.insert("task_document_markdown".to_string(), document);
                updates.push((
                    rowid,
                    serde_json::to_string(&value)
                        .map_err(|error| HookError::Hook(error.to_string()))?,
                ));
            }
        }
        drop(statement);
        let update = format!("UPDATE {table} SET authorization_context_json = ?2 WHERE rowid = ?1");
        for (rowid, json) in updates {
            transaction.execute(&update, rusqlite::params![rowid, json])?;
        }
    }
    Ok(())
}

fn sql_io(error: rusqlite::Error) -> std::io::Error {
    std::io::Error::other(error)
}

pub(crate) fn convert_legacy_task_files(transaction: &Transaction<'_>) -> HookResult {
    let database_path: String = transaction.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |row| row.get(0),
    )?;
    let home_root = infer_home_root(Path::new(&database_path));
    let mut statement = transaction.prepare(
        r#"SELECT t.task_id, t.title, t.description_markdown, t.cwd_override,
                  t.task_directory, p.folder, c.request_markdown,
                  c.execution_plan_markdown, s.summary, s.result_markdown,
                  r.overall_feedback
           FROM tasks AS t
           LEFT JOIN projects AS p ON p.project_id = t.project_id
           LEFT JOIN task_execution_contracts AS c
             ON c.contract_id = t.current_contract_id
           LEFT JOIN task_submissions AS s
             ON s.submission_id = t.latest_submission_id
           LEFT JOIN task_reviews AS r
             ON r.review_id = t.latest_review_id
           ORDER BY t.task_id"#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(LegacyTask {
            task_id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            cwd_override: row.get(3)?,
            task_directory: row.get(4)?,
            project_folder: row.get(5)?,
            request: row.get(6)?,
            plan: row.get(7)?,
            summary: row.get(8)?,
            result: row.get(9)?,
            review: row.get(10)?,
        })
    })?;
    for row in rows {
        let task = row?;
        convert_task(&home_root, &task).map_err(|error| HookError::Hook(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn isolate_explicit_task_directories(transaction: &Transaction<'_>) -> HookResult {
    let database_path: String = transaction.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |row| row.get(0),
    )?;
    let home_root = infer_home_root(Path::new(&database_path));
    let mut statement = transaction.prepare(
        r#"SELECT t.task_id, t.title, t.description_markdown, t.cwd_override,
                  t.task_directory, p.folder, c.request_markdown,
                  c.execution_plan_markdown, s.summary, s.result_markdown,
                  r.overall_feedback
           FROM tasks AS t
           LEFT JOIN projects AS p ON p.project_id = t.project_id
           LEFT JOIN task_execution_contracts AS c
             ON c.contract_id = t.current_contract_id
           LEFT JOIN task_submissions AS s
             ON s.submission_id = t.latest_submission_id
           LEFT JOIN task_reviews AS r
             ON r.review_id = t.latest_review_id
           ORDER BY t.task_id"#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(LegacyTask {
            task_id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            cwd_override: row.get(3)?,
            task_directory: row.get(4)?,
            project_folder: row.get(5)?,
            request: row.get(6)?,
            plan: row.get(7)?,
            summary: row.get(8)?,
            result: row.get(9)?,
            review: row.get(10)?,
        })
    })?;
    for row in rows {
        let task = row?;
        convert_task_to_subdirectory(&home_root, &task)
            .map_err(|error| HookError::Hook(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn create_result_documents(transaction: &Transaction<'_>) -> HookResult {
    let database_path: String = transaction.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |row| row.get(0),
    )?;
    let home_root = infer_home_root(Path::new(&database_path));
    let mut statement = transaction.prepare(
        r#"SELECT t.task_id, t.cwd_override, t.task_directory, p.folder
           FROM tasks AS t
           JOIN workflow_stages AS stage
             ON stage.workflow_id = t.workflow_id AND stage.stage_id = t.stage_id
           LEFT JOIN projects AS p ON p.project_id = t.project_id
           WHERE stage.system_behavior = 'terminal_success'
              OR EXISTS (
                   SELECT 1 FROM agent_runs AS run
                   WHERE run.task_id = t.task_id AND run.run_kind = 'reviewer'
              )
           ORDER BY t.task_id"#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        let (task_id, cwd_override, task_directory, project_folder) = row?;
        let task_root = cwd_override
            .map(|root| PathBuf::from(root).join(&task_directory))
            .or_else(|| project_folder.map(|project| PathBuf::from(project).join(&task_directory)))
            .unwrap_or_else(|| home_root.join("tasks").join(&task_directory));
        copy_result_document(&task_id, &task_root)
            .map_err(|error| HookError::Hook(error.to_string()))?;
    }
    Ok(())
}

fn copy_result_document(task_id: &str, task_root: &Path) -> std::io::Result<()> {
    reject_symlink_components(task_root)?;
    let result_path = task_root.join(crate::TASK_RESULT);
    if entry_exists(&result_path)? {
        return Ok(());
    }
    let task_path = task_root.join(crate::TASK_DOCUMENT);
    let metadata = fs::symlink_metadata(&task_path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("required TASK.md is unsafe for {task_id}"),
        ));
    }
    if metadata.len() > TASK_FILE_TEXT_LIMIT as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("required TASK.md exceeds 64 KiB for {task_id}"),
        ));
    }
    let content = fs::read(&task_path)?;
    std::str::from_utf8(&content).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("required TASK.md is not UTF-8 for {task_id}"),
        )
    })?;
    atomic_write(&result_path, &content)
}

struct LegacyTask {
    task_id: String,
    title: String,
    description: String,
    cwd_override: Option<String>,
    task_directory: Option<String>,
    project_folder: Option<String>,
    request: Option<String>,
    plan: Option<String>,
    summary: Option<String>,
    result: Option<String>,
    review: Option<String>,
}

fn convert_task(home_root: &Path, task: &LegacyTask) -> std::io::Result<()> {
    let directory = task
        .task_directory
        .clone()
        .unwrap_or_else(|| task.task_id.trim_start_matches("task:").replace(':', "-"));
    let task_root = task
        .cwd_override
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| {
            task.project_folder
                .as_deref()
                .map(|project| Path::new(project).join(&directory))
        })
        .unwrap_or_else(|| home_root.join("tasks").join(directory));
    fs::create_dir_all(&task_root)?;
    reject_symlink_components(&task_root)?;

    let content = legacy_content(task);
    if content.len() > TASK_FILE_TEXT_LIMIT {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("converted TASK.md exceeds 64 KiB for {}", task.task_id),
        ));
    }
    let task_path = task_root.join(crate::TASK_DOCUMENT);
    if entry_exists(&task_path)? {
        write_if_absent(&task_root.join("legacy-task.md"), content.as_bytes())?;
    } else {
        atomic_write(&task_path, content.as_bytes())?;
    }
    if let Some(review) = task
        .review
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        write_if_absent(
            &task_root.join(crate::TASK_REVIEW),
            review.trim().as_bytes(),
        )?;
    }
    Ok(())
}

fn convert_task_to_subdirectory(home_root: &Path, task: &LegacyTask) -> std::io::Result<()> {
    let directory = task
        .task_directory
        .clone()
        .unwrap_or_else(|| task.task_id.trim_start_matches("task:").replace(':', "-"));
    let task_root = task
        .cwd_override
        .as_deref()
        .map(|root| Path::new(root).join(&directory))
        .or_else(|| {
            task.project_folder
                .as_deref()
                .map(|project| Path::new(project).join(&directory))
        })
        .unwrap_or_else(|| home_root.join("tasks").join(directory));
    fs::create_dir_all(&task_root)?;
    reject_symlink_components(&task_root)?;
    let content = legacy_content(task);
    if content.len() > TASK_FILE_TEXT_LIMIT {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("converted TASK.md exceeds 64 KiB for {}", task.task_id),
        ));
    }
    write_if_absent(&task_root.join(crate::TASK_DOCUMENT), content.as_bytes())?;
    if let Some(review) = task
        .review
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        write_if_absent(
            &task_root.join(crate::TASK_REVIEW),
            review.trim().as_bytes(),
        )?;
    }
    Ok(())
}

fn legacy_content(task: &LegacyTask) -> String {
    let mut sections = vec![format!("# {}", task.title)];
    push_section(&mut sections, None, Some(&task.description));
    push_section(&mut sections, Some("Request"), task.request.as_deref());
    push_section(&mut sections, Some("Plan"), task.plan.as_deref());
    push_section(
        &mut sections,
        Some("Result summary"),
        task.summary.as_deref(),
    );
    push_section(&mut sections, Some("Result"), task.result.as_deref());
    format!("{}\n", sections.join("\n\n"))
}

fn push_section(sections: &mut Vec<String>, title: Option<&str>, content: Option<&str>) {
    let Some(content) = content.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let value = title.map_or_else(
        || content.to_string(),
        |title| format!("## {title}\n\n{content}"),
    );
    if !sections.iter().any(|section| section == &value) {
        sections.push(value);
    }
}

fn write_if_absent(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if entry_exists(path)? {
        return Ok(());
    }
    atomic_write(path, content)
}

fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "file name is missing")
    })?;
    let temporary = path.with_file_name(format!(
        ".{}.noema-migration-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(content)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn entry_exists(path: &Path) -> std::io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn reject_symlink_components(path: &Path) -> std::io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Task path contains a symbolic link",
            ));
        }
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
