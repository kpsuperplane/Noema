/// Current schema version for pre-stable local SQLite data.
pub const STORE_SCHEMA_VERSION: i64 = 3;

/// Stable marker row identifying the exact schema accepted by this binary.
pub(super) const STORE_SCHEMA_MARKER: &str = "sqlite_store_v3";

/// SQLite bootstrap used by the Noema store.
pub const STORE_SCHEMA_SQL: &str = concat!(
    include_str!("schema/base.sql"),
    include_str!("schema/work_v3.sql")
);
