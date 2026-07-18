use serde::{Deserialize, Serialize};

/// Bounded complexity selected for a delegated task or execution contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    /// Small, low-risk work.
    Simple,
    /// Typical multi-step work.
    Medium,
    /// Large or reasoning-intensive work.
    Difficult,
}

impl TaskComplexity {
    /// Return the stable persistence/API value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Difficult => "difficult",
        }
    }
}

impl std::fmt::Display for TaskComplexity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskComplexity {
    type Err = crate::WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "simple" => Ok(Self::Simple),
            "medium" => Ok(Self::Medium),
            "difficult" => Ok(Self::Difficult),
            other => Err(crate::WorkDomainError::InvalidInput {
                field: "task_complexity",
                message: format!("unknown value {other}"),
            }),
        }
    }
}
