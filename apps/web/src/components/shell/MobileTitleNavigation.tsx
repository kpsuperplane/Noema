import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import type { Ref } from "react";

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
  return (
    <h1 data-slot="shell-mobile-title" {...stylex.props(styles.title)}>
      <Button
        ref={triggerRef}
        type="button"
        variant="secondary"
        size="sm"
        label={`${navOpen ? "Close" : "Open"} ${label} navigation`}
        endContent={<ChevronDown aria-hidden="true" size={15} />}
        aria-controls="noema-shell-sidebar"
        aria-expanded={navOpen}
        xstyle={buttonXStyle(styles.button)}
        onClick={onToggle}
      >
        {label}
      </Button>
    </h1>
  );
}

const styles = stylex.create({
  title: {
    display: "none",
    "@media (max-width: 760px)": {
      position: "absolute",
      top: "calc(52px + var(--spacing-1))",
      left: "50%",
      zIndex: 45,
      display: "block",
      maxWidth: "calc(100vw - var(--spacing-8))",
      margin: "var(--spacing-0)",
      transform: "translateX(-50%)"
    }
  },
  button: {
    maxWidth: "100%",
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--pure-white)",
    boxShadow: "var(--shadow-shell-control)",
    color: "var(--pine-700)",
    fontFamily: "var(--font-heading)",
    fontWeight: 600,
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "var(--pure-white)"
      }
    },
    ":active": {
      backgroundColor: "var(--pure-white)"
    }
  }
});

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
