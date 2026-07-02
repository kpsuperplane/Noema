import React from "react";
import { SendHorizontal } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { TextArea } from "@astryxdesign/core/TextArea";
import * as stylex from "@stylexjs/stylex";

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
    rows: 1
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
  const textareaNativeProps = {
    enterKeyHint: "send"
  } as const;

  return (
    <form
      data-slot="composer-shell"
      {...stylex.props(styles.shell)}
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div
        data-slot="composer-bubble"
        {...stylex.props(styles.bubble)}
      >
        <div
          data-slot="composer-textarea-wrap"
          {...stylex.props(styles.textareaWrap)}
          style={textareaWrapStyle}
        >
          <TextArea
            ref={setTextareaRef}
            data-slot="composer-textarea"
            label="Message"
            isLabelHidden
            value={value}
            isDisabled={isComposerTextareaDisabled({ ready })}
            placeholder={placeholder}
            rows={textareaProps.rows}
            style={textareaStyle}
            xstyle={styles.textareaChrome as never}
            {...textareaNativeProps}
            onChange={(nextValue) => onChange(nextValue)}
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
          label={submitState.label}
          isIconOnly
          icon={<SendHorizontal aria-hidden="true" />}
          size="lg"
          variant="secondary"
          xstyle={styles.submit as never}
          isDisabled={submitState.disabled}
          onClick={submit}
        />
      </div>
    </form>
  );
});

const styles = stylex.create({
  shell: {
    display: "flex",
    justifyContent: "flex-end",
    width: "var(--chat-column-width)",
    marginInline: "auto",
    paddingTop: 14
  },
  bubble: {
    position: "relative",
    width: "fit-content",
    minWidth: "min(13rem, 100%)",
    maxWidth: "100%",
    borderRadius: "calc(var(--radius) * 2.6)",
    backgroundColor: "var(--primary)",
    padding: 6,
    paddingRight: {
      default: 48,
      "@media (hover: none) and (pointer: coarse)": 56
    },
    color: "var(--primary-foreground)",
    boxShadow: "0 8px 24px rgba(23, 22, 15, 0.08)"
  },
  textareaWrap: {
    minWidth: 0
  },
  textareaChrome: {
    "--color-text-primary": "var(--primary-foreground)",
    "--color-text-secondary": "color-mix(in srgb, var(--primary-foreground) 70%, transparent)",
    minHeight: 36,
    minWidth: 0,
    width: "100%",
    borderColor: "transparent",
    backgroundColor: "transparent",
    paddingBlock: 6,
    paddingInline: 10,
    opacity: {
      default: 1,
      ":disabled": 0.7
    },
    outline: {
      default: null,
      ":focus-visible": "none"
    },
    boxShadow: "none"
  },
  submit: {
    position: "absolute",
    right: {
      default: 6,
      "@media (hover: none) and (pointer: coarse)": 4
    },
    bottom: {
      default: 6,
      "@media (hover: none) and (pointer: coarse)": 4
    },
    width: {
      default: 36,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    height: {
      default: 36,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    touchAction: "manipulation",
    borderRadius: 999,
    backgroundColor: "var(--primary-foreground)",
    color: {
      default: "var(--primary)",
      ":disabled": "color-mix(in srgb, var(--primary) 70%, transparent)"
    },
    backgroundImage: {
      default: null,
      ":hover": {
        "@media (hover: hover)": "linear-gradient(rgba(255, 255, 255, 0.1), rgba(255, 255, 255, 0.1))"
      }
    }
  }
});
