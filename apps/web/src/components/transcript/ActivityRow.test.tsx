import assert from "node:assert/strict";
import { test } from "node:test";
// @ts-expect-error Bun provides this test-only module without shipping ambient types here.
import { mock } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { TurnTranscriptItem } from "@/shared/types";

mock.module("@stylexjs/stylex", () => ({
  create: (styles: Record<string, unknown>) => styles,
  props: () => ({})
}));

const { ActivityRow } = await import("./ActivityRow");
mock.restore();

test("ActivityRow renders neutral checkpoint detail only when expanded", () => {
  const item: Extract<TurnTranscriptItem, { kind: "activity" }> = {
    kind: "activity",
    id: "checkpoint-1",
    activity_kind: "context_checkpoint",
    status: "COMPLETED",
    title: "Context compacted",
    summary: "Retained task context",
    metadata: {
      detail: "complete persisted checkpoint detail",
      presentation: { tone: "neutral" }
    }
  };

  const collapsed = renderToStaticMarkup(<ActivityRow item={item} open={false} onToggle={() => undefined} />);
  assert.match(collapsed, /data-tone="default"/);
  assert.match(collapsed, /aria-expanded="false"/);
  assert.doesNotMatch(collapsed, /complete persisted checkpoint detail/);

  const expanded = renderToStaticMarkup(<ActivityRow item={item} open onToggle={() => undefined} />);
  assert.match(expanded, /aria-expanded="true"/);
  assert.match(expanded, /complete persisted checkpoint detail/);
  assert.match(expanded, /id="checkpoint-1-detail"/);
});
