import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { basename, join, resolve } from "node:path";

export type Backend = "metal" | "cuda" | "vulkan" | "cpu";
export type RuntimeAsset = {
  target_triple: string;
  backend: Backend;
  archive_name: string;
  sha256: string;
  role: "server_bundle" | "runtime_libraries";
};
export type RuntimeManifest = {
  release_tag: string;
  commit: string;
  assets: RuntimeAsset[];
};

export type RuntimePreparationStep = {
  asset: RuntimeAsset;
  destinationBackend: Backend;
};

export type RuntimePreparationPlan = {
  targetTriple: string;
  serverName: string;
  steps: RuntimePreparationStep[];
};

export type PrepareLocalRuntimeOptions = {
  targetTriple?: string;
};

export async function prepareGoServer(options: PrepareLocalRuntimeOptions = {}) {
  const targetTriple = options.targetTriple ?? requestedTargetTriple();
  const target = goTarget(targetTriple);
  const executable = target.goos === "windows" ? "noema-server.exe" : "noema-server";
  const destination = join(desktopRoot, "binaries", executable);
  rmSync(join(desktopRoot, "binaries", target.goos === "windows" ? "noema-server" : "noema-server.exe"), { force: true });
  const command = ["go", "build", "-trimpath", "-buildvcs=false"];
  if (target.goos === "windows") {
    command.push("-ldflags=-H=windowsgui");
  }
  command.push("-o", destination, "./cmd/noema");
  const build = Bun.spawn(command, {
    cwd: workspaceRoot,
    env: { ...process.env, CGO_ENABLED: "0", GOOS: target.goos, GOARCH: target.goarch },
    stdout: "inherit",
    stderr: "inherit"
  });
  if (await build.exited !== 0) {
    throw new Error(`Could not build the Go server for ${targetTriple}.`);
  }
  if (target.goos !== "windows") {
    chmodSync(destination, 0o755);
  }
}

export function goTarget(targetTriple: string) {
  const targets: Record<string, { goos: string; goarch: string }> = {
    "aarch64-apple-darwin": { goos: "darwin", goarch: "arm64" },
    "x86_64-apple-darwin": { goos: "darwin", goarch: "amd64" },
    "aarch64-unknown-linux-gnu": { goos: "linux", goarch: "arm64" },
    "x86_64-unknown-linux-gnu": { goos: "linux", goarch: "amd64" },
    "x86_64-pc-windows-msvc": { goos: "windows", goarch: "amd64" }
  };
  const target = targets[targetTriple];
  if (!target) {
    throw new Error(`No Go server target is available for ${targetTriple}.`);
  }
  return target;
}

const desktopRoot = resolve(import.meta.dir, "..");
const workspaceRoot = resolve(desktopRoot, "../..");
const manifestPath = resolve(
  desktopRoot,
  "../noema-providers/resources/local-models/runtime-assets.json"
);
const runtimeRoot = resolve(desktopRoot, "binaries/runtime");
const cacheRoot = resolve(workspaceRoot, "target/noema-local-runtime-cache");
const releaseBaseUrl = "https://github.com/ggml-org/llama.cpp/releases/download";

export async function prepareLocalRuntime(options: PrepareLocalRuntimeOptions = {}) {
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as RuntimeManifest;
  const plan = runtimePreparationPlan(
    manifest,
    options.targetTriple ?? requestedTargetTriple()
  );
  const assets = plan.steps.map(({ asset }) => asset);

  const targetRoot = join(runtimeRoot, plan.targetTriple);
  const expectedStamp = JSON.stringify(
    { release_tag: manifest.release_tag, commit: manifest.commit, assets },
    null,
    2
  );
  const stampPath = join(targetRoot, ".manifest.json");
  if (
    existsSync(stampPath) &&
    readFileSync(stampPath, "utf8") === expectedStamp &&
    runtimePlanIsPrepared(targetRoot, plan)
  ) {
    return;
  }

  mkdirSync(runtimeRoot, { recursive: true });
  mkdirSync(cacheRoot, { recursive: true });
  const temporaryTarget = join(runtimeRoot, `.${plan.targetTriple}.tmp-${process.pid}`);
  rmSync(temporaryTarget, { recursive: true, force: true });
  mkdirSync(temporaryTarget, { recursive: true });

  try {
    for (const [index, step] of plan.steps.entries()) {
      const { asset } = step;
      const archivePath = await verifiedArchive(manifest.release_tag, asset);
      const extractRoot = join(cacheRoot, `extract-${process.pid}-${index}`);
      rmSync(extractRoot, { recursive: true, force: true });
      mkdirSync(extractRoot, { recursive: true });
      try {
        const extraction = Bun.spawnSync(["tar", "-xf", archivePath, "-C", extractRoot], {
          stdout: "inherit",
          stderr: "inherit"
        });
        if (extraction.exitCode !== 0) {
          throw new Error(`Could not extract ${asset.archive_name}.`);
        }
        stageRuntimeAsset(extractRoot, temporaryTarget, step, plan.serverName);
      } finally {
        rmSync(extractRoot, { recursive: true, force: true });
      }
    }

    if (!runtimePlanIsPrepared(temporaryTarget, plan)) {
      throw new Error(`Pinned llama.cpp archives did not contain every required runtime file.`);
    }
    writeFileSync(join(temporaryTarget, ".manifest.json"), expectedStamp);
    rmSync(targetRoot, { recursive: true, force: true });
    renameSync(temporaryTarget, targetRoot);
  } catch (error) {
    rmSync(temporaryTarget, { recursive: true, force: true });
    throw error;
  }
}

