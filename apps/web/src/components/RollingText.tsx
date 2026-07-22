import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { forwardRef, type ComponentProps } from "react";
import { springs } from "@/motion/springs";

const WORD_STAGGER_SECONDS = 0.016;
const MAX_STAGGER_INDEX = 7;

type RollingTextProps = Omit<ComponentProps<typeof m.span>, "children"> & {
  value: string;
};

const styles = stylex.create({
  root: {
    display: "inline-grid",
    maxWidth: "100%",
    minWidth: 0,
    overflow: "hidden",
    verticalAlign: "bottom"
  },
  empty: {
    display: "none"
  },
  visual: {
    position: "relative",
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
    display: "inline-block",
    backfaceVisibility: "hidden",
    transformOrigin: "50% 50%"
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
  const reduceMotion = useReducedMotion();
  const rootStyles = stylex.props(styles.root, !value && styles.empty);
  const mergedClassName = [rootStyles.className, className].filter(Boolean).join(" ") || undefined;

  return (
    <m.span
      {...props}
      layout={reduceMotion ? false : "size"}
      className={mergedClassName}
      style={{ ...rootStyles.style, ...style }}
      transition={{ layout: springs.standard }}
    >
      <span {...stylex.props(styles.srOnly)}>{value}</span>
      <span aria-hidden="true" {...stylex.props(styles.visual)}>
        <AnimatePresence initial={false} mode="popLayout">
          <RollingTextLayer key={value} reduceMotion={Boolean(reduceMotion)} value={value} />
        </AnimatePresence>
      </span>
    </m.span>
  );
}

const RollingTextLayer = forwardRef<HTMLSpanElement, { value: string; reduceMotion: boolean }>(
  function RollingTextLayer({ value, reduceMotion }, ref) {
    const segments = rollingTextSegments(value);
    return (
      <m.span
        ref={ref}
        layout={reduceMotion ? false : "position"}
        {...stylex.props(styles.layer)}
      >
        {segments.map((segment, index) => {
          if (segment.wordIndex === null) {
            return segment.text;
          }
          const delay = Math.min(segment.wordIndex, MAX_STAGGER_INDEX) * WORD_STAGGER_SECONDS;
          return (
            <m.span
              key={`${index}:${segment.text}`}
              initial={reduceMotion ? false : { opacity: 0, y: "0.8em", rotateX: -22 }}
              animate={{ opacity: 1, y: 0, rotateX: 0 }}
              exit={reduceMotion ? undefined : { opacity: 0, y: "-0.6em", rotateX: 22 }}
              transition={{ ...springs.micro, delay }}
              {...stylex.props(styles.word)}
            >
              {segment.text}
            </m.span>
          );
        })}
      </m.span>
    );
  }
);

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
