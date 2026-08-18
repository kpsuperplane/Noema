//! One-time conversion from stored Task content into mutable Task files.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::Transaction;
use rusqlite_migration::{HookError, HookResult};

use crate::TASK_FILE_TEXT_LIMIT;

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
