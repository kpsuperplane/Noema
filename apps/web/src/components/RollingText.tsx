import * as stylex from "@stylexjs/stylex";
import { type HTMLAttributes, type Ref, useLayoutEffect, useRef, useState } from "react";

const WORD_STAGGER_MS = 16;
const MAX_STAGGER_INDEX = 7;
const TEXT_MOTION_MS = 220;
const WIDTH_MOTION_MS = 240;
const SETTLE_DELAY_MS = TEXT_MOTION_MS + MAX_STAGGER_INDEX * WORD_STAGGER_MS + 40;
const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

type RollingTextProps = Omit<HTMLAttributes<HTMLSpanElement>, "children"> & {
  value: string;
};

type RollingTextMotion = {
  current: string;
  previous: string | null;
  generation: number;
  width: number | null;
};

const rollIn = stylex.keyframes({
  from: { opacity: 0, transform: "translateY(0.6em)" },
  to: { opacity: 1, transform: "translateY(0)" }
});

const rollOut = stylex.keyframes({
  from: { opacity: 1, transform: "translateY(0)" },
  to: { opacity: 0, transform: "translateY(-0.6em)" }
});

const styles = stylex.create({
  root: {
    display: "inline-grid",
    maxWidth: "100%",
    minWidth: 0,
    overflow: "hidden",
    verticalAlign: "bottom",
    transitionDuration: `${WIDTH_MOTION_MS}ms`,
    transitionProperty: "width",
    transitionTimingFunction: "cubic-bezier(0.16, 1, 0.3, 1)",
    "@media (prefers-reduced-motion: reduce)": {
      transitionDuration: "0ms"
    }
  },
  empty: {
    display: "none"
  },
  visual: {
    display: "grid",
    maxWidth: "100%",
    minWidth: 0
  },
  layer: {
    gridArea: "1 / 1",
    display: "block",
    width: "max-content",
    maxWidth: "100%",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "pre"
  },
  word: {
    display: "inline-block"
  },
  incomingWord: {
    animationDuration: `${TEXT_MOTION_MS}ms`,
    animationFillMode: "both",
    animationName: rollIn,
    animationTimingFunction: "cubic-bezier(0.16, 1, 0.3, 1)",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none"
    }
  },
  outgoingWord: {
    animationDuration: `${TEXT_MOTION_MS}ms`,
    animationFillMode: "both",
    animationName: rollOut,
    animationTimingFunction: "cubic-bezier(0.4, 0, 1, 1)",
    "@media (prefers-reduced-motion: reduce)": {
      display: "none",
      animationName: "none"
    }
  },
  srOnly: {
    position: "absolute",
    width: 1,
    height: 1,
    overflow: "hidden",
    clip: "rect(0 0 0 0)",
    whiteSpace: "nowrap"
  }
});

export function RollingText({ value, className, style, ...props }: RollingTextProps) {
  const rootRef = useRef<HTMLSpanElement>(null);
  const incomingRef = useRef<HTMLSpanElement>(null);
  const [motion, setMotion] = useState<RollingTextMotion>(() => ({
    current: value,
    previous: null,
    generation: 0,
    width: null
  }));

  useLayoutEffect(() => {
    if (value === motion.current) {
      return;
    }
    const reducedMotion = window.matchMedia(REDUCED_MOTION_QUERY).matches;
    const previousWidth = rootRef.current?.getBoundingClientRect().width ?? 0;
    setMotion((current) => ({
      current: value,
      previous: reducedMotion ? null : current.current,
      generation: current.generation + 1,
      width: reducedMotion ? null : previousWidth
    }));
  }, [motion, value]);

  useLayoutEffect(() => {
    if (motion.previous === null) {
      return;
    }
    const generation = motion.generation;
    const nextWidth = incomingRef.current?.scrollWidth ?? 0;
    const frame = window.requestAnimationFrame(() => {
      setMotion((current) => current.generation === generation ? { ...current, width: nextWidth } : current);
    });
    const timeout = window.setTimeout(() => {
      setMotion((current) => current.generation === generation
        ? { ...current, previous: null, width: null }
        : current);
    }, SETTLE_DELAY_MS);
    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(timeout);
    };
  }, [motion.generation, motion.previous]);

  const rootStyles = stylex.props(styles.root, motion.previous === null && !motion.current && styles.empty);
  const mergedClassName = [rootStyles.className, className].filter(Boolean).join(" ") || undefined;
  const mergedStyle = motion.width === null
    ? { ...rootStyles.style, ...style }
    : { ...rootStyles.style, ...style, width: motion.width };

  return (
    <span {...props} ref={rootRef} className={mergedClassName} style={mergedStyle}>
      <span {...stylex.props(styles.srOnly)}>{motion.current}</span>
      <span aria-hidden="true" {...stylex.props(styles.visual)}>
        {motion.previous !== null ? (
          <RollingTextLayer animated direction="out" value={motion.previous} />
        ) : null}
        <RollingTextLayer
          ref={incomingRef}
          animated={motion.previous !== null}
          direction="in"
          value={motion.current}
        />
      </span>
    </span>
  );
}

function RollingTextLayer({
  value,
  animated,
  direction,
  ref
}: {
  value: string;
  animated: boolean;
  direction: "in" | "out";
  ref?: Ref<HTMLSpanElement>;
}) {
  const segments = rollingTextSegments(value);
  return (
    <span ref={ref} {...stylex.props(styles.layer)}>
      {segments.map((segment, index) => {
        if (segment.wordIndex === null) {
          return segment.text;
        }
        const delay = Math.min(segment.wordIndex, MAX_STAGGER_INDEX) * WORD_STAGGER_MS;
        return (
          <span
            key={`${index}:${segment.text}`}
            {...stylex.props(
              styles.word,
              animated && direction === "in" && styles.incomingWord,
              animated && direction === "out" && styles.outgoingWord
            )}
            style={animated ? { animationDelay: `${delay}ms` } : undefined}
          >
            {segment.text}
          </span>
        );
      })}
    </span>
  );
}

function rollingTextSegments(value: string): Array<{ text: string; wordIndex: number | null }> {
  let wordIndex = 0;
  return value.split(/(\s+)/u).map((text) => {
    if (!text || /^\s+$/u.test(text)) {
      return { text, wordIndex: null };
    }
    const segment = { text, wordIndex };
    wordIndex += 1;
    return segment;
  });
}
