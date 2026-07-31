import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
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
    <VStack
      as="article"
      gap={3}
      className={stylex.props(styles.card).className}
    >
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
        <VStack gap={2} className={stylex.props(styles.content).className}>
          <VStack gap={1} className={stylex.props(styles.heading).className}>
            {label || meta || badge ? (
              <HStack as="div" wrap="wrap" gap={2} align="center" className={stylex.props(styles.eyebrow).className}>
                {label ? <span>{label}</span> : null}
                {meta ? <span>{meta}</span> : null}
                {badge ? <span {...stylex.props(styles.badge)}>{badge}</span> : null}
              </HStack>
            ) : null}
            {title ? <strong {...stylex.props(styles.title)}>{title}</strong> : null}
            {description ? <div {...stylex.props(styles.description)}>{description}</div> : null}
          </VStack>
          {children}
          {error ? <div role="alert" {...stylex.props(styles.error)}>{error}</div> : null}
        </VStack>
      )}
      {actions ? (
        <HStack
          gap={actionLayout === "response" ? 0 : 1}
          justify="end"
          className={stylex.props(styles.actions, actionLayout === "response" && styles.responseActions).className}
        >
          {actions}
        </HStack>
      ) : null}
    </VStack>
  );
}

const styles = stylex.create({
  card: {
    position: "relative",
    minWidth: 0,
    paddingBlockStart: "var(--spacing-3)",
    paddingBlockEnd: "calc(var(--spacing-3) + var(--human-intervention-card-overlap, 0px))",
    paddingInline: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderStartStartRadius: "var(--human-intervention-card-radius, var(--radius-element))",
    borderStartEndRadius: "var(--human-intervention-card-radius, var(--radius-element))",
    borderEndStartRadius: "var(--human-intervention-card-bottom-radius, var(--human-intervention-card-radius, var(--radius-element)))",
    borderEndEndRadius: "var(--human-intervention-card-bottom-radius, var(--human-intervention-card-radius, var(--radius-element)))",
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "var(--shadow-low)"
  },
  dismiss: {
    position: "absolute",
    insetBlockStart: "var(--spacing-1)",
    insetInlineEnd: "var(--spacing-1)"
  },
  content: {
    minWidth: 0,
  },
  heading: {
    minWidth: 0,
  },
  eyebrow: {
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
    minWidth: 0,
  },
  responseActions: {
    display: "block",
    width: "100%"
  }
});
