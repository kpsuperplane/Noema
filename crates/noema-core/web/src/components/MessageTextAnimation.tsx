import * as React from "react";
import * as stylex from "@stylexjs/stylex";

type MessageTextAnimationToken =
  | { kind: "word"; value: string; wordIndex: number }
  | { kind: "space"; value: string; previousWordIndex: number | null; nextWordIndex: number | null };

const WORD_REVEAL_INTERVAL_MS = 42;

export function AnimatedMessageText({
  animate,
  text
}: {
  animate: boolean;
  text: string;
}) {
  const prefersReducedMotion = usePrefersReducedMotion();
  const shouldAnimate = animate && !prefersReducedMotion;
  const tokens = React.useMemo(() => messageTextAnimationTokens(text), [text]);
  const wordCount = React.useMemo(() => messageTextWordCount(tokens), [tokens]);
  const [visibleWordCount, setVisibleWordCount] = React.useState(() => (shouldAnimate ? 0 : wordCount));
  const visibleWordCountRef = React.useRef(visibleWordCount);

  React.useEffect(() => {
    visibleWordCountRef.current = visibleWordCount;
  }, [visibleWordCount]);

  React.useEffect(() => {
    if (!shouldAnimate) {
      return;
    }

    const currentVisibleWordCount = Math.min(visibleWordCountRef.current, wordCount);
    if (currentVisibleWordCount >= wordCount) {
      return;
    }

    const timers = Array.from({ length: wordCount - currentVisibleWordCount }, (_, offset) =>
      window.setTimeout(() => {
        const index = currentVisibleWordCount + offset;
        setVisibleWordCount((current) => Math.max(current, index + 1));
      }, offset * WORD_REVEAL_INTERVAL_MS)
    );

    return () => {
      timers.forEach((timer) => window.clearTimeout(timer));
    };
  }, [shouldAnimate, wordCount]);

  const visibleTokens = shouldAnimate
    ? visibleMessageTextAnimationTokens(tokens, Math.min(visibleWordCount, wordCount))
    : tokens;

  return (
    <span {...stylex.props(styles.text)}>
      {visibleTokens.map((token, index) => {
        if (token.kind === "space") {
          return <React.Fragment key={`space-${index}`}>{renderMessageTextSpace(token.value, index)}</React.Fragment>;
        }
        return (
          <span
            key={`word-${index}`}
            {...stylex.props(shouldAnimate && styles.wordFade)}
          >
            {token.value}
          </span>
        );
      })}
    </span>
  );
}

function renderMessageTextSpace(value: string, index: number): React.ReactNode {
  const parts = value.split(/(\r\n|\n|\r)/);
  if (parts.length === 1) {
    return value;
  }

  return parts.map((part, partIndex) =>
    part === "\r\n" || part === "\n" || part === "\r" ? (
      <br key={`line-${index}-${partIndex}`} />
    ) : (
      part
    )
  );
}

function usePrefersReducedMotion(): boolean {
  const [prefersReducedMotion, setPrefersReducedMotion] = React.useState(() => {
    if (typeof window === "undefined") {
      return false;
    }
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  });

  React.useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    const handleChange = () => setPrefersReducedMotion(mediaQuery.matches);

    handleChange();
    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  return prefersReducedMotion;
}

export function messageTextAnimationTokens(text: string): MessageTextAnimationToken[] {
  const parts = text.match(/\S+|\s+/g) ?? [];
  const tokens: MessageTextAnimationToken[] = [];
  let wordIndex = 0;

  for (const value of parts) {
    if (/^\s+$/.test(value)) {
      tokens.push({ kind: "space", value, previousWordIndex: wordIndex > 0 ? wordIndex - 1 : null, nextWordIndex: null });
      continue;
    }
    const lastToken = tokens.at(-1);
    if (lastToken?.kind === "space") {
      lastToken.nextWordIndex = wordIndex;
    }
    tokens.push({ kind: "word", value, wordIndex: wordIndex++ });
  }

  return tokens;
}

export function visibleMessageTextAnimationTokens(
  tokens: MessageTextAnimationToken[],
  visibleWordCount: number
): MessageTextAnimationToken[] {
  return tokens.filter((token) => {
    if (token.kind === "word") {
      return token.wordIndex < visibleWordCount;
    }
    return (
      token.previousWordIndex !== null &&
      token.nextWordIndex !== null &&
      token.previousWordIndex < visibleWordCount &&
      token.nextWordIndex < visibleWordCount
    );
  });
}

function messageTextWordCount(tokens: MessageTextAnimationToken[]): number {
  return tokens.filter((token) => token.kind === "word").length;
}

const styles = stylex.create({
  text: {
    display: "block",
    minWidth: 0
  },
  wordFade: {
    animation: {
      default: "message-word-fade-in 180ms ease-out both",
      "@media (prefers-reduced-motion: reduce)": "none"
    }
  }
});
