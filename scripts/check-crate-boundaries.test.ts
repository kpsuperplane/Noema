import { describe, expect, test } from "bun:test";

import {
  type CargoMetadata,
  validateMetadata,
} from "./check-crate-boundaries";

type DependencyInput = {
  name: string;
  rename?: string | null;
  kind?: "dev" | "build" | null;
  features?: string[];
  usesDefaultFeatures?: boolean;
};

type PackageInput = {
  name: string;
  defaultRun?: string | null;
  dependencies?: DependencyInput[];
  features?: Record<string, string[]>;
};

function packageId(name: string): string {
  return `workspace:${name}`;
}

function metadata(
  packages: PackageInput[],
  workspacePackageNames = packages.map((pkg) => pkg.name),
  workspaceDefaultMemberNames = ["noema-server"],
): CargoMetadata {
  return {
    workspace_members: workspacePackageNames.map(packageId),
    workspace_default_members: workspaceDefaultMemberNames.map(packageId),
    packages: packages.map((pkg) => ({
      id: packageId(pkg.name),
      name: pkg.name,
      default_run:
        pkg.defaultRun === undefined
          ? pkg.name === "noema-server"
            ? "noema_web"
            : null
          : pkg.defaultRun,
      dependencies: (pkg.dependencies ?? []).map((dependency) => ({
        name: dependency.name,
        rename: dependency.rename ?? null,
        kind: dependency.kind ?? null,
        features: dependency.features ?? [],
        uses_default_features: dependency.usesDefaultFeatures ?? true,
      })),
      features: pkg.features ?? {},
    })),
  };
}

const contract = (name: string): DependencyInput => ({
  name,
  usesDefaultFeatures: false,
});

