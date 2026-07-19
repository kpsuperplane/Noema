import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

export type SourceSnapshot = Map<string, string>;

export type SizeSummary = {
  totalLines: number;
  productionLines: number;
  testLines: number;
  testDeclarations: number;
  files: Record<string, { totalLines: number; testLines: number }>;
};

const TEST_DECLARATION =
  /^[ \t]*#\[(?:test|tokio::test(?:\([^\]\r\n]*\))?|async_std::test(?:\([^\]\r\n]*\))?)\]\s*(?:#\[[^\]\r\n]+\]\s*)*(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*/gm;

function sourceLines(source: string): string[] {
  if (source.length === 0) return [];
  const lines = source.split(/\r?\n/);
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

export function isDedicatedTestPath(path: string): boolean {
  return /(^|\/)(?:test|tests)(?:\/|\.rs$)|(?:^|\/)[^/]+_tests?\.rs$|(?:^|\/)test_support\.rs$/.test(
    path,
  );
}

export function estimateTestLines(path: string, source: string): number {
  const lines = sourceLines(source);
  if (isDedicatedTestPath(path)) return lines.length;

  for (let index = 0; index < lines.length; index += 1) {
    if (!/^\s*#\[cfg\(test\)\]\s*$/.test(lines[index] ?? "")) continue;
    let moduleLine = index + 1;
    while (moduleLine < lines.length && /^\s*$/.test(lines[moduleLine] ?? "")) {
      moduleLine += 1;
    }
    if (/^\s*mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/.test(lines[moduleLine] ?? "")) {
      // Inline test modules in this repository live at module tails. Counting
      // the tail keeps the report deterministic without pretending to be a
      // Rust parser; dedicated test files remain exact.
      return lines.length - index;
    }
  }
  return 0;
}

export function summarizeSnapshot(sources: SourceSnapshot): SizeSummary {
  const summary: SizeSummary = {
    totalLines: 0,
    productionLines: 0,
    testLines: 0,
    testDeclarations: 0,
    files: {},
  };
  for (const [path, source] of sources) {
    const totalLines = sourceLines(source).length;
    const testLines = estimateTestLines(path, source);
    summary.totalLines += totalLines;
    summary.testLines += testLines;
    summary.testDeclarations += [...source.matchAll(TEST_DECLARATION)].length;
    summary.files[path] = { totalLines, testLines };
  }
  summary.productionLines = summary.totalLines - summary.testLines;
  return summary;
}

function loadCurrentSources(root: string): SourceSnapshot {
  const sources: SourceSnapshot = new Map();
  const visit = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === "target" || entry.name === "node_modules") continue;
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && entry.name.endsWith(".rs")) {
        sources.set(relative(root, path).split(sep).join("/"), readFileSync(path, "utf8"));
      }
    }
  };
  visit(resolve(root, "crates"));
  return sources;
}

function loadGitSources(root: string, reference: string): SourceSnapshot {
  const result = Bun.spawnSync(
    ["git", "grep", "-n", "-I", "-e", "", reference, "--", "*.rs"],
    { cwd: root, stdout: "pipe", stderr: "pipe" },
  );
  if (result.exitCode !== 0) {
    throw new Error(
      `could not read Rust sources at ${reference}: ${result.stderr.toString().trim()}`,
    );
  }

  const byPath = new Map<string, string[]>();
  for (const line of result.stdout.toString().split(/\r?\n/)) {
    if (line.length === 0) continue;
    const match = /^[^:]+:(.*?):(\d+):(.*)$/.exec(line);
    if (!match) throw new Error(`could not parse git grep output: ${line}`);
    const [, path, rawLine, content] = match;
    if (!path || !rawLine) continue;
    const lineNumber = Number.parseInt(rawLine, 10);
    const lines = byPath.get(path) ?? [];
    lines[lineNumber - 1] = content ?? "";
    byPath.set(path, lines);
  }
  return new Map([...byPath].map(([path, lines]) => [path, `${lines.join("\n")}\n`]));
}

type Options = {
  base: string;
  maxProductionNet?: number;
  maxTestNet?: number;
  maxNewTests?: number;
  requireNetNegative: boolean;
  checkFileLimits: boolean;
  jsonPath?: string;
};

function parseNonnegative(value: string | undefined, flag: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`${flag} requires a nonnegative integer`);
  }
  return parsed;
}

