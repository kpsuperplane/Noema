import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

function run(source: string, output = "") {
  const result = spawnSync(process.execPath, ["-e", source], {
    cwd: import.meta.dirname,
    env: { ...process.env, NOEMA_DEV_ASSET_DIR: output },
    encoding: "utf8",
  });
  expect(result.stderr).toBe("");
  expect(result.status).toBe(0);
  return result.stdout.trim();
}

const printOutput = 'const {assetOutDir} = await import("./web-assets.ts"); console.log(assetOutDir);';

test("normal builds use the development launcher destination", () => {
  expect(run(printOutput)).toBe(
    process.platform === "linux" && process.getuid?.() === 0
      ? "/run/noema-dev/web-assets"
      : path.resolve(import.meta.dirname, "../../target/web-assets"),
  );
  expect(run(`process.getuid = () => 1000; ${printOutput}`)).toBe(
    path.resolve(import.meta.dirname, "../../target/web-assets"),
  );
});

test("release packaging explicitly selects its staging source", () => {
  const output = path.resolve(import.meta.dirname, "../../target/web-assets");
  expect(run(printOutput, output)).toBe(output);
});

test("builds make private output readable by the development server", () => {
  const output = mkdtempSync(path.join(tmpdir(), "noema-web-assets-"));
  try {
    writeFileSync(path.join(output, "index.html"), "app", { mode: 0o600 });
    writeFileSync(path.join(output, "graphiql.html"), "graphiql", { mode: 0o600 });
    run('const {serviceReadableAssets} = await import("./web-assets.ts"); await serviceReadableAssets.closeBundle.handler();', output);
    expect(statSync(output).mode & 0o777).toBe(0o755);
    for (const name of ["index.html", "graphiql.html"]) {
      expect(statSync(path.join(output, name)).mode & 0o777).toBe(0o644);
    }
  } finally {
    rmSync(output, { recursive: true, force: true });
  }
});
