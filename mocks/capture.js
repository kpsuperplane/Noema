const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const { spawn } = require("node:child_process");

const root = __dirname;
const outputDir = path.join(root, "screenshots", "chat-first");
const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const screens = [
  "first-chat",
  "memory-event",
  "memory-expanded",
  "threads",
  "work-reveal",
  "workspace",
  "memory-settings",
  "advanced-admin",
];
const args = process.argv.slice(2);
const mobile = args.includes("--mobile");
const screenArg = args.find((arg) => arg.startsWith("--screens="));
const captureScreens = screenArg ? screenArg.slice("--screens=".length).split(",") : screens;
const viewport = mobile
  ? { width: 390, height: 900, prefix: "mobile-" }
  : { width: 1440, height: 1000, prefix: "" };

if (!fs.existsSync(chrome)) {
  console.error(`Chrome was not found at ${chrome}`);
  process.exit(1);
}

fs.mkdirSync(outputDir, { recursive: true });

function capture(screen) {
  return new Promise((resolve, reject) => {
    const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "noema-mocks-chrome-"));
    const fileUrl = `file://${path.join(root, "index.html")}?screen=${screen}&capture=1`;
    const output = path.join(outputDir, `${viewport.prefix}${screen}.png`);
    let stderr = "";
    let settled = false;

    fs.rmSync(output, { force: true });

    const child = spawn(chrome, [
      "--headless=new",
      "--disable-gpu",
      "--disable-background-networking",
      "--disable-component-update",
      "--hide-scrollbars",
      "--no-default-browser-check",
      "--no-first-run",
      "--run-all-compositor-stages-before-draw",
      "--virtual-time-budget=1200",
      `--window-size=${viewport.width},${viewport.height}`,
      `--user-data-dir=${userDataDir}`,
      `--screenshot=${output}`,
      fileUrl,
    ]);

    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearInterval(interval);
      clearTimeout(timeout);
      if (child.exitCode === null) child.kill("SIGTERM");
      try {
        fs.rmSync(userDataDir, { recursive: true, force: true });
      } catch {
        // Chrome can still be releasing profile files after the screenshot lands.
      }
      if (error) {
        reject(error);
        return;
      }
      console.log(`Captured ${path.relative(process.cwd(), output)}`);
      resolve();
    };

    const hasImage = () => fs.existsSync(output) && fs.statSync(output).size > 10000;
    const interval = setInterval(() => {
      if (hasImage()) finish();
    }, 250);
    const timeout = setTimeout(() => {
      finish(new Error(stderr || `Timed out capturing ${screen}`));
    }, 15000);

    child.on("exit", (code) => {
      if (hasImage()) {
        finish();
        return;
      }
      finish(new Error(stderr || `Chrome exited with code ${code} while capturing ${screen}`));
    });
  });
}

(async () => {
  for (const screen of captureScreens) {
    await capture(screen);
  }
})().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
