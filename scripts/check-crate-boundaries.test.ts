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
  dependencies?: DependencyInput[];
  features?: Record<string, string[]>;
};

function packageId(name: string): string {
  return `workspace:${name}`;
}

function metadata(
  packages: PackageInput[],
  workspacePackageNames = packages.map((pkg) => pkg.name),
): CargoMetadata {
  return {
    workspace_members: workspacePackageNames.map(packageId),
    packages: packages.map((pkg) => ({
      id: packageId(pkg.name),
      name: pkg.name,
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
];

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
      return { default: [], "eval-support": [] };
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
    case "noema-server":
      return { "dev-no-auth": [] };
    default:
      return {};
  }
}

describe("crate boundary metadata policy", () => {
  test("accepts the current core transition and transitional dependency owners", () => {
    const errors = validateMetadata(
      metadata([
        {
          name: "noema-core",
          features: { "local-model-evals": ["dep:sysinfo"] },
          dependencies: [
            { name: "figment" },
            { name: "rmcp" },
            { name: "rusqlite" },
          ],
        },
        {
          name: "noema-server",
          dependencies: [
            { name: "noema-core" },
            { name: "async-graphql-axum" },
            { name: "axum" },
            { name: "tower", kind: "dev" },
            { name: "tower-http" },
            { name: "tower-sessions" },
          ],
        },
        {
          name: "noema-desktop",
          dependencies: [{ name: "noema-core" }, { name: "tauri" }],
        },
        {
          name: "noema-model-evals",
          dependencies: [{ name: "noema-core", features: ["local-model-evals"] }],
        },
      ]),
    );

    expect(errors).toEqual([]);
  });

  test("accepts the complete target workspace and final dependency owners", () => {
    const packageOverrides = new Map<string, DependencyInput[]>([
      ["noema-capabilities-mcp", [{ name: "rmcp" }]],
      ["noema-store", [{ name: "rusqlite" }]],
      [
        "noema-host",
        [
          { name: "figment" },
          {
            name: "noema-artifacts",
            usesDefaultFeatures: false,
          },
          {
            name: "noema-providers",
            usesDefaultFeatures: false,
          },
          {
            name: "noema-capabilities-mcp",
            usesDefaultFeatures: false,
          },
          {
            name: "noema-memory",
            usesDefaultFeatures: false,
          },
          { name: "noema-store", usesDefaultFeatures: false },
        ],
      ],
      [
        "noema-server",
        [
          { name: "async-graphql-axum" },
          { name: "axum" },
          { name: "tower", kind: "dev" },
          { name: "tower-http" },
          { name: "tower-sessions" },
          {
            name: "noema-host",
            features: ["composition"],
            usesDefaultFeatures: false,
          },
        ],
      ],
      [
        "noema-desktop",
        [
          { name: "tauri" },
          {
            name: "noema-host",
            features: ["composition"],
            usesDefaultFeatures: false,
          },
        ],
      ],
      [
        "noema-api",
        [{ name: "noema-host", usesDefaultFeatures: false }],
      ],
      [
        "noema-model-evals",
        [
          {
            name: "noema-providers",
            features: ["local-model-evals"],
            usesDefaultFeatures: false,
          },
          {
            name: "noema-runtime",
            features: ["eval-support"],
            usesDefaultFeatures: false,
          },
        ],
      ],
    ]);
    const errors = validateMetadata(
      metadata(
        targetPackageNames.map((name) => ({
          name,
          dependencies: packageOverrides.get(name),
          features: targetFeatures(name),
        })),
      ),
    );

    expect(errors).toEqual([]);
  });

  test("rejects a forbidden internal edge", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        {
          name: "noema-artifacts",
          dependencies: [{ name: "noema-store", usesDefaultFeatures: false }],
        },
        { name: "noema-store" },
      ]),
    );

    expect(errors).toContain(
      "noema-artifacts -> noema-store is not an allowed direct edge",
    );
  });

  test("rejects an internal dependency that is not a workspace member", () => {
    const errors = validateMetadata(
      metadata([
        {
          name: "noema-core",
          dependencies: [
            { name: "noema-home", usesDefaultFeatures: false },
            { name: "noema-mystery", usesDefaultFeatures: false },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-core depends on internal package outside the workspace: noema-home",
    );
    expect(errors).toContain(
      "noema-core depends on internal package outside the workspace: noema-mystery",
    );
  });

  test("rejects every unknown workspace package, regardless of its name", () => {
    const errors = validateMetadata(
      metadata([{ name: "noema-core" }, { name: "workspace-tool" }]),
    );

    expect(errors).toContain("unknown workspace package: workspace-tool");
  });

  test("rejects a workspace member without package metadata", () => {
    const errors = validateMetadata(
      metadata([{ name: "noema-core" }], ["noema-core", "missing-package"]),
    );

    expect(errors).toContain(
      "workspace member is missing package metadata: workspace:missing-package",
    );
  });

  test("requires the complete package set after core is removed", () => {
    const errors = validateMetadata(metadata([{ name: "noema-home" }]));

    expect(errors).toContain(
      "post-core workspace is missing target package: noema-runtime",
    );
  });

  test("rejects an expired core edge", () => {
    const packages = targetPackageNames.map((name) =>
      name === "noema-server"
        ? { name, dependencies: [{ name: "noema-core" }] }
        : { name },
    );
    const errors = validateMetadata(metadata(packages));

    expect(errors).toContain(
      "noema-server has expired or forbidden noema-core dependency",
    );
  });

  test("allows and expires the server home transition", () => {
    const transitional = metadata([
      { name: "noema-core" },
      { name: "noema-home" },
      { name: "noema-server", dependencies: [{ name: "noema-home" }] },
    ]);
    expect(validateMetadata(transitional)).toEqual([]);

    const expired = metadata([
      { name: "noema-core" },
      { name: "noema-home" },
      { name: "noema-host" },
      { name: "noema-server", dependencies: [{ name: "noema-home" }] },
    ]);
    expect(validateMetadata(expired)).toContain(
      "noema-server -> noema-home is not an allowed direct edge",
    );
  });

  test("allows and expires the eval home transition", () => {
    const transitional = metadata([
      { name: "noema-core" },
      { name: "noema-home" },
      { name: "noema-model-evals", dependencies: [{ name: "noema-home" }] },
    ]);
    expect(validateMetadata(transitional)).toEqual([]);

    const expired = metadata([
      { name: "noema-core" },
      { name: "noema-home" },
      { name: "noema-runtime" },
      { name: "noema-model-evals", dependencies: [{ name: "noema-home" }] },
    ]);
    expect(validateMetadata(expired)).toContain(
      "noema-model-evals -> noema-home is not an allowed direct edge",
    );
  });

  test("rejects framework leakage", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-runtime", dependencies: [{ name: "async-graphql" }] },
      ]),
    );

    expect(errors).toContain(
      "noema-runtime contract boundary may not depend on async-graphql",
    );
  });

  for (const dependency of ["readabilityrs", "reqwest", "tokio"]) {
    test(`keeps noema-capabilities free of ${dependency}`, () => {
      const errors = validateMetadata(
        metadata([
          { name: "noema-core" },
          { name: "noema-capabilities", dependencies: [{ name: dependency }] },
        ]),
      );

      expect(errors).toContain(
        `noema-capabilities contract boundary may not depend on ${dependency}`,
      );
    });
  }

  const directDependencyOwnerCases = [
    ["async-graphql-axum", "noema-server"],
    ["axum", "noema-server"],
    ["tower", "noema-server"],
    ["tower-http", "noema-server"],
    ["tower-sessions", "noema-server"],
    ["tauri", "noema-desktop"],
    ["rmcp", "noema-capabilities-mcp"],
    ["rusqlite", "noema-store"],
    ["figment", "noema-host"],
  ] as const;

  for (const [dependency, owner] of directDependencyOwnerCases) {
    test(`reserves direct ${dependency} dependencies for ${owner}`, () => {
      const errors = validateMetadata(
        metadata([
          { name: "noema-core" },
          { name: "noema-runtime", dependencies: [{ name: dependency }] },
        ]),
      );

      expect(errors).toContain(
        `noema-runtime may not depend directly on ${dependency}; owner is ${owner}`,
      );
    });
  }

  test("requires core to own extracted implementation features before host exists", () => {
    const implementationDependencies: DependencyInput[] = [
      {
        name: "noema-artifacts",
        features: ["filesystem"],
        usesDefaultFeatures: false,
      },
      {
        name: "noema-providers",
        features: ["adapters", "local-models"],
        usesDefaultFeatures: false,
      },
      {
        name: "noema-capabilities-mcp",
        features: ["transport"],
        usesDefaultFeatures: false,
      },
      {
        name: "noema-memory",
        features: ["service"],
        usesDefaultFeatures: false,
      },
    ];
    const errors = validateMetadata(
      metadata([
        { name: "noema-core", dependencies: implementationDependencies },
        { name: "noema-artifacts", features: { default: [], filesystem: [] } },
        {
          name: "noema-providers",
          features: { default: [], adapters: [], "local-models": [] },
        },
        {
          name: "noema-capabilities-mcp",
          features: { default: [], transport: [] },
        },
        { name: "noema-memory", features: { default: [], service: [] } },
      ]),
    );

    expect(errors).toEqual([]);
  });

  test("transfers every implementation feature from core to host", () => {
    const implementationDependencies: DependencyInput[] = [
      {
        name: "noema-artifacts",
        usesDefaultFeatures: false,
      },
      {
        name: "noema-providers",
        usesDefaultFeatures: false,
      },
      {
        name: "noema-capabilities-mcp",
        usesDefaultFeatures: false,
      },
      {
        name: "noema-memory",
        usesDefaultFeatures: false,
      },
    ];
    const packages: PackageInput[] = [
      { name: "noema-core" },
      {
        name: "noema-host",
        dependencies: implementationDependencies,
        features: targetFeatures("noema-host"),
      },
      {
        name: "noema-server",
        dependencies: [
          {
            name: "noema-host",
            features: ["composition"],
            usesDefaultFeatures: false,
          },
        ],
      },
      {
        name: "noema-desktop",
        dependencies: [
          {
            name: "noema-host",
            features: ["composition"],
            usesDefaultFeatures: false,
          },
        ],
      },
      { name: "noema-artifacts", features: { default: [], filesystem: [] } },
      {
        name: "noema-providers",
        features: { default: [], adapters: [], "local-models": [] },
      },
      {
        name: "noema-capabilities-mcp",
        features: { default: [], transport: [] },
      },
      { name: "noema-memory", features: { default: [], service: [] } },
    ];
    expect(validateMetadata(metadata(packages))).toEqual([]);

    packages[0] = {
      name: "noema-core",
      dependencies: [
        {
          name: "noema-artifacts",
          features: ["filesystem"],
          usesDefaultFeatures: false,
        },
      ],
    };
    expect(validateMetadata(metadata(packages))).toContain(
      "noema-core may not enable implementation feature noema-artifacts/filesystem",
    );
  });

  test("reserves host composition for executable shells", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        {
          name: "noema-host",
          features: { default: [], composition: [] },
        },
        {
          name: "noema-api",
          dependencies: [
            {
              name: "noema-host",
              features: ["composition"],
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-api may not enable shell feature noema-host/composition",
    );
  });

  test("rejects concrete child features directly on host dependencies", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        {
          name: "noema-host",
          features: { default: [], composition: [] },
          dependencies: [
            {
              name: "noema-artifacts",
              features: ["filesystem"],
              usesDefaultFeatures: false,
            },
          ],
        },
        { name: "noema-artifacts", features: { default: [], filesystem: [] } },
      ]),
    );

    expect(errors).toContain(
      "noema-host must forward concrete dependency features only through noema-host/composition",
    );
  });

  test("rejects a missing composition-owner implementation feature", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-artifacts", features: { default: [], filesystem: [] } },
      ]),
    );

    expect(errors).toContain(
      "noema-core must directly enable implementation feature noema-artifacts/filesystem",
    );
  });

  const implementationFeatureCases = [
    ["noema-artifacts", "filesystem"],
    ["noema-providers", "adapters"],
    ["noema-providers", "local-models"],
    ["noema-capabilities-mcp", "transport"],
    ["noema-memory", "service"],
  ] as const;

  for (const [dependency, feature] of implementationFeatureCases) {
    test(`rejects non-owner use of ${dependency}/${feature}`, () => {
      const errors = validateMetadata(
        metadata([
          { name: "noema-core" },
          {
            name: "noema-runtime",
            dependencies: [
              {
                name: dependency,
                features: [feature],
                usesDefaultFeatures: false,
              },
            ],
          },
          {
            name: dependency,
            features: targetFeatures(dependency),
          },
        ]),
      );

      expect(errors).toContain(
        `noema-runtime may not enable implementation feature ${dependency}/${feature}`,
      );
    });
  }

  test("requires implementation-bearing dependencies to disable default features", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-store" },
        { name: "noema-api", dependencies: [{ name: "noema-store" }] },
      ]),
    );

    expect(errors).toContain(
      "noema-api must declare noema-store with default-features = false",
    );

    const providerErrors = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-providers" },
        { name: "noema-tasks", dependencies: [{ name: "noema-providers" }] },
      ]),
    );
    expect(providerErrors).toContain(
      "noema-tasks must declare noema-providers with default-features = false",
    );

    const valid = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-store", features: { default: [], "test-support": [] } },
        {
          name: "noema-api",
          dependencies: [{ name: "noema-store", usesDefaultFeatures: false }],
        },
      ]),
    );
    expect(valid).toEqual([]);
  });

  test("reserves store test support for dev dependencies", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-store", features: { default: [], "test-support": [] } },
        {
          name: "noema-api",
          dependencies: [
            {
              name: "noema-store",
              features: ["test-support"],
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-api enables noema-store/test-support outside dev-dependencies",
    );

    const valid = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-store", features: { default: [], "test-support": [] } },
        {
          name: "noema-api",
          dependencies: [
            {
              name: "noema-store",
              kind: "dev",
              features: ["test-support"],
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );
    expect(valid).toEqual([]);
  });

  test("does not require provider local models before that milestone is declared", () => {
    const errors = validateMetadata(
      metadata([
        {
          name: "noema-core",
          dependencies: [
            {
              name: "noema-providers",
              features: ["adapters"],
              usesDefaultFeatures: false,
            },
          ],
        },
        {
          name: "noema-providers",
          features: { default: [], adapters: [] },
        },
      ]),
    );

    expect(errors).toEqual([]);
  });

  test("requires provider local models once the feature is declared", () => {
    const packages: PackageInput[] = [
      {
        name: "noema-core",
        dependencies: [
          {
            name: "noema-providers",
            features: ["adapters"],
            usesDefaultFeatures: false,
          },
        ],
      },
      {
        name: "noema-providers",
        features: { default: [], adapters: [], "local-models": [] },
      },
    ];

    expect(validateMetadata(metadata(packages))).toContain(
      "noema-core must directly enable implementation feature noema-providers/local-models",
    );

    packages[0] = {
      name: "noema-core",
      dependencies: [
        {
          name: "noema-providers",
          features: ["adapters", "local-models"],
          usesDefaultFeatures: false,
        },
      ],
    };
    expect(validateMetadata(metadata(packages))).toEqual([]);
  });

  test("rejects feature forwarding through a renamed dependency", () => {
    const errors = validateMetadata(
      metadata([
        {
          name: "noema-core",
          dependencies: [
            {
              name: "noema-providers",
              features: ["adapters", "local-models"],
              usesDefaultFeatures: false,
            },
          ],
        },
        {
          name: "noema-providers",
          features: { default: [], adapters: [], "local-models": [] },
        },
        {
          name: "noema-api",
          features: { leak: ["providers?/local-models"] },
          dependencies: [
            {
              name: "noema-providers",
              rename: "providers",
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-api may not enable implementation feature noema-providers/local-models",
    );
  });

  test("rejects unknown internal dependency features and declarations", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        { name: "noema-store", features: { default: [], surprise: [] } },
        {
          name: "noema-api",
          dependencies: [
            {
              name: "noema-store",
              features: ["surprise"],
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-store declares unknown internal feature surprise",
    );
    expect(errors).toContain(
      "noema-api enables unknown internal feature noema-store/surprise",
    );
  });

  test("allows ordinary third-party dependency features and forwarding", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        {
          name: "noema-server",
          features: { "dev-no-auth": ["tower/util"] },
          dependencies: [
            {
              name: "tower",
              features: ["util"],
            },
          ],
        },
      ]),
    );

    expect(errors).toEqual([]);
  });

  for (const [packageName, feature] of [
    ["noema-artifacts", "filesystem"],
    ["noema-providers", "adapters"],
    ["noema-capabilities-mcp", "transport"],
    ["noema-memory", "service"],
    ["noema-store", "test-support"],
    ["noema-runtime", "eval-support"],
    ["noema-host", "composition"],
  ] as const) {
    test(`requires an empty default feature for ${packageName}`, () => {
      const errors = validateMetadata(
        metadata([
          { name: "noema-core" },
          {
            name: packageName,
            features: { default: [feature], [feature]: [] },
          },
        ]),
      );

      expect(errors).toContain(
        `${packageName} must declare an empty default feature`,
      );
    });
  }

  test("requires provider evaluation support to imply local models", () => {
    const errors = validateMetadata(
      metadata([
        {
          name: "noema-core",
          features: { "local-model-evals": [] },
          dependencies: [
            {
              name: "noema-providers",
              features: ["adapters", "local-models", "local-model-evals"],
              usesDefaultFeatures: false,
            },
          ],
        },
        {
          name: "noema-providers",
          features: {
            default: [],
            adapters: [],
            "local-models": [],
            "local-model-evals": [],
          },
        },
        {
          name: "noema-model-evals",
          dependencies: [
            { name: "noema-core", features: ["local-model-evals"] },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-providers/local-model-evals must imply noema-providers/local-models",
    );
  });

  test("allows the temporary core-owned provider evaluation feature", () => {
    const errors = validateMetadata(
      metadata([
        {
          name: "noema-core",
          features: { "local-model-evals": [] },
          dependencies: [
            {
              name: "noema-providers",
              features: ["adapters", "local-models", "local-model-evals"],
              usesDefaultFeatures: false,
            },
          ],
        },
        {
          name: "noema-providers",
          features: {
            default: [],
            adapters: [],
            "local-models": [],
            "local-model-evals": ["local-models"],
          },
        },
        {
          name: "noema-model-evals",
          dependencies: [
            { name: "noema-core", features: ["local-model-evals"] },
          ],
        },
      ]),
    );

    expect(errors).toEqual([]);
  });

  test("reserves final evaluation features for the model eval crate", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        {
          name: "noema-providers",
          features: {
            default: [],
            adapters: [],
            "local-models": [],
            "local-model-evals": ["local-models"],
          },
        },
        {
          name: "noema-runtime",
          features: { default: [], "eval-support": [] },
          dependencies: [
            {
              name: "noema-providers",
              features: ["local-model-evals"],
              usesDefaultFeatures: false,
            },
          ],
        },
        {
          name: "noema-api",
          dependencies: [
            {
              name: "noema-runtime",
              features: ["eval-support"],
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-runtime may not enable evaluation feature noema-providers/local-model-evals",
    );
    expect(errors).toContain(
      "noema-api may not enable evaluation feature noema-runtime/eval-support",
    );
  });

  test("expires the model eval dependency on core once runtime exists", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core", features: { "local-model-evals": [] } },
        { name: "noema-runtime", features: { default: [], "eval-support": [] } },
        {
          name: "noema-model-evals",
          dependencies: [
            { name: "noema-core", features: ["local-model-evals"] },
            {
              name: "noema-runtime",
              features: ["eval-support"],
              usesDefaultFeatures: false,
            },
          ],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-model-evals has expired or forbidden noema-core dependency",
    );
    expect(errors).toContain(
      "noema-model-evals may not enable transitional feature noema-core/local-model-evals",
    );
  });

  test("keeps the model eval crate framework-free", () => {
    const errors = validateMetadata(
      metadata([
        { name: "noema-core" },
        {
          name: "noema-model-evals",
          dependencies: [{ name: "async-graphql" }],
        },
      ]),
    );

    expect(errors).toContain(
      "noema-model-evals contract boundary may not depend on async-graphql",
    );
  });
});
