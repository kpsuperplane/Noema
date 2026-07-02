import React from "react";
import { SendHorizontal } from "lucide-react";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import { TextArea, type TextAreaProps } from "@astryxdesign/core/TextArea";
import * as stylex from "@stylexjs/stylex";

const composerMinWidthCh = 18;
const composerWidthBufferCh = 5;
const composerWidthBufferPx = 32;
const composerMeasuredTextSlackPx = 4;
const composerBubbleInlineReservePx = 54;
const composerMinTextHeightPx = 24;
const composerTextareaFontSize = "1rem";
const composerTextareaLineHeight = "1.5rem";

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
  pending,
  value
}: {
  ready: boolean;
  pending: boolean;
  value: string;
}) {
  return {
    disabled: !canSend({ ready, value }),
    label: pending ? "Sending message" : "Send message"
  };
}

export function canSend({ ready, value }: { ready: boolean; value: string }) {
  return ready && value.trim().length > 0;
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

export function measureTextHeight({
  value,
  placeholder,
  measureText,
  availableWidthPx,
  lineHeightPx,
  paddingBlockPx
}: {
  value: string;
  placeholder: string;
  measureText: (text: string) => number;
  availableWidthPx: number;
  lineHeightPx: number;
  paddingBlockPx: number;
}): number {
  const content = value.length > 0 ? value : placeholder;
  const lines = content.split(/\r\n|\n|\r/);
  const width = Math.max(1, availableWidthPx);
  let visualLineCount = 0;

  for (const line of lines) {
    if (line.length === 0) {
      visualLineCount += 1;
      continue;
    }

    let currentLine = "";
    for (const character of Array.from(line)) {
      const nextLine = `${currentLine}${character}`;
      if (currentLine.length === 0 || measureText(nextLine) <= width) {
        currentLine = nextLine;
        continue;
      }

      visualLineCount += 1;
      currentLine = character;
    }
    visualLineCount += 1;
  }

  return Math.ceil(visualLineCount * lineHeightPx + paddingBlockPx);
}

function longestDraftLine(content: string): string {
  return content.split(/\r\n|\n|\r/).reduce((longest, line) => {
    return Array.from(line).length > Array.from(longest).length ? line : longest;
  }, "");
}

function parsedPixelValue(value: string, fallback: number): number {
  const parsed = Number.parseFloat(value || "");
  return Number.isFinite(parsed) ? parsed : fallback;
}

function inlineBoxReservePx(styles: CSSStyleDeclaration): number {
  return (
    parsedPixelValue(styles.paddingLeft, 0) +
    parsedPixelValue(styles.paddingRight, 0) +
    parsedPixelValue(styles.borderLeftWidth, 0) +
    parsedPixelValue(styles.borderRightWidth, 0)
  );
}

export function syncHeight({
  textarea,
  value,
  placeholder
}: {
  textarea: HTMLTextAreaElement;
  value: string;
  placeholder: string;
}) {
  if (typeof document === "undefined") {
    return;
  }

  textarea.style.minHeight = `${composerMinTextHeightPx}px`;
  textarea.style.width = "100%";
  textarea.style.resize = "none";
  textarea.style.overflowY = "hidden";
  textarea.style.fontSize = composerTextareaFontSize;
  textarea.style.lineHeight = composerTextareaLineHeight;
  textarea.style.height = `${composerMinTextHeightPx}px`;

  const context = document.createElement("canvas").getContext("2d");
  if (!context) {
    return;
  }

  const styles = window.getComputedStyle(textarea);
  context.font = styles.font;
  const fontSizePx = parsedPixelValue(styles.fontSize, 16);
  const lineHeightPx = parsedPixelValue(styles.lineHeight, fontSizePx * 1.5);
  const paddingBlockPx =
    parsedPixelValue(styles.paddingTop, 0) + parsedPixelValue(styles.paddingBottom, 0);
  const paddingInlinePx =
    parsedPixelValue(styles.paddingLeft, 0) + parsedPixelValue(styles.paddingRight, 0);
  const availableWidthPx = textarea.clientWidth - paddingInlinePx;
  const measuredHeight = measureTextHeight({
    value,
    placeholder,
    measureText: (text) => context.measureText(text || " ").width,
    availableWidthPx,
    lineHeightPx,
    paddingBlockPx
  });
  textarea.style.height = `${Math.max(composerMinTextHeightPx, measuredHeight)}px`;
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

export function composerBubbleStyle({
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
    width: `min(calc(${size.width} + ${composerBubbleInlineReservePx}px), 100%)`,
    minWidth: `min(calc(${size.minWidth} + ${composerBubbleInlineReservePx}px), 100%)`
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

type AstryxXStyle = TextAreaProps["xstyle"] | ButtonProps["xstyle"];

function astryxXStyle(xstyle: unknown): AstryxXStyle {
  return xstyle as unknown as AstryxXStyle;
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
      if (textarea) {
        syncHeight({ textarea, value: textarea.value, placeholder });
      }
      assignComposerTextareaRef(forwardedRef, textarea);
    },
    [forwardedRef, placeholder]
  );

  function submit() {
    const nextValue = textareaRef.current?.value ?? value;
    if (!canSend({ ready, value: nextValue })) {
      refocusComposerTextarea(textareaRef.current);
      return;
    }

    onSubmit(nextValue);
    refocusComposerTextarea(textareaRef.current);
  }

  const submitState = composerSubmitState({ ready, pending, value });
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
    const textareaChromeStyles = textarea.parentElement
      ? window.getComputedStyle(textarea.parentElement)
      : null;
    const inlineReservePx =
      inlineBoxReservePx(styles) +
      (textareaChromeStyles ? inlineBoxReservePx(textareaChromeStyles) : 0);
    const nextSize = composerDraftInlineSize({
      value,
      placeholder,
      measureText: (text) => context.measureText(text || " ").width,
      widthBufferPx: composerMeasuredWidthBuffer(inlineReservePx)
    });
    setMeasuredInlineSize((previous) =>
      previous?.key === sizeKey &&
      previous.size.width === nextSize.width &&
      previous.size.minWidth === nextSize.minWidth
        ? previous
        : { key: sizeKey, size: nextSize }
    );
  }, [placeholder, sizeKey, value]);

  useBrowserLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }

    syncHeight({ textarea, value, placeholder });
  }, [placeholder, value]);

  React.useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea || typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      syncHeight({ textarea, value: textarea.value, placeholder });
    });
    observer.observe(textarea);
    return () => observer.disconnect();
  }, [placeholder]);

  const currentInlineSize = measuredInlineSize?.key === sizeKey ? measuredInlineSize.size : fallbackInlineSize;
  const textareaWrapStyle = composerTextareaWrapStyle({
    value,
    placeholder,
    inlineSize: currentInlineSize
  });
  const bubbleStyle = composerBubbleStyle({
    value,
    placeholder,
    inlineSize: currentInlineSize
  });
  // Astryx omits enterKeyHint from its public BaseProps, but forwards rest props to the native textarea.
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
        style={bubbleStyle}
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
            width={textareaWrapStyle.width}
            value={value}
            isDisabled={isComposerTextareaDisabled({ ready })}
            placeholder={placeholder}
            rows={textareaProps.rows}
            xstyle={astryxXStyle(styles.textareaChrome)}
            {...textareaNativeProps}
            onChange={(nextValue, event) => {
              syncHeight({ textarea: event.currentTarget, value: nextValue, placeholder });
              onChange(nextValue);
            }}
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
          xstyle={astryxXStyle(styles.submit)}
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
