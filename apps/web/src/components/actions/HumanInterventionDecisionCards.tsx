import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import {
  ResolveGovernedActionDocument,
  SkipAdapterAuthenticationDocument,
  SkipMcpAuthenticationDocument,
  StartAdapterAuthenticationDocument,
  StartMcpAuthenticationDocument,
  type ExecutionReviewRoute,
  type GovernedActionDecision,
  type PendingHumanInterventionsQuery
} from "@/generated/graphql";
import { useMcpOAuthController } from "@/components/mcp/useMcpOAuthController";
import { reserveExternalAuthNavigation } from "@/graphql/externalUrls";
import { mcpOAuthRedirectUri } from "@/graphql/mcpOAuthCallback";
import { TaskActions } from "@/components/work/TaskActions";
import { HumanInterventionCard } from "./HumanInterventionCard";

type PendingHumanIntervention = PendingHumanInterventionsQuery["pendingHumanInterventions"][number];
type PendingGovernedAction = Extract<PendingHumanIntervention, { __typename: "GovernedAction" }>;
type PendingMcpAuthentication = Extract<PendingHumanIntervention, { __typename: "McpAuthenticationIntervention" }>;
type PendingAdapterAuthentication = Extract<PendingHumanIntervention, { __typename: "AdapterAuthenticationIntervention" }>;
type PendingTaskAttention = Extract<PendingHumanIntervention, { __typename: "TaskAttention" }>;

export function TaskGateInterventionCard({
  attention,
  onResolved
}: {
  attention: PendingTaskAttention;
  onResolved?: () => void;
}) {
  const gate = attention.gate;
  if (!gate) return null;
  const context = gate.contextMarkdown.trim();
  return (
    <TaskActions
      inlineResponse
      answerChoices={gate.kind === "CLARIFICATION" ? gate.suggestedAnswers : []}
      task={attention.task}
      validActions={attention.validActions}
      onUpdated={() => onResolved?.()}
    >
      {(actions) => (
        <HumanInterventionCard
          title={gate.prompt.trim() || attention.summary}
          actions={actions}
          actionLayout="response"
        >
          {context ? (
            <Markdown
              autolink="gfm"
              className="noema-assistant-markdown"
              contentWidth="100%"
              density="compact"
              headingLevelStart={4}
              xstyle={markdownXStyle(styles.context)}
            >
              {context}
            </Markdown>
          ) : null}
        </HumanInterventionCard>
      )}
    </TaskActions>
  );
}

export function GovernedActionCard({
  action,
  onResolved
}: {
  action: PendingGovernedAction;
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
    <HumanInterventionCard
      label={reviewLabel(action.reviewRoute, action.behavior?.readOnly)}
      meta={action.taskId ? "Background task" : "Primary conversation"}
      title={action.safeSummary}
      error={error}
      actions={(
        <>
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
        </>
      )}
    >
      <span {...stylex.props(styles.capability)}>{action.capabilityName}</span>
      <details {...stylex.props(styles.details)}>
        <summary>Review exact arguments</summary>
        <pre {...stylex.props(styles.arguments)}>{formatArguments(action.arguments)}</pre>
      </details>
    </HumanInterventionCard>
  );
}

