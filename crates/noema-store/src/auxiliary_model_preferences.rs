use std::str::FromStr;

use rusqlite::OptionalExtension;

use noema_providers::{
    ModelPreferenceSelection, ProviderInstanceKey, ProviderKind, ProviderReadySelection,
    ProviderSelectionSnapshot,
};

use super::{
    NoemaStore, StoreError,
    provider_selections::{
        CanonicalPreferenceOwner, resolve_new_canonical_selection_tx, write_preference_tx,
    },
    sqlite::{model_preference_selection_column, parse_column},
};

macro_rules! auxiliary_model_tasks {
    ($($variant:ident => ($id:literal, $docs:literal)),+ $(,)?) => {
        /// Closed set of built-in settings backed by an auxiliary model preference.
        #[repr(usize)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum AuxiliaryModelTask {
            $(#[doc = $docs] $variant),+
        }

        impl AuxiliaryModelTask {
            pub(crate) const ALL: &[Self] = &[$(Self::$variant),+];

            /// Return the stable persisted task id.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $id),+
                }
            }
        }

        impl std::fmt::Display for AuxiliaryModelTask {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for AuxiliaryModelTask {
            type Err = StoreError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($id => Ok(Self::$variant)),+,
                    other => Err(StoreError::InvalidEnum {
                        kind: "auxiliary_model_preference_task_id",
                        value: other.to_string(),
                    }),
                }
            }
        }
    };
}

auxiliary_model_tasks! {
    WebFetchSummarizer => ("web_fetch_summarizer", "`web.fetch` summarization."),
    ToolProgressAudit => ("tool_progress_audit", "Provider tool-continuation progress audits."),
    ActionReviewer => ("action_reviewer", "Governed write and export action review."),
    MemoryConsolidation => ("memory_extraction", "Native Markdown memory consolidation."),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuxiliaryModelDefault {
    ConfiguredProvider,
    ExplicitSelectionRequired,
}

impl AuxiliaryModelTask {
    /// Initial-selection policy is exhaustive across both supported axes.
    pub(crate) const fn initial_default(self, provider: &ProviderKind) -> AuxiliaryModelDefault {
        use AuxiliaryModelDefault::{
            ConfiguredProvider as Configured, ExplicitSelectionRequired as Explicit,
        };
        let defaults: [AuxiliaryModelDefault; Self::ALL.len()] = match provider {
            ProviderKind::Codex => [Configured, Configured, Configured, Configured],
            ProviderKind::OpenAi => [Configured, Configured, Configured, Configured],
            ProviderKind::OpenRouter => [Configured, Configured, Configured, Configured],
            ProviderKind::FoundationLocal => [Configured, Configured, Explicit, Configured],
            ProviderKind::LocalModels => [Configured, Configured, Explicit, Configured],
        };
        defaults[self as usize]
    }
}

/// New or updated auxiliary model preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuxiliaryModelPreference {
    /// Durable auxiliary task id.
    pub task: AuxiliaryModelTask,
    /// Provider kind selected for this auxiliary task.
    pub provider_kind: String,
    /// Provider account id selected for this auxiliary task.
    pub provider_account_id: String,
    /// Whether Noema or the human chooses the concrete model.
    pub selection: ModelPreferenceSelection,
    /// Whether this preference requests faster service.
    pub fast_mode: bool,
}

/// Persisted auxiliary model preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxiliaryModelPreferenceRecord {
    /// Durable auxiliary task id.
    pub task: AuxiliaryModelTask,
    /// Provider kind selected for this auxiliary task.
    pub provider_kind: String,
    /// Provider account id selected for this auxiliary task.
    pub provider_account_id: String,
    /// Exact provider process selected for this auxiliary task.
    pub provider_instance_key: ProviderInstanceKey,
    /// Whether Noema or the human chooses the concrete model.
    pub selection: ModelPreferenceSelection,
    /// Whether this preference requests faster service.
    pub fast_mode: bool,
}

impl NoemaStore {
    /// Return one auxiliary model preference by task id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_auxiliary_model_preference(
        &self,
        task: AuxiliaryModelTask,
    ) -> Result<Option<AuxiliaryModelPreferenceRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT task_id, provider_kind, provider_account_id,
                       provider_instance_key, selection_mode, model_profile, reasoning_effort,
                       fast_mode
                FROM auxiliary_model_preferences
                WHERE task_id = ?1
                LIMIT 1
                "#,
                [task.as_str()],
                preference_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Create or update one auxiliary preference while retaining a registry
    /// readiness lease through commit. All new selections must use this API.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the task or provider selection is invalid,
    /// the proof does not cover the exact route, or SQLite fails.
    pub async fn upsert_auxiliary_model_preference_with_ready_selection(
        &self,
        preference: NewAuxiliaryModelPreference,
        ready_selection: &ProviderReadySelection,
    ) -> Result<AuxiliaryModelPreferenceRecord, StoreError> {
        let mut explicit = match &preference.selection {
            ModelPreferenceSelection::ExplicitProfile {
                model_profile,
                reasoning_effort,
            } => ProviderSelectionSnapshot::explicit(
                &preference.provider_kind,
                &preference.provider_account_id,
                model_profile,
                *reasoning_effort,
                Some(format!("auxiliary_model_preference:{}", preference.task)),
            ),
            ModelPreferenceSelection::NoemaRecommended => ready_selection.selection().clone(),
        };
        explicit.fast_mode = preference.fast_mode;
        self.with_immediate_transaction_retry(|transaction| {
            let selection =
                resolve_new_canonical_selection_tx(transaction, &explicit, Some(ready_selection))?;
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Auxiliary(preference.task.as_str()),
                &selection,
                &preference.selection,
                true,
            )?;
            Ok(AuxiliaryModelPreferenceRecord {
                task: preference.task,
                provider_kind: selection.provider_kind.clone(),
                provider_account_id: selection.provider_account_id.clone(),
                provider_instance_key: selection
                    .provider_instance_key
                    .clone()
                    .ok_or(StoreError::ProviderInstanceKeyMissing)?,
                selection: preference.selection.clone(),
                fast_mode: preference.fast_mode,
            })
        })
        .await
    }
}

fn preference_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<AuxiliaryModelPreferenceRecord> {
    Ok(AuxiliaryModelPreferenceRecord {
        task: parse_column(row, 0)?,
        provider_kind: row.get(1)?,
        provider_account_id: row.get(2)?,
        provider_instance_key: parse_column(row, 3)?,
        selection: model_preference_selection_column(row, 4, 5, 6)?,
        fast_mode: row.get::<_, i64>(7)? != 0,
    })
}
