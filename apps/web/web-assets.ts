import path from "node:path";
import { chmod, readdir } from "node:fs/promises";

// Match the Linux root development launcher, including builds outside its shell.
export const assetOutDir = process.env.NOEMA_DEV_ASSET_DIR || (
  process.platform === "linux" && process.getuid?.() === 0
    ? "/run/noema-dev/web-assets"
    : path.resolve(import.meta.dirname, "../../target/web-assets")
);

// The development server runs as another user. Both app and GraphiQL builds
// must remain readable even when the build shell uses a private umask.
export const serviceReadableAssets = {
  name: "service-readable-assets",
  apply: "build" as const,
  closeBundle: {
    order: "post" as const,
    async handler() {
      await chmod(assetOutDir, 0o755);
      const entries = await readdir(assetOutDir, { withFileTypes: true });
      await Promise.all(entries.filter((entry) => entry.isFile())
        .map((entry) => chmod(path.join(assetOutDir, entry.name), 0o644)));
    }
  }
};
