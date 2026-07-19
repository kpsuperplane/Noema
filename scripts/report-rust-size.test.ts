import { describe, expect, test } from "bun:test";

import {
  estimateTestLines,
  isDedicatedTestPath,
  summarizeSnapshot,
  type SourceSnapshot,
} from "./report-rust-size";

describe("Rust size reporting", () => {
  test("classifies dedicated test paths", () => {
    expect(isDedicatedTestPath("crates/example/src/tests/store.rs")).toBe(true);
    expect(isDedicatedTestPath("crates/example/src/store_tests.rs")).toBe(true);
    expect(isDedicatedTestPath("crates/example/src/test_support.rs")).toBe(true);
    expect(isDedicatedTestPath("crates/example/src/store.rs")).toBe(false);
  });

  test("estimates an inline test module at the module tail", () => {
    const source = [
      "pub fn value() -> usize { 1 }",
      "",
      "#[cfg(test)]",
      "mod tests {",
      "    #[test]",
      "    fn value_is_one() {}",
      "}",
      "",
    ].join("\n");
    expect(estimateTestLines("crates/example/src/lib.rs", source)).toBe(5);
  });

  test("summarizes production, test, and declaration lines", () => {
    const sources: SourceSnapshot = new Map([
      ["crates/example/src/lib.rs", "pub fn value() -> usize { 1 }\n"],
      [
        "crates/example/src/tests.rs",
        "#[test]\nfn value_is_one() {}\n\n#[tokio::test]\nasync fn async_value_is_one() {}\n",
      ],
    ]);
    expect(summarizeSnapshot(sources)).toMatchObject({
      totalLines: 6,
      productionLines: 1,
      testLines: 5,
      testDeclarations: 2,
    });
  });
});
