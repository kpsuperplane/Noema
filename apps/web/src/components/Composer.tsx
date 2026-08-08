import React from "react";
import { SendHorizontal } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { TextArea } from "@astryxdesign/core/TextArea";
import * as stylex from "@stylexjs/stylex";
import {
  canSend,
  composerSubmitLayerStyle,
  composerSubmitState,
  composerTextareaProps,
  shouldSubmitFromBeforeInput,
  shouldSubmitFromPointerDown,
  shouldSubmitFromTouchStart
} from "./composerModel";

export {
  canSend,
  composerSubmitLayerStyle,
  composerSubmitState,
  composerTextareaProps,
  isComposerTextareaDisabled,
  shouldSubmitFromBeforeInput,
  shouldSubmitFromPointerDown,
  shouldSubmitFromTouchStart
} from "./composerModel";

const composerMinWidthCh = 18;
const composerWidthBufferCh = 5;
const composerWidthBufferPx = 32;
const composerMeasuredTextSlackPx = 4;
const composerBubbleInlineReservePx = 62;
const composerMinTextHeightPx = 24;
const composerTextareaFontSize = "1rem";
const composerTextareaLineHeight = "1.5rem";

type ComposerInlineSize = {
  minWidth: string;
  width: string;
};

const useBrowserLayoutEffect = typeof window === "undefined" ? React.useEffect : React.useLayoutEffect;

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

export function measureComposerDraftInlineSize({
  textarea,
  value,
  placeholder
}: {
  textarea: HTMLTextAreaElement;
  value: string;
  placeholder: string;
}): ComposerInlineSize | null {
  if (typeof document === "undefined") {
    return null;
  }
  const context = document.createElement("canvas").getContext("2d");
  if (!context) {
    return null;
  }

  const styles = window.getComputedStyle(textarea);
  context.font = styles.font;
  const textareaChromeStyles = textarea.parentElement
    ? window.getComputedStyle(textarea.parentElement)
    : null;
  const inlineReservePx =
    inlineBoxReservePx(styles) +
    (textareaChromeStyles ? inlineBoxReservePx(textareaChromeStyles) : 0);
  return composerDraftInlineSize({
    value,
    placeholder,
    measureText: (text) => context.measureText(text || " ").width,
    widthBufferPx: composerMeasuredWidthBuffer(inlineReservePx)
  });
}

