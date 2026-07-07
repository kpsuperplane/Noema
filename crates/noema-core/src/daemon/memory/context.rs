use std::path::Path;

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ConversationMemoryContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub turn_index: u64,
    pub user_item_id: String,
    pub assistant_item_id: Option<String>,
}

pub(in crate::daemon) fn project_scope_from_cwd(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd?.trim();
    if cwd.is_empty() {
        return None;
    }

    let path = Path::new(cwd);
    if !path.join(".git").exists() {
        return None;
    }

    let name = path.file_name()?.to_string_lossy();
    Some(format!("project:{}", slug_fragment(&name)))
}

fn slug_fragment(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug
    }
}
