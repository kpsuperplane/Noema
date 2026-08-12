import { createLink } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { forwardRef, type ComponentPropsWithoutRef } from "react";
import type { StyleXStyles } from "@stylexjs/stylex";

type ListCardAnchorProps = ComponentPropsWithoutRef<"a"> & {
  selected?: boolean;
  xstyle?: StyleXStyles;
};

const ListCardAnchor = forwardRef<HTMLAnchorElement, ListCardAnchorProps>(function ListCardAnchor({
  className,
  selected = false,
  style,
  xstyle,
  ...props
}, ref) {
  const cardProps = stylex.props(styles.card, selected && styles.selected, xstyle);
  return (
    <a
      {...props}
      ref={ref}
      className={[cardProps.className, className].filter(Boolean).join(" ")}
      style={{ ...cardProps.style, ...style }}
    />
  );
});

export const ListCardLink = createLink(ListCardAnchor);

const styles = stylex.create({
  card: {
    display: "grid",
    minWidth: 0,
    borderWidth: "var(--border-width)",
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 10,
    backgroundColor: "var(--noema-surface-card)",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-3)",
    color: "var(--noema-text-secondary)",
    textDecoration: "none",
    boxShadow: "0 1px 2px color-mix(in srgb, black 4%, transparent)",
    ":hover": {
      borderColor: "var(--noema-border-default)",
      backgroundColor: "var(--noema-surface-hover)"
    },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--noema-pine-500)",
      outlineOffset: 1
    }
  },
  selected: {
    borderColor: "color-mix(in srgb, var(--noema-pine-500) 26%, var(--noema-border-subtle))",
    backgroundColor: "color-mix(in srgb, var(--noema-pine-50) 70%, var(--noema-surface-card))",
    boxShadow: "0 2px 8px color-mix(in srgb, var(--noema-pine-700) 9%, transparent)"
  }
});
