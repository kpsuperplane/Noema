import { describe, expect, test } from "bun:test";

import {
  FOCUSED_TREE_POLICIES,
  cargoTreeArgs,
  parseTreeFacts,
  validateFocusedTree,
  validateTargetPackagePresence,
} from "./check-focused-dependency-trees";

function focusedPolicy(packageName: string) {
  const policy = FOCUSED_TREE_POLICIES.find(
    (candidate) => candidate.packageName === packageName,
  );
  if (!policy) {
    throw new Error(`missing focused policy for ${packageName}`);
  }
  return policy;
}

describe("focused dependency-tree policy", () => {
  test("parses exact package and feature facts without matching substrings", () => {
    const facts = parseTreeFacts(`
noema-store v0.1.0 (/workspace/crates/noema-store)
noema-providers feature "default"
not-reqwest v1.0.0
rusqlite v0.38.0
`);

    expect([...facts.packages].sort()).toEqual([
      "noema-store",
      "not-reqwest",
      "rusqlite",
    ]);
    expect([...facts.features]).toEqual(["noema-providers/default"]);
  });

  test("accepts provider-neutral contract edges", () => {
    const output = `
noema-tasks v0.1.0
noema-providers feature "default"
noema-providers v0.1.0
noema-artifacts v0.1.0
noema-conversations v0.1.0
serde v1.0.0
`;
    expect(validateFocusedTree(focusedPolicy("noema-tasks"), output)).toEqual(
      [],
    );
  });

  test("keeps the workspace contract tree isolated from backends", () => {
    const clean = `
noema-workspaces v0.1.0
serde v1.0.0
thiserror v2.0.0
`;
    expect(
      validateFocusedTree(focusedPolicy("noema-workspaces"), clean),
    ).toEqual([]);

    const leaked = `${clean}
rmcp v0.8.0
reqwest v0.12.0
rusqlite v0.38.0
`;
    expect(
      validateFocusedTree(focusedPolicy("noema-workspaces"), leaked),
    ).toEqual([
      "noema-workspaces contract-only tree contains forbidden package reqwest",
      "noema-workspaces contract-only tree contains forbidden package rmcp",
      "noema-workspaces contract-only tree contains forbidden package rusqlite",
    ]);
  });

  test("rejects hosted and local provider implementation leakage", () => {
    const output = `
noema-providers v0.1.0
noema-providers feature "adapters"
noema-providers feature "local-models"
reqwest v0.12.0
`;
    expect(validateFocusedTree(focusedPolicy("noema-providers"), output)).toEqual([
      "noema-providers contract-only tree contains forbidden feature noema-providers/adapters",
      "noema-providers contract-only tree contains forbidden feature noema-providers/local-models",
      "noema-providers contract-only tree contains forbidden package reqwest",
    ]);
  });

  test("rejects MCP transport and persistence leakage", () => {
    const output = `
noema-capabilities-mcp v0.1.0
noema-capabilities-mcp feature "transport"
rmcp v0.8.0
reqwest v0.12.0
rusqlite v0.38.0
`;
    expect(
      validateFocusedTree(focusedPolicy("noema-capabilities-mcp"), output),
    ).toEqual([
      "noema-capabilities-mcp contract-only tree contains forbidden feature noema-capabilities-mcp/transport",
      "noema-capabilities-mcp contract-only tree contains forbidden package reqwest",
      "noema-capabilities-mcp contract-only tree contains forbidden package rmcp",
      "noema-capabilities-mcp contract-only tree contains forbidden package rusqlite",
    ]);
  });

  test("allows SQLite in store while rejecting concrete subsystem backends", () => {
    const clean = `
noema-store v0.1.0
rusqlite feature "bundled"
rusqlite v0.38.0
noema-memory v0.1.0
`;
    expect(validateFocusedTree(focusedPolicy("noema-store"), clean)).toEqual([]);

    const leaked = `${clean}
noema-memory feature "service"
noema-artifacts feature "filesystem"
rmcp v0.8.0
`;
    expect(validateFocusedTree(focusedPolicy("noema-store"), leaked)).toEqual([
      "noema-store contract-only tree contains forbidden feature noema-artifacts/filesystem",
      "noema-store contract-only tree contains forbidden feature noema-memory/service",
      "noema-store contract-only tree contains forbidden package rmcp",
    ]);
  });

  test("allows runtime's store edge but rejects eval and provider implementations", () => {
    const output = `
noema-runtime v0.1.0
noema-store v0.1.0
rusqlite v0.38.0
noema-runtime feature "eval-support"
noema-providers feature "local-model-evals"
reqwest v0.12.0
`;
    expect(validateFocusedTree(focusedPolicy("noema-runtime"), output)).toEqual([
      "noema-runtime contract-only tree contains forbidden feature noema-providers/local-model-evals",
      "noema-runtime contract-only tree contains forbidden feature noema-runtime/eval-support",
      "noema-runtime contract-only tree contains forbidden package reqwest",
    ]);
  });

  test("rejects concrete backend leakage from host and API contract builds", () => {
    const output = `
noema-api v0.1.0
noema-host feature "composition"
noema-providers feature "local-models"
noema-memory feature "service"
reqwest v0.12.0
`;
    expect(validateFocusedTree(focusedPolicy("noema-api"), output)).toEqual([
      "noema-api contract-only tree contains forbidden feature noema-host/composition",
      "noema-api contract-only tree contains forbidden feature noema-memory/service",
      "noema-api contract-only tree contains forbidden feature noema-providers/local-models",
      "noema-api contract-only tree contains forbidden package reqwest",
    ]);
    expect(validateFocusedTree(focusedPolicy("noema-host"), output)).toEqual([
      "noema-host contract-only tree contains forbidden feature noema-host/composition",
      "noema-host contract-only tree contains forbidden feature noema-memory/service",
      "noema-host contract-only tree contains forbidden feature noema-providers/local-models",
      "noema-host contract-only tree contains forbidden package reqwest",
    ]);
  });

  test("requires every final target package", () => {
    const errors = validateTargetPackagePresence(new Set(["noema-home"]));
    expect(errors).toContain(
      "workspace is missing target package: noema-runtime",
    );
    expect(errors).toContain(
      "workspace is missing target package: noema-workspaces",
    );
  });

  test("always asks Cargo for an isolated no-default feature tree", () => {
    expect(cargoTreeArgs("noema-store")).toEqual([
      "tree",
      "--locked",
      "--offline",
      "--target",
      "all",
      "-p",
      "noema-store",
      "--no-default-features",
      "-e",
      "normal,build,features",
      "--prefix",
      "none",
      "--charset",
      "ascii",
    ]);
  });
});
