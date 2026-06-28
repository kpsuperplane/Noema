import React from "react";
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
    label: pending ? "Sending" : "Send"
  };
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

  return (
    <form
      className="mx-auto grid w-[var(--chat-column-width)] grid-cols-[minmax(0,1fr)_auto] items-end gap-2.5 border-t border-[var(--border-subtle)] bg-background pt-3.5 max-[760px]:grid-cols-1"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <Textarea
        ref={textareaRef}
        value={value}
        disabled={isComposerTextareaDisabled({ ready })}
        placeholder={placeholder}
        rows={3}
        onChange={(event) => onChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            submit();
          }
        }}
      />
      <Button type="submit" disabled={submitState.disabled}>
        {submitState.label}
      </Button>
    </form>
  );
}
