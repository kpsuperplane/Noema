type CargoDependency = {
  name: string;
  rename: string | null;
  kind: "dev" | "build" | null;
  features: string[];
  uses_default_features: boolean;
};

type CargoPackage = {
  id: string;
  name: string;
  default_run: string | null;
  dependencies: CargoDependency[];
  features: Record<string, string[]>;
};

export type CargoMetadata = {
  workspace_members: string[];
  workspace_default_members: string[];
  packages: CargoPackage[];
};

const TARGET_DEPENDENCIES = new Map<string, ReadonlySet<string>>([
  ["noema-home", new Set()],
  ["noema-conversations", new Set()],
  ["noema-artifacts", new Set(["noema-home"])],
  ["noema-capabilities", new Set()],
  ["noema-providers", new Set(["noema-capabilities", "noema-home"])],
  ["noema-workspaces", new Set()],
  [
    "noema-tasks",
    new Set(["noema-artifacts", "noema-providers", "noema-workspaces"]),
  ],
  [
    "noema-capabilities-mcp",
    new Set(["noema-capabilities", "noema-home"]),
  ],
  [
    "noema-memory",
    new Set(["noema-capabilities", "noema-providers", "noema-home"]),
  ],
  [
    "noema-store",
    new Set([
      "noema-conversations",
      "noema-artifacts",
      "noema-tasks",
      "noema-providers",
      "noema-capabilities-mcp",
      "noema-memory",
      "noema-workspaces",
    ]),
  ],
  [
    "noema-runtime",
    new Set([
      "noema-home",
      "noema-conversations",
      "noema-artifacts",
      "noema-tasks",
      "noema-capabilities",
      "noema-capabilities-mcp",
      "noema-providers",
      "noema-memory",
      "noema-store",
      "noema-workspaces",
    ]),
  ],
  [
    "noema-host",
    new Set([
      "noema-home",
      "noema-artifacts",
      "noema-capabilities",
      "noema-capabilities-mcp",
      "noema-providers",
      "noema-memory",
      "noema-store",
      "noema-runtime",
    ]),
  ],
  [
    "noema-api",
    new Set([
      "noema-conversations",
      "noema-artifacts",
      "noema-tasks",
      "noema-capabilities",
      "noema-capabilities-mcp",
      "noema-providers",
      "noema-memory",
      "noema-store",
      "noema-runtime",
      "noema-host",
      "noema-workspaces",
    ]),
  ],
  ["noema-server", new Set(["noema-artifacts", "noema-api", "noema-host"])],
  ["noema-desktop", new Set(["noema-api", "noema-host"])],
  ["noema-model-evals", new Set(["noema-providers", "noema-runtime"])],
]);

const RETIRED_PACKAGES = new Set(["noema-core"]);

const FINAL_DIRECT_DEPENDENCY_OWNERS = new Map<string, string>([
  ["async-graphql-axum", "noema-server"],
  ["axum", "noema-server"],
  ["tower", "noema-server"],
  ["tower-http", "noema-server"],
  ["tower-sessions", "noema-server"],
  ["tauri", "noema-desktop"],
  ["rmcp", "noema-capabilities-mcp"],
  ["rusqlite", "noema-store"],
  ["figment", "noema-host"],
]);

const COMPOSITION_FEATURES = new Set([
  "noema-artifacts/filesystem",
  "noema-providers/adapters",
  "noema-providers/local-models",
  "noema-capabilities-mcp/transport",
  "noema-memory/service",
]);

const FEATURE_REQUIRED_WITH_PACKAGE = new Set([
  "noema-artifacts/filesystem",
  "noema-providers/adapters",
  "noema-capabilities-mcp/transport",
  "noema-memory/service",
]);

