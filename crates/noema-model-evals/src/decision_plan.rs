use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use noema_runtime::eval_support::{
    RUNTIME_EVAL_SUITE_VERSION, runtime_eval_case_descriptors_for_roles,
};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};

use crate::{
    manifest::SuiteConfig, matrix_manifest::EvaluationCandidate,
    matrix_runner::validate_decision_candidates, orchestrator::workspace_root,
    role_policy::RolePolicyManifest,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionPlan {
    pub schema_version: u32,
    pub decision_id: String,
    pub created_at_unix_seconds: u64,
    pub git_commit: String,
    pub git_dirty: bool,
    pub runtime_suite_version: u32,
    pub suite: SuiteConfig,
    pub policies: RolePolicyManifest,
    pub candidates: Vec<EvaluationCandidate>,
    pub estimated_max_cost_usd: f64,
    pub spend_ceiling_usd: f64,
    pub content_fingerprint: String,
}

impl DecisionPlan {
    pub(crate) fn create(
        suite: SuiteConfig,
        policies: RolePolicyManifest,
        candidates: Vec<EvaluationCandidate>,
    ) -> Result<Self, String> {
        validate_decision_candidates(&candidates, &policies)?;
        let (git_commit, git_dirty) = git_state()?;
        let created_at_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
            .as_secs();
        let decision_id = format!(
            "decision-{created_at_unix_seconds}-{}",
            git_commit.get(..12).unwrap_or(&git_commit)
        );
        let estimated_max_cost_usd = estimate_max_cost(&suite, &policies, &candidates)?;
        let mut plan = Self {
            schema_version: 1,
            decision_id,
            created_at_unix_seconds,
            git_commit,
            git_dirty,
            runtime_suite_version: RUNTIME_EVAL_SUITE_VERSION,
            suite,
            policies,
            candidates,
            estimated_max_cost_usd,
            spend_ceiling_usd: estimated_max_cost_usd,
            content_fingerprint: String::new(),
        };
        plan.content_fingerprint = plan.fingerprint()?;
        Ok(plan)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.runtime_suite_version != RUNTIME_EVAL_SUITE_VERSION {
            return Err("decision plan schema or runtime suite version is unsupported".to_string());
        }
        validate_decision_candidates(&self.candidates, &self.policies)?;
        let estimate = estimate_max_cost(&self.suite, &self.policies, &self.candidates)?;
        if (estimate - self.estimated_max_cost_usd).abs() > 0.000_001
            || self.spend_ceiling_usd + f64::EPSILON < estimate
            || self.content_fingerprint != self.fingerprint()?
        {
            return Err("decision plan content, estimate, or spend ceiling is invalid".to_string());
        }
        Ok(())
    }

    pub(crate) fn write_new(&self) -> Result<PathBuf, String> {
        let directory = workspace_root().join("target/noema-model-evals/plans");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        let path = directory.join(format!("{}.json", self.decision_id));
        if path.exists() {
            return Err(format!("decision plan already exists: {}", path.display()));
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("failed to serialize decision plan: {error}"))?;
        fs::write(&path, bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let plan: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        plan.validate()?;
        Ok(plan)
    }

    pub(crate) fn validate_execution_git_state(&self) -> Result<(), String> {
        let (commit, dirty) = git_state()?;
        if self.git_dirty || dirty || commit != self.git_commit {
            return Err(
                "defaults run requires the exact clean Git commit recorded by a clean plan"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn fingerprint(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self)
            .map_err(|error| format!("failed to fingerprint decision plan: {error}"))?;
        value["content_fingerprint"] = serde_json::Value::String(String::new());
        let bytes = serde_json::to_vec(&value)
            .map_err(|error| format!("failed to fingerprint decision plan: {error}"))?;
        Ok(digest(&SHA256, &bytes)
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

fn estimate_max_cost(
    suite: &SuiteConfig,
    policies: &RolePolicyManifest,
    candidates: &[EvaluationCandidate],
) -> Result<f64, String> {
    let mut total = 0.0;
    for candidate in candidates {
        let pricing = candidate
            .pricing
            .ok_or_else(|| format!("candidate {} has no pricing", candidate.id))?;
        let cases = runtime_eval_case_descriptors_for_roles(
            &candidate.model,
            &candidate.roles,
            candidate.reasoning_effort,
        )?;
        for case in cases {
            let calls = f64::from(case.maximum_provider_calls);
            total += calls * f64::from(suite.context_window_tokens) * pricing.input_usd_per_million
                / 1_000_000.0;
            total += calls * f64::from(case.maximum_output_tokens) * pricing.output_usd_per_million
                / 1_000_000.0;
        }
    }
    total *= f64::from(suite.decision_repetitions());
    let judge_pricing = candidates
        .iter()
        .find(|candidate| {
            candidate.model == policies.judge.model
                && candidate.reasoning_effort == policies.judge.reasoning_effort
        })
        .and_then(|candidate| candidate.pricing)
        .ok_or_else(|| "pinned judge must have a matching priced candidate".to_string())?;
    let comparisons = policies
        .policies
        .iter()
        .filter(|policy| !policy.judge_case_ids.is_empty())
        .map(|policy| {
            candidates
                .iter()
                .filter(|candidate| candidate.roles.contains(&policy.role))
                .count()
                .saturating_sub(1)
        })
        .sum::<usize>();
    total += comparisons as f64
        * (f64::from(suite.context_window_tokens) * judge_pricing.input_usd_per_million
            + f64::from(policies.judge.maximum_output_tokens)
                * judge_pricing.output_usd_per_million)
        / 1_000_000.0;
    Ok(total)
}

fn git_state() -> Result<(String, bool), String> {
    let root = workspace_root();
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .map_err(|error| format!("failed to inspect Git commit: {error}"))?;
    if !commit.status.success() {
        return Err("failed to inspect Git commit".to_string());
    }
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to inspect Git status: {error}"))?;
    Ok((
        String::from_utf8_lossy(&commit.stdout).trim().to_string(),
        !status.stdout.is_empty(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{matrix_manifest::load_evaluation_candidates, role_policy::load_role_policies};

    #[test]
    fn plan_round_trip_detects_mutation_and_has_a_cost_bound() {
        let root = workspace_root();
        let candidates =
            load_evaluation_candidates(&root.join("evals/model-matrix/candidates.toml"))
                .expect("candidates")
                .candidates;
        let policies = load_role_policies(&root.join("evals/model-matrix/role-policies.toml"))
            .expect("policies");
        let suite = crate::manifest::load_suite(&root.join("evals/model-matrix/suite.toml"))
            .expect("suite");
        let mut plan = DecisionPlan::create(suite, policies, candidates).expect("plan");
        assert!(plan.estimated_max_cost_usd > 0.0);
        assert_eq!(plan.validate(), Ok(()));
        plan.spend_ceiling_usd /= 2.0;
        assert!(plan.validate().is_err());
    }

    #[test]
    fn estimate_requires_a_priced_candidate_for_the_pinned_judge() {
        let root = workspace_root();
        let candidates =
            load_evaluation_candidates(&root.join("evals/model-matrix/candidates.toml"))
                .expect("candidates")
                .candidates;
        let mut policies = load_role_policies(&root.join("evals/model-matrix/role-policies.toml"))
            .expect("policies");
        policies.judge.model = "missing/judge".to_string();
        let suite = crate::manifest::load_suite(&root.join("evals/model-matrix/suite.toml"))
            .expect("suite");

        assert!(estimate_max_cost(&suite, &policies, &candidates).is_err());
    }

    #[test]
    fn dirty_plan_cannot_be_executed() {
        let root = workspace_root();
        let candidates =
            load_evaluation_candidates(&root.join("evals/model-matrix/candidates.toml"))
                .expect("candidates")
                .candidates;
        let policies = load_role_policies(&root.join("evals/model-matrix/role-policies.toml"))
            .expect("policies");
        let suite = crate::manifest::load_suite(&root.join("evals/model-matrix/suite.toml"))
            .expect("suite");
        let mut plan = DecisionPlan::create(suite, policies, candidates).expect("plan");
        plan.git_dirty = true;

        assert!(plan.validate_execution_git_state().is_err());
    }
}
