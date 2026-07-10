import { describe, expect, test } from "bun:test";

import { boundedCleanup } from "./cleanup";

describe("bounded cleanup", () => {
  test("rejects a hung operation at its cleanup deadline", async () => {
    await expect(
      boundedCleanup(new Promise<void>(() => {}), 5)
    ).rejects.toThrow(
      "cleanup timed out"
    );
  });
});