const targetPackageNames = [
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

function targetFeatures(name: string): Record<string, string[]> {
  switch (name) {
    case "noema-artifacts":
      return { default: [], filesystem: [] };
    case "noema-providers":
      return {
        default: [],
        adapters: [],
        "local-models": [],
        "local-model-evals": ["local-models"],
      };
    case "noema-capabilities-mcp":
      return { default: [], transport: [] };
    case "noema-memory":
      return { default: [], service: [] };
    case "noema-store":
      return { default: [], "test-support": [] };
    case "noema-runtime":
      return { default: [], "eval-support": [], "test-support": [] };
    case "noema-host":
      return {
        default: [],
        composition: [
          "noema-artifacts/filesystem",
          "noema-providers/adapters",
          "noema-providers/local-models",
          "noema-capabilities-mcp/transport",
          "noema-memory/service",
        ],
      };
    case "noema-api":
      return { default: [], "test-support": [] };
    case "noema-server":
      return { "dev-no-auth": [] };
    default:
      return { default: [] };
  }
}

function targetDependencies(name: string): DependencyInput[] {
  switch (name) {
    case "noema-artifacts":
      return [contract("noema-home")];
    case "noema-providers":
      return [contract("noema-capabilities"), contract("noema-home")];
    case "noema-tasks":
      return [contract("noema-artifacts"), contract("noema-providers")];
    case "noema-capabilities-mcp":
      return [contract("noema-capabilities"), contract("noema-home"), { name: "rmcp" }];
    case "noema-memory":
      return [
        contract("noema-capabilities"),
        contract("noema-providers"),
        contract("noema-home"),
      ];
    case "noema-store":
      return [
        contract("noema-conversations"),
        contract("noema-artifacts"),
        contract("noema-tasks"),
        contract("noema-providers"),
        contract("noema-capabilities-mcp"),
        contract("noema-memory"),
        { name: "rusqlite" },
      ];
    case "noema-runtime":
      return [
        contract("noema-home"),
        contract("noema-conversations"),
        contract("noema-artifacts"),
        contract("noema-tasks"),
        contract("noema-capabilities"),
        contract("noema-capabilities-mcp"),
        contract("noema-providers"),
        contract("noema-memory"),
        contract("noema-store"),
      ];
    case "noema-host":
      return [
        contract("noema-home"),
        contract("noema-artifacts"),
        contract("noema-capabilities"),
        contract("noema-capabilities-mcp"),
        contract("noema-providers"),
        contract("noema-memory"),
        contract("noema-store"),
        contract("noema-runtime"),
        { name: "figment" },
      ];
    case "noema-api":
      return [
        contract("noema-conversations"),
        contract("noema-artifacts"),
        contract("noema-tasks"),
        contract("noema-capabilities"),
        contract("noema-capabilities-mcp"),
        contract("noema-providers"),
        contract("noema-memory"),
        contract("noema-store"),
        contract("noema-runtime"),
        contract("noema-host"),
      ];
    case "noema-server":
      return [
        contract("noema-artifacts"),
        contract("noema-api"),
        {
          ...contract("noema-host"),
          features: ["composition"],
        },
        { name: "async-graphql-axum" },
        { name: "axum" },
        { name: "tower" },
        { name: "tower-http" },
        { name: "tower-sessions" },
      ];
    case "noema-desktop":
      return [
        contract("noema-api"),
        {
          ...contract("noema-host"),
          features: ["composition"],
        },
        { name: "tauri" },
      ];
    case "noema-model-evals":
      return [
        {
          ...contract("noema-providers"),
          features: ["local-model-evals"],
        },
        {
          ...contract("noema-runtime"),
          features: ["eval-support"],
        },
      ];
    default:
      return [];
  }
}

function targetWorkspace(): PackageInput[] {
  return targetPackageNames.map((name) => ({
    name,
    dependencies: targetDependencies(name),
    features: targetFeatures(name),
  }));
}

function findPackage(packages: PackageInput[], name: string): PackageInput {
  const pkg = packages.find((candidate) => candidate.name === name);
  if (!pkg) throw new Error(`missing fixture package ${name}`);
  return pkg;
}

function findDependency(pkg: PackageInput, name: string): DependencyInput {
  const dependency = pkg.dependencies?.find((candidate) => candidate.name === name);
  if (!dependency) throw new Error(`missing fixture dependency ${pkg.name} -> ${name}`);
  return dependency;
}

describe("crate boundary metadata policy", () => {
  test("accepts the complete final workspace and dependency owners", () => {
    expect(validateMetadata(metadata(targetWorkspace()))).toEqual([]);
  });

  test("requires the server default member and web runnable", () => {
    expect(
      validateMetadata(
        metadata(targetWorkspace(), [...targetPackageNames], ["noema-api"]),
      ),
    ).toContain("workspace default member must be noema-server");

    const packages = targetWorkspace();
    findPackage(packages, "noema-server").defaultRun = null;
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-server default runnable binary must be noema_web",
    );
  });

  test("requires every target package and rejects unknown workspace members", () => {
    const missing = targetWorkspace().filter((pkg) => pkg.name !== "noema-memory");
    expect(validateMetadata(metadata(missing))).toContain(
      "workspace is missing target package: noema-memory",
    );

    const unknown = targetWorkspace();
    unknown.push({ name: "workspace-tool" });
    expect(validateMetadata(metadata(unknown))).toContain(
      "unknown workspace package: workspace-tool",
    );
  });

  test("rejects a workspace member without package metadata", () => {
    expect(
      validateMetadata(
        metadata(targetWorkspace(), [...targetPackageNames, "missing-package"]),
      ),
    ).toContain("workspace member is missing package metadata: workspace:missing-package");
  });

  test("keeps the retired package name only as an explicit rejection fixture", () => {
    const packages = targetWorkspace();
    packages.push({ name: "noema-core" });
    expect(validateMetadata(metadata(packages))).toContain(
      "workspace contains retired package: noema-core",
    );

    const dependencyPackages = targetWorkspace();
    findPackage(dependencyPackages, "noema-server").dependencies?.push({
      name: "noema-core",
    });
    expect(validateMetadata(metadata(dependencyPackages))).toContain(
      "noema-server depends on retired package noema-core",
    );
  });

  test("rejects forbidden, missing, and reversed internal edges", () => {
    const forbidden = targetWorkspace();
    findPackage(forbidden, "noema-capabilities").dependencies?.push(
      contract("noema-capabilities-mcp"),
    );
    expect(validateMetadata(metadata(forbidden))).toContain(
      "noema-capabilities -> noema-capabilities-mcp is not an allowed direct edge",
    );

    const missing = targetWorkspace();
    findPackage(missing, "noema-server").dependencies?.push(contract("noema-mystery"));
    expect(validateMetadata(metadata(missing))).toContain(
      "noema-server depends on internal package outside the workspace: noema-mystery",
    );
  });

  test("keeps framework dependencies in API and executable shells", () => {
    const packages = targetWorkspace();
    findPackage(packages, "noema-runtime").dependencies?.push({ name: "async-graphql" });
    const errors = validateMetadata(metadata(packages));
    expect(errors).toContain(
      "noema-runtime contract boundary may not depend on async-graphql",
    );

    for (const dependency of ["readabilityrs", "reqwest", "tokio"]) {
      const capabilityPackages = targetWorkspace();
      findPackage(capabilityPackages, "noema-capabilities").dependencies?.push({
        name: dependency,
      });
      expect(validateMetadata(metadata(capabilityPackages))).toContain(
        `noema-capabilities contract boundary may not depend on ${dependency}`,
      );
    }
  });

  for (const [dependency, owner] of [
    ["async-graphql-axum", "noema-server"],
    ["axum", "noema-server"],
    ["tower", "noema-server"],
    ["tower-http", "noema-server"],
    ["tower-sessions", "noema-server"],
    ["tauri", "noema-desktop"],
    ["rmcp", "noema-capabilities-mcp"],
    ["rusqlite", "noema-store"],
    ["figment", "noema-host"],
  ] as const) {
    test(`reserves direct ${dependency} dependencies for ${owner}`, () => {
      const packages = targetWorkspace();
      findPackage(packages, "noema-runtime").dependencies?.push({ name: dependency });
      expect(validateMetadata(metadata(packages))).toContain(
        `noema-runtime may not depend directly on ${dependency}; owner is ${owner}`,
      );
    });
  }

  test("requires contract dependencies to disable default features", () => {
    const packages = targetWorkspace();
    findDependency(findPackage(packages, "noema-api"), "noema-store").usesDefaultFeatures =
      true;
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-api must declare noema-store with default-features = false",
    );
  });

  test("requires host composition to forward all implementation features", () => {
    const packages = targetWorkspace();
    findPackage(packages, "noema-host").features!.composition = findPackage(
      packages,
      "noema-host",
    ).features!.composition.filter(
      (feature) => feature !== "noema-memory/service",
    );
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-host/composition must forward implementation feature noema-memory/service",
    );
  });

  test("rejects concrete features directly on host dependencies", () => {
    const packages = targetWorkspace();
    findDependency(findPackage(packages, "noema-host"), "noema-artifacts").features = [
      "filesystem",
    ];
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-host must forward concrete dependency features only through noema-host/composition",
    );
  });

  test("rejects implementation feature use outside host composition", () => {
    const packages = targetWorkspace();
    findDependency(findPackage(packages, "noema-runtime"), "noema-artifacts").features = [
      "filesystem",
    ];
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-runtime may not enable implementation feature noema-artifacts/filesystem",
    );
  });

  test("reserves host composition for executable shells", () => {
    const packages = targetWorkspace();
    findDependency(findPackage(packages, "noema-api"), "noema-host").features = [
      "composition",
    ];
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-api may not enable shell feature noema-host/composition",
    );
  });

  test("requires both executable shells to enable host composition", () => {
    const packages = targetWorkspace();
    findDependency(findPackage(packages, "noema-desktop"), "noema-host").features = [];
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-desktop must directly enable shell feature noema-host/composition",
    );
  });

  for (const [packageName, feature] of [
    ["noema-artifacts", "filesystem"],
    ["noema-providers", "adapters"],
    ["noema-capabilities-mcp", "transport"],
    ["noema-memory", "service"],
    ["noema-store", "test-support"],
    ["noema-runtime", "eval-support"],
    ["noema-host", "composition"],
    ["noema-api", "test-support"],
  ] as const) {
    test(`requires an empty default feature for ${packageName}`, () => {
      const packages = targetWorkspace();
      findPackage(packages, packageName).features!.default = [feature];
      expect(validateMetadata(metadata(packages))).toContain(
        `${packageName} must declare an empty default feature`,
      );
    });
  }

  for (const [packageName, dependencyName, feature] of [
    ["noema-api", "noema-store", "test-support"],
    ["noema-api", "noema-runtime", "test-support"],
    ["noema-server", "noema-api", "test-support"],
  ] as const) {
    test(`reserves ${dependencyName}/${feature} for dev dependencies`, () => {
      const packages = targetWorkspace();
      findDependency(findPackage(packages, packageName), dependencyName).features = [feature];
      expect(validateMetadata(metadata(packages))).toContain(
        `${packageName} enables ${dependencyName}/${feature} outside dev-dependencies`,
      );

      findDependency(findPackage(packages, packageName), dependencyName).kind = "dev";
      expect(validateMetadata(metadata(packages))).not.toContain(
        `${packageName} enables ${dependencyName}/${feature} outside dev-dependencies`,
      );
    });
  }

  test("rejects unknown internal features and renamed feature forwarding", () => {
    const unknown = targetWorkspace();
    findPackage(unknown, "noema-store").features!.surprise = [];
    findDependency(findPackage(unknown, "noema-api"), "noema-store").features = [
      "surprise",
    ];
    const unknownErrors = validateMetadata(metadata(unknown));
    expect(unknownErrors).toContain("noema-store declares unknown internal feature surprise");
    expect(unknownErrors).toContain(
      "noema-api enables unknown internal feature noema-store/surprise",
    );

    const renamed = targetWorkspace();
    findPackage(renamed, "noema-api").features!.leak = ["providers?/local-models"];
    findDependency(findPackage(renamed, "noema-api"), "noema-providers").rename =
      "providers";
    expect(validateMetadata(metadata(renamed))).toContain(
      "noema-api may not enable implementation feature noema-providers/local-models",
    );
  });

  test("requires provider evaluation support to imply local models", () => {
    const packages = targetWorkspace();
    findPackage(packages, "noema-providers").features!["local-model-evals"] = [];
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-providers/local-model-evals must imply noema-providers/local-models",
    );
  });

  test("reserves evaluation features for the model eval crate", () => {
    const packages = targetWorkspace();
    findDependency(findPackage(packages, "noema-api"), "noema-providers").features = [
      "local-model-evals",
    ];
    findDependency(findPackage(packages, "noema-api"), "noema-runtime").features = [
      "eval-support",
    ];
    const errors = validateMetadata(metadata(packages));
    expect(errors).toContain(
      "noema-api may not enable evaluation feature noema-providers/local-model-evals",
    );
    expect(errors).toContain(
      "noema-api may not enable evaluation feature noema-runtime/eval-support",
    );
  });

  test("requires model evals to enable both final evaluation features", () => {
    const packages = targetWorkspace();
    findDependency(
      findPackage(packages, "noema-model-evals"),
      "noema-providers",
    ).features = [];
    findDependency(
      findPackage(packages, "noema-model-evals"),
      "noema-runtime",
    ).features = [];
    const errors = validateMetadata(metadata(packages));
    expect(errors).toContain(
      "noema-model-evals must directly enable evaluation feature noema-providers/local-model-evals",
    );
    expect(errors).toContain(
      "noema-model-evals must directly enable evaluation feature noema-runtime/eval-support",
    );
  });
});
