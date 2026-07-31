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
