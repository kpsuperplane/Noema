type CargoPackage = {
  name: string;
};

type CargoMetadata = {
  packages: CargoPackage[];
};

export type FocusedTreePolicy = {
  packageName: string;
  forbiddenPackages: ReadonlySet<string>;
  forbiddenFeatures: ReadonlySet<string>;
};

export type TreeFacts = {
  packages: ReadonlySet<string>;
  features: ReadonlySet<string>;
};

const TARGET_PACKAGES = [
  "noema-home",
  "noema-conversations",
  "noema-artifacts",
  "noema-capabilities",
  "noema-providers",
  "noema-tasks",
  "noema-capabilities-mcp",
  "noema-memory",
  "noema-store",
  "noema-runtime",
  "noema-host",
  "noema-api",
  "noema-server",
  "noema-desktop",
  "noema-model-evals",
] as const;

const CONCRETE_FEATURES = new Set([
  "noema-artifacts/filesystem",
  "noema-providers/adapters",
  "noema-providers/local-models",
  "noema-providers/local-model-evals",
  "noema-capabilities-mcp/transport",
  "noema-memory/service",
  "noema-store/test-support",
  "noema-runtime/eval-support",
  "noema-host/composition",
]);

const CONTRACT_BACKEND_PACKAGES = new Set(["rmcp", "reqwest", "rusqlite"]);
const COMPOSED_BACKEND_PACKAGES = new Set(["rmcp", "reqwest"]);

function policy(
  packageName: string,
  forbiddenPackages: ReadonlySet<string>,
): FocusedTreePolicy {
  return {
    packageName,
    forbiddenPackages,
    forbiddenFeatures: CONCRETE_FEATURES,
  };
}

export const FOCUSED_TREE_POLICIES: readonly FocusedTreePolicy[] = [
  policy("noema-home", CONTRACT_BACKEND_PACKAGES),
  policy("noema-conversations", CONTRACT_BACKEND_PACKAGES),
  policy("noema-artifacts", CONTRACT_BACKEND_PACKAGES),
  policy("noema-capabilities", CONTRACT_BACKEND_PACKAGES),
  policy("noema-providers", CONTRACT_BACKEND_PACKAGES),
  policy("noema-tasks", CONTRACT_BACKEND_PACKAGES),
  policy("noema-capabilities-mcp", CONTRACT_BACKEND_PACKAGES),
  policy("noema-memory", CONTRACT_BACKEND_PACKAGES),
  // Store and runtime intentionally include SQLite through noema-store. They
  // must still exclude transport, sidecar, filesystem, and provider backends.
  policy("noema-store", COMPOSED_BACKEND_PACKAGES),
  policy("noema-runtime", COMPOSED_BACKEND_PACKAGES),
  policy("noema-host", COMPOSED_BACKEND_PACKAGES),
  policy("noema-api", COMPOSED_BACKEND_PACKAGES),
];

const ANSI_ESCAPE = /\u001B\[[0-?]*[ -/]*[@-~]/g;
const FEATURE_LINE = /^([A-Za-z0-9_.-]+) feature "([^"]+)"/;
const PACKAGE_LINE = /^([A-Za-z0-9_.-]+) v\d/;

export function parseTreeFacts(output: string): TreeFacts {
  const packages = new Set<string>();
  const features = new Set<string>();

  for (const rawLine of output.split(/\r?\n/)) {
    const line = rawLine.replace(ANSI_ESCAPE, "").trim();
    const feature = FEATURE_LINE.exec(line);
    if (feature) {
      features.add(`${feature[1]}/${feature[2]}`);
      continue;
    }

    const packageMatch = PACKAGE_LINE.exec(line);
    if (packageMatch) {
      packages.add(packageMatch[1]);
    }
  }

  return { packages, features };
}

export function validateFocusedTree(
  policy: FocusedTreePolicy,
  output: string,
): string[] {
  const facts = parseTreeFacts(output);
  const errors: string[] = [];

  for (const packageName of policy.forbiddenPackages) {
    if (facts.packages.has(packageName)) {
      errors.push(
        `${policy.packageName} contract-only tree contains forbidden package ${packageName}`,
      );
    }
  }

  for (const feature of policy.forbiddenFeatures) {
    if (facts.features.has(feature)) {
      errors.push(
        `${policy.packageName} contract-only tree contains forbidden feature ${feature}`,
      );
    }
  }

  return errors.sort();
}

export function validateTargetPackagePresence(
  packageNames: ReadonlySet<string>,
): string[] {
  return TARGET_PACKAGES.filter((name) => !packageNames.has(name)).map(
    (name) => `workspace is missing target package: ${name}`,
  );
}

export function cargoTreeArgs(packageName: string): string[] {
  return [
    "tree",
    "--locked",
    "--offline",
    "--target",
    "all",
    "-p",
    packageName,
    "--no-default-features",
    "-e",
    "normal,build,features",
    "--prefix",
    "none",
    "--charset",
    "ascii",
  ];
}

function loadWorkspaceMetadata(): CargoMetadata {
  const result = Bun.spawnSync(
    [
      "cargo",
      "metadata",
      "--locked",
      "--offline",
      "--no-deps",
      "--format-version",
      "1",
    ],
    { stdout: "pipe", stderr: "inherit" },
  );
  if (result.exitCode !== 0) {
    throw new Error(`cargo metadata failed with exit code ${result.exitCode}`);
  }
  return JSON.parse(new TextDecoder().decode(result.stdout)) as CargoMetadata;
}

function loadFocusedTree(packageName: string): string {
  const result = Bun.spawnSync(["cargo", ...cargoTreeArgs(packageName)], {
    stdout: "pipe",
    stderr: "inherit",
  });
  if (result.exitCode !== 0) {
    throw new Error(
      `cargo tree failed for ${packageName} with exit code ${result.exitCode}`,
    );
  }
  return new TextDecoder().decode(result.stdout);
}

if (import.meta.main) {
  const metadata = loadWorkspaceMetadata();
  const packageNames = new Set(metadata.packages.map((pkg) => pkg.name));
  const errors = validateTargetPackagePresence(packageNames);

  for (const focusedPolicy of FOCUSED_TREE_POLICIES) {
    if (!packageNames.has(focusedPolicy.packageName)) {
      continue;
    }
    errors.push(
      ...validateFocusedTree(
        focusedPolicy,
        loadFocusedTree(focusedPolicy.packageName),
      ),
    );
  }

  if (errors.length > 0) {
    console.error("focused dependency-tree violations:");
    for (const error of errors.sort()) {
      console.error(`- ${error}`);
    }
    process.exit(1);
  }

  console.log("focused dependency trees are valid");
}
