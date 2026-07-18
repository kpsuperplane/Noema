import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  createTestInventory,
  loadTestInventory,
  parseRustSourceTestDeclarations,
  parseRustUnitTestLeaves,
  validateRustSourceTestInventory,
  validateQualifiedTestInventory,
  validateTestInventory,
} from "./verify-rust-test-inventory";

const BASELINE = `
conversation::tests::rejects_empty_title: test
store::tests::preserves_rows: test
runtime::tests::preserves_rows: test
src/lib.rs - example (line 12): test
benchmark_case: benchmark
`;

function expectInvalidInventory(inventory: unknown): void {
  const directory = mkdtempSync(join(tmpdir(), "noema-test-inventory-"));
  const path = join(directory, "inventory.json");
  try {
    writeFileSync(path, JSON.stringify(inventory));
    expect(() => loadTestInventory(path)).toThrow(
      "invalid Rust unit-test inventory",
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

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

  test("rejects a leaf alias shared by multiple subsystems", () => {
    const inventory = createTestInventory("legacy::fetch_contract: test\n");
    inventory.acceptedAliases.fetch_contract = ["shared_parser_contract"];
    const errors = validateTestInventory(
      inventory,
      `
web::fetch::tests::shared_parser_contract: test
web::search::tests::shared_parser_contract: test
`,
    );

    expect(errors).toContain(
      "fetch_contract accepted alias shared_parser_contract is ambiguous: matched 2 qualified tests; use a module-qualified alias",
    );
    expect(errors).toContain(
      "fetch_contract lost owned tests: expected at least 1 across [fetch_contract, shared_parser_contract], found 0",
    );
  });

  test("accepts an unambiguous module-qualified alias", () => {
    const inventory = createTestInventory("legacy::fetch_contract: test\n");
    inventory.acceptedAliases.fetch_contract = [
      "web::fetch::tests::shared_parser_contract",
    ];

    expect(
      validateTestInventory(
        inventory,
        `
web::fetch::tests::shared_parser_contract: test
web::search::tests::shared_parser_contract: test
`,
      ),
    ).toEqual([]);
  });

  test("requires every owner in a split alias", () => {
    const inventory = createTestInventory("legacy::combined_contract: test\n");
    inventory.acceptedAliases.combined_contract = [
      "first_contract",
      "second_contract",
    ];

    expect(
      validateTestInventory(inventory, "module::first_contract: test\n"),
    ).toContain("combined_contract lost required split owner second_contract");
  });

  test("crate-qualified ownership rejects a same-named wrong-crate test", () => {
    const oldId = "old-crate/old_target::tests::contract";
    const expectedId = "expected-crate/expected_target::tests::contract";
    const wrongId = "wrong-crate/wrong_target::tests::contract";
    const inventory = createTestInventory(
      "",
      new Map(),
      new Map([[oldId, 1]]),
      new Map([[oldId, 1]]),
    );
    inventory.acceptedQualifiedAliases[oldId] = [expectedId];

    const errors = validateQualifiedTestInventory(
      inventory,
      new Map([[wrongId, 1]]),
      new Map([[wrongId, 1]]),
    );
    expect(errors).toContain(
      `${oldId} lost default qualified tests: expected at least 1 across [${oldId}, ${expectedId}], found 0`,
    );
    expect(errors).toContain(
      `${wrongId} is an unowned current default qualified test`,
    );

    expect(
      validateQualifiedTestInventory(
        inventory,
        new Map([[expectedId, 1]]),
        new Map([[expectedId, 1]]),
      ),
    ).toEqual([]);
  });

  test("tracks default and all-feature test sets independently", () => {
    const defaultId = "server/server::tests::authority_boundary";
    const developmentId = "server/server::tests::development_boundary";
    const inventory = createTestInventory(
      "",
      new Map(),
      new Map([[defaultId, 1]]),
      new Map([[developmentId, 1]]),
    );

    expect(
      validateQualifiedTestInventory(
        inventory,
        new Map([[defaultId, 1]]),
        new Map([[developmentId, 1]]),
      ),
    ).toEqual([]);
    expect(
      validateQualifiedTestInventory(
        inventory,
        new Map([[developmentId, 1]]),
        new Map([[defaultId, 1]]),
      ),
    ).not.toEqual([]);
  });

  test("requires every crate-qualified split owner", () => {
    const oldId = "old/old::tests::combined";
    const first = "new/new::tests::first";
    const second = "new/new::tests::second";
    const inventory = createTestInventory(
      "",
      new Map(),
      new Map([[oldId, 1]]),
      new Map([[oldId, 1]]),
    );
    inventory.acceptedQualifiedAliases[oldId] = [first, second];

    expect(
      validateQualifiedTestInventory(
        inventory,
        new Map([[first, 1]]),
        new Map([
          [first, 1],
          [second, 1],
        ]),
      ),
    ).toContain(`${oldId} lost required default qualified split owner ${second}`);
  });

  test("source inventory includes feature-gated test declarations", () => {
    const sourceTests = parseRustSourceTestDeclarations(
      new Map([
        [
          "crates/example/src/lib.rs",
          `
#[cfg(feature = "local")]
#[tokio::test(flavor = "multi_thread")]
async fn preserves_feature_contract() {}

#[test]
fn preserves_default_contract() {}
`,
        ],
      ]),
    );

    expect([...sourceTests.entries()]).toEqual([
      ["crates/example/src/lib.rs::preserves_feature_contract", 1],
      ["crates/example/src/lib.rs::preserves_default_contract", 1],
    ]);
  });

  test("source inventory requires an explicit path-qualified move", () => {
    const oldId = "crates/old/src/lib.rs::preserves_contract";
    const newId = "crates/new/src/lib.rs::preserves_contract";
    const inventory = createTestInventory("", new Map([[oldId, 1]]));
    const current = new Map([[newId, 1]]);

    const errors = validateRustSourceTestInventory(inventory, current);
    expect(errors).toContain(
      `${oldId} lost source tests: expected at least 1 across [${oldId}], found 0`,
    );
    expect(errors).toContain(`${newId} is an unowned current source test`);

    inventory.acceptedSourceAliases[oldId] = [newId];
    expect(validateRustSourceTestInventory(inventory, current)).toEqual([]);
  });

  test("source inventory rejects a newly added unowned declaration", () => {
    const ownedId = "crates/example/src/lib.rs::owned_contract";
    const unownedId = "crates/example/src/lib.rs::unowned_contract";
    const inventory = createTestInventory("", new Map([[ownedId, 1]]));

    expect(
      validateRustSourceTestInventory(
        inventory,
        new Map([
          [ownedId, 1],
          [unownedId, 1],
        ]),
      ),
    ).toContain(`${unownedId} is an unowned current source test`);
  });

  test("skips only requirements with an explicit retirement", () => {
    const inventory = createTestInventory(BASELINE);
    inventory.retiredRequirements.rejects_empty_title =
      "The product contract was intentionally removed.";

    expect(
      validateTestInventory(
        inventory,
        `
store::preserves_rows: test
runtime::preserves_rows: test
`,
      ),
    ).toEqual([]);
  });

  test("rejects retirement IDs absent from the requirement baseline", () => {
    const inventory = createTestInventory(BASELINE);
    inventory.retiredRequirements.unknown_requirement =
      "This ID was never part of the baseline.";

    expectInvalidInventory(inventory);
  });

  test("rejects aliases on explicitly retired requirements", () => {
    const inventory = createTestInventory(BASELINE);
    inventory.retiredRequirements.rejects_empty_title =
      "The product contract was intentionally removed.";
    inventory.acceptedAliases.rejects_empty_title = ["rejects_blank_title"];

    expectInvalidInventory(inventory);
  });
});
