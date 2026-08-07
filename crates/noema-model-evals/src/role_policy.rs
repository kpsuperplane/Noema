use std::{collections::HashSet, fs, path::Path};

use noema_runtime::eval_support::RuntimeEvalRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RolePolicyManifest {
    pub schema_version: u32,
    pub policies: Vec<RolePolicy>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RolePolicy {
    pub role: RuntimeEvalRole,
    pub incumbent_candidate_id: String,
    pub minimum_cases: usize,
    pub minimum_quality_score: f64,
    pub maximum_error_rate: f64,
    pub maximum_p95_latency_ms: u64,
    pub replacement_quality_margin: f64,
}

impl RolePolicyManifest {
    pub(crate) fn policy(&self, role: RuntimeEvalRole) -> &RolePolicy {
        self.policies
            .iter()
            .find(|policy| policy.role == role)
            .expect("validated policy manifest covers every role")
    }
}

pub(crate) fn load_role_policies(path: &Path) -> Result<RolePolicyManifest, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest: RolePolicyManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    validate_role_policies(&manifest)?;
    Ok(manifest)
}

fn validate_role_policies(manifest: &RolePolicyManifest) -> Result<(), String> {
    if manifest.schema_version == 0 {
        return Err("role policy schema version must be positive".to_string());
    }
    let mut roles = HashSet::with_capacity(manifest.policies.len());
    for policy in &manifest.policies {
        if !roles.insert(policy.role) {
            return Err(format!("duplicate role policy: {}", policy.role));
        }
        if policy.incumbent_candidate_id.trim().is_empty() {
            return Err(format!("{} policy has no incumbent", policy.role));
        }
        if policy.minimum_cases < 5 {
            return Err(format!(
                "{} policy must require at least five cases",
                policy.role
            ));
        }
        for (name, value) in [
            ("minimum_quality_score", policy.minimum_quality_score),
            ("maximum_error_rate", policy.maximum_error_rate),
            (
                "replacement_quality_margin",
                policy.replacement_quality_margin,
            ),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(format!(
                    "{} policy {name} must be between zero and one",
                    policy.role
                ));
            }
        }
        if policy.maximum_p95_latency_ms == 0 {
            return Err(format!("{} policy has no latency ceiling", policy.role));
        }
    }
    let missing = RuntimeEvalRole::ALL
        .iter()
        .filter(|role| !roles.contains(role))
        .map(|role| role.as_str())
        .collect::<Vec<_>>();
    if !missing.is_empty() || roles.len() != RuntimeEvalRole::ALL.len() {
        return Err(format!(
            "role policy manifest must cover every runtime role exactly once; missing {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_role_policies_cover_every_role() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../evals/model-matrix/role-policies.toml");
        let policies = load_role_policies(&path).expect("role policies");
        assert_eq!(policies.policies.len(), RuntimeEvalRole::ALL.len());
    }
}
