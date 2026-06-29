import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Composer,
  composerDraftInlineSize,
  composerSubmitState,
  composerTextareaProps,
  isComposerTextareaDisabled,
  refocusComposerTextarea
} from "./Composer";

describe("composer disabled states", () => {
  test("keeps textarea and submit available while an agent turn is pending", () => {
    assert.equal(isComposerTextareaDisabled({ ready: true }), false);
    assert.deepEqual(composerSubmitState({ ready: true, value: "Interrupt", pending: true }), {
      disabled: false,
      label: "Sending message"
    });
  });

  test("disables submit only when unavailable or blank", () => {
    assert.equal(composerSubmitState({ ready: false, value: "Hello", pending: false }).disabled, true);
    assert.equal(composerSubmitState({ ready: true, value: "   ", pending: false }).disabled, true);
  });
});

describe("composer submit presentation", () => {
  test("uses accessible labels for the icon-only send button", () => {
    assert.equal(composerSubmitState({ ready: true, value: "Hello", pending: false }).label, "Send message");
    assert.equal(composerSubmitState({ ready: true, value: "Hello", pending: true }).label, "Sending message");
  });
});

describe("composer textarea presentation", () => {
  test("starts as one line with local compact sizing", () => {
    const props = composerTextareaProps();

    assert.equal(props.rows, 1);
    assert.match(props.className, /min-h-9/);
    assert.match(props.className, /max-h-40/);
  });
});

describe("composer human bubble presentation", () => {
  test("renders the composer as a right-aligned green human bubble", () => {
    const markup = renderComposer();

    const shellClassName = dataSlotClassName(markup, "composer-shell");
    assert.match(shellClassName, /\bjustify-end\b/);
    assert.doesNotMatch(shellClassName, /\bborder-t\b/);

    const bubbleClassName = dataSlotClassName(markup, "composer-bubble");
    assert.match(bubbleClassName, /\bflex\b/);
    assert.match(bubbleClassName, /\bw-fit\b/);
    assert.match(bubbleClassName, /min-w-\[min\(13rem,100%\)\]/);
    assert.match(bubbleClassName, /max-w-\[88%\]/);
    assert.match(bubbleClassName, /\bbg-primary\b/);
    assert.match(bubbleClassName, /\brounded-3xl\b/);
    assert.doesNotMatch(bubbleClassName, /minmax\(0,auto\)/);

    const textareaClassName = dataSlotClassName(markup, "textarea");
    assert.match(textareaClassName, /\bfield-sizing-content\b/);
    assert.match(textareaClassName, /\bw-auto\b/);
    assert.match(textareaClassName, /\bbg-transparent\b/);
    assert.match(textareaClassName, /\btext-primary-foreground\b/);
    assert.match(markup, /<textarea[^>]*style="width:14ch"/);

    assert.match(markup, /<button[^>]*class="[^"]*\bbg-primary-foreground\b/);
  });
});

describe("composer draft bubble sizing", () => {
  test("starts compact from the placeholder, then expands with longer drafts", () => {
    assert.equal(composerDraftInlineSize({ value: "", placeholder: "Message Noema" }), "15ch");
    assert.equal(composerDraftInlineSize({ value: "Tiny", placeholder: "Message Noema" }), "14ch");
    assert.equal(
      composerDraftInlineSize({
        value: "This draft is long enough to widen the human bubble",
        placeholder: "Message Noema"
      }),
      "53ch"
    );
  });

  test("caps width and uses the longest line for multiline drafts", () => {
    assert.equal(
      composerDraftInlineSize({
        value: "short\nthis line is the one that should set the width",
        placeholder: "Message Noema"
      }),
      "48ch"
    );
    assert.equal(composerDraftInlineSize({ value: "x".repeat(90), placeholder: "Message Noema" }), "58ch");
  });
});

describe("refocusComposerTextarea", () => {
  test("schedules focus after submit handling", () => {
    let scheduled = false;
    let focused = false;

    refocusComposerTextarea(
      { focus: () => (focused = true) },
      (callback) => {
        scheduled = true;
        assert.equal(focused, false);
        callback();
      }
    );

    assert.equal(scheduled, true);
    assert.equal(focused, true);
  });
});

function renderComposer(): string {
  return renderToStaticMarkup(
    React.createElement(Composer, {
      value: "Hello Noema",
      ready: true,
      pending: false,
      placeholder: "Message Noema",
      onChange: () => {},
      onSubmit: () => {}
    })
  );
}

function dataSlotClassName(markup: string, slot: string): string {
  const match = markup.match(new RegExp(`data-slot="${slot}"[^>]*class="([^"]*)"`));
  assert.ok(match, `expected markup to include ${slot}`);
  return match[1];
}