export function McpAuthenticationCard({
  request,
  onResolved
}: {
  request: PendingMcpAuthentication;
  onResolved?: () => void;
}) {
  const [startAuthentication, startState] = useMutation(StartMcpAuthenticationDocument);
  const [skipAuthentication, skipState] = useMutation(SkipMcpAuthenticationDocument);
  const [error, setError] = React.useState<string | null>(null);
  const oauth = useMcpOAuthController<string>({
    onCompleted: () => onResolved?.(),
    onFailed: (message) => setError(message)
  });
  const start = async () => {
    const navigation = reserveExternalAuthNavigation();
    setError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startAuthentication({ variables: { input: {
        requestId: request.requestId,
        expectedRevision: request.revision,
        redirectUri
      } } });
      const attempt = response.data?.startMcpAuthentication;
      if (!attempt) throw new Error("Noema did not return an MCP OAuth attempt.");
      await oauth.begin(attempt, request.requestId, navigation);
      onResolved?.();
    } catch (caught: unknown) {
      navigation.cancel();
      setError(caught instanceof Error ? caught.message : "Sign-in could not be started.");
    }
  };
  const skip = async () => {
    setError(null);
    try {
      await skipAuthentication({ variables: { input: {
        requestId: request.requestId,
        expectedRevision: request.revision
      } } });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The tool call could not be skipped.");
    }
  };
  const authorizing = startState.loading || oauth.active?.context === request.requestId;
  return (
    <HumanInterventionCard
      label="Sign-in required"
      meta={request.taskId ? "Task" : "Chat"}
      title={`Sign in to ${request.serverDisplayName}`}
      description={request.failureCode
        ? "The previous sign-in did not finish. Try again to continue."
        : request.taskId
          ? "This task is paused until you sign in."
          : "Your request is paused until you sign in."}
      error={error}
      actions={(
        <>
          <Button
            size="sm"
            variant="ghost"
            label="Skip this call"
            isDisabled={authorizing || skipState.loading}
            onClick={() => void skip()}
          />
          <Button
            size="sm"
            variant="primary"
            label="Continue in browser"
            isLoading={authorizing}
            isDisabled={authorizing || skipState.loading}
            onClick={() => void start()}
          />
        </>
      )}
    />
  );
}

export function AdapterAuthenticationCard({
  request,
  onResolved
}: {
  request: PendingAdapterAuthentication;
  onResolved?: () => void;
}) {
  const [startAuthentication, startState] = useMutation(StartAdapterAuthenticationDocument);
  const [skipAuthentication, skipState] = useMutation(SkipAdapterAuthenticationDocument);
  const [error, setError] = React.useState<string | null>(null);
  const start = async () => {
    const navigation = reserveExternalAuthNavigation();
    setError(null);
    try {
      const response = await startAuthentication({ variables: { input: {
        requestId: request.requestId,
        expectedRevision: request.revision
      } } });
      const attempt = response.data?.startAdapterAuthentication;
      if (!attempt) throw new Error("Noema did not return an OAuth attempt.");
      await navigation.open(attempt.authorizationUrl);
      onResolved?.();
    } catch (caught: unknown) {
      navigation.cancel();
      setError(caught instanceof Error ? caught.message : "Sign-in could not be started.");
    }
  };
  const skip = async () => {
    setError(null);
    try {
      await skipAuthentication({ variables: { input: {
        requestId: request.requestId,
        expectedRevision: request.revision
      } } });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The tool call could not be skipped.");
    }
  };
  const busy = startState.loading || skipState.loading;
  return (
    <HumanInterventionCard
      label="Sign-in required"
      meta={request.taskId ? "Task" : "Chat"}
      title={`Sign in to ${request.serviceDisplayName}`}
      description={request.state === "AUTHORIZING"
        ? "A sign-in was already opened. You can continue it or start again."
        : request.taskId
          ? "This task is paused until you sign in."
          : "Your request is paused until you sign in."}
      error={error}
      actions={(
        <>
          <Button
            size="sm"
            variant="ghost"
            label="Skip this call"
            isDisabled={busy}
            onClick={() => void skip()}
          />
          <Button
            size="sm"
            variant="primary"
            label={request.state === "AUTHORIZING" ? "Open sign-in again" : "Continue in browser"}
            isLoading={startState.loading}
            isDisabled={busy}
            onClick={() => void start()}
          />
        </>
      )}
    />
  );
}

function reviewLabel(route: ExecutionReviewRoute, readOnly?: boolean) {
  const behavior = readOnly ? "Read only" : "Can make changes";
  switch (route) {
    case "HUMAN_REVIEW":
      return `${behavior} · Human review`;
    case "LLM_REVIEW":
      return `${behavior} · LLM review`;
  }
}

function formatArguments(value: unknown) {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "Arguments could not be displayed.";
  }
}

function markdownXStyle(...xstyle: unknown[]): MarkdownProps["xstyle"] {
  return xstyle as MarkdownProps["xstyle"];
}

const styles = stylex.create({
  context: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  },
  capability: {
    color: "var(--noema-text-muted)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 10,
    overflowWrap: "anywhere"
  },
  details: {
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
  }
});
