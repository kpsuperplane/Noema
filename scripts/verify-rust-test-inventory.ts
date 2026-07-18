import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

export const DEFAULT_TEST_INVENTORY_MANIFEST =
  "docs/superpowers/baselines/rust-unit-test-inventory.json";

export type TestInventory = {
  formatVersion: 2;
  normalization: "rust-unit-test-leaf-v1";
  requirements: Record<string, number>;
  acceptedAliases: Record<string, string[]>;
  retiredRequirements: Record<string, string>;
  sourceRequirements: Record<string, number>;
  acceptedSourceAliases: Record<string, string[]>;
  retiredSourceRequirements: Record<string, string>;
  defaultQualifiedRequirements: Record<string, number>;
  allFeatureQualifiedRequirements: Record<string, number>;
  acceptedQualifiedAliases: Record<string, string[]>;
  retiredQualifiedRequirements: Record<string, string>;
};

type ParsedRustUnitTests = {
  leafCounts: Map<string, number>;
  qualifiedCounts: Map<string, number>;
};

function parseRustUnitTests(output: string): ParsedRustUnitTests {
  const leafCounts = new Map<string, number>();
  const qualifiedCounts = new Map<string, number>();
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
    leafCounts.set(leaf, (leafCounts.get(leaf) ?? 0) + 1);
    qualifiedCounts.set(testName, (qualifiedCounts.get(testName) ?? 0) + 1);
  }
  return { leafCounts, qualifiedCounts };
}

export function parseRustUnitTestLeaves(output: string): Map<string, number> {
  return parseRustUnitTests(output).leafCounts;
}

const RUST_TEST_DECLARATION =
  /^[ \t]*#\[(?:test|tokio::test(?:\([^\]\r\n]*\))?|async_std::test(?:\([^\]\r\n]*\))?)\]\s*(?:#\[[^\]\r\n]+\]\s*)*(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)/gm;

export function parseRustSourceTestDeclarations(
  sources: ReadonlyMap<string, string>,
): Map<string, number> {
  const counts = new Map<string, number>();
  for (const [path, source] of sources) {
    for (const match of source.matchAll(RUST_TEST_DECLARATION)) {
      const id = `${path.replaceAll(sep, "/")}::${match[1]}`;
      counts.set(id, (counts.get(id) ?? 0) + 1);
    }
  }
  return counts;
}

export function loadRustSourceTestDeclarations(
  root: string,
): Map<string, number> {
  const sources = new Map<string, string>();
  const cratesRoot = resolve(root, "crates");
  const visit = (directory: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        sources.set(relative(root, path), readFileSync(path, "utf8"));
      }
    }
  };
  visit(cratesRoot);
  return parseRustSourceTestDeclarations(sources);
}

