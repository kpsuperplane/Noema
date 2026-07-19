import { readdirSync, readFileSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

export const DEFAULT_CRITICAL_TEST_MANIFEST =
  "docs/superpowers/baselines/rust-critical-test-contracts.json";

export const CRITICAL_TEST_CATEGORIES = [
  "security",
  "privacy",
  "data-loss",
  "durable-state",
  "concurrency",
  "provider-protocol",
] as const;

export type CriticalTestCategory = (typeof CRITICAL_TEST_CATEGORIES)[number];

export type CriticalTest = {
  category: CriticalTestCategory;
  path: string;
  name: string;
};

export type CriticalTestManifest = {
  formatVersion: 1;
  tests: CriticalTest[];
};

type SourceTestDeclarations = Map<string, number>;

const RUST_TEST_DECLARATION =
  /^[ \t]*#\[(?:test|tokio::test(?:\([^\]\r\n]*\))?|async_std::test(?:\([^\]\r\n]*\))?)\]\s*(?:#\[[^\]\r\n]+\]\s*)*(?:pub(?:\([^\]\r\n]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)/gm;

function testId(test: Pick<CriticalTest, "path" | "name">): string {
  return `${test.path}::${test.name}`;
}

function isCriticalTestCategory(value: unknown): value is CriticalTestCategory {
  return (
    typeof value === "string" &&
    (CRITICAL_TEST_CATEGORIES as readonly string[]).includes(value)
  );
}

function isRustSourcePath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.startsWith("crates/") &&
    value.endsWith(".rs") &&
    !value.includes("\\")
  );
}

function isRustTestName(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z_][A-Za-z0-9_]*$/.test(value);
}

export function parseRustSourceTestDeclarations(
  sources: ReadonlyMap<string, string>,
): SourceTestDeclarations {
  const declarations: SourceTestDeclarations = new Map();
  for (const [sourcePath, source] of sources) {
    for (const match of source.matchAll(RUST_TEST_DECLARATION)) {
      const id = `${sourcePath.replaceAll(sep, "/")}::${match[1]}`;
      declarations.set(id, (declarations.get(id) ?? 0) + 1);
    }
  }
  return declarations;
}

export function loadRustSourceTestDeclarations(
  root: string,
): SourceTestDeclarations {
  const sources = new Map<string, string>();
  const cratesRoot = resolve(root, "crates");

  const visit = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        sources.set(relative(root, path), readFileSync(path, "utf8"));
      }
    }
  };

  visit(cratesRoot);
  return parseRustSourceTestDeclarations(sources);
}

export function validateCriticalTestManifest(
  manifest: CriticalTestManifest,
  declarations: ReadonlyMap<string, number>,
): string[] {
  const errors: string[] = [];
  const seen = new Set<string>();
  const categoryCounts = new Map<CriticalTestCategory, number>();

  for (const test of manifest.tests) {
    const id = testId(test);
    if (seen.has(id)) {
      errors.push(`duplicate critical test: ${id}`);
    }
    seen.add(id);
    categoryCounts.set(
      test.category,
      (categoryCounts.get(test.category) ?? 0) + 1,
    );

    const declarationCount = declarations.get(id) ?? 0;
    if (declarationCount === 0) {
      errors.push(
        `${id} (${test.category}) is missing from Rust source test declarations`,
      );
    } else if (declarationCount > 1) {
      errors.push(
        `${id} (${test.category}) is declared ${declarationCount} times; critical tests must have one source owner`,
      );
    }
  }

  for (const category of CRITICAL_TEST_CATEGORIES) {
    if (!categoryCounts.has(category)) {
      errors.push(`critical test category ${category} has no tests`);
    }
  }

  return errors.sort();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function loadCriticalTestManifest(path: string): CriticalTestManifest {
  const value = JSON.parse(readFileSync(path, "utf8")) as {
    formatVersion?: unknown;
    tests?: unknown;
  };

  if (
    value.formatVersion !== 1 ||
    !Array.isArray(value.tests) ||
    value.tests.length === 0 ||
    !value.tests.every(
      (test): test is CriticalTest =>
        isRecord(test) &&
        isCriticalTestCategory(test.category) &&
        isRustSourcePath(test.path) &&
        isRustTestName(test.name),
    )
  ) {
    throw new Error(`invalid Rust critical-test manifest at ${path}`);
  }

  const manifest = value as CriticalTestManifest;
  const ids = new Set<string>();
  for (const test of manifest.tests) {
    const id = testId(test);
    if (ids.has(id)) {
      throw new Error(`invalid Rust critical-test manifest at ${path}`);
    }
    ids.add(id);
  }
  return manifest;
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
    const root = resolve(argumentValue(args, "--root") ?? ".");
    const manifest = loadCriticalTestManifest(
      resolve(
        root,
        argumentValue(args, "--manifest") ?? DEFAULT_CRITICAL_TEST_MANIFEST,
      ),
    );
    const declarations = loadRustSourceTestDeclarations(root);
    const errors = validateCriticalTestManifest(manifest, declarations);
    if (errors.length > 0) {
      throw new Error(
        `Rust critical-test contract violations:\n${errors.join("\n")}`,
      );
    }
    console.log(
      `Rust critical-test contracts are valid (${manifest.tests.length} tests)`,
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
