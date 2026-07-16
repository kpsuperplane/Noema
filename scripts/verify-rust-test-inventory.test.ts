import { describe, expect, test } from "bun:test";

import {
  createTestInventory,
  parseRustUnitTestLeaves,
  validateTestInventory,
} from "./verify-rust-test-inventory";

const BASELINE = `
conversation::tests::rejects_empty_title: test
store::tests::preserves_rows: test
runtime::tests::preserves_rows: test
src/lib.rs - example (line 12): test
benchmark_case: benchmark
`;

describe("Rust unit-test ownership inventory", () => {
  test("normalizes module moves to stable leaf names and excludes doctests", () => {
    expect([...parseRustUnitTestLeaves(BASELINE).entries()]).toEqual([
      ["rejects_empty_title", 1],
      ["preserves_rows", 2],
    ]);
  });

  test("accepts crate and module moves without lowering owned counts", () => {
    const inventory = createTestInventory(BASELINE);
    const moved = `
noema_conversations::records::rejects_empty_title: test
noema_store::rows::preserves_rows: test
noema_runtime::persistence::preserves_rows: test
new_behavior: test
`;
    expect(validateTestInventory(inventory, moved)).toEqual([]);
  });

  test("fails when a normalized owned behavior disappears", () => {
    const inventory = createTestInventory(BASELINE);
    const errors = validateTestInventory(
      inventory,
      "noema_store::rows::preserves_rows: test\n",
    );
    expect(errors).toEqual([
      "preserves_rows lost owned tests: expected at least 2 across [preserves_rows], found 1",
      "rejects_empty_title lost owned tests: expected at least 1 across [rejects_empty_title], found 0",
    ]);
  });

  test("supports explicit reviewed rename and split aliases", () => {
    const inventory = createTestInventory(BASELINE);
    inventory.acceptedAliases.rejects_empty_title = ["rejects_blank_title"];
    expect(
      validateTestInventory(
        inventory,
        `
conversation::rejects_blank_title: test
store::preserves_rows: test
runtime::preserves_rows: test
`,
      ),
    ).toEqual([]);
  });
});
