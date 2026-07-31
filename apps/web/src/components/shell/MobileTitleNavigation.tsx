import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import { useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import type { ReactNode, Ref } from "react";
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
        layout={reduceMotion ? false : "size"}
        layoutDependency={label}
        transition={{ layout: springs.standard }}
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
          xstyle={buttonXStyle(styles.button)}
          onClick={onToggle}
        >
          <RollingText value={label} />
        </Button>
      </m.span>
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
      backgroundImage:
        "linear-gradient(to bottom, var(--background) 0%, 80%, rgb(255 255 255 / 0) 100%)"
    }
  },
  chevron: {
    display: "inline-flex"
  },
  buttonLayout: {
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
