import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

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

  test("extracts the bootstrap SQL from Rust include_str! sources", () => {
    const sourcePath = schemaSourcePath(undefined);
    const source = readFileSync(sourcePath, "utf8");
    const schemaDirectory = dirname(sourcePath);
    expect(extractStoreSchemaSql(source, sourcePath)).toBe(
      readFileSync(resolve(schemaDirectory, "schema/base.sql"), "utf8") +
        readFileSync(resolve(schemaDirectory, "schema/work_v3.sql"), "utf8"),
    );
  });

  test("rejects a schema source without the canonical include declaration", () => {
    expect(() => extractStoreSchemaSql("const OTHER: &str = \"\";")).toThrow(
      "STORE_SCHEMA_SQL concat! include_str! declaration was not found",
    );
  });

  test("rejects include paths that escape the schema directory", () => {
    const sourcePath = schemaSourcePath(undefined);
    expect(() =>
      extractStoreSchemaSql(
        'pub const STORE_SCHEMA_SQL: &str = concat!(include_str!("../Cargo.toml"));',
        sourcePath,
      ),
    ).toThrow("STORE_SCHEMA_SQL include_str! path escapes the schema directory");
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
