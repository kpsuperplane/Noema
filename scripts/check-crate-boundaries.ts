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
  dependencies: CargoDependency[];
  features: Record<string, string[]>;
};

export type CargoMetadata = {
  workspace_members: string[];
  packages: CargoPackage[];
};

const TARGET_DEPENDENCIES = new Map<string, ReadonlySet<string>>([
  ["noema-home", new Set()],
  ["noema-conversations", new Set()],
  ["noema-artifacts", new Set(["noema-home"])],
  ["noema-capabilities", new Set()],
  ["noema-providers", new Set(["noema-capabilities", "noema-home"])],
  [
    "noema-tasks",
    new Set(["noema-artifacts", "noema-providers"]),
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
    ]),
  ],
  ["noema-server", new Set(["noema-artifacts", "noema-api", "noema-host"])],
  ["noema-desktop", new Set(["noema-api", "noema-host"])],
  ["noema-model-evals", new Set(["noema-providers", "noema-runtime"])],
]);

const TRANSITIONAL_CORE_CONSUMERS = new Set([
  "noema-server",
  "noema-desktop",
  "noema-model-evals",
]);

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

const TRANSITIONAL_CORE_DEPENDENCIES = new Set([
  "rmcp",
  "rusqlite",
  "figment",
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
  ["noema-core", new Set(["local-model-evals"])],
  ["noema-home", new Set()],
  ["noema-conversations", new Set()],
  ["noema-artifacts", new Set(["filesystem"])],
  ["noema-capabilities", new Set()],
  ["noema-providers", new Set(["adapters", "local-models", "local-model-evals"])],
  ["noema-tasks", new Set()],
  ["noema-capabilities-mcp", new Set(["transport"])],
  ["noema-memory", new Set(["service"])],
  ["noema-store", new Set(["test-support"])],
  ["noema-runtime", new Set(["eval-support"])],
  ["noema-host", new Set(["composition"])],
  ["noema-api", new Set()],
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
  return name === "noema-core" || name.startsWith("noema-");
}

function isAllowedTransition(
  source: string,
  destination: string,
  packageNames: ReadonlySet<string>,
): boolean {
  if (!packageNames.has("noema-core")) {
    return false;
  }

  if (
    source === "noema-server" &&
    destination === "noema-home" &&
    !packageNames.has("noema-host")
  ) {
    return true;
  }

  return (
    source === "noema-model-evals" &&
    destination === "noema-home" &&
    !packageNames.has("noema-runtime")
  );
}

function splitFeatureKey(featureKey: string): [string, string] {
  const separator = featureKey.indexOf("/");
  return [featureKey.slice(0, separator), featureKey.slice(separator + 1)];
}

function packageDeclaresFeature(pkg: CargoPackage | undefined, feature: string): boolean {
  return pkg !== undefined && Object.hasOwn(pkg.features, feature);
}

function directlyEnablesFeature(
  pkg: CargoPackage | undefined,
  dependencyName: string,
  feature: string,
): boolean {
  return pkg?.dependencies.some(
    (dependency) =>
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
  packageNames: ReadonlySet<string>,
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
    const compositionOwner = packageNames.has("noema-host")
      ? "noema-host"
      : "noema-core";
    if (source.name !== compositionOwner) {
      errors.add(`${source.name} may not enable implementation feature ${featureKey}`);
    }
    return;
  }

  if (featureKey === "noema-providers/local-model-evals") {
    const evaluationOwner = packageNames.has("noema-runtime")
      ? "noema-model-evals"
      : "noema-core";
    if (source.name !== evaluationOwner) {
      errors.add(`${source.name} may not enable evaluation feature ${featureKey}`);
    }
    return;
  }

  if (featureKey === "noema-runtime/eval-support") {
    if (source.name !== "noema-model-evals") {
      errors.add(`${source.name} may not enable evaluation feature ${featureKey}`);
    }
    return;
  }

  if (featureKey === "noema-host/composition") {
    if (source.name !== "noema-server" && source.name !== "noema-desktop") {
      errors.add(`${source.name} may not enable shell feature ${featureKey}`);
    }
    return;
  }

  if (featureKey === "noema-core/local-model-evals") {
    if (source.name !== "noema-model-evals" || packageNames.has("noema-runtime")) {
      errors.add(`${source.name} may not enable transitional feature ${featureKey}`);
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
  const hasCore = packageNames.has("noema-core");

  for (const packageName of packageNames) {
    if (packageName !== "noema-core" && !TARGET_DEPENDENCIES.has(packageName)) {
      errors.add(`unknown workspace package: ${packageName}`);
    }
  }

  if (!hasCore) {
    for (const packageName of TARGET_DEPENDENCIES.keys()) {
      if (!packageNames.has(packageName)) {
        errors.add(`post-core workspace is missing target package: ${packageName}`);
      }
    }
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
      (pkg.features.default?.length ?? 0) > 0
    ) {
      errors.add(`${pkg.name} must declare an empty default feature`);
    }

    for (const dependency of pkg.dependencies) {
      const dependencyName = dependency.name;

      if (dependencyName === "noema-core") {
        const evalTransitionExpired =
          pkg.name === "noema-model-evals" && packageNames.has("noema-runtime");
        if (
          !hasCore ||
          !TRANSITIONAL_CORE_CONSUMERS.has(pkg.name) ||
          evalTransitionExpired
        ) {
          errors.add(`${pkg.name} has expired or forbidden noema-core dependency`);
        }
      } else if (isNoemaPackage(dependencyName) && !packageNames.has(dependencyName)) {
        errors.add(
          `${pkg.name} depends on internal package outside the workspace: ${dependencyName}`,
        );
      } else if (isNoemaPackage(dependencyName)) {
        if (pkg.name === "noema-core") {
          if (!TARGET_DEPENDENCIES.has(dependencyName)) {
            errors.add(`noema-core has unknown internal dependency: ${dependencyName}`);
          }
        } else {
          const allowed = TARGET_DEPENDENCIES.get(pkg.name);
          if (
            !allowed?.has(dependencyName) &&
            !isAllowedTransition(pkg.name, dependencyName, packageNames)
          ) {
            errors.add(`${pkg.name} -> ${dependencyName} is not an allowed direct edge`);
          }
        }
      }

      const finalOwner = FINAL_DIRECT_DEPENDENCY_OWNERS.get(dependencyName);
      const isTransitionalCoreOwner =
        pkg.name === "noema-core" &&
        TRANSITIONAL_CORE_DEPENDENCIES.has(dependencyName) &&
        finalOwner !== undefined &&
        !packageNames.has(finalOwner);
      if (
        finalOwner &&
        pkg.name !== finalOwner &&
        !isTransitionalCoreOwner
      ) {
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
        dependency.features.some((feature) =>
          COMPOSITION_FEATURES.has(`${dependencyName}/${feature}`),
        )
      ) {
        errors.add(
          `noema-host must forward concrete dependency features only through noema-host/composition`,
        );
      }

      for (const feature of dependency.features) {
        validateInternalFeatureActivation(pkg, dependency, feature, packageNames, errors);
      }
    }

    for (const { dependency, feature } of forwardedDependencyFeatures(pkg)) {
      validateInternalFeatureActivation(pkg, dependency, feature, packageNames, errors);
    }
  }

  const compositionOwner = packageNames.has("noema-host")
    ? "noema-host"
    : hasCore
      ? "noema-core"
      : undefined;
  for (const featureKey of COMPOSITION_FEATURES) {
    const [dependencyName, feature] = splitFeatureKey(featureKey);
    const dependencyPackage = packagesByName.get(dependencyName);
    if (!dependencyPackage) {
      continue;
    }

    const mustDeclareFeature =
      FEATURE_REQUIRED_WITH_PACKAGE.has(featureKey) ||
      (featureKey === "noema-providers/local-models" && packageNames.has("noema-host"));
    const declaresFeature = packageDeclaresFeature(dependencyPackage, feature);
    if (mustDeclareFeature && !declaresFeature) {
      errors.add(`${dependencyName} must declare implementation feature ${feature}`);
    }
    if (!declaresFeature) {
      continue;
    }

    if (!compositionOwner) {
      errors.add(`no active composition owner enables implementation feature ${featureKey}`);
      continue;
    }
    const owner = packagesByName.get(compositionOwner);
    const ownerEnablesFeature =
      compositionOwner === "noema-host" && owner
        ? localFeatureForwardsDependencyFeature(
            owner,
            "composition",
            dependencyName,
            feature,
          )
        : directlyEnablesFeature(owner, dependencyName, feature);
    if (!ownerEnablesFeature) {
      errors.add(
        compositionOwner === "noema-host"
          ? `noema-host/composition must forward implementation feature ${featureKey}`
          : `${compositionOwner} must directly enable implementation feature ${featureKey}`,
      );
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
    const evaluationOwner = packageNames.has("noema-runtime")
      ? "noema-model-evals"
      : "noema-core";
    if (
      !directlyEnablesFeature(
        packagesByName.get(evaluationOwner),
        "noema-providers",
        "local-model-evals",
      )
    ) {
      errors.add(
        `${evaluationOwner} must directly enable evaluation feature noema-providers/local-model-evals`,
      );
    }
  } else if (packageNames.has("noema-runtime") && providers) {
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

  if (hasCore && !packageNames.has("noema-runtime")) {
    const core = packagesByName.get("noema-core");
    if (
      packageDeclaresFeature(core, "local-model-evals") &&
      !directlyEnablesFeature(
        packagesByName.get("noema-model-evals"),
        "noema-core",
        "local-model-evals",
      )
    ) {
      errors.add(
        "noema-model-evals must directly enable transitional feature noema-core/local-model-evals",
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
