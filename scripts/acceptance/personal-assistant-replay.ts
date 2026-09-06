#!/usr/bin/env bun

import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

type AssessmentCase = { number: number; task: string; status: string; gap: string };

const ROOT = resolve(import.meta.dir, "../..");
const REPORT = resolve(ROOT, "docs/difficult-digital-personal-assistant-tasks.md");
const MANIFEST = resolve(ROOT, "docs/validation/evidence/personal-assistant-replay/manifest.json");
const PREFIX = "PA-REPLAY-20260906";

function usage(): never {
  console.error(`Usage: bun run scripts/acceptance/personal-assistant-replay.ts [--execute] [--case N] [--limit N] [--socket PATH]\n\nChecks the committed 100-case replay pack against the current report and existing noema-dev home.\nExecution is opt-in and sends synthetic, read-only prompts through the normal live runner.`);
  process.exit(2);
}

function assessmentCases(): AssessmentCase[] {
  if (!existsSync(REPORT)) throw new Error(`missing report: ${REPORT}`);
  const rows: AssessmentCase[] = [];
  let inAssessment = false;
  const report = readFileSync(REPORT, "utf8");
  for (const line of report.split("\n")) {
    if (line.startsWith("| # | Task | Status | Noema gap or improvement |")) inAssessment = true;
    if (!inAssessment) continue;
    const match = line.match(/^\|\s*(\d+)\s*\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|\s*(.*?)\s*\|\s*$/);
    if (!match) continue;
    rows.push({ number: Number(match[1]), task: match[2], status: match[3], gap: match[4] });
  }
  const unique = new Map(rows.map((item) => [item.number, item]));
  return [...unique.values()].sort((a, b) => a.number - b.number);
}

async function main() {
  const args = Bun.argv.slice(2);
  if (args.includes("--help")) usage();
  const manifest = JSON.parse(await Bun.file(MANIFEST).text());
  const cases = assessmentCases();
  if (manifest.cases.count !== 100 || cases.length !== 100) {
    throw new Error(`replay pack must contain 100 cases (manifest=${manifest.cases.count}, report=${cases.length})`);
  }
  const selected = args.includes("--case")
    ? cases.filter((item) => item.number === Number(args[args.indexOf("--case") + 1]))
    : cases.slice(0, args.includes("--limit") ? Number(args[args.indexOf("--limit") + 1]) : cases.length);
  if (!selected.length) throw new Error("no cases selected");
  const home = process.env.NOEMA_HOME ?? (existsSync("/var/lib/noema-dev") ? "/var/lib/noema-dev" : resolve(process.env.HOME ?? ".", ".noema"));
  const homeExists = existsSync(home);
  const execute = args.includes("--execute");
  const socket = args.includes("--socket") ? args[args.indexOf("--socket") + 1] : "/run/noema-dev/noema";
  const results = selected.map((item) => ({
    caseId: `${PREFIX}-${String(item.number).padStart(3, "0")}`,
    number: item.number,
    task: item.task,
    prompt: manifest.cases.promptTemplate.replace("{number}", String(item.number)).replace("{task}", item.task)
      + ` Fixture gateway: ${manifest.fixture.baseUrl}?run=${PREFIX}-${String(item.number).padStart(3, "0")}.`,
    priorStatus: item.status,
    gap: item.gap,
    terminalState: execute ? "queued" : "not_run",
    evidence: [],
    blockedReason: execute ? null : "Validation-only mode. Add --execute to use the normal live runner.",
  }));
  if (execute) {
    const health = await fetch(`${manifest.fixture.baseUrl}/health`).catch(() => undefined);
    if (!health?.ok) throw new Error(`mock fixture is not reachable at ${manifest.fixture.baseUrl}; start run-mock-personal-assistant-services.ts first`);
    for (const result of results) {
      const child = Bun.spawn(["bun", "run", resolve(ROOT, "scripts/run-live-noema-case.ts"), "--socket", socket, "--timeout-ms", "600000", "--", result.prompt], { stdout: "pipe", stderr: "pipe" });
      const [exitCode, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      result.terminalState = exitCode === 0 ? "completed" : "blocked";
      result.evidence = [{ runnerExitCode: exitCode, output: stdout.slice(-12000), error: stderr.slice(-4000) }];
      result.blockedReason = exitCode === 0 ? null : "The live runner returned a non-zero exit code; inspect the attached runner output.";
    }
  }
  console.log(JSON.stringify({
    format: "noema.personal-assistant-replay-result/v1",
    casePrefix: PREFIX,
    home,
    homeExists,
    providerMode: manifest.providerMode,
    selected: results.length,
    results,
  }, null, 2));
}

await main();
