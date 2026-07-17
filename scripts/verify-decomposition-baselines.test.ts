import { describe, expect, test } from "bun:test";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  BASELINE_MANIFEST_NAME,
  type ArtifactPaths,
  baselineRootDisposition,
  captureBaselines,
  loadBaselineManifest,
  parseCliOptions,
  updateApprovedSqliteBootstrapBaseline,
  updateApprovedSqliteBaseline,
  verifyBaselines,
} from "./verify-decomposition-baselines";

function fixture() {
  const root = mkdtempSync(join(tmpdir(), "noema-decomposition-baseline-"));
  const paths: ArtifactPaths = {
    graphqlSdl: join(root, "schema.graphql"),
    graphqlTypes: join(root, "graphql.ts"),
    routeTree: join(root, "routeTree.gen.ts"),
    localModelCatalog: join(root, "catalog.toml"),
    runtimeAssets: join(root, "runtime-assets.json"),
    sqliteBootstrap: join(root, "bootstrap.sql"),
    sqliteSchemaShape: join(root, "sqlite-shape.txt"),
  };
  writeFileSync(paths.graphqlSdl, "type Query { version: String! }\n");
  writeFileSync(paths.graphqlTypes, "export type Query = { version: string };\n");
  writeFileSync(paths.routeTree, "export const routeTree = {};\n");
  writeFileSync(paths.localModelCatalog, "[[models]]\nid = \"model\"\n");
  writeFileSync(paths.runtimeAssets, '{"assets":[]}\n');
  writeFileSync(paths.sqliteBootstrap, "CREATE TABLE agents (id TEXT);\n");
  writeFileSync(paths.sqliteSchemaShape, "table:agents(id:text)\n");
  return {
    root,
    baselineRoot: join(root, "ignored-baseline"),
    paths,
    cleanup: () => rmSync(root, { recursive: true, force: true }),
  };
}