export function syncHeight({
  textarea
}: {
  textarea: HTMLTextAreaElement;
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

  if (textarea.value.length === 0) {
    return;
  }

  textarea.style.height = `${Math.max(composerMinTextHeightPx, textarea.scrollHeight)}px`;
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
    width: `100%`,
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
    width: `var(--composer-bubble-width, min(calc(${size.width} + ${composerBubbleInlineReservePx}px), 100%))`,
    minWidth: `var(--composer-bubble-min-width, min(calc(${size.minWidth} + ${composerBubbleInlineReservePx}px), 100%))`
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
  editable?: boolean;
  pending: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSubmit: (value: string) => void;
};

export const Composer = React.forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  {
    value,
    ready,
    editable = ready,
    pending,
    placeholder,
    onChange,
    onSubmit
  },
  forwardedRef
) {
  const textareaRef = React.useRef<HTMLTextAreaElement>(null);
  const ignoreNextClickRef = React.useRef(false);
  const earlyActivationSubmittedRef = React.useRef(false);
  const setTextareaRef = React.useCallback(
    (textarea: HTMLTextAreaElement | null) => {
      textareaRef.current = textarea;
      if (textarea) {
        syncHeight({ textarea });
      }
      assignComposerTextareaRef(forwardedRef, textarea);
    },
    [forwardedRef]
  );

  function submit(nextValue = textareaRef.current?.value ?? value) {
    if (!canSend({ ready, value: nextValue })) {
      refocusComposerTextarea(textareaRef.current);
      return false;
    }

    onSubmit(nextValue);
    refocusComposerTextarea(textareaRef.current);
    return true;
  }

  function suppressSyntheticActivation() {
    ignoreNextClickRef.current = true;
    earlyActivationSubmittedRef.current = true;
    window.setTimeout(() => {
      ignoreNextClickRef.current = false;
      earlyActivationSubmittedRef.current = false;
    }, 500);
  }

  function submitFromEarlyActivation(nextValue: string) {
    if (earlyActivationSubmittedRef.current) {
      return;
    }

    suppressSyntheticActivation();
    submit(nextValue);
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
  const currentInlineSize =
    measuredInlineSize?.key === sizeKey ? measuredInlineSize.size : fallbackInlineSize;

  useBrowserLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }
    const nextSize = measureComposerDraftInlineSize({ textarea, value, placeholder });
    if (!nextSize) {
      return;
    }
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

    syncHeight({ textarea });
  }, [currentInlineSize, value]);

  React.useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea || typeof ResizeObserver === "undefined") {
      return;
    }

    const observer = new ResizeObserver(() => {
      syncHeight({ textarea });
    });
    observer.observe(textarea);
    return () => observer.disconnect();
  }, []);

  const textareaWrapStyle = composerTextareaWrapStyle({
    value,
    placeholder,
    inlineSize: currentInlineSize
  });
  const submitLayerStyle = composerSubmitLayerStyle();
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
            isDisabled={!editable}
            placeholder={placeholder}
            rows={textareaProps.rows}
            xstyle={styles.textareaChrome}
            {...textareaNativeProps}
            onBeforeInput={(event) => {
              const nativeEvent = event.nativeEvent;
              const inputType =
                "inputType" in nativeEvent && typeof nativeEvent.inputType === "string"
                  ? nativeEvent.inputType
                  : null;
              const nextValue = (event.currentTarget as HTMLTextAreaElement).value;
              if (!shouldSubmitFromBeforeInput({ ready, value: nextValue, inputType })) {
                return;
              }

              event.preventDefault();
              submit(nextValue);
            }}
            onChange={(nextValue, event) => {
              syncHeight({ textarea: event.currentTarget });
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
          style={submitLayerStyle}
          xstyle={styles.submit}
          isDisabled={submitState.disabled}
          onTouchStart={(event) => {
            const nextValue = textareaRef.current?.value ?? value;
            if (
              !shouldSubmitFromTouchStart({
                ready,
                value: nextValue,
                touchCount: event.touches.length
              })
            ) {
              return;
            }

            event.preventDefault();
            submitFromEarlyActivation(nextValue);
          }}
          onPointerDown={(event) => {
            const nextValue = textareaRef.current?.value ?? value;
            if (
              !shouldSubmitFromPointerDown({
                ready,
                value: nextValue,
                button: event.button,
                isPrimary: event.isPrimary,
                pointerType: event.pointerType
              })
            ) {
              return;
            }

            event.preventDefault();
            submitFromEarlyActivation(nextValue);
          }}
          onClick={(event) => {
            if (ignoreNextClickRef.current) {
              event.preventDefault();
              ignoreNextClickRef.current = false;
              return;
            }

            submit();
          }}
        />
      </div>
    </form>
  );
});

const styles = stylex.create({
  shell: {
    display: "flex",
    justifyContent: "flex-end",
    width: "var(--composer-shell-width, var(--chat-column-width))",
    marginInline: "auto",
  },
  bubble: {
    position: "relative",
    flexGrow: "var(--composer-bubble-grow, 0)",
    width: "fit-content",
    minWidth: "min(13rem, 100%)",
    maxWidth: "100%",
    transitionProperty: "flex-grow",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    borderRadius: "calc(var(--radius) * 2.6)",
    cornerShape: "var(--corner-shape-element)",
    backgroundColor: "var(--composer-bubble-background, var(--primary))",
    padding: "var(--spacing-1-5)",
    paddingRight: {
      default: 48,
      "@media (hover: none) and (pointer: coarse)": 56
    },
    color: "var(--primary-foreground)",
    boxShadow: "var(--composer-bubble-shadow, var(--shadow-composer))"
  },
  textareaWrap: {
    minWidth: 0,
    marginInlineEnd: "auto"
  },
  textareaChrome: {
    "--color-text-primary": "var(--primary-foreground)",
    "--color-text-secondary": "color-mix(in srgb, var(--primary-foreground) 70%, transparent)",
    minHeight: 36,
    minWidth: 0,
    width: "100%",
    borderColor: "transparent",
    backgroundColor: "transparent",
    paddingBlock: "var(--spacing-1-5)",
    paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))",
    opacity: {
      default: 1,
      ":disabled": 0.7
    },
    outline: {
      default: null,
      ":focus-visible": "none"
    },
    boxShadow: "none",
    "::placeholder": {
      whiteSpace: "nowrap"
    }
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
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--primary-foreground)",
    color: {
      default: "var(--primary)",
      ":disabled": "color-mix(in srgb, var(--primary) 70%, transparent)"
    },
    backgroundImage: null,
    "@media (hover: hover)": {
      ":hover": {
        backgroundImage: "linear-gradient(rgba(255, 255, 255, 0.1), rgba(255, 255, 255, 0.1))"
      }
    }
  }
});
