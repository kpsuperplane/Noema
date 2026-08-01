import { stat } from "node:fs/promises";

const schemaPath = "../../graphql/schema.graphql";
const generatedPaths = [schemaPath, "src/generated", "src/routeTree.gen.ts"];
const strayApiWebPath = "../../crates/noema-api/web";

async function pathExists(path: string): Promise<boolean> {
  try {
    await stat(path);
    return true;
  } catch (error) {
    if (
      error instanceof Error &&
      "code" in error &&
      error.code === "ENOENT"
    ) {
      return false;
    }
    throw error;
  }
}

if (!(await pathExists(schemaPath))) {
  throw new Error(
    `${schemaPath} is missing; gen:schema must export the shared client schema`,
  );
}

if (await pathExists(strayApiWebPath)) {
  throw new Error(
    `${strayApiWebPath} must not exist; the API exporter writes only to the caller's explicit output`,
  );
}

const result = Bun.spawnSync(
  [
    "git",
    "status",
    "--porcelain=v1",
    "--untracked-files=all",
    "--",
    ...generatedPaths,
  ],
  { stdout: "pipe", stderr: "inherit" },
);

if (result.exitCode !== 0) {
  throw new Error(`git status failed with exit code ${result.exitCode}`);
}

const drift = new TextDecoder().decode(result.stdout).trim();
if (drift) {
  console.error("generated frontend artifacts are not committed exactly:");
  console.error(drift);
  process.exit(1);
}

console.log("generated frontend artifacts are clean");
