import { describe, expect, test } from "bun:test";

import {
  canonicalSqliteSchemaShape,
  extractStoreSchemaSql,
  schemaSourcePath,
} from "./export-sqlite-schema-shape";

describe("SQLite schema-shape export", () => {
  test("finds the live schema source without Array.map argument leakage", () => {
    expect(schemaSourcePath(undefined)).toMatch(
      /crates\/noema-store\/src\/schema\.rs$/,
    );
  });

  test("extracts the bootstrap raw string from Rust source", () => {
    expect(
      extractStoreSchemaSql(
        'pub const STORE_SCHEMA_SQL: &str = r#"CREATE TABLE things (id TEXT);"#;',
      ),
    ).toBe("CREATE TABLE things (id TEXT);");
  });

  test("rejects a schema source without the bootstrap constant", () => {
    expect(() => extractStoreSchemaSql("const OTHER: &str = \"\";")).toThrow(
      "STORE_SCHEMA_SQL raw string was not found",
    );
  });

  test("ignores bootstrap data and insignificant SQL whitespace", () => {
    const first = canonicalSqliteSchemaShape(`
      CREATE TABLE schema_state (name TEXT PRIMARY KEY, version INTEGER NOT NULL);
      INSERT INTO schema_state VALUES ('store', 1);
      CREATE INDEX schema_state_version ON schema_state(version);
    `);
    const second = canonicalSqliteSchemaShape(`
      CREATE TABLE schema_state(
        name TEXT PRIMARY KEY,
        version INTEGER NOT NULL
      );
      INSERT INTO schema_state VALUES ('store', 99);
      CREATE INDEX schema_state_version ON schema_state ( version );
    `);

    expect(second).toBe(first);
    expect(first).not.toContain("'store'");
  });

  test("detects a material schema-shape change", () => {
    const before = canonicalSqliteSchemaShape(
      "CREATE TABLE agents (agent_id TEXT PRIMARY KEY);",
    );
    const after = canonicalSqliteSchemaShape(
      "CREATE TABLE agents (agent_id TEXT PRIMARY KEY, display_name TEXT);",
    );

    expect(after).not.toBe(before);
  });

  test("preserves whitespace that belongs to SQL string literals", () => {
    const withSpace = canonicalSqliteSchemaShape(
      "CREATE TABLE labels (value TEXT CHECK (value = 'a, b'));",
    );
    const withoutSpace = canonicalSqliteSchemaShape(
      "CREATE TABLE labels (value TEXT CHECK (value = 'a,b'));",
    );

    expect(withSpace).not.toBe(withoutSpace);
  });
});
