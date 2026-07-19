import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  loadCriticalTestManifest,
  parseRustSourceTestDeclarations,
  validateCriticalTestManifest,
  type CriticalTestManifest,
} from "./verify-rust-critical-tests";

const SOURCES = new Map([
  [
    "crates/example/src/lib.rs",
    `
#[test]
fn keeps_the_owner() {}

#[tokio::test]
async fn waits_for_the_owner() {}
`,
  ],
  [
    "crates/example/src/privacy.rs",
    `
#[test]
fn redacts_the_secret() {}
`,
  ],
]);

function manifestWith(
  ...tests: CriticalTestManifest["tests"]
): CriticalTestManifest {
  return { formatVersion: 1, tests };
}

describe("Rust critical-test contract verifier", () => {
  test("parses source-owned sync and async test declarations", () => {
    expect([...parseRustSourceTestDeclarations(SOURCES).entries()]).toEqual([
      ["crates/example/src/lib.rs::keeps_the_owner", 1],
      ["crates/example/src/lib.rs::waits_for_the_owner", 1],
      ["crates/example/src/privacy.rs::redacts_the_secret", 1],
    ]);
  });

  test("requires an exact source path and function name but ignores other tests", () => {
    const manifest = manifestWith(
      {
        category: "security",
        path: "crates/example/src/lib.rs",
        name: "keeps_the_owner",
      },
      {
        category: "privacy",
        path: "crates/example/src/privacy.rs",
        name: "redacts_the_secret",
      },
      {
        category: "data-loss",
        path: "crates/example/src/lib.rs",
        name: "waits_for_the_owner",
      },
      {
        category: "durable-state",
        path: "crates/example/src/lib.rs",
        name: "keeps_the_owner",
      },
      {
        category: "concurrency",
        path: "crates/example/src/lib.rs",
        name: "keeps_the_owner",
      },
      {
        category: "provider-protocol",
        path: "crates/example/src/lib.rs",
        name: "keeps_the_owner",
      },
    );
    expect(
      validateCriticalTestManifest(
        manifest,
        parseRustSourceTestDeclarations(SOURCES),
      ),
    ).toEqual([
      "duplicate critical test: crates/example/src/lib.rs::keeps_the_owner",
      "duplicate critical test: crates/example/src/lib.rs::keeps_the_owner",
      "duplicate critical test: crates/example/src/lib.rs::keeps_the_owner",
    ]);
  });

  test("reports missing source declarations and missing categories", () => {
    const manifest = manifestWith({
      category: "security",
      path: "crates/example/src/lib.rs",
      name: "renamed_owner",
    });
    expect(
      validateCriticalTestManifest(
        manifest,
        parseRustSourceTestDeclarations(SOURCES),
      ),
    ).toEqual([
      "crates/example/src/lib.rs::renamed_owner (security) is missing from Rust source test declarations",
      "critical test category concurrency has no tests",
      "critical test category data-loss has no tests",
      "critical test category durable-state has no tests",
      "critical test category privacy has no tests",
      "critical test category provider-protocol has no tests",
    ]);
  });

  test("rejects malformed manifests before source verification", () => {
    const directory = mkdtempSync(join(tmpdir(), "noema-critical-tests-"));
    const path = join(directory, "manifest.json");
    try {
      writeFileSync(
        path,
        JSON.stringify({
          formatVersion: 1,
          tests: [
            {
              category: "security",
              path: "src/lib.rs",
              name: "not_a_crate_path",
            },
          ],
        }),
      );
      expect(() => loadCriticalTestManifest(path)).toThrow(
        "invalid Rust critical-test manifest",
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  test("rejects duplicate manifest entries", () => {
    const directory = mkdtempSync(join(tmpdir(), "noema-critical-tests-"));
    const path = join(directory, "manifest.json");
    const entry = {
      category: "security",
      path: "crates/example/src/lib.rs",
      name: "keeps_the_owner",
    };
    try {
      writeFileSync(
        path,
        JSON.stringify({ formatVersion: 1, tests: [entry, entry] }),
      );
      expect(() => loadCriticalTestManifest(path)).toThrow(
        "invalid Rust critical-test manifest",
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
});
