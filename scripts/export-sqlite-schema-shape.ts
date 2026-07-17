import { Database } from "bun:sqlite";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const SCHEMA_SOURCE = "crates/noema-store/src/schema.rs";

type SchemaEntry = {
  type: string;
  name: string;
  tableName: string;
  sql: string;
};

export function extractStoreSchemaSql(source: string): string {
  const match =
    /pub const STORE_SCHEMA_SQL:\s*&str\s*=\s*r#"([\s\S]*?)"#;/.exec(source);
  if (!match) {
    throw new Error("STORE_SCHEMA_SQL raw string was not found in the schema source");
  }
  return match[1];
}

function normalizeSql(sql: string): string {
  let normalized = "";
  let pendingSpace = false;
  let quote: "'" | '"' | "`" | "]" | undefined;

  for (let index = 0; index < sql.length; index += 1) {
    const character = sql[index];
    if (quote) {
      normalized += character;
      if (character === quote) {
        const escaped = quote !== "]" && sql[index + 1] === quote;
        if (escaped) {
          normalized += sql[index + 1];
          index += 1;
        } else {
          quote = undefined;
        }
      }
      continue;
    }

    if (/\s/.test(character)) {
      pendingSpace = true;
      continue;
    }

    if (character === "'" || character === '"' || character === "`") {
      if (
        pendingSpace &&
        normalized.length > 0 &&
        !normalized.endsWith("(") &&
        !normalized.endsWith(",")
      ) {
        normalized += " ";
      }
      normalized += character;
      quote = character;
      pendingSpace = false;
      continue;
    }
    if (character === "[") {
      if (
        pendingSpace &&
        normalized.length > 0 &&
        !normalized.endsWith("(") &&
        !normalized.endsWith(",")
      ) {
        normalized += " ";
      }
      normalized += character;
      quote = "]";
      pendingSpace = false;
      continue;
    }

    if (character === "(" || character === ")" || character === ",") {
      normalized = normalized.trimEnd();
      normalized += character;
      pendingSpace = false;
      continue;
    }

    if (
      pendingSpace &&
      normalized.length > 0 &&
      !normalized.endsWith("(") &&
      !normalized.endsWith(",")
    ) {
      normalized += " ";
    }
    normalized += character;
    pendingSpace = false;
  }

  return normalized.trim();
}

export function canonicalSqliteSchemaShape(schemaSql: string): string {
  const database = new Database(":memory:", { create: true, strict: true });
  try {
    database.exec(schemaSql);
    const entries = database
      .query<
        { type: string; name: string; table_name: string; sql: string | null },
        []
      >(
        `
          SELECT type, name, tbl_name AS table_name, sql
          FROM sqlite_schema
          WHERE name NOT LIKE 'sqlite_%'
          ORDER BY type, name, tbl_name
        `,
      )
      .all()
      .map(
        (entry): SchemaEntry => ({
          type: entry.type,
          name: entry.name,
          tableName: entry.table_name,
          sql: normalizeSql(entry.sql ?? ""),
        }),
      );
    return `${entries.map((entry) => JSON.stringify(entry)).join("\n")}\n`;
  } finally {
    database.close();
  }
}

export function schemaSourcePath(explicit: string | undefined): string {
  if (explicit) return resolve(explicit);
  const path = resolve(SCHEMA_SOURCE);
  if (!existsSync(path)) {
    throw new Error(`expected schema source does not exist: ${SCHEMA_SOURCE}`);
  }
  return path;
}

function argumentValue(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

if (import.meta.main) {
  try {
    const args = Bun.argv.slice(2);
    const sourcePath = schemaSourcePath(argumentValue(args, "--schema-source"));
    const schemaSql = extractStoreSchemaSql(readFileSync(sourcePath, "utf8"));
    const shape = canonicalSqliteSchemaShape(schemaSql);
    const sqlOutput = argumentValue(args, "--sql-output");
    if (sqlOutput) {
      writeFileSync(resolve(sqlOutput), schemaSql);
    }
    const output = argumentValue(args, "--output");
    if (output) {
      writeFileSync(resolve(output), shape);
    } else {
      process.stdout.write(shape);
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
