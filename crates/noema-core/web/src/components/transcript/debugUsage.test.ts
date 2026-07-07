import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { formatCacheHitRatio, parseProviderUsageDebug, providerUsageDebugRows } from "./debugUsage";

describe("parseProviderUsageDebug", () => {
  test("accepts valid provider usage metadata", () => {
    const parsed = parseProviderUsageDebug({
      provider_usage: {
        provider: "codex",
        model: "gpt-test",
        phase: "initial",
        response_index: 0,
        output_index: 0,
        input_tokens: 12000,
        cached_input_tokens: 9600,
        cache_hit_ratio: 0.8,
        output_tokens: 900,
        total_tokens: 12900
      }
    });

    assert.deepEqual(parsed, {
      provider: "codex",
      model: "gpt-test",
      phase: "initial",
      responseIndex: 0,
      outputIndex: 0,
      inputTokens: 12000,
      cachedInputTokens: 9600,
      cacheHitRatio: 0.8,
      outputTokens: 900,
      totalTokens: 12900
    });
  });

  test("preserves explicit zero cached tokens", () => {
    const parsed = parseProviderUsageDebug({
      provider_usage: {
        provider: "codex",
        model: "gpt-test",
        phase: "continuation",
        response_index: 1,
        input_tokens: 2048,
        cached_input_tokens: 0,
        cache_hit_ratio: 0,
        output_tokens: 12,
        total_tokens: 2060
      }
    });

    assert.equal(parsed?.cachedInputTokens, 0);
    assert.equal(parsed?.cacheHitRatio, 0);
    assert.equal(formatCacheHitRatio(parsed?.cacheHitRatio ?? null), "0%");
  });

  test("rejects malformed provider usage metadata", () => {
    assert.equal(parseProviderUsageDebug(null), null);
    assert.equal(parseProviderUsageDebug({ provider_usage: "bad" }), null);
    assert.equal(
      parseProviderUsageDebug({
        provider_usage: {
          provider: "codex",
          model: "gpt-test",
          phase: "initial",
          response_index: 0,
          input_tokens: "12000",
          output_tokens: 900,
          total_tokens: 12900
        }
      }),
      null
    );
  });

  test("formats rows without inventing cache hits", () => {
    const parsed = parseProviderUsageDebug({
      provider_usage: {
        provider: "codex",
        model: "gpt-test",
        phase: "initial",
        response_index: 0,
        input_tokens: 12000,
        output_tokens: 900,
        total_tokens: 12900
      }
    });

    assert.ok(parsed);
    const rows = providerUsageDebugRows(parsed);
    assert.equal(rows.some((row) => row.label === "Cache hit ratio"), false);
    assert.equal(formatCacheHitRatio(null), "Unavailable");
  });
});