export function createTestInventory(
  output: string,
  sourceTests: ReadonlyMap<string, number> = new Map(),
  defaultQualifiedTests: ReadonlyMap<string, number> = new Map(),
  allFeatureQualifiedTests: ReadonlyMap<string, number> = new Map(),
): TestInventory {
  const counts = parseRustUnitTestLeaves(output);
  return {
    formatVersion: 2,
    normalization: "rust-unit-test-leaf-v1",
    requirements: Object.fromEntries(
      [...counts.entries()].sort(([left], [right]) => left.localeCompare(right)),
    ),
    acceptedAliases: {},
    retiredRequirements: {},
    sourceRequirements: Object.fromEntries(
      [...sourceTests.entries()].sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
    acceptedSourceAliases: {},
    retiredSourceRequirements: {},
    defaultQualifiedRequirements: Object.fromEntries(
      [...defaultQualifiedTests.entries()].sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
    allFeatureQualifiedRequirements: Object.fromEntries(
      [...allFeatureQualifiedTests.entries()].sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
    acceptedQualifiedAliases: {},
    retiredQualifiedRequirements: {},
  };
}

export function validateTestInventory(
  inventory: TestInventory,
  output: string,
): string[] {
  const { leafCounts, qualifiedCounts } = parseRustUnitTests(output);
  const errors: string[] = [];
  for (const [id, minimumCount] of Object.entries(inventory.requirements)) {
    if (Object.hasOwn(inventory.retiredRequirements, id)) continue;
    const aliases = inventory.acceptedAliases[id] ?? [];
    let actual = leafCounts.get(id) ?? 0;
    if (actual >= minimumCount) continue;
    for (const alias of aliases) {
      let matchCount = 0;
      if (!alias.includes("::")) {
        matchCount = leafCounts.get(alias) ?? 0;
        if (matchCount > 1) {
          errors.push(
            `${id} accepted alias ${alias} is ambiguous: matched ${matchCount} qualified tests; use a module-qualified alias`,
          );
          matchCount = 0;
        }
      } else {
        const matches = [...qualifiedCounts.entries()].filter(
          ([testName]) =>
            testName === alias ||
            testName.endsWith(`::${alias}`) ||
            alias.endsWith(`::${testName}`),
        );
        matchCount = matches.reduce((total, [, count]) => total + count, 0);
        if (matchCount > 1) {
          errors.push(
            `${id} accepted alias ${alias} is ambiguous: matched ${matchCount} qualified tests`,
          );
          matchCount = 0;
        }
      }
      if (aliases.length > 1 && matchCount === 0) {
        errors.push(`${id} lost required split owner ${alias}`);
      } else {
        actual += matchCount;
      }
    }
    if (actual < minimumCount) {
      const acceptedNames = [id, ...aliases];
      errors.push(
        `${id} lost owned tests: expected at least ${minimumCount} across [${acceptedNames.join(", ")}], found ${actual}`,
      );
    }
  }
  return errors.sort();
}

export function validateRustSourceTestInventory(
  inventory: TestInventory,
  sourceTests: ReadonlyMap<string, number>,
): string[] {
  return validateExactOwnerRequirements(
    inventory.sourceRequirements,
    inventory.acceptedSourceAliases,
    inventory.retiredSourceRequirements,
    sourceTests,
    "source",
    true,
  );
}

function validateExactOwnerRequirements(
  requirements: Record<string, number>,
  aliases: Record<string, string[]>,
  retirements: Record<string, string>,
  actualTests: ReadonlyMap<string, number>,
  label: string,
  closedWorld: boolean,
): string[] {
  const errors: string[] = [];
  const ownedCurrentIds = new Set<string>();
  for (const [id, minimumCount] of Object.entries(requirements)) {
    if (Object.hasOwn(retirements, id)) continue;
    const acceptedIds = aliases[id] ?? [];
    ownedCurrentIds.add(id);
    acceptedIds.forEach((acceptedId) => ownedCurrentIds.add(acceptedId));
    let actual = actualTests.get(id) ?? 0;
    if (actual >= minimumCount) continue;
    for (const acceptedId of acceptedIds) {
      const count = actualTests.get(acceptedId) ?? 0;
      if (acceptedIds.length > 1 && count === 0) {
        errors.push(`${id} lost required ${label} split owner ${acceptedId}`);
      } else {
        actual += count;
      }
    }
    if (actual < minimumCount) {
      errors.push(
        `${id} lost ${label} tests: expected at least ${minimumCount} across [${[id, ...acceptedIds].join(", ")}], found ${actual}`,
      );
    }
  }
  if (closedWorld) {
    for (const id of actualTests.keys()) {
      if (!ownedCurrentIds.has(id)) {
        errors.push(`${id} is an unowned current ${label} test`);
      }
    }
  }
  return errors.sort();
}

export function validateQualifiedTestInventory(
  inventory: TestInventory,
  defaultTests: ReadonlyMap<string, number>,
  allFeatureTests: ReadonlyMap<string, number>,
): string[] {
  return [
    ...validateExactOwnerRequirements(
      inventory.defaultQualifiedRequirements,
      inventory.acceptedQualifiedAliases,
      inventory.retiredQualifiedRequirements,
      defaultTests,
      "default qualified",
      true,
    ),
    ...validateExactOwnerRequirements(
      inventory.allFeatureQualifiedRequirements,
      inventory.acceptedQualifiedAliases,
      inventory.retiredQualifiedRequirements,
      allFeatureTests,
      "all-feature qualified",
      true,
    ),
  ].sort();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validRequirements(
  value: unknown,
  sourceQualified: boolean,
  compiledQualified = false,
): value is Record<string, number> {
  return (
    isRecord(value) &&
    Object.entries(value).every(
      ([id, count]) =>
        id.length > 0 &&
        (!sourceQualified || id.includes(".rs::")) &&
        (!compiledQualified || (id.includes("/") && id.includes("::"))) &&
        Number.isInteger(count) &&
        (count as number) >= 1,
    )
  );
}

function validRetirements(
  value: unknown,
  requirements: Record<string, unknown>,
): value is Record<string, string> {
  return (
    isRecord(value) &&
    Object.entries(value).every(
      ([id, reason]) =>
        Object.hasOwn(requirements, id) &&
        typeof reason === "string" &&
        reason.trim().length > 0,
    )
  );
}

function validAliases(
  value: unknown,
  requirements: Record<string, unknown>,
  retirements: Record<string, unknown>,
  sourceQualified: boolean,
  compiledQualified = false,
): value is Record<string, string[]> {
  return (
    isRecord(value) &&
    Object.entries(value).every(
      ([id, aliases]) =>
        Object.hasOwn(requirements, id) &&
        !Object.hasOwn(retirements, id) &&
        Array.isArray(aliases) &&
        aliases.length > 0 &&
        aliases.every(
          (name) =>
            typeof name === "string" &&
            name.trim().length > 0 &&
            (!sourceQualified || name.includes(".rs::")) &&
            (!compiledQualified ||
              (name.includes("/") && name.includes("::"))),
        ),
    )
  );
}

export function loadTestInventory(path: string): TestInventory {
  const value = JSON.parse(readFileSync(path, "utf8")) as Partial<TestInventory>;
  if (
    value.formatVersion !== 2 ||
    value.normalization !== "rust-unit-test-leaf-v1" ||
    !validRequirements(value.requirements, false) ||
    !validRetirements(value.retiredRequirements, value.requirements) ||
    !validAliases(
      value.acceptedAliases,
      value.requirements,
      value.retiredRequirements,
      false,
    ) ||
    !validRequirements(value.sourceRequirements, true) ||
    !validRetirements(
      value.retiredSourceRequirements,
      value.sourceRequirements,
    ) ||
    !validAliases(
      value.acceptedSourceAliases,
      value.sourceRequirements,
      value.retiredSourceRequirements,
      true,
    ) ||
    !validRequirements(value.defaultQualifiedRequirements, false, true) ||
    !validRequirements(value.allFeatureQualifiedRequirements, false, true) ||
    !validRetirements(value.retiredQualifiedRequirements, {
      ...value.defaultQualifiedRequirements,
      ...value.allFeatureQualifiedRequirements,
    }) ||
    !validAliases(
      value.acceptedQualifiedAliases,
      {
        ...value.defaultQualifiedRequirements,
        ...value.allFeatureQualifiedRequirements,
      },
      value.retiredQualifiedRequirements,
      false,
      true,
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

type CargoTestListing = {
  output: string;
  qualifiedCounts: Map<string, number>;
};

function workspacePackageKey(packageId: string): string {
  const marker = "/crates/";
  const markerIndex = packageId.indexOf(marker);
  const versionIndex = packageId.lastIndexOf("#");
  if (markerIndex < 0 || versionIndex < markerIndex) {
    throw new Error(`cannot derive workspace package from ${packageId}`);
  }
  return packageId.slice(markerIndex + marker.length, versionIndex);
}

export function listCargoArtifactTests(artifactOutput: string): CargoTestListing {
  const qualifiedCounts = new Map<string, number>();
  const outputs: string[] = [];
  const executables = new Set<string>();
  for (const line of artifactOutput.split(/\r?\n/)) {
    if (line.trim().length === 0) continue;
    const artifact = JSON.parse(line) as {
      reason?: string;
      package_id?: string;
      executable?: string | null;
      profile?: { test?: boolean };
      target?: { name?: string };
    };
    if (
      artifact.reason !== "compiler-artifact" ||
      artifact.profile?.test !== true ||
      !artifact.executable ||
      !artifact.package_id ||
      !artifact.target?.name ||
      executables.has(artifact.executable)
    ) {
      continue;
    }
    executables.add(artifact.executable);
    const listed = Bun.spawnSync([artifact.executable, "--list"]);
    if (listed.exitCode !== 0) {
      throw new Error(
        `${artifact.executable} --list failed with exit code ${listed.exitCode}`,
      );
    }
    const output = new TextDecoder().decode(listed.stdout);
    outputs.push(output);
    const packageKey = workspacePackageKey(artifact.package_id);
    for (const rawLine of output.split(/\r?\n/)) {
      const testLine = rawLine.trim();
      if (!testLine.endsWith(": test") || testLine.includes(" - ")) continue;
      const testName = testLine.slice(0, -": test".length);
      const id = `${packageKey}/${artifact.target.name}::${testName}`;
      qualifiedCounts.set(id, (qualifiedCounts.get(id) ?? 0) + 1);
    }
  }
  return { output: outputs.join("\n"), qualifiedCounts };
}

function loadCargoTestListing(
  artifactInput: string | undefined,
  allFeatures: boolean,
): CargoTestListing {
  if (artifactInput) {
    return listCargoArtifactTests(readFileSync(resolve(artifactInput), "utf8"));
  }
  const command = ["cargo", "test", "--workspace"];
  if (allFeatures) command.push("--all-features");
  command.push("--no-run", "--message-format=json");
  const result = Bun.spawnSync(
    command,
    { stdout: "pipe", stderr: "inherit" },
  );
  if (result.exitCode !== 0) {
    throw new Error(`cargo test --no-run failed with exit code ${result.exitCode}`);
  }
  return listCargoArtifactTests(new TextDecoder().decode(result.stdout));
}

if (import.meta.main) {
  try {
    const args = Bun.argv.slice(2);
    const command = args.includes("capture") ? "capture" : "verify";
    const manifestPath = resolve(
      argumentValue(args, "--manifest") ?? DEFAULT_TEST_INVENTORY_MANIFEST,
    );
    const defaultListing = loadCargoTestListing(
      argumentValue(args, "--default-artifacts-input"),
      false,
    );
    const allFeatureListing = loadCargoTestListing(
      argumentValue(args, "--all-features-artifacts-input"),
      true,
    );
    const testList = argumentValue(args, "--input")
      ? readFileSync(resolve(argumentValue(args, "--input")!), "utf8")
      : defaultListing.output;
    const sourceTests = loadRustSourceTestDeclarations(
      resolve(argumentValue(args, "--source-root") ?? "."),
    );

    if (command === "capture") {
      if (existsSync(manifestPath)) {
        throw new Error(
          `Rust unit-test inventory already exists at ${manifestPath}; edit acceptedAliases explicitly for reviewed renames or splits`,
        );
      }
      writeFileSync(
        manifestPath,
        `${JSON.stringify(
          createTestInventory(
            testList,
            sourceTests,
            defaultListing.qualifiedCounts,
            allFeatureListing.qualifiedCounts,
          ),
          null,
          2,
        )}\n`,
      );
      console.log(`captured Rust unit-test inventory at ${manifestPath}`);
    } else {
      const inventory = loadTestInventory(manifestPath);
      const errors = [
        ...validateTestInventory(inventory, testList),
        ...validateRustSourceTestInventory(inventory, sourceTests),
        ...validateQualifiedTestInventory(
          inventory,
          defaultListing.qualifiedCounts,
          allFeatureListing.qualifiedCounts,
        ),
      ].sort();
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
