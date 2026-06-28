/// Namespace used by the embedded Noema store.
pub const NOEMA_NAMESPACE: &str = "noema";

/// Database used by the embedded Noema store.
pub const NOEMA_DATABASE: &str = "main";

/// Current schema version for pre-stable local data.
pub const STORE_SCHEMA_VERSION: i64 = 1;

/// Minimal bootstrap used by the first store-runtime test.
pub const STORE_SCHEMA_SQL: &str = r#"
DEFINE TABLE IF NOT EXISTS schema_state SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS version ON TABLE schema_state TYPE int ASSERT $value >= 1;
DEFINE FIELD IF NOT EXISTS name ON TABLE schema_state TYPE string;
DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_state TYPE datetime DEFAULT time::now();
UPSERT schema_state:current SET version = 1, name = 'surreal_graph_store_v1', applied_at = time::now();
"#;
