import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

export const DEFAULT_TEST_INVENTORY_MANIFEST =
  "docs/superpowers/baselines/rust-unit-test-inventory.json";

export type TestInventory = {
  formatVersion: 1;
  normalization: "rust-unit-test-leaf-v1";
  requirements: Record<string, number>;
  acceptedAliases: Record<string, string[]>;
};

export function parseRustUnitTestLeaves(output: string): Map<string, number> {
  const counts = new Map<string, number>();
  for (const rawLine of output.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line.endsWith(": test")) continue;
    const testName = line.slice(0, -": test".length);
    // Rustdoc names contain source paths and line numbers, which legitimately
    // move during this refactor. This inventory covers executable unit and
    // integration tests; doctests remain covered by the workspace test gate.
    if (testName.includes(" - ")) continue;
    const leaf = testName.split("::").at(-1)?.trim();
    if (!leaf) continue;
    counts.set(leaf, (counts.get(leaf) ?? 0) + 1);
  }
  return counts;
}

export function createTestInventory(output: string): TestInventory {
  const counts = parseRustUnitTestLeaves(output);
  return {
    formatVersion: 1,
    normalization: "rust-unit-test-leaf-v1",
    requirements: Object.fromEntries(
      [...counts.entries()].sort(([left], [right]) => left.localeCompare(right)),
    ),
    acceptedAliases: {},
  };
}

export function validateTestInventory(
  inventory: TestInventory,
  output: string,
): string[] {
  const counts = parseRustUnitTestLeaves(output);
  const errors: string[] = [];
  for (const [id, minimumCount] of Object.entries(inventory.requirements)) {
    const acceptedLeafNames = [id, ...(inventory.acceptedAliases[id] ?? [])];
    const actual = acceptedLeafNames.reduce(
      (total, name) => total + (counts.get(name) ?? 0),
      0,
    );
    if (actual < minimumCount) {
      errors.push(
        `${id} lost owned tests: expected at least ${minimumCount} across [${acceptedLeafNames.join(", ")}], found ${actual}`,
      );
    }
  }
  return errors.sort();
}

export function loadTestInventory(path: string): TestInventory {
  const value = JSON.parse(readFileSync(path, "utf8")) as Partial<TestInventory>;
  if (
    value.formatVersion !== 1 ||
    value.normalization !== "rust-unit-test-leaf-v1" ||
    !value.requirements ||
    Array.isArray(value.requirements) ||
    Object.entries(value.requirements).some(
      ([id, count]) =>
        id.length === 0 || !Number.isInteger(count) || (count as number) < 1,
    ) ||
    !value.acceptedAliases ||
    Array.isArray(value.acceptedAliases) ||
    Object.entries(value.acceptedAliases).some(
      ([id, aliases]) =>
        !Object.hasOwn(value.requirements, id) ||
        !Array.isArray(aliases) ||
        aliases.length === 0 ||
        aliases.some((name) => typeof name !== "string" || name.length === 0),
    )
  ) {
    throw new Error(`invalid Rust unit-test inventory at ${path}`);
  }
  return value as TestInventory;
}

function argumentValue(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function loadCurrentTestList(input: string | undefined): string {
  if (input) return readFileSync(resolve(input), "utf8");
  const result = Bun.spawnSync(
    ["cargo", "test", "--workspace", "--no-fail-fast", "--", "--list"],
    { stdout: "pipe", stderr: "inherit" },
  );
  if (result.exitCode !== 0) {
    throw new Error(`cargo test --list failed with exit code ${result.exitCode}`);
  }
  return new TextDecoder().decode(result.stdout);
}

if (import.meta.main) {
  try {
    const args = Bun.argv.slice(2);
    const command = args.includes("capture") ? "capture" : "verify";
    const manifestPath = resolve(
      argumentValue(args, "--manifest") ?? DEFAULT_TEST_INVENTORY_MANIFEST,
    );
    const testList = loadCurrentTestList(argumentValue(args, "--input"));

    if (command === "capture") {
      if (existsSync(manifestPath)) {
        throw new Error(
          `Rust unit-test inventory already exists at ${manifestPath}; edit acceptedAliases explicitly for reviewed renames or splits`,
        );
      }
      writeFileSync(
        manifestPath,
        `${JSON.stringify(createTestInventory(testList), null, 2)}\n`,
      );
      console.log(`captured Rust unit-test inventory at ${manifestPath}`);
    } else {
      const errors = validateTestInventory(
        loadTestInventory(manifestPath),
        testList,
      );
      if (errors.length > 0) {
        throw new Error(`Rust unit-test inventory violations:\n${errors.join("\n")}`);
      }
      console.log("Rust unit-test ownership inventory is valid");
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
