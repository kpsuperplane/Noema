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

type Backend = "metal" | "cuda" | "vulkan" | "cpu";
type RuntimeAsset = {
  target_triple: string;
  backend: Backend;
  archive_name: string;
  sha256: string;
  role: "server_bundle" | "runtime_libraries";
};
type RuntimeManifest = {
  release_tag: string;
  commit: string;
  assets: RuntimeAsset[];
};

const desktopRoot = resolve(import.meta.dir, "..");
const workspaceRoot = resolve(desktopRoot, "../..");
const manifestPath = resolve(
  desktopRoot,
  "../noema-core/resources/local-models/runtime-assets.json"
);
const runtimeRoot = resolve(desktopRoot, "binaries/runtime");
const cacheRoot = resolve(workspaceRoot, "target/noema-local-runtime-cache");
const releaseBaseUrl = "https://github.com/ggml-org/llama.cpp/releases/download";

export async function prepareLocalRuntime() {
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as RuntimeManifest;
  const targetTriple = requestedTargetTriple();
  const assets = manifest.assets.filter((asset) => asset.target_triple === targetTriple);
  if (assets.length === 0) {
    throw new Error(`No pinned llama.cpp runtime is available for ${targetTriple}.`);
  }

  const targetRoot = join(runtimeRoot, targetTriple);
  const expectedStamp = JSON.stringify(
    { release_tag: manifest.release_tag, commit: manifest.commit, assets },
    null,
    2
  );
  const stampPath = join(targetRoot, ".manifest.json");
  if (
    existsSync(stampPath) &&
    readFileSync(stampPath, "utf8") === expectedStamp &&
    serverBundlesExist(targetRoot, assets)
  ) {
    return;
  }

  mkdirSync(runtimeRoot, { recursive: true });
  mkdirSync(cacheRoot, { recursive: true });
  const temporaryTarget = join(runtimeRoot, `.${targetTriple}.tmp-${process.pid}`);
  rmSync(temporaryTarget, { recursive: true, force: true });
  mkdirSync(temporaryTarget, { recursive: true });

  try {
    for (const [index, asset] of assets.entries()) {
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
        copyRuntimeFiles(extractRoot, join(temporaryTarget, asset.backend));
      } finally {
        rmSync(extractRoot, { recursive: true, force: true });
      }
    }

    if (!serverBundlesExist(temporaryTarget, assets)) {
      throw new Error(`Pinned llama.cpp archives did not contain every required llama-server.`);
    }
    writeFileSync(join(temporaryTarget, ".manifest.json"), expectedStamp);
    rmSync(targetRoot, { recursive: true, force: true });
    renameSync(temporaryTarget, targetRoot);
  } catch (error) {
    rmSync(temporaryTarget, { recursive: true, force: true });
    throw error;
  }
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

function copyRuntimeFiles(sourceRoot: string, destinationRoot: string) {
  mkdirSync(destinationRoot, { recursive: true });
  for (const source of recursiveFiles(sourceRoot)) {
    const name = basename(source);
    if (!isRuntimeFile(name)) {
      continue;
    }
    const destination = join(destinationRoot, name);
    copyFileSync(source, destination);
    if (name === serverName()) {
      chmodSync(destination, 0o755);
    }
  }
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

function isRuntimeFile(name: string) {
  const lower = name.toLowerCase();
  return (
    lower === serverName() ||
    lower === "license" ||
    lower.endsWith(".dll") ||
    lower.endsWith(".dylib") ||
    lower.includes(".so")
  );
}

function serverBundlesExist(targetRoot: string, assets: RuntimeAsset[]) {
  const backends = new Set(
    assets.filter((asset) => asset.role === "server_bundle").map((asset) => asset.backend)
  );
  return [...backends].every((backend) => existsSync(join(targetRoot, backend, serverName())));
}

function serverName() {
  return process.platform === "win32" ? "llama-server.exe" : "llama-server";
}

function requestedTargetTriple() {
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
  throw new Error(`Noema V1 does not package llama.cpp for ${process.platform}/${process.arch}.`);
}

if (import.meta.main) {
  await prepareLocalRuntime();
}
