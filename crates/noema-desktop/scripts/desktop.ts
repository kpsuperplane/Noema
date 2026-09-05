import { resolve } from "node:path";

import {
  prepareGoServer,
  prepareLocalRuntime
} from "./prepare-local-runtime";

const mode = process.argv[2];
if (mode !== "build" && mode !== "dev") {
  throw new Error("Desktop preparation expects either `build` or `dev`.");
}

await prepareGoServer();
await prepareLocalRuntime();
const webRoot = resolve(import.meta.dir, "../../../apps/web");
const frontend = Bun.spawn(["bun", "run", mode === "build" ? "build:tauri" : "dev:tauri"], {
  cwd: webRoot,
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit"
});
process.exit(await frontend.exited);
