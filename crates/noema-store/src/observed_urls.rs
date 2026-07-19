//! Exact structured URL observations emitted by governed web adapters.

use rusqlite::{OptionalExtension, params};

use crate::{NoemaStore, StoreError};

const MAX_URLS_PER_OBSERVATION: usize = 256;

/// Adapter-owned source of one normalized URL observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedUrlSource {
    /// A destination returned in a structured search result.
    SearchResult,
    /// A link extracted structurally from fetched response bytes.
    FetchedLink,
}

impl ObservedUrlSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SearchResult => "search_result",
            Self::FetchedLink => "fetched_link",
        }
    }
}

impl NoemaStore {
    /// Upsert a bounded adapter-produced set of already-normalized URLs.
    ///
    /// Observations intentionally have no expiry; URL identity is only a
    /// narrow egress predicate and every fetch still re-runs network policy.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the source reference or URLs are malformed,
    /// the batch exceeds its bound, or SQLite fails.
    pub async fn record_observed_urls(
        &self,
        source: ObservedUrlSource,
        source_event_reference: &str,
        normalized_urls: &[String],
    ) -> Result<(), StoreError> {
        if source_event_reference.trim().is_empty()
            || normalized_urls.len() > MAX_URLS_PER_OBSERVATION
            || normalized_urls
                .iter()
                .any(|url| url.trim().is_empty() || url.len() > 2_048)
        {
            return Err(StoreError::InvariantViolation {
                message: "observed URL input is invalid".to_string(),
            });
        }
        self.with_immediate_transaction_retry(|transaction| {
            for url in normalized_urls {
                transaction.execute(
                    r#"
                    INSERT INTO observed_urls (
                      normalized_url, source_kind, source_event_reference
                    ) VALUES (?1, ?2, ?3)
                    ON CONFLICT (normalized_url) DO UPDATE SET
                      source_kind = excluded.source_kind,
                      source_event_reference = excluded.source_event_reference,
                      last_observed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    "#,
                    params![url, source.as_str(), source_event_reference],
                )?;
            }
            Ok(())
        })
        .await
    }

    /// Return whether one exact normalized URL was emitted by a web adapter.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded read fails.
    pub async fn has_observed_url(&self, normalized_url: &str) -> Result<bool, StoreError> {
        let normalized_url = normalized_url.to_string();
        self.with_connection(move |connection| {
            Ok(connection
                .query_row(
                    "SELECT 1 FROM observed_urls WHERE normalized_url = ?1",
                    [normalized_url],
                    |_| Ok(()),
                )
                .optional()?
                .is_some())
        })
        .await
    }
}
