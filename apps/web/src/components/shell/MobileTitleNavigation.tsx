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
        endContent={<ChevronDown aria-hidden="true" size={18} />}
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
      position: "absolute",
      top: "calc(52px + var(--spacing-1))",
      left: "50%",
      zIndex: 25,
      display: "block",
      maxWidth: "calc(100vw - var(--spacing-8))",
      margin: "var(--spacing-0)",
      transform: "translateX(-50%)"
    }
  },
  button: {
    maxWidth: "100%",
    borderColor: "transparent",
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "color-mix(in srgb, var(--pine-100) 48%, transparent)",
    boxShadow: "none",
    color: "var(--pine-700)",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    fontWeight: 600,
    lineHeight: "22px",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 64%, transparent)"
      }
    },
    ":active": {
      backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)"
    }
  }
});

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
