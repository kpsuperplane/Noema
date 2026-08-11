#[cfg(feature = "composition")]
use super::ConfigError;
use serde::{Deserialize, Serialize};

/// Browser worker limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserConfig {
    /// Maximum concurrent browser sessions.
    pub max_sessions: usize,
    /// Maximum V8 old-space size for each browser worker.
    pub max_old_space_mb: usize,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            max_sessions: 2,
            max_old_space_mb: 1_024,
        }
    }
}

#[cfg(feature = "composition")]
impl BrowserConfig {
    pub(super) fn validate(self) -> Result<Self, ConfigError> {
        validate_range(self.max_sessions, 1..=8, "NOEMA_BROWSER__MAX_SESSIONS")?;
        validate_range(
            self.max_old_space_mb,
            256..=4_096,
            "NOEMA_BROWSER__MAX_OLD_SPACE_MB",
        )?;
        Ok(self)
    }
}

#[cfg(feature = "composition")]
fn validate_range(
    value: usize,
    range: std::ops::RangeInclusive<usize>,
    name: &str,
) -> Result<(), ConfigError> {
    if range.contains(&value) {
        Ok(())
    } else {
        Err(ConfigError::InvalidInteger {
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}
