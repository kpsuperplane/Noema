use ring::digest::{SHA256, digest};
use rusqlite::Connection;

use super::StoreError;

/// Supported pre-stable SQLite schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaVersion {
    /// The reset-only canonical schema introduced before Noema V1.
    V2,
}

impl SchemaVersion {
    /// Return the durable integer representation.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        match self {
            Self::V2 => 2,
        }
    }
}

/// Current schema version for pre-stable local SQLite data.
pub const STORE_SCHEMA_VERSION: i64 = SchemaVersion::V2.as_i64();

/// Durable marker name for the canonical schema.
pub const STORE_SCHEMA_MARKER: &str = "sqlite_store_v2";

/// Fingerprint generated from the normalized canonical v2 `sqlite_schema` rows.
pub const EXPECTED_STRUCTURAL_FINGERPRINT: &str =
    "c8ec7dbc9a1256dea43f471474dcb227acee57904e17e94e99ef0e0adbc82c53";

/// SQLite bootstrap used by the Noema store.
pub const STORE_SCHEMA_SQL: &str = include_str!("schema_v2.sql");

/// Create the canonical v2 schema and persist its marker in one transaction.
pub fn bootstrap_schema_v2(conn: &mut Connection) -> Result<(), StoreError> {
    let transaction = conn.transaction()?;
    transaction.execute_batch(STORE_SCHEMA_SQL)?;
    let fingerprint = structural_fingerprint(&transaction)?;
    if fingerprint != EXPECTED_STRUCTURAL_FINGERPRINT {
        return Err(StoreError::Schema(format!(
            "canonical schema fingerprint drifted: found {fingerprint}, expected {EXPECTED_STRUCTURAL_FINGERPRINT}"
        )));
    }
    transaction.execute(
        "INSERT INTO schema_state (name, version, structural_fingerprint) VALUES (?1, ?2, ?3)",
        rusqlite::params![STORE_SCHEMA_MARKER, STORE_SCHEMA_VERSION, fingerprint],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Compute the normalized fingerprint of every application-owned schema object.
pub fn structural_fingerprint(conn: &Connection) -> Result<String, StoreError> {
    let mut statement = conn.prepare(
        r#"
        SELECT type, name, tbl_name, sql
        FROM sqlite_schema
        WHERE name NOT LIKE 'sqlite_%'
        ORDER BY type, name, tbl_name
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        ))
    })?;
    let mut canonical = String::new();
    for row in rows {
        let (kind, name, table, sql) = row?;
        canonical.push_str(&kind);
        canonical.push('|');
        canonical.push_str(&name);
        canonical.push('|');
        canonical.push_str(&table);
        canonical.push('|');
        canonical.push_str(&normalize_sql(&sql));
        canonical.push('\n');
    }
    Ok(hex_digest(canonical.as_bytes()))
}

/// Return the fingerprint compiled from the canonical schema source.
pub const fn expected_structural_fingerprint() -> &'static str {
    EXPECTED_STRUCTURAL_FINGERPRINT
}

fn normalize_sql(sql: &str) -> String {
    let mut normalized = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut quote = None;
    let mut pending_space = false;
    while let Some(character) = chars.next() {
        if let Some(closing) = quote {
            normalized.push(character);
            if character == closing {
                if chars.peek() == Some(&closing) {
                    normalized.push(chars.next().expect("peeked quote"));
                } else {
                    quote = None;
                }
            }
            continue;
        }
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        normalized.push(character);
        quote = match character {
            '\'' => Some('\''),
            '"' => Some('"'),
            '`' => Some('`'),
            '[' => Some(']'),
            _ => None,
        };
    }
    normalized
}

fn hex_digest(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::normalize_sql;

    #[test]
    fn normalization_collapses_only_unquoted_whitespace() {
        assert_eq!(
            normalize_sql("CREATE   TABLE\nexample ( value TEXT )"),
            normalize_sql("CREATE TABLE example ( value TEXT )")
        );
        assert_ne!(
            normalize_sql("CHECK (value = 'a  b')"),
            normalize_sql("CHECK (value = 'a b')")
        );
        assert_ne!(
            normalize_sql("CREATE TABLE \"a  b\" (id TEXT)"),
            normalize_sql("CREATE TABLE \"a b\" (id TEXT)")
        );
    }

    #[test]
    fn normalization_preserves_escaped_quotes() {
        assert_eq!(
            normalize_sql("CHECK (value = 'it''s  exact')"),
            "CHECK (value = 'it''s  exact')"
        );
    }
}