describe("decomposition preservation baselines", () => {
  test("ordinary CI skips only when no baseline root is configured", () => {
    expect(baselineRootDisposition(undefined, false)).toBe("skip");
    expect(baselineRootDisposition("target/baseline", false)).toBe("run");
    expect(() => baselineRootDisposition(undefined, true)).toThrow(
      "NOEMA_DECOMPOSITION_BASELINE_ROOT is required",
    );
  });

  test("parses an explicit command independently of option values", () => {
    const options = parseCliOptions(
      ["--graphql-sdl", "some-schema.graphql", "update-sqlite"],
      {},
    );
    expect(options.command).toBe("update-sqlite");
    expect(options.artifactPaths).toBeUndefined();
  });

  test("resolves the live default artifact paths when a baseline is configured", () => {
    const value = fixture();
    try {
      const options = parseCliOptions(
        [
          "verify",
          "--baseline-root",
          value.baselineRoot,
          "--sqlite-bootstrap",
          value.paths.sqliteBootstrap,
          "--sqlite-schema-shape",
          value.paths.sqliteSchemaShape,
        ],
        {},
      );
      expect(options.artifactPaths?.graphqlSdl).toMatch(
        /apps\/web\/src\/generated\/schema\.graphql$/,
      );
      expect(options.artifactPaths?.localModelCatalog).toMatch(
        /crates\/noema-providers\/resources\/local-models\/catalog\.toml$/,
      );
      expect(options.artifactPaths?.runtimeAssets).toMatch(
        /crates\/noema-providers\/resources\/local-models\/runtime-assets\.json$/,
      );
    } finally {
      value.cleanup();
    }
  });

  test("captures and verifies all seven canonical artifacts", () => {
    const value = fixture();
    try {
      captureBaselines(value.baselineRoot, value.paths);
      expect(
        verifyBaselines(
          loadBaselineManifest(value.baselineRoot),
          value.paths,
        ),
      ).toEqual([]);
      expect(() => captureBaselines(value.baselineRoot, value.paths)).toThrow(
        "baseline manifest already exists",
      );
    } finally {
      value.cleanup();
    }
  });

  test("reports deterministic drift for every preserved artifact", () => {
    const value = fixture();
    try {
      captureBaselines(value.baselineRoot, value.paths);
      const original = new Map(
        Object.values(value.paths).map((path) => [path, readFileSync(path)]),
      );
      for (const [key, path] of Object.entries(value.paths)) {
        writeFileSync(path, "drift\n");
        expect(
          verifyBaselines(
            loadBaselineManifest(value.baselineRoot),
            value.paths,
          )[0],
        ).toStartWith(`${key} drifted:`);
        writeFileSync(path, original.get(path)!);
      }
    } finally {
      value.cleanup();
    }
  });

  test("updates only bootstrap text at the schema-handshake checkpoint", () => {
    const value = fixture();
    try {
      captureBaselines(value.baselineRoot, value.paths);
      const before = loadBaselineManifest(value.baselineRoot);
      writeFileSync(
        value.paths.sqliteBootstrap,
        "BEGIN; CREATE TABLE agents (id TEXT); COMMIT;\n",
      );
      updateApprovedSqliteBootstrapBaseline(value.baselineRoot, value.paths);
      const after = loadBaselineManifest(value.baselineRoot);
      expect(after.hashes.graphqlSdl).toBe(before.hashes.graphqlSdl);
      expect(after.hashes.graphqlTypes).toBe(before.hashes.graphqlTypes);
      expect(after.hashes.routeTree).toBe(before.hashes.routeTree);
      expect(after.hashes.localModelCatalog).toBe(
        before.hashes.localModelCatalog,
      );
      expect(after.hashes.runtimeAssets).toBe(before.hashes.runtimeAssets);
      expect(after.hashes.sqliteSchemaShape).toBe(
        before.hashes.sqliteSchemaShape,
      );
      expect(after.hashes.sqliteBootstrap).not.toBe(
        before.hashes.sqliteBootstrap,
      );
      expect(verifyBaselines(after, value.paths)).toEqual([]);
    } finally {
      value.cleanup();
    }
  });

  test("updates both approved SQLite hashes after other artifacts verify", () => {
    const value = fixture();
    try {
      captureBaselines(value.baselineRoot, value.paths);
      const before = loadBaselineManifest(value.baselineRoot);
      writeFileSync(
        value.paths.sqliteBootstrap,
        "CREATE TABLE agents (id TEXT, key TEXT);\n",
      );
      writeFileSync(value.paths.sqliteSchemaShape, "table:agents(id:text,key:text)\n");
      updateApprovedSqliteBaseline(value.baselineRoot, value.paths);
      const after = loadBaselineManifest(value.baselineRoot);
      expect(after.hashes.graphqlSdl).toBe(before.hashes.graphqlSdl);
      expect(after.hashes.graphqlTypes).toBe(before.hashes.graphqlTypes);
      expect(after.hashes.routeTree).toBe(before.hashes.routeTree);
      expect(after.hashes.localModelCatalog).toBe(
        before.hashes.localModelCatalog,
      );
      expect(after.hashes.runtimeAssets).toBe(before.hashes.runtimeAssets);
      expect(after.hashes.sqliteBootstrap).not.toBe(
        before.hashes.sqliteBootstrap,
      );
      expect(after.hashes.sqliteSchemaShape).not.toBe(
        before.hashes.sqliteSchemaShape,
      );
      expect(verifyBaselines(after, value.paths)).toEqual([]);
    } finally {
      value.cleanup();
    }
  });

  test("refuses to bless SQLite when another baseline drifted", () => {
    const value = fixture();
    try {
      captureBaselines(value.baselineRoot, value.paths);
      writeFileSync(value.paths.graphqlSdl, "type Query { changed: Boolean! }\n");
      writeFileSync(value.paths.sqliteSchemaShape, "changed schema\n");
      expect(() =>
        updateApprovedSqliteBaseline(value.baselineRoot, value.paths),
      ).toThrow("refusing SQLite baseline update");
      expect(
        readFileSync(
          join(value.baselineRoot, BASELINE_MANIFEST_NAME),
          "utf8",
        ),
      ).toContain('"formatVersion": 1');
    } finally {
      value.cleanup();
    }
  });

  test("refuses checkpoint updates when the declared SQLite delta is absent", () => {
    const value = fixture();
    try {
      captureBaselines(value.baselineRoot, value.paths);
      expect(() =>
        updateApprovedSqliteBootstrapBaseline(value.baselineRoot, value.paths),
      ).toThrow("bootstrap text did not change");

      writeFileSync(value.paths.sqliteBootstrap, "CREATE TABLE changed (id TEXT);\n");
      expect(() =>
        updateApprovedSqliteBaseline(value.baselineRoot, value.paths),
      ).toThrow("schema shape did not change");
    } finally {
      value.cleanup();
    }
  });
});