const KNOWN_INTERNAL_FEATURES = new Map<string, ReadonlySet<string>>([
  ["noema-home", new Set()],
  ["noema-conversations", new Set()],
  ["noema-artifacts", new Set(["filesystem"])],
  ["noema-capabilities", new Set()],
  ["noema-providers", new Set(["adapters", "local-models", "local-model-evals"])],
  ["noema-workspaces", new Set()],
  ["noema-tasks", new Set()],
  ["noema-capabilities-mcp", new Set(["transport"])],
  ["noema-memory", new Set(["service"])],
  ["noema-store", new Set(["test-support"])],
  ["noema-runtime", new Set(["eval-support", "test-support"])],
  ["noema-host", new Set(["composition"])],
  ["noema-api", new Set(["test-support"])],
  ["noema-server", new Set(["dev-no-auth"])],
  ["noema-desktop", new Set()],
  ["noema-model-evals", new Set()],
]);

const EMPTY_DEFAULT_FEATURE_PACKAGES = new Set([
  "noema-home",
  "noema-conversations",
  "noema-artifacts",
  "noema-capabilities",
  "noema-providers",
  "noema-workspaces",
  "noema-tasks",
  "noema-capabilities-mcp",
  "noema-memory",
  "noema-store",
  "noema-runtime",
  "noema-host",
  "noema-api",
]);

const CONTRACT_ONLY_DEPENDENCIES = new Set([
  "noema-artifacts",
  "noema-providers",
  "noema-workspaces",
  "noema-capabilities-mcp",
  "noema-memory",
  "noema-store",
  "noema-runtime",
  "noema-host",
]);

const FRAMEWORK_FREE_PACKAGES = new Set([
  "noema-home",
  "noema-conversations",
  "noema-artifacts",
  "noema-capabilities",
  "noema-providers",
  "noema-workspaces",
  "noema-tasks",
  "noema-capabilities-mcp",
  "noema-memory",
  "noema-store",
  "noema-runtime",
  "noema-host",
  "noema-model-evals",
]);

const FORBIDDEN_FRAMEWORKS = new Set([
  "async-graphql",
  "async-graphql-axum",
  "axum",
  "tower",
  "tower-http",
  "tower-sessions",
  "tauri",
]);

const PACKAGE_FORBIDDEN_DEPENDENCIES = new Map<string, ReadonlySet<string>>([
  ["noema-capabilities", new Set(["readabilityrs", "reqwest", "tokio"])],
]);

function isNoemaPackage(name: string): boolean {
  return name.startsWith("noema-");
}

function splitFeatureKey(featureKey: string): [string, string] {
  const separator = featureKey.indexOf("/");
  return [featureKey.slice(0, separator), featureKey.slice(separator + 1)];
}

function packageDeclaresFeature(pkg: CargoPackage | undefined, feature: string): boolean {
  return pkg !== undefined && Object.hasOwn(pkg.features, feature);
}

function isProductionOrBuildDependency(dependency: CargoDependency): boolean {
  return dependency.kind !== "dev";
}

function directlyEnablesFeature(
  pkg: CargoPackage | undefined,
  dependencyName: string,
  feature: string,
): boolean {
  return pkg?.dependencies.some(
    (dependency) =>
      isProductionOrBuildDependency(dependency) &&
      dependency.name === dependencyName && dependency.features.includes(feature),
  ) ?? false;
}

function localFeatureEnables(
  pkg: CargoPackage,
  source: string,
  target: string,
  visited = new Set<string>(),
): boolean {
  if (source === target) {
    return true;
  }
  if (visited.has(source)) {
    return false;
  }
  visited.add(source);

  return (pkg.features[source] ?? []).some((activation) => {
    if (activation.startsWith("dep:") || activation.includes("/")) {
      return false;
    }
    return localFeatureEnables(pkg, activation, target, new Set(visited));
  });
}

