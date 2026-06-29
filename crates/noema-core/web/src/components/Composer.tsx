import React from "react";
import { SendHorizontal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

export function isComposerTextareaDisabled({ ready }: { ready: boolean }) {
  return !ready;
}

export function composerSubmitState({
  ready,
  value,
  pending
}: {
  ready: boolean;
  value: string;
  pending: boolean;
}) {
  return {
    disabled: !ready || !value.trim(),
    label: pending ? "Sending message" : "Send message"
  };
}

export function composerTextareaProps() {
  return {
    rows: 1,
    className:
      "min-h-9 max-h-40 min-w-0 w-auto max-w-[min(58ch,calc(var(--chat-column-width)_-_5rem))] overflow-y-auto border-transparent bg-transparent px-2.5 py-1.5 leading-6 text-primary-foreground placeholder:text-primary-foreground/70 focus-visible:border-transparent focus-visible:ring-0 disabled:opacity-70"
  };
}

export function composerDraftInlineSize({
  value,
  placeholder
}: {
  value: string;
  placeholder: string;
}): string {
  const content = value.length > 0 ? value : placeholder;
  const longestLine = content
    .split(/\r\n|\n|\r/)
    .reduce((longest, line) => Math.max(longest, Array.from(line).length), 0);
  const widthInCh = Math.min(58, Math.max(14, longestLine + 2));
  return `${widthInCh}ch`;
}

export function refocusComposerTextarea(
  textarea: Pick<HTMLTextAreaElement, "focus"> | null,
  schedule: (callback: () => void) => void = (callback) => window.requestAnimationFrame(callback)
) {
  if (!textarea) {
    return;
  }

  schedule(() => {
    textarea.focus();
  });
}

export function Composer({
  value,
  ready,
  pending,
  placeholder,
  onChange,
  onSubmit
}: {
  value: string;
  ready: boolean;
  pending: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
}) {
  const textareaRef = React.useRef<HTMLTextAreaElement>(null);

  function submit() {
    onSubmit();
    refocusComposerTextarea(textareaRef.current);
  }

  const submitState = composerSubmitState({ ready, value, pending });
  const textareaProps = composerTextareaProps();
  const textareaInlineSize = composerDraftInlineSize({ value, placeholder });

  return (
    <form
      data-slot="composer-shell"
      className="mx-auto flex w-[var(--chat-column-width)] justify-end pt-3.5"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div
        data-slot="composer-bubble"
        className="flex w-fit min-w-[min(13rem,100%)] max-w-[88%] items-end gap-1.5 rounded-3xl bg-primary p-1.5 text-primary-foreground shadow-[0_8px_24px_rgba(23,22,15,0.08)] max-[760px]:max-w-full"
      >
        <Textarea
          ref={textareaRef}
          value={value}
          disabled={isComposerTextareaDisabled({ ready })}
          placeholder={placeholder}
          rows={textareaProps.rows}
          style={{ width: textareaInlineSize }}
          className={textareaProps.className}
          onChange={(event) => onChange(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              submit();
            }
          }}
        />
        <Button
          type="submit"
          size="icon-lg"
          className="rounded-full bg-primary-foreground text-primary hover:bg-primary-foreground/90 disabled:text-primary/70"
          aria-label={submitState.label}
          disabled={submitState.disabled}
        >
          <SendHorizontal aria-hidden="true" />
        </Button>
      </div>
    </form>
  );
}