function parseOptions(args: string[]): Options {
  const options: Options = {
    base: "HEAD",
    requireNetNegative: false,
    checkFileLimits: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    switch (flag) {
      case "--base":
        options.base = args[++index] ?? "";
        if (!options.base) throw new Error("--base requires a git reference");
        break;
      case "--max-production-net":
        options.maxProductionNet = parseNonnegative(args[++index], flag);
        break;
      case "--max-test-net":
        options.maxTestNet = parseNonnegative(args[++index], flag);
        break;
      case "--max-new-tests":
        options.maxNewTests = parseNonnegative(args[++index], flag);
        break;
      case "--require-net-negative":
        options.requireNetNegative = true;
        break;
      case "--check-file-limits":
        options.checkFileLimits = true;
        break;
      case "--json":
        options.jsonPath = args[++index];
        if (!options.jsonPath) throw new Error("--json requires a path");
        break;
      case "--help":
        console.log(
          "Usage: bun run scripts/report-rust-size.ts [--base REF] [--max-production-net N] [--max-test-net N] [--max-new-tests N] [--require-net-negative] [--check-file-limits] [--json PATH]",
        );
        process.exit(0);
      default:
        throw new Error(`unknown argument: ${flag}`);
    }
  }
  return options;
}

function signed(value: number): string {
  return value >= 0 ? `+${value}` : String(value);
}

function main(): void {
  const root = resolve(import.meta.dir, "..");
  const options = parseOptions(Bun.argv.slice(2));
  const base = summarizeSnapshot(loadGitSources(root, options.base));
  const current = summarizeSnapshot(loadCurrentSources(root));
  const delta = {
    totalLines: current.totalLines - base.totalLines,
    productionLines: current.productionLines - base.productionLines,
    testLines: current.testLines - base.testLines,
    testDeclarations: current.testDeclarations - base.testDeclarations,
  };

  console.log(`Rust size against ${options.base}`);
  console.log(
    `Production  ${base.productionLines} -> ${current.productionLines} (${signed(delta.productionLines)})`,
  );
  console.log(`Tests       ${base.testLines} -> ${current.testLines} (${signed(delta.testLines)})`);
  console.log(`Total       ${base.totalLines} -> ${current.totalLines} (${signed(delta.totalLines)})`);
  console.log(
    `Test decls  ${base.testDeclarations} -> ${current.testDeclarations} (${signed(delta.testDeclarations)})`,
  );

  const paths = new Set([...Object.keys(base.files), ...Object.keys(current.files)]);
  const changed = [...paths]
    .map((path) => ({
      path,
      delta: (current.files[path]?.totalLines ?? 0) - (base.files[path]?.totalLines ?? 0),
    }))
    .filter(({ delta }) => delta !== 0)
    .sort((left, right) => Math.abs(right.delta) - Math.abs(left.delta))
    .slice(0, 10);
  if (changed.length > 0) {
    console.log("Largest file deltas");
    for (const entry of changed) console.log(`${signed(entry.delta).padStart(8)}  ${entry.path}`);
  }

  const violations: string[] = [];
  if (options.maxProductionNet !== undefined && delta.productionLines > options.maxProductionNet) {
    violations.push(
      `production delta ${signed(delta.productionLines)} exceeds +${options.maxProductionNet}`,
    );
  }
  if (options.maxTestNet !== undefined && delta.testLines > options.maxTestNet) {
    violations.push(`test delta ${signed(delta.testLines)} exceeds +${options.maxTestNet}`);
  }
  if (options.maxNewTests !== undefined && delta.testDeclarations > options.maxNewTests) {
    violations.push(
      `test declaration delta ${signed(delta.testDeclarations)} exceeds +${options.maxNewTests}`,
    );
  }
  if (options.requireNetNegative && delta.totalLines > 0) {
    violations.push(`refactor grew Rust by ${delta.totalLines} lines`);
  }
  if (options.checkFileLimits) {
    for (const [path, file] of Object.entries(current.files)) {
      const previous = base.files[path]?.totalLines;
      if (file.totalLines > 750 && (previous === undefined || file.totalLines > previous)) {
        violations.push(
          `${path} has ${file.totalLines} lines and is ${previous === undefined ? "new" : "growing"}`,
        );
      }
    }
  }

  const report = { base: options.base, before: base, after: current, delta, violations };
  if (options.jsonPath) writeFileSync(resolve(root, options.jsonPath), `${JSON.stringify(report, null, 2)}\n`);
  if (violations.length > 0) {
    for (const violation of violations) console.error(`budget violation: ${violation}`);
    process.exitCode = 1;
  }
}

if (import.meta.main) main();