function localFeatureForwardsDependencyFeature(
  pkg: CargoPackage,
  source: string,
  dependencyName: string,
  feature: string,
  visited = new Set<string>(),
): boolean {
  if (visited.has(source)) {
    return false;
  }
  visited.add(source);
  const dependenciesByManifestName = new Map(
    pkg.dependencies.map((dependency) => [dependency.rename ?? dependency.name, dependency]),
  );

  return (pkg.features[source] ?? []).some((activation) => {
    const separator = activation.indexOf("/");
    if (separator >= 1) {
      const alias = activation.slice(0, separator).replace(/\?$/, "");
      const dependency = dependenciesByManifestName.get(alias);
      return (
        dependency?.name === dependencyName &&
        isProductionOrBuildDependency(dependency) &&
        activation.slice(separator + 1) === feature
      );
    }
    if (activation.startsWith("dep:")) {
      return false;
    }
    return localFeatureForwardsDependencyFeature(
      pkg,
      activation,
      dependencyName,
      feature,
      new Set(visited),
    );
  });
}

function forwardedDependencyFeatures(
  pkg: CargoPackage,
): Array<{ dependency: CargoDependency; feature: string }> {
  const dependenciesByManifestName = new Map(
    pkg.dependencies.map((dependency) => [dependency.rename ?? dependency.name, dependency]),
  );
  const forwarded: Array<{ dependency: CargoDependency; feature: string }> = [];

  for (const activations of Object.values(pkg.features)) {
    for (const activation of activations) {
      const separator = activation.indexOf("/");
      if (separator < 1) {
        continue;
      }
      const dependencyAlias = activation.slice(0, separator).replace(/\?$/, "");
      const dependency = dependenciesByManifestName.get(dependencyAlias);
      if (dependency) {
        forwarded.push({
          dependency,
          feature: activation.slice(separator + 1),
        });
      }
    }
  }

  return forwarded;
}

function validateInternalFeatureActivation(
  source: CargoPackage,
  dependency: CargoDependency,
  feature: string,
  errors: Set<string>,
): void {
  if (!isNoemaPackage(dependency.name)) {
    return;
  }

  const featureKey = `${dependency.name}/${feature}`;
  const knownFeatures = KNOWN_INTERNAL_FEATURES.get(dependency.name);
  if (!knownFeatures?.has(feature)) {
    errors.add(`${source.name} enables unknown internal feature ${featureKey}`);
    return;
  }

  if (COMPOSITION_FEATURES.has(featureKey)) {
    if (
      isProductionOrBuildDependency(dependency) &&
      source.name !== "noema-host"
    ) {
      errors.add(`${source.name} may not enable implementation feature ${featureKey}`);
    }
    return;
  }

  if (featureKey === "noema-providers/local-model-evals") {
    if (
      source.name !== "noema-model-evals" ||
      !isProductionOrBuildDependency(dependency)
    ) {
      errors.add(`${source.name} may not enable evaluation feature ${featureKey}`);
    }
    return;
  }

  if (featureKey === "noema-runtime/eval-support") {
    if (
      source.name !== "noema-model-evals" ||
      !isProductionOrBuildDependency(dependency)
    ) {
      errors.add(`${source.name} may not enable evaluation feature ${featureKey}`);
    }
    return;
  }

  if (
    featureKey === "noema-runtime/test-support" &&
    dependency.kind !== "dev"
  ) {
    errors.add(`${source.name} enables noema-runtime/test-support outside dev-dependencies`);
    return;
  }

  if (
    featureKey === "noema-api/test-support" &&
    dependency.kind !== "dev"
  ) {
    errors.add(`${source.name} enables noema-api/test-support outside dev-dependencies`);
    return;
  }

  if (featureKey === "noema-host/composition") {
    if (
      !isProductionOrBuildDependency(dependency) ||
      (source.name !== "noema-server" && source.name !== "noema-desktop")
    ) {
      errors.add(`${source.name} may not enable shell feature ${featureKey}`);
    }
    return;
  }

  if (
    featureKey === "noema-store/test-support" &&
    dependency.kind !== "dev"
  ) {
    errors.add(`${source.name} enables noema-store/test-support outside dev-dependencies`);
  }
}