export function runtimePreparationPlan(
  manifest: RuntimeManifest,
  targetTriple: string
): RuntimePreparationPlan {
  const assets = manifest.assets.filter((asset) => asset.target_triple === targetTriple);
  if (assets.length === 0) {
    throw new Error(`No pinned llama.cpp runtime is available for ${targetTriple}.`);
  }
  return {
    targetTriple,
    serverName: serverNameForTarget(targetTriple),
    steps: assets.map((asset) => ({
      asset,
      destinationBackend: asset.backend
    }))
  };
}

async function verifiedArchive(releaseTag: string, asset: RuntimeAsset) {
  const archivePath = join(cacheRoot, asset.archive_name);
  if (!existsSync(archivePath) || hashFile(archivePath) !== asset.sha256) {
    rmSync(archivePath, { force: true });
    const response = await fetch(`${releaseBaseUrl}/${releaseTag}/${asset.archive_name}`);
    if (!response.ok) {
      throw new Error(`Could not download ${asset.archive_name}: HTTP ${response.status}.`);
    }
    await Bun.write(archivePath, await response.arrayBuffer());
  }
  const actual = hashFile(archivePath);
  if (actual !== asset.sha256) {
    rmSync(archivePath, { force: true });
    throw new Error(
      `Checksum mismatch for ${asset.archive_name}: expected ${asset.sha256}, got ${actual}.`
    );
  }
  return archivePath;
}

function hashFile(path: string) {
  return new Bun.CryptoHasher("sha256").update(readFileSync(path)).digest("hex");
}

export function copyRuntimeFiles(
  sourceRoot: string,
  destinationRoot: string,
  serverName: string
) {
  mkdirSync(destinationRoot, { recursive: true });
  const copiedFiles: string[] = [];
  for (const source of recursiveFiles(sourceRoot)) {
    const name = basename(source);
    if (!isRuntimeFile(name, serverName)) {
      continue;
    }
    const destination = join(destinationRoot, name);
    copyFileSync(source, destination);
    copiedFiles.push(name);
    if (name.toLowerCase() === serverName.toLowerCase()) {
      chmodSync(destination, 0o755);
    }
  }
  return copiedFiles;
}

export function stageRuntimeAsset(
  sourceRoot: string,
  targetRoot: string,
  step: RuntimePreparationStep,
  serverName: string
) {
  const backendRoot = join(targetRoot, step.destinationBackend);
  const files = copyRuntimeFiles(sourceRoot, backendRoot, serverName);
  writeFileSync(
    join(backendRoot, assetStampName(step.asset)),
    JSON.stringify(
      {
        archive_name: step.asset.archive_name,
        sha256: step.asset.sha256,
        files
      },
      null,
      2
    )
  );
}

function recursiveFiles(root: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(root)) {
    const path = join(root, entry);
    if (statSync(path).isDirectory()) {
      files.push(...recursiveFiles(path));
    } else {
      files.push(path);
    }
  }
  return files;
}

function isRuntimeFile(name: string, serverName: string) {
  const lower = name.toLowerCase();
  return (
    lower === serverName.toLowerCase() ||
    lower === "license" ||
    lower.endsWith(".dll") ||
    lower.endsWith(".dylib") ||
    lower.includes(".so")
  );
}

export function runtimePlanIsPrepared(targetRoot: string, plan: RuntimePreparationPlan) {
  return plan.steps.every((step) => {
    const backendRoot = join(targetRoot, step.destinationBackend);
    const stampPath = join(backendRoot, assetStampName(step.asset));
    if (!existsSync(stampPath)) {
      return false;
    }
    try {
      const stamp = JSON.parse(readFileSync(stampPath, "utf8")) as {
        archive_name?: string;
        sha256?: string;
        files?: string[];
      };
      if (
        stamp.archive_name !== step.asset.archive_name ||
        stamp.sha256 !== step.asset.sha256 ||
        !Array.isArray(stamp.files) ||
        !stamp.files.every(
          (file) => basename(file) === file && existsSync(join(backendRoot, file))
        )
      ) {
        return false;
      }
      if (step.asset.role === "server_bundle") {
        return stamp.files.includes(plan.serverName);
      }
      return stamp.files.some(isRuntimeLibraryFile);
    } catch {
      return false;
    }
  });
}

function assetStampName(asset: RuntimeAsset) {
  return `.asset-${asset.sha256}.json`;
}

function isRuntimeLibraryFile(name: string) {
  const lower = name.toLowerCase();
  return lower.endsWith(".dll") || lower.endsWith(".dylib") || lower.includes(".so");
}

function serverNameForTarget(targetTriple: string) {
  return targetTriple.includes("-windows-") ? "llama-server.exe" : "llama-server";
}

export function requestedTargetTriple() {
  const explicit =
    process.env.NOEMA_DESKTOP_TARGET ??
    process.env.TAURI_ENV_TARGET_TRIPLE ??
    process.env.TARGET;
  if (explicit) {
    return explicit;
  }
  if (process.platform === "darwin" && process.arch === "arm64") {
    return "aarch64-apple-darwin";
  }
  if (process.platform === "darwin" && process.arch === "x64") {
    return "x86_64-apple-darwin";
  }
  if (process.platform === "win32" && process.arch === "x64") {
    return "x86_64-pc-windows-msvc";
  }
  if (process.platform === "linux" && process.arch === "x64") {
    return "x86_64-unknown-linux-gnu";
  }
  if (process.platform === "linux" && process.arch === "arm64") {
    return "aarch64-unknown-linux-gnu";
  }
  throw new Error(`Noema V1 does not package llama.cpp for ${process.platform}/${process.arch}.`);
}

if (import.meta.main) {
  await prepareLocalRuntime();
}
