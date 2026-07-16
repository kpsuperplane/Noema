const generatedPaths = ["src/generated", "src/routeTree.gen.ts"];

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
