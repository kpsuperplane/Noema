import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import { useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import type { Ref } from "react";
import { springs } from "@/motion/springs";

export function MobileTitleNavigation({
  label,
  navOpen,
  triggerRef,
  onToggle
}: {
  label: string;
  navOpen: boolean;
  triggerRef: Ref<HTMLButtonElement>;
  onToggle: () => void;
}) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();

  return (
    <m.h1
      data-slot="shell-mobile-title"
      aria-hidden={isPresent ? undefined : "true"}
      inert={!isPresent}
      initial={reduceMotion ? false : { opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={reduceMotion ? { duration: 0 } : springs.surface}
      {...stylex.props(styles.title)}
    >
      <Button
        ref={triggerRef}
        type="button"
        variant="ghost"
        size="lg"
        label={`${navOpen ? "Close" : "Open"} ${label} navigation`}
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
        xstyle={buttonXStyle(styles.button)}
        onClick={onToggle}
      >
        {label}
      </Button>
    </m.h1>
  );
}

const styles = stylex.create({
  title: {
    display: "none",
    "@media (max-width: 760px)": {
      position: "relative",
      zIndex: 25,
      display: "flex",
      gridRow: 1,
      gridColumn: 1,
      width: "100%",
      alignItems: "center",
      justifyContent: "center",
      margin: "var(--spacing-0)",
      pointerEvents: "none",
      backgroundImage:
        "linear-gradient(to bottom, var(--background) 0%, 80%, rgb(255 255 255 / 0) 100%)"
    }
  },
  chevron: {
    display: "inline-flex"
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

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
