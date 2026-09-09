import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import { useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { useLayoutEffect, useRef, useState, type ReactNode, type Ref } from "react";
import { RollingText } from "@/components/RollingText";
import { springs } from "@/motion/springs";

export function MobileTitleNavigation({
  icon,
  label,
  navOpen,
  triggerRef,
  onToggle
}: {
  icon: ReactNode;
  label: string;
  navOpen: boolean;
  triggerRef: Ref<HTMLButtonElement>;
  onToggle: () => void;
}) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const measureButtonRef = useRef<HTMLButtonElement>(null);
  const [pillWidth, setPillWidth] = useState<number | null>(null);

  useLayoutEffect(() => {
    const button = measureButtonRef.current;
    if (!button) return;

    const updateWidth = () => {
      const nextWidth = button.getBoundingClientRect().width;
      if (nextWidth > 0) {
        setPillWidth((currentWidth) => currentWidth === nextWidth ? currentWidth : nextWidth);
      }
    };
    updateWidth();
    const observer = new ResizeObserver(updateWidth);
    observer.observe(button);
    return () => observer.disconnect();
  }, []);

  return (
    <m.h1
      data-slot="shell-mobile-title"
      aria-hidden={isPresent ? undefined : "true"}
      inert={!isPresent}
      initial={reduceMotion ? false : { opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={reduceMotion ? { duration: 0 } : springs.micro}
      {...stylex.props(styles.title)}
    >
      <m.span
        initial={false}
        animate={pillWidth === null ? undefined : { width: pillWidth }}
        transition={{ width: reduceMotion ? { duration: 0 } : springs.standard }}
        {...stylex.props(styles.buttonLayout)}
      >
        <Button
          ref={triggerRef}
          type="button"
          variant="ghost"
          size="lg"
          label={`${navOpen ? "Close" : "Open"} ${label} navigation`}
          icon={icon}
          endContent={(
            <m.span
              aria-hidden="true"
              initial={false}
              animate={{ rotate: navOpen ? 180 : 0 }}
              transition={reduceMotion ? { duration: 0 } : springs.micro}
              {...stylex.props(styles.chevron)}
            >
              <ChevronDown size={18} />
            </m.span>
          )}
          aria-controls="noema-shell-sidebar"
          aria-expanded={navOpen}
          xstyle={[styles.button, styles.buttonFill]}
          onClick={onToggle}
        >
          <RollingText value={label} />
        </Button>
      </m.span>
      <Button
        ref={measureButtonRef}
        type="button"
        variant="ghost"
        size="lg"
        label={label}
        icon={icon}
        endContent={<ChevronDown aria-hidden="true" size={18} />}
        aria-hidden="true"
        tabIndex={-1}
        xstyle={[styles.button, styles.measureButton]}
      />
    </m.h1>
  );
}

const styles = stylex.create({
  title: {
    display: "none",
    "@media (max-width: 760px)": {
      position: "absolute",
      top: 0,
      right: 0,
      left: 0,
      zIndex: 25,
      display: "flex",
      height: "calc(var(--spacing-12) + var(--spacing-1))",
      alignItems: "center",
      justifyContent: "center",
      margin: "var(--spacing-0)",
      pointerEvents: "none",
      backgroundColor: "var(--background)",
      borderBottomWidth: "var(--border-width)",
      borderBottomStyle: "solid",
      borderBottomColor: "var(--noema-border-subtle)"
    }
  },
  chevron: {
    display: "inline-flex"
  },
  buttonLayout: {
    display: "inline-flex",
    maxWidth: "calc(100vw - var(--spacing-8))"
  },
  buttonFill: {
    width: "100%"
  },
  measureButton: {
    position: "absolute",
    visibility: "hidden",
    width: "max-content",
    pointerEvents: "none"
  },
  button: {
    maxWidth: "calc(100vw - var(--spacing-8))",
    pointerEvents: "auto",
    borderColor: "transparent",
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "color-mix(in srgb, var(--pine-100) 48%, var(--background))",
    boxShadow: "var(--shadow-shell-pill)",
    color: "var(--pine-700)",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    fontWeight: 600,
    lineHeight: "22px",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 64%, var(--background))"
      }
    },
    ":active": {
      backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, var(--background))"
    }
  }
});
