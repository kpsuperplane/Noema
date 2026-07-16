import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { resolve } from "node:path";

export const BASELINE_MANIFEST_NAME = "decomposition-baselines.json";

const ARTIFACT_KEYS = [
  "graphqlSdl",
  "graphqlTypes",
  "routeTree",
  "localModelCatalog",
  "runtimeAssets",
  "sqliteBootstrap",
  "sqliteSchemaShape",
] as const;

export type ArtifactKey = (typeof ARTIFACT_KEYS)[number];
export type ArtifactPaths = Record<ArtifactKey, string>;

export type BaselineManifest = {
  formatVersion: 1;
  hashAlgorithm: "sha256";
  hashes: Record<ArtifactKey, string>;
};

export function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function createBaselineManifest(paths: ArtifactPaths): BaselineManifest {
  return {
    formatVersion: 1,
    hashAlgorithm: "sha256",
    hashes: Object.fromEntries(
      ARTIFACT_KEYS.map((key) => [key, sha256File(paths[key])]),
    ) as Record<ArtifactKey, string>,
  };
}

function baselineManifestPath(root: string): string {
  return resolve(root, BASELINE_MANIFEST_NAME);
}

function writeManifest(root: string, manifest: BaselineManifest): void {
  mkdirSync(root, { recursive: true });
  writeFileSync(
    baselineManifestPath(root),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

export function captureBaselines(root: string, paths: ArtifactPaths): void {
  const manifestPath = baselineManifestPath(root);
  if (existsSync(manifestPath)) {
    throw new Error(
      `baseline manifest already exists at ${manifestPath}; remove it explicitly to recapture`,
    );
  }
  writeManifest(root, createBaselineManifest(paths));
}

export function loadBaselineManifest(root: string): BaselineManifest {
  const manifestPath = baselineManifestPath(root);
  if (!existsSync(manifestPath)) {
    throw new Error(`baseline manifest does not exist at ${manifestPath}`);
  }

  const value = JSON.parse(readFileSync(manifestPath, "utf8")) as Partial<BaselineManifest>;
  if (
    value.formatVersion !== 1 ||
    value.hashAlgorithm !== "sha256" ||
    !value.hashes ||
    ARTIFACT_KEYS.some(
      (key) => !/^[a-f0-9]{64}$/.test(value.hashes?.[key] ?? ""),
    )
  ) {
    throw new Error(`invalid decomposition baseline manifest at ${manifestPath}`);
  }
  return value as BaselineManifest;
}

export function verifyBaselines(
  manifest: BaselineManifest,
  paths: ArtifactPaths,
  keys: readonly ArtifactKey[] = ARTIFACT_KEYS,
): string[] {
  const errors: string[] = [];
  for (const key of keys) {
    const actual = sha256File(paths[key]);
    const expected = manifest.hashes[key];
    if (actual !== expected) {
      errors.push(
        `${key} drifted: expected ${expected}, got ${actual} (${paths[key]})`,
      );
    }
  }
  return errors.sort();
}

export function updateApprovedSqliteBaseline(
  root: string,
  paths: ArtifactPaths,
): void {
  const manifest = loadBaselineManifest(root);
  const unchangedKeys = ARTIFACT_KEYS.filter(
    (key): key is Exclude<ArtifactKey, "sqliteBootstrap" | "sqliteSchemaShape"> =>
      key !== "sqliteBootstrap" && key !== "sqliteSchemaShape",
  );
  const errors = verifyBaselines(manifest, paths, unchangedKeys);
  if (errors.length > 0) {
    throw new Error(
      `refusing SQLite baseline update because preserved artifacts drifted:\n${errors.join("\n")}`,
    );
  }
  const sqliteBootstrap = sha256File(paths.sqliteBootstrap);
  const sqliteSchemaShape = sha256File(paths.sqliteSchemaShape);
  if (sqliteBootstrap === manifest.hashes.sqliteBootstrap) {
    throw new Error("refusing SQLite baseline update because bootstrap text did not change");
  }
  if (sqliteSchemaShape === manifest.hashes.sqliteSchemaShape) {
    throw new Error("refusing SQLite baseline update because schema shape did not change");
  }
  manifest.hashes.sqliteBootstrap = sqliteBootstrap;
  manifest.hashes.sqliteSchemaShape = sqliteSchemaShape;
  writeManifest(root, manifest);
}

export function updateApprovedSqliteBootstrapBaseline(
  root: string,
  paths: ArtifactPaths,
): void {
  const manifest = loadBaselineManifest(root);
  const unchangedKeys = ARTIFACT_KEYS.filter(
    (key): key is Exclude<ArtifactKey, "sqliteBootstrap"> =>
      key !== "sqliteBootstrap",
  );
  const errors = verifyBaselines(manifest, paths, unchangedKeys);
  if (errors.length > 0) {
    throw new Error(
      `refusing SQLite bootstrap baseline update because preserved artifacts drifted:\n${errors.join("\n")}`,
    );
  }
  const sqliteBootstrap = sha256File(paths.sqliteBootstrap);
  if (sqliteBootstrap === manifest.hashes.sqliteBootstrap) {
    throw new Error(
      "refusing SQLite bootstrap baseline update because bootstrap text did not change",
    );
  }
  manifest.hashes.sqliteBootstrap = sqliteBootstrap;
  writeManifest(root, manifest);
}

export function baselineRootDisposition(
  root: string | undefined,
  finalVerification: boolean,
): "run" | "skip" {
  if (root) {
    return "run";
  }
  if (finalVerification) {
    throw new Error(
      "NOEMA_DECOMPOSITION_BASELINE_ROOT is required for final decomposition verification",
    );
  }
  return "skip";
}

type Command = "capture" | "verify" | "update-bootstrap" | "update-sqlite";
type CliOptions = {
  command: Command;
  baselineRoot?: string;
  finalVerification: boolean;
  artifactPaths?: ArtifactPaths;
};

function argumentValue(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function firstExisting(explicit: string | undefined, candidates: string[]): string {
  if (explicit) return resolve(explicit);
  const candidate = candidates.map((path) => resolve(path)).find(existsSync);
  if (!candidate) {
    throw new Error(`none of the expected artifact paths exist: ${candidates.join(", ")}`);
  }
  return candidate;
}

export function parseCliOptions(
  args: string[],
  env: Record<string, string | undefined>,
): CliOptions {
  const commandToken = args.find(
    (arg) =>
      arg === "capture" ||
      arg === "verify" ||
      arg === "update-bootstrap" ||
      arg === "update-sqlite",
  );
  const command: Command =
    commandToken === "capture" ||
    commandToken === "verify" ||
    commandToken === "update-bootstrap" ||
    commandToken === "update-sqlite"
      ? commandToken
      : "verify";
  const baselineRoot =
    argumentValue(args, "--baseline-root") ??
    env.NOEMA_DECOMPOSITION_BASELINE_ROOT;
  const finalVerification =
    args.includes("--final") ||
    /^(1|true|yes)$/i.test(env.NOEMA_DECOMPOSITION_FINAL_VERIFY ?? "");

  if (baselineRootDisposition(baselineRoot, finalVerification) === "skip") {
    return { command, baselineRoot, finalVerification };
  }

  const sqliteBootstrap =
    argumentValue(args, "--sqlite-bootstrap") ??
    env.NOEMA_DECOMPOSITION_SQLITE_BOOTSTRAP;
  const sqliteSchemaShape =
    argumentValue(args, "--sqlite-schema-shape") ??
    env.NOEMA_DECOMPOSITION_SQLITE_SCHEMA_SHAPE;
  if (!sqliteBootstrap || !sqliteSchemaShape) {
    throw new Error(
      "--sqlite-bootstrap/NOEMA_DECOMPOSITION_SQLITE_BOOTSTRAP and --sqlite-schema-shape/NOEMA_DECOMPOSITION_SQLITE_SCHEMA_SHAPE are required when a baseline root is configured",
    );
  }

  return {
    command,
    baselineRoot: resolve(baselineRoot!),
    finalVerification,
    artifactPaths: {
      graphqlSdl: firstExisting(
        argumentValue(args, "--graphql-sdl") ??
          env.NOEMA_DECOMPOSITION_GRAPHQL_SDL,
        [
          "apps/web/src/generated/schema.graphql",
          "crates/noema-core/web/src/generated/schema.graphql",
        ],
      ),
      graphqlTypes: firstExisting(
        argumentValue(args, "--graphql-types") ??
          env.NOEMA_DECOMPOSITION_GRAPHQL_TYPES,
        [
          "apps/web/src/generated/graphql.ts",
          "crates/noema-core/web/src/generated/graphql.ts",
        ],
      ),
      routeTree: firstExisting(
        argumentValue(args, "--route-tree") ??
          env.NOEMA_DECOMPOSITION_ROUTE_TREE,
        [
          "apps/web/src/routeTree.gen.ts",
          "crates/noema-core/web/src/routeTree.gen.ts",
        ],
      ),
      localModelCatalog: firstExisting(
        argumentValue(args, "--local-model-catalog") ??
          env.NOEMA_DECOMPOSITION_LOCAL_MODEL_CATALOG,
        [
          "crates/noema-providers/resources/local-models/catalog.toml",
          "crates/noema-core/resources/local-models/catalog.toml",
        ],
      ),
      runtimeAssets: firstExisting(
        argumentValue(args, "--runtime-assets") ??
          env.NOEMA_DECOMPOSITION_RUNTIME_ASSETS,
        [
          "crates/noema-providers/resources/local-models/runtime-assets.json",
          "crates/noema-core/resources/local-models/runtime-assets.json",
        ],
      ),
      sqliteBootstrap: resolve(sqliteBootstrap),
      sqliteSchemaShape: resolve(sqliteSchemaShape),
    },
  };
}

if (import.meta.main) {
  try {
    const options = parseCliOptions(Bun.argv.slice(2), process.env);
    if (!options.baselineRoot) {
      console.log(
        "decomposition baseline verification skipped: NOEMA_DECOMPOSITION_BASELINE_ROOT is not configured",
      );
      process.exit(0);
    }

    const paths = options.artifactPaths!;
    if (options.command === "capture") {
      captureBaselines(options.baselineRoot, paths);
      console.log(`captured decomposition baselines in ${options.baselineRoot}`);
    } else if (options.command === "update-bootstrap") {
      updateApprovedSqliteBootstrapBaseline(options.baselineRoot, paths);
      console.log("updated the approved SQLite bootstrap-text baseline");
    } else if (options.command === "update-sqlite") {
      updateApprovedSqliteBaseline(options.baselineRoot, paths);
      console.log("updated the approved SQLite bootstrap-text and schema-shape baselines");
    } else {
      const errors = verifyBaselines(
        loadBaselineManifest(options.baselineRoot),
        paths,
      );
      if (errors.length > 0) {
        throw new Error(`decomposition baseline violations:\n${errors.join("\n")}`);
      }
      console.log("decomposition preservation baselines are valid");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
