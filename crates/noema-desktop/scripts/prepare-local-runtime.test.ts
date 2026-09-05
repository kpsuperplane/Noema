import { describe, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { tmpdir } from "node:os";

import {
  runtimePlanIsPrepared,
  runtimePreparationPlan,
  goTarget,
  stageRuntimeAsset,
  type RuntimeManifest,
  type RuntimePreparationPlan
} from "./prepare-local-runtime";

const manifestPath = resolve(
  import.meta.dir,
  "../../noema-providers/resources/local-models/runtime-assets.json"
);
const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as RuntimeManifest;

function withSyntheticPackagedRoot(run: (root: string) => void) {
  const root = mkdtempSync(join(tmpdir(), "noema-desktop-runtime-"));
  try {
    run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function materializePlan(root: string, plan: RuntimePreparationPlan) {
  for (const [index, step] of plan.steps.entries()) {
    const extractRoot = join(root, ".synthetic-archives", index.toString(), "nested");
    mkdirSync(extractRoot, { recursive: true });
    if (step.asset.role === "server_bundle") {
      writeFileSync(join(extractRoot, plan.serverName), "synthetic server");
    } else {
      writeFileSync(
        join(extractRoot, "cudart64_12.dll"),
        "synthetic runtime library"
      );
    }
    writeFileSync(join(extractRoot, `${basename(step.asset.archive_name)}.README`), "ignored");
    stageRuntimeAsset(extractRoot, root, step, plan.serverName);
  }
}

describe("runtime preparation target mapper", () => {
  const targetTriples = [...new Set(manifest.assets.map((asset) => asset.target_triple))];

  for (const targetTriple of targetTriples) {
    test(`maps and validates ${targetTriple} beneath a packaged root`, () => {
      const plan = runtimePreparationPlan(manifest, targetTriple);
      const expectedAssets = manifest.assets.filter(
        (asset) => asset.target_triple === targetTriple
      );

      expect(plan.targetTriple).toBe(targetTriple);
      expect(plan.steps.map(({ asset }) => asset)).toEqual(expectedAssets);
      expect(plan.steps.map(({ asset, destinationBackend }) => [
        asset.backend,
        asset.role,
        destinationBackend
      ])).toEqual(
        expectedAssets.map((asset) => [asset.backend, asset.role, asset.backend])
      );

      withSyntheticPackagedRoot((packagedRoot) => {
        const targetRoot = join(packagedRoot, targetTriple);
        materializePlan(targetRoot, plan);
        expect(runtimePlanIsPrepared(targetRoot, plan)).toBe(true);

        for (const backend of new Set(
          expectedAssets
            .filter((asset) => asset.role === "server_bundle")
            .map((asset) => asset.backend)
        )) {
          const serverPath = join(targetRoot, backend, plan.serverName);
          rmSync(serverPath);
          expect(runtimePlanIsPrepared(targetRoot, plan)).toBe(false);
          writeFileSync(serverPath, "synthetic server");
        }
      });
    });
  }

  for (const asset of manifest.assets) {
    test(`preserves ${asset.target_triple}/${asset.backend}/${asset.role}`, () => {
      const plan = runtimePreparationPlan(manifest, asset.target_triple);
      const step = plan.steps.find(
        ({ asset: mappedAsset }) => mappedAsset.archive_name === asset.archive_name
      );

      expect(step).toBeDefined();
      expect(step?.asset.role).toBe(asset.role);
      expect(step?.destinationBackend).toBe(asset.backend);
    });
  }

  test("maps Windows CUDA server and cudart archives into one backend", () => {
    const plan = runtimePreparationPlan(manifest, "x86_64-pc-windows-msvc");
    const cudaSteps = plan.steps.filter(
      ({ destinationBackend }) => destinationBackend === "cuda"
    );

    expect(plan.serverName).toBe("llama-server.exe");
    expect(cudaSteps.map(({ asset }) => asset.role)).toEqual([
      "server_bundle",
      "runtime_libraries"
    ]);
    expect(
      cudaSteps.find(({ asset }) => asset.role === "runtime_libraries")?.asset.archive_name
    ).toStartWith("cudart-");

    withSyntheticPackagedRoot((packagedRoot) => {
      const targetRoot = join(packagedRoot, plan.targetTriple);
      materializePlan(targetRoot, plan);
      expect(existsSync(join(targetRoot, "cuda", "llama-server.exe"))).toBe(true);
      expect(
        existsSync(join(targetRoot, "cuda", "cudart64_12.dll"))
      ).toBe(true);
      expect(runtimePlanIsPrepared(targetRoot, plan)).toBe(true);
      rmSync(join(targetRoot, "cuda", "cudart64_12.dll"));
      expect(runtimePlanIsPrepared(targetRoot, plan)).toBe(false);
    });
  });

  test("prepares Linux ARM64 CPU and Vulkan runtimes", () => {
    const plan = runtimePreparationPlan(manifest, "aarch64-unknown-linux-gnu");
    expect(plan.steps.map(({ asset }) => asset.backend)).toEqual(["cpu", "vulkan"]);
  });
});

describe("Go server target mapper", () => {
  test("maps all release targets without CGo-specific state", () => {
    expect(goTarget("aarch64-apple-darwin")).toEqual({ goos: "darwin", goarch: "arm64" });
    expect(goTarget("x86_64-unknown-linux-gnu")).toEqual({ goos: "linux", goarch: "amd64" });
    expect(goTarget("x86_64-pc-windows-msvc")).toEqual({ goos: "windows", goarch: "amd64" });
    expect(() => goTarget("riscv64-unknown-linux-gnu")).toThrow(
      "No Go server target is available for riscv64-unknown-linux-gnu."
    );
  });
});

describe("Foundation bridge staging", () => {
  test.each([
    ["darwin", "arm64", "arm64"],
    ["darwin", "amd64", "x86_64"]
  ])("stages the %s/%s bridge beside the server", (goos, goarch, swiftArch) => {
    withSyntheticPackagedRoot((root) => {
      const fakeBin = join(root, "bin");
      const swiftBin = join(root, "swift-output");
      const stage = join(root, "stage");
      mkdirSync(fakeBin, { recursive: true });
      mkdirSync(swiftBin, { recursive: true });
      writeFileSync(join(swiftBin, "noema-foundation-bridge"), "bridge");
      const swift = join(fakeBin, "swift");
      writeFileSync(swift, `#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$FAKE_SWIFT_LOG"
if [[ " $* " == *" --show-bin-path "* ]]; then printf '%s\\n' "$FAKE_SWIFT_BIN"; fi
`);
      chmodSync(swift, 0o755);

      const result = Bun.spawnSync(
        [
          resolve(import.meta.dir, "../../../scripts/build-foundation-bridge"),
          goos,
          goarch,
          stage
        ],
        {
          env: {
            ...process.env,
            PATH: `${fakeBin}:${process.env.PATH ?? ""}`,
            FAKE_SWIFT_BIN: swiftBin,
            FAKE_SWIFT_LOG: join(root, "swift.log")
          }
        }
      );

      expect(result.exitCode).toBe(0);
      expect(readFileSync(join(stage, "noema-foundation-bridge"), "utf8")).toBe("bridge");
      expect(readFileSync(join(root, "swift.log"), "utf8")).toContain(`--arch ${swiftArch}`);
    });
  });

  test.each(["linux", "windows"])("skips Swift for %s", (goos) => {
    withSyntheticPackagedRoot((root) => {
      const stage = join(root, "stage");
      mkdirSync(stage);
      writeFileSync(join(stage, "noema-foundation-bridge"), "stale");
      const result = Bun.spawnSync(
        [
          resolve(import.meta.dir, "../../../scripts/build-foundation-bridge"),
          goos,
          "amd64",
          stage
        ],
        { env: { ...process.env, PATH: "/usr/bin:/bin" } }
      );

      expect(result.exitCode).toBe(0);
      expect(existsSync(join(stage, "noema-foundation-bridge"))).toBe(false);
    });
  });
});