export function validateMetadata(metadata: CargoMetadata): string[] {
  const errors = new Set<string>();
  const packagesById = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const packages: CargoPackage[] = [];

  for (const workspaceMember of metadata.workspace_members) {
    const pkg = packagesById.get(workspaceMember);
    if (pkg) {
      packages.push(pkg);
    } else {
      errors.add(`workspace member is missing package metadata: ${workspaceMember}`);
    }
  }

  const packagesByName = new Map(packages.map((pkg) => [pkg.name, pkg]));
  const packageNames = new Set(packages.map((pkg) => pkg.name));
  const defaultPackageNames = metadata.workspace_default_members
    .map((id) => packagesById.get(id)?.name)
    .filter((name): name is string => name !== undefined);

  if (
    metadata.workspace_default_members.length !== 1 ||
    defaultPackageNames[0] !== "noema-server"
  ) {
    errors.add("workspace default member must be noema-server");
  }

  for (const packageName of packageNames) {
    if (RETIRED_PACKAGES.has(packageName)) {
      errors.add(`workspace contains retired package: ${packageName}`);
    } else if (!TARGET_DEPENDENCIES.has(packageName)) {
      errors.add(`unknown workspace package: ${packageName}`);
    }
  }

  for (const packageName of TARGET_DEPENDENCIES.keys()) {
    if (!packageNames.has(packageName)) {
      errors.add(`workspace is missing target package: ${packageName}`);
    }
  }

  if (packagesByName.get("noema-server")?.default_run !== "noema_web") {
    errors.add("noema-server default runnable binary must be noema_web");
  }

  for (const pkg of packages) {
    const knownPackageFeatures = KNOWN_INTERNAL_FEATURES.get(pkg.name);
    for (const feature of Object.keys(pkg.features)) {
      if (feature !== "default" && knownPackageFeatures && !knownPackageFeatures.has(feature)) {
        errors.add(`${pkg.name} declares unknown internal feature ${feature}`);
      }
    }
    if (
      EMPTY_DEFAULT_FEATURE_PACKAGES.has(pkg.name) &&
      (pkg.features.default === undefined || pkg.features.default.length > 0)
    ) {
      errors.add(`${pkg.name} must declare an empty default feature`);
    }

    for (const dependency of pkg.dependencies) {
      const dependencyName = dependency.name;

      if (RETIRED_PACKAGES.has(dependencyName)) {
        errors.add(`${pkg.name} depends on retired package ${dependencyName}`);
      } else if (isNoemaPackage(dependencyName) && !packageNames.has(dependencyName)) {
        errors.add(
          `${pkg.name} depends on internal package outside the workspace: ${dependencyName}`,
        );
      } else if (isNoemaPackage(dependencyName)) {
        const allowed = TARGET_DEPENDENCIES.get(pkg.name);
        if (
          isProductionOrBuildDependency(dependency) &&
          !allowed?.has(dependencyName)
        ) {
          errors.add(`${pkg.name} -> ${dependencyName} is not an allowed direct edge`);
        }
      }

      const finalOwner = FINAL_DIRECT_DEPENDENCY_OWNERS.get(dependencyName);
      if (finalOwner && pkg.name !== finalOwner) {
        errors.add(
          `${pkg.name} may not depend directly on ${dependencyName}; owner is ${finalOwner}`,
        );
      }

      if (
        FRAMEWORK_FREE_PACKAGES.has(pkg.name) &&
        FORBIDDEN_FRAMEWORKS.has(dependencyName)
      ) {
        errors.add(`${pkg.name} contract boundary may not depend on ${dependencyName}`);
      }

      if (PACKAGE_FORBIDDEN_DEPENDENCIES.get(pkg.name)?.has(dependencyName)) {
        errors.add(`${pkg.name} contract boundary may not depend on ${dependencyName}`);
      }

      if (CONTRACT_ONLY_DEPENDENCIES.has(dependencyName) && dependency.uses_default_features) {
        errors.add(
          `${pkg.name} must declare ${dependencyName} with default-features = false`,
        );
      }

      if (
        pkg.name === "noema-host" &&
        isProductionOrBuildDependency(dependency) &&
        dependency.features.some((feature) =>
          COMPOSITION_FEATURES.has(`${dependencyName}/${feature}`),
        )
      ) {
        errors.add(
          `noema-host must forward concrete dependency features only through noema-host/composition`,
        );
      }

      for (const feature of dependency.features) {
        validateInternalFeatureActivation(pkg, dependency, feature, errors);
      }
    }

    for (const { dependency, feature } of forwardedDependencyFeatures(pkg)) {
      if (!isProductionOrBuildDependency(dependency)) {
        errors.add(
          `${pkg.name} may not forward features through dev-dependency ${dependency.name}`,
        );
      } else {
        validateInternalFeatureActivation(pkg, dependency, feature, errors);
      }
    }
  }

  for (const featureKey of COMPOSITION_FEATURES) {
    const [dependencyName, feature] = splitFeatureKey(featureKey);
    const dependencyPackage = packagesByName.get(dependencyName);
    if (!dependencyPackage) {
      continue;
    }

    const mustDeclareFeature =
      FEATURE_REQUIRED_WITH_PACKAGE.has(featureKey) ||
      featureKey === "noema-providers/local-models";
    const declaresFeature = packageDeclaresFeature(dependencyPackage, feature);
    if (mustDeclareFeature && !declaresFeature) {
      errors.add(`${dependencyName} must declare implementation feature ${feature}`);
    }
    if (!declaresFeature) {
      continue;
    }

    const owner = packagesByName.get("noema-host");
    const ownerEnablesFeature = owner
      ? localFeatureForwardsDependencyFeature(
          owner,
          "composition",
          dependencyName,
          feature,
        )
      : false;
    if (!ownerEnablesFeature) {
      errors.add(`noema-host/composition must forward implementation feature ${featureKey}`);
    }
  }

  const host = packagesByName.get("noema-host");
  if (host) {
    if (!packageDeclaresFeature(host, "composition")) {
      errors.add("noema-host must declare shell feature composition");
    }
    for (const shell of ["noema-server", "noema-desktop"]) {
      if (
        packageNames.has(shell) &&
        !directlyEnablesFeature(
          packagesByName.get(shell),
          "noema-host",
          "composition",
        )
      ) {
        errors.add(`${shell} must directly enable shell feature noema-host/composition`);
      }
    }
  }

  const providers = packagesByName.get("noema-providers");
  const providerEvalDeclared = packageDeclaresFeature(providers, "local-model-evals");
  if (providerEvalDeclared && providers) {
    if (!localFeatureEnables(providers, "local-model-evals", "local-models")) {
      errors.add(
        "noema-providers/local-model-evals must imply noema-providers/local-models",
      );
    }
    if (
      !directlyEnablesFeature(
        packagesByName.get("noema-model-evals"),
        "noema-providers",
        "local-model-evals",
      )
    ) {
      errors.add(
        "noema-model-evals must directly enable evaluation feature noema-providers/local-model-evals",
      );
    }
  } else if (providers) {
    errors.add("noema-providers must declare evaluation feature local-model-evals");
  }

  const runtime = packagesByName.get("noema-runtime");
  if (runtime) {
    if (!packageDeclaresFeature(runtime, "eval-support")) {
      errors.add("noema-runtime must declare evaluation feature eval-support");
    } else if (
      !directlyEnablesFeature(
        packagesByName.get("noema-model-evals"),
        "noema-runtime",
        "eval-support",
      )
    ) {
      errors.add(
        "noema-model-evals must directly enable evaluation feature noema-runtime/eval-support",
      );
    }
  }

  return [...errors].sort();
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

if (import.meta.main) {
  const errors = validateMetadata(loadWorkspaceMetadata());
  if (errors.length > 0) {
    console.error("crate boundary violations:");
    for (const error of errors) {
      console.error(`- ${error}`);
    }
    process.exit(1);
  }

  console.log("crate boundaries are valid");
}
