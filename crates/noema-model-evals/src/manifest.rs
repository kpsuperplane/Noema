use std::{collections::HashSet, fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateManifest {
    pub candidates: Vec<ModelCandidate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelCandidate {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub revision: String,
    pub file: String,
    pub sha256: String,
    pub bytes: u64,
    pub license: String,
    pub context_tokens: u32,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_source: Option<String>,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SuiteConfig {
    pub context_window_tokens: u32,
    pub generation_timeout_seconds: u64,
    pub startup_timeout_seconds: u64,
    pub worker_timeout_seconds: u64,
    pub repetitions: u32,
}

pub(crate) fn load_candidates(path: &Path) -> Result<CandidateManifest, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest: CandidateManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    validate_candidates(&manifest)?;
    Ok(manifest)
}

pub(crate) fn load_suite(path: &Path) -> Result<SuiteConfig, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let suite: SuiteConfig = toml::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    if suite.context_window_tokens == 0
        || suite.generation_timeout_seconds == 0
        || suite.startup_timeout_seconds == 0
        || suite.worker_timeout_seconds == 0
        || suite.repetitions == 0
    {
        return Err("suite numeric settings must all be greater than zero".to_string());
    }
    Ok(suite)
}

fn validate_candidates(manifest: &CandidateManifest) -> Result<(), String> {
    if manifest.candidates.is_empty() {
        return Err("candidate manifest must contain at least one model".to_string());
    }
    let mut ids = HashSet::with_capacity(manifest.candidates.len());
    for candidate in &manifest.candidates {
        if !ids.insert(candidate.id.as_str()) {
            return Err(format!("duplicate candidate id: {}", candidate.id));
        }
        for (field, value) in [
            ("id", candidate.id.as_str()),
            ("name", candidate.name.as_str()),
            ("repo", candidate.repo.as_str()),
            ("file", candidate.file.as_str()),
            ("license", candidate.license.as_str()),
            ("source", candidate.source.as_str()),
            ("notes", candidate.notes.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("candidate {} has empty {field}", candidate.id));
            }
        }
        if candidate.repo.split('/').count() != 2 {
            return Err(format!(
                "candidate {} repo must use owner/name form",
                candidate.id
            ));
        }
        if !candidate.file.to_ascii_lowercase().ends_with(".gguf") {
            return Err(format!("candidate {} file is not a GGUF", candidate.id));
        }
        if !is_lower_hex(&candidate.revision, 40) {
            return Err(format!(
                "candidate {} revision is not an immutable commit",
                candidate.id
            ));
        }
        if !is_lower_hex(&candidate.sha256, 64) {
            return Err(format!("candidate {} sha256 is malformed", candidate.id));
        }
        if candidate.bytes == 0 || candidate.context_tokens == 0 {
            return Err(format!(
                "candidate {} size and context must be positive",
                candidate.id
            ));
        }
        if !candidate.source.starts_with("https://")
            || candidate
                .artifact_source
                .as_deref()
                .is_some_and(|source| !source.starts_with("https://"))
        {
            return Err(format!(
                "candidate {} has an invalid source URL",
                candidate.id
            ));
        }
    }
    Ok(())
}

fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_candidate_and_suite_manifests_are_valid() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/local-models/candidates.toml");
        let manifest = load_candidates(&path).expect("candidate manifest");
        assert_eq!(manifest.candidates.len(), 21);
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/local-models/suite.toml");
        let suite = load_suite(&path).expect("suite config");
        assert_eq!(suite.context_window_tokens, 8_192);
    }
}
