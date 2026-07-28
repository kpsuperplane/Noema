import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { X } from "lucide-react";

export function HumanInterventionCard({
  label,
  meta,
  badge,
  title,
  description,
  children,
  content,
  error,
  actions,
  actionLayout = "buttons",
  dismissLabel,
  onDismiss
}: {
  label?: React.ReactNode;
  meta?: React.ReactNode;
  badge?: React.ReactNode;
  title?: React.ReactNode;
  description?: React.ReactNode;
  children?: React.ReactNode;
  content?: React.ReactNode;
  error?: React.ReactNode;
  actions?: React.ReactNode;
  actionLayout?: "buttons" | "response";
  dismissLabel?: string;
  onDismiss?: () => void;
}) {
  return (
    <article {...stylex.props(styles.card, onDismiss && styles.dismissibleCard)}>
      {onDismiss && dismissLabel ? (
        <div {...stylex.props(styles.dismiss)}>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            label={dismissLabel}
            tooltip={dismissLabel}
            icon={<X aria-hidden="true" size={14} />}
            isIconOnly
            onClick={onDismiss}
          />
        </div>
      ) : null}
      {content ?? (
        <div {...stylex.props(styles.content)}>
          <div {...stylex.props(styles.heading)}>
            {label || meta || badge ? (
              <div {...stylex.props(styles.eyebrow)}>
                {label ? <span>{label}</span> : null}
                {meta ? <span>{meta}</span> : null}
                {badge ? <span {...stylex.props(styles.badge)}>{badge}</span> : null}
              </div>
            ) : null}
            {title ? <strong {...stylex.props(styles.title)}>{title}</strong> : null}
            {description ? <div {...stylex.props(styles.description)}>{description}</div> : null}
          </div>
          {children}
          {error ? <div role="alert" {...stylex.props(styles.error)}>{error}</div> : null}
        </div>
      )}
      {actions ? <div {...stylex.props(styles.actions, actionLayout === "response" && styles.responseActions)}>{actions}</div> : null}
    </article>
  );
}

const styles = stylex.create({
  card: {
    position: "relative",
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-3)",
    padding: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 10,
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "0 2px 8px color-mix(in srgb, black 7%, transparent)"
  },
  dismissibleCard: {
    paddingInlineEnd: "calc(var(--spacing-8) + var(--spacing-2))"
  },
  dismiss: {
    position: "absolute",
    insetBlockStart: "var(--spacing-1)",
    insetInlineEnd: "var(--spacing-1)"
  },
  content: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-2)"
  },
  heading: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-1)"
  },
  eyebrow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: "var(--spacing-2)",
    color: "var(--noema-text-muted)",
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: "0.045em"
  },
  badge: {
    borderRadius: "var(--radius-full)",
    backgroundColor: "var(--noema-surface-sunken)",
    paddingBlock: "var(--spacing-0-5)",
    paddingInline: "var(--spacing-1)",
    color: "var(--noema-text-secondary)",
    fontSize: 10,
    letterSpacing: 0,
    textTransform: "none"
  },
  title: {
    minWidth: 0,
    color: "var(--noema-text-primary)",
    fontSize: 13,
    lineHeight: 1.35,
    overflowWrap: "anywhere"
  },
  description: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45,
    overflowWrap: "anywhere"
  },
  error: {
    color: "var(--noema-text-danger)",
    fontSize: 12,
    lineHeight: 1.4
  },
  actions: {
    display: "flex",
    minWidth: 0,
    justifyContent: "flex-end",
    gap: "var(--spacing-1)"
  },
  responseActions: {
    display: "block",
    width: "100%"
  }
});
