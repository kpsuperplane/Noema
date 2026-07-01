import React from "react";
import { SendHorizontal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

const composerMinWidthCh = 18;
const composerWidthBufferCh = 5;
const composerWidthBufferPx = 32;
const composerMeasuredTextSlackPx = 4;

type ComposerInlineSize = {
  minWidth: string;
  width: string;
};

const useBrowserLayoutEffect = typeof window === "undefined" ? React.useEffect : React.useLayoutEffect;

export function isComposerTextareaDisabled({ ready }: { ready: boolean }) {
  return !ready;
}

export function composerSubmitState({
  ready,
  pending
}: {
  ready: boolean;
  pending: boolean;
}) {
  return {
    disabled: !ready,
    label: pending ? "Sending message" : "Send message"
  };
}

export function composerTextareaProps() {
  return {
    rows: 1,
    className:
      "min-h-9 min-w-0 w-full resize-none overflow-y-hidden border-transparent bg-transparent px-2.5 py-1.5 text-base leading-6 text-primary-foreground placeholder:text-primary-foreground/70 focus-visible:border-transparent focus-visible:ring-0 disabled:opacity-70"
  };
}

export function composerDraftInlineSize({
  value,
  placeholder,
  measureText,
  widthBufferPx = composerWidthBufferPx
}: {
  value: string;
  placeholder: string;
  measureText?: (text: string) => number;
  widthBufferPx?: number;
}): ComposerInlineSize {
  const content = value.length > 0 ? value : placeholder;
  const longestLine = longestDraftLine(content);
  const longestPlaceholderLine = longestDraftLine(placeholder);

  if (measureText) {
    const placeholderWidth = Math.ceil(measureText(longestPlaceholderLine) + widthBufferPx);
    const contentWidth = Math.ceil(measureText(longestLine) + widthBufferPx);
    return {
      minWidth: `${placeholderWidth}px`,
      width: `${Math.max(placeholderWidth, contentWidth)}px`
    };
  }

  const minWidthInCh = Math.max(
    composerMinWidthCh,
    Array.from(longestPlaceholderLine).length + composerWidthBufferCh
  );
  const widthInCh = Math.max(minWidthInCh, Array.from(longestLine).length + composerWidthBufferCh);
  return { minWidth: `${minWidthInCh}ch`, width: `${widthInCh}ch` };
}

export function composerMeasuredWidthBuffer(textareaPaddingInlinePx: number): number {
  return textareaPaddingInlinePx + composerMeasuredTextSlackPx;
}

function longestDraftLine(content: string): string {
  return content.split(/\r\n|\n|\r/).reduce((longest, line) => {
    return Array.from(line).length > Array.from(longest).length ? line : longest;
  }, "");
}

export function composerTextareaStyle(): React.CSSProperties {
  return {
    fieldSizing: "content"
  };
}

export function composerTextareaWrapStyle({
  inlineSize,
  value,
  placeholder
}: {
  inlineSize?: ComposerInlineSize;
  value: string;
  placeholder: string;
}): React.CSSProperties {
  const size = inlineSize ?? composerDraftInlineSize({ value, placeholder });
  return {
    width: `min(${size.width}, 100%)`,
    minWidth: `min(${size.minWidth}, 100%)`
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

export function assignComposerTextareaRef(
  forwardedRef: React.ForwardedRef<HTMLTextAreaElement>,
  textarea: HTMLTextAreaElement | null
) {
  if (typeof forwardedRef === "function") {
    forwardedRef(textarea);
    return;
  }

  if (forwardedRef) {
    forwardedRef.current = textarea;
  }
}

export type ComposerProps = {
  value: string;
  ready: boolean;
  pending: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSubmit: (value: string) => void;
};

export const Composer = React.forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  {
    value,
    ready,
    pending,
    placeholder,
    onChange,
    onSubmit
  },
  forwardedRef
) {
  const textareaRef = React.useRef<HTMLTextAreaElement>(null);
  const setTextareaRef = React.useCallback(
    (textarea: HTMLTextAreaElement | null) => {
      textareaRef.current = textarea;
      assignComposerTextareaRef(forwardedRef, textarea);
    },
    [forwardedRef]
  );

  function submit() {
    const nextValue = textareaRef.current?.value ?? value;
    if (!nextValue.trim()) {
      refocusComposerTextarea(textareaRef.current);
      return;
    }

    onSubmit(nextValue);
    refocusComposerTextarea(textareaRef.current);
  }

  const submitState = composerSubmitState({ ready, pending });
  const textareaProps = composerTextareaProps();
  const sizeKey = `${value}\u0000${placeholder}`;
  const fallbackInlineSize = React.useMemo(
    () => composerDraftInlineSize({ value, placeholder }),
    [value, placeholder]
  );
  const [measuredInlineSize, setMeasuredInlineSize] = React.useState<{
    key: string;
    size: ComposerInlineSize;
  } | null>(null);

  useBrowserLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea || typeof document === "undefined") {
      return;
    }
    const context = document.createElement("canvas").getContext("2d");
    if (!context) {
      return;
    }

    const styles = window.getComputedStyle(textarea);
    context.font = styles.font;
    const padding =
      Number.parseFloat(styles.paddingLeft || "0") + Number.parseFloat(styles.paddingRight || "0");
    const nextSize = composerDraftInlineSize({
      value,
      placeholder,
      measureText: (text) => context.measureText(text || " ").width,
      widthBufferPx: composerMeasuredWidthBuffer(padding)
    });
    setMeasuredInlineSize((previous) =>
      previous?.key === sizeKey &&
      previous.size.width === nextSize.width &&
      previous.size.minWidth === nextSize.minWidth
        ? previous
        : { key: sizeKey, size: nextSize }
    );
  }, [placeholder, sizeKey, value]);

  const textareaStyle = composerTextareaStyle();
  const textareaWrapStyle = composerTextareaWrapStyle({
    value,
    placeholder,
    inlineSize: measuredInlineSize?.key === sizeKey ? measuredInlineSize.size : fallbackInlineSize
  });

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
        className="relative w-fit min-w-[min(13rem,100%)] max-w-full rounded-4xl bg-primary p-1.5 pr-12 text-primary-foreground shadow-[0_8px_24px_rgba(23,22,15,0.08)]"
      >
        <div data-slot="composer-textarea-wrap" className="min-w-0" style={textareaWrapStyle}>
          <Textarea
            ref={setTextareaRef}
            value={value}
            disabled={isComposerTextareaDisabled({ ready })}
            placeholder={placeholder}
            enterKeyHint="send"
            rows={textareaProps.rows}
            style={textareaStyle}
            className={textareaProps.className}
            onChange={(event) => onChange(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                submit();
              }
            }}
          />
        </div>
        <Button
          data-slot="composer-submit"
          type="button"
          size="icon-lg"
          className="absolute right-1.5 bottom-1.5 touch-manipulation rounded-full bg-primary-foreground text-primary hover:bg-primary-foreground/90 disabled:text-primary/70"
          aria-label={submitState.label}
          disabled={submitState.disabled}
          onClick={submit}
        >
          <SendHorizontal aria-hidden="true" />
        </Button>
      </div>
    </form>
  );
});
