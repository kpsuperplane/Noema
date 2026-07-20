import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  PendingGovernedActionsDocument,
  ResolveGovernedActionDocument,
  type GovernedActionDecision,
  type GovernedActionEffect,
  type PendingGovernedActionsQuery
} from "@/generated/graphql";

export type PendingGovernedAction = PendingGovernedActionsQuery["pendingGovernedActions"][number];

type Scope = {
  conversationId?: string | null;
  taskId?: string;
};

export function usePendingGovernedActions(scope: Scope = {}) {
  return useQuery(PendingGovernedActionsDocument, {
    variables: {
      conversationId: scope.conversationId ?? undefined,
      taskId: scope.taskId,
      first: 50
    },
    skip: scope.conversationId === null,
    fetchPolicy: "cache-and-network",
    pollInterval: 2_000
  });
}

export function PendingGovernedActions({
  conversationId,
  taskId,
  compact = false
}: Scope & { compact?: boolean }) {
  const result = usePendingGovernedActions({ conversationId, taskId });
  const actions = result.data?.pendingGovernedActions ?? [];
  if (!actions.length) return null;
  return (
    <GovernedActionList
      actions={actions}
      compact={compact}
      onResolved={() => void result.refetch()}
    />
  );
}

export function GovernedActionList({
  actions,
  compact = false,
  onResolved
}: {
  actions: PendingGovernedAction[];
  compact?: boolean;
  onResolved?: () => void;
}) {
  return (
    <section aria-label="External actions awaiting approval" {...stylex.props(styles.list, compact && styles.compactList)}>
      {actions.map((action) => (
        <GovernedActionCard
          action={action}
          compact={compact}
          key={`${action.actionId}:${action.revision}`}
          onResolved={onResolved}
        />
      ))}
    </section>
  );
}

function GovernedActionCard({
  action,
  compact,
  onResolved
}: {
  action: PendingGovernedAction;
  compact: boolean;
  onResolved?: () => void;
}) {
  const [resolveAction, resolution] = useMutation(ResolveGovernedActionDocument);
  const [error, setError] = React.useState<string | null>(null);
  const decide = async (decision: GovernedActionDecision) => {
    setError(null);
    try {
      await resolveAction({
        variables: {
          input: {
            actionId: action.actionId,
            expectedRevision: action.revision,
            decision
          }
        }
      });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The action could not be resolved.");
    }
  };
  return (
    <article {...stylex.props(styles.card, compact && styles.compactCard)}>
      <div {...stylex.props(styles.copy)}>
        <div {...stylex.props(styles.eyebrow)}>
          <span>{effectLabel(action.effect)}</span>
          {action.taskId ? <span>Background task</span> : <span>Primary conversation</span>}
        </div>
        <strong {...stylex.props(styles.summary)}>{action.safeSummary}</strong>
        <span {...stylex.props(styles.capability)}>{action.capabilityName}</span>
        <details {...stylex.props(styles.details)}>
          <summary>Review exact arguments</summary>
          <pre {...stylex.props(styles.arguments)}>{formatArguments(action.arguments)}</pre>
        </details>
        {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
      </div>
      <div {...stylex.props(styles.actions)}>
        <Button
          size="sm"
          variant="ghost"
          label="Decline"
          isDisabled={resolution.loading}
          onClick={() => void decide("DECLINE")}
        />
        <Button
          size="sm"
          variant="primary"
          label="Approve once"
          isLoading={resolution.loading}
          isDisabled={resolution.loading}
          onClick={() => void decide("APPROVE")}
        />
      </div>
    </article>
  );
}

function effectLabel(effect: GovernedActionEffect) {
  switch (effect) {
    case "WRITE":
      return "External write";
    case "EXPORT":
      return "Data export";
    case "WRITE_AND_EXPORT":
      return "External write + export";
  }
}

function formatArguments(value: unknown) {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "Arguments could not be displayed.";
  }
}

const styles = stylex.create({
  list: {
    display: "grid",
    gap: "var(--spacing-2)",
    width: "100%"
  },
  compactList: {
    width: "100%",
    maxWidth: "none",
    margin: 0
  },
  card: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "var(--spacing-3)",
    padding: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 12,
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "0 8px 24px rgb(0 0 0 / 0.06)",
    "@media (max-width: 640px)": {
      flexDirection: "column"
    }
  },
  compactCard: {
    gap: "var(--spacing-2)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    boxShadow: "none",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)"
  },
  copy: {
    display: "grid",
    gap: "var(--spacing-1)",
    minWidth: 0
  },
  eyebrow: {
    display: "flex",
    gap: 8,
    color: "var(--noema-text-muted)",
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: "0.045em"
  },
  summary: {
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.35
  },
  capability: {
    color: "var(--noema-text-muted)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 10,
    overflowWrap: "anywhere"
  },
  details: {
    marginTop: "var(--spacing-0-5)",
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    cursor: "pointer"
  },
  arguments: {
    maxHeight: 180,
    marginBlock: "var(--spacing-2)",
    padding: "var(--spacing-2)",
    overflow: "auto",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--noema-text-primary)",
    fontSize: 11,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
    cursor: "text"
  },
  actions: {
    display: "flex",
    flexShrink: 0,
    gap: "var(--spacing-1)"
  },
  error: {
    color: "var(--noema-text-danger)",
    fontSize: 12
  }
});
