import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Markdown } from "@astryxdesign/core/Markdown";
import { MetadataList, MetadataListItem } from "@astryxdesign/core/MetadataList";
import { VStack } from "@astryxdesign/core/VStack";
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
import { TaskActions } from "@/components/tasks/TaskActions";
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
              xstyle={styles.context}
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
  const [pendingDecision, setPendingDecision] = React.useState<GovernedActionDecision | null>(null);
  const browserPreview = parseBrowserActionPreview(action.arguments);
  const browserSessionEnded = action.browserSessionAvailable === false;
  const decide = async (decision: GovernedActionDecision) => {
    setError(null);
    setPendingDecision(decision);
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
    } finally {
      setPendingDecision(null);
    }
  };
  return (
    <HumanInterventionCard
      label={reviewLabel(action.reviewRoute, action.behavior?.readOnly)}
      meta={action.taskId ? "Background task" : "Primary conversation"}
      title={browserPreview ? browserActionTitle(browserPreview) : actionRequestTitle(action)}
      error={error}
      actions={(
        <>
          <Button
            size="sm"
            variant="ghost"
            label="Decline"
            isLoading={pendingDecision === "DECLINE"}
            isDisabled={resolution.loading}
            onClick={() => void decide("DECLINE")}
          />
          <Button
            size="sm"
            variant="primary"
            label={browserSessionEnded ? "Session ended" : "Approve once"}
            isLoading={pendingDecision === "APPROVE"}
            isDisabled={resolution.loading || browserSessionEnded}
            onClick={() => void decide("APPROVE")}
          />
        </>
      )}
    >
      <VStack gap={2}>
        {browserSessionEnded ? (
          <span {...stylex.props(styles.sessionEnded)}>
            The browser session ended. This action can no longer run; retry it from the conversation.
          </span>
        ) : null}
        {browserPreview ? (
          <BrowserInteractionDetails preview={browserPreview} capabilityName={action.capabilityName} />
        ) : (
          <VStack gap={2}>
            <span {...stylex.props(styles.consequence)}>{actionRequestConsequence(action)}</span>
            <details {...stylex.props(styles.details)}>
              <summary>Review details</summary>
              <VStack gap={2} className={stylex.props(styles.reviewDetails).className}>
                <MetadataList columns="single" label={{ position: "top" }}>
                  {action.target?.serviceName ? (
                    <MetadataListItem label="Service">{action.target.serviceName}</MetadataListItem>
                  ) : null}
                  {action.target?.connectionLabel ? (
                    <MetadataListItem label="Account">{action.target.connectionLabel}</MetadataListItem>
                  ) : null}
                  <MetadataListItem label="Effect">{actionBehaviorEvidence(action)}</MetadataListItem>
                </MetadataList>
                <span {...stylex.props(styles.capability)}>{action.capabilityName}</span>
                <pre {...stylex.props(styles.arguments)}>{formatArguments(action.arguments)}</pre>
                {action.assessment ? <AssessmentDetails assessment={action.assessment} /> : null}
              </VStack>
            </details>
          </VStack>
        )}
        {browserPreview && action.assessment ? <AssessmentDetails assessment={action.assessment} /> : null}
      </VStack>
    </HumanInterventionCard>
  );
}

function actionRequestTitle(action: PendingGovernedAction) {
  const target = actionTargetName(action);
  if (action.behavior?.readOnly) return `Share request data with ${target}?`;
  if (action.behavior?.destructive) return `Allow a destructive change in ${target}?`;
  return `Allow this change in ${target}?`;
}

function actionRequestConsequence(action: PendingGovernedAction) {
  const target = actionTargetName(action);
  if (action.behavior?.readOnly) return `${target} receives the request data shown in Review details.`;
  if (action.behavior?.destructive) return `This can remove or overwrite data in ${target}.`;
  if (action.behavior?.openWorld) return `This changes data outside Noema in ${target}.`;
  return `This changes data in ${target}.`;
}

function actionTargetName(action: PendingGovernedAction) {
  return action.target?.connectionLabel
    || action.target?.serviceName
    || action.target?.serviceId
    || action.capabilityName;
}

function actionBehaviorEvidence(action: PendingGovernedAction) {
  const behavior = action.behavior;
  if (!behavior) return "No behavior evidence is available.";
  return [
    behavior.readOnly ? "Read only" : "Can make changes",
    behavior.destructive ? "Destructive" : null,
    behavior.openWorld ? "External system" : null,
    behavior.idempotent ? "Safe to repeat" : "May repeat the effect"
  ].filter(Boolean).join(" · ");
}

function AssessmentDetails({
  assessment
}: {
  assessment: NonNullable<PendingGovernedAction["assessment"]>;
}) {
  const signals = [
    assessment.risk ? `${sentenceCase(assessment.risk)} risk` : null,
    assessment.authorization ? `${sentenceCase(assessment.authorization)} authorization` : null,
    ...assessment.reasonCodes.map(sentenceCase)
  ].filter((value): value is string => value !== null);
  return (
    <VStack gap={1} className={stylex.props(styles.assessment).className}>
      <span {...stylex.props(styles.assessmentHeading)}>Why approval is required</span>
      <span>{assessment.explanation}</span>
      {signals.length > 0 ? (
        <span {...stylex.props(styles.assessmentSignals)}>{signals.join(" · ")}</span>
      ) : null}
    </VStack>
  );
}

type BrowserActionPreview = {
  action: "click" | "fill" | "type" | "press_key" | "select_option";
  value?: string;
  snapshotRevision: number;
  page?: { url: string; title: string };
  target: { reference: string; role?: string; name?: string };
};

function BrowserInteractionDetails({
  preview,
  capabilityName
}: {
  preview: BrowserActionPreview;
  capabilityName: string;
}) {
  const valueLabel = preview.action === "press_key"
    ? "Key"
    : preview.action === "select_option"
      ? "Option"
      : "Text to enter";
  return (
    <VStack gap={2}>
      <MetadataList columns="single" label={{ position: "top" }}>
        {preview.page ? (
          <MetadataListItem label="Page">
            <VStack gap={0.5}>
              <span {...stylex.props(styles.pageTitle)}>{browserPageTitle(preview.page)}</span>
              <span {...stylex.props(styles.pageUrl)}>{preview.page.url}</span>
            </VStack>
          </MetadataListItem>
        ) : null}
        {preview.value !== undefined ? (
          <MetadataListItem label={valueLabel}>
            <pre {...stylex.props(styles.previewValue)}>{preview.value}</pre>
          </MetadataListItem>
        ) : null}
      </MetadataList>
      <details {...stylex.props(styles.details)}>
        <summary>Technical details</summary>
        <VStack gap={1} className={stylex.props(styles.technicalDetails).className}>
          <span {...stylex.props(styles.capability)}>{capabilityName}</span>
          <span>Snapshot {preview.snapshotRevision} · {preview.target.reference}</span>
        </VStack>
      </details>
    </VStack>
  );
}

function parseBrowserActionPreview(value: unknown): BrowserActionPreview | null {
  if (!isRecord(value) || value.kind !== "browser_interaction") return null;
  if (!isBrowserAction(value.action) || typeof value.snapshot_revision !== "number") return null;
  const target = value.target;
  if (!isRecord(target) || typeof target.ref !== "string") return null;
  const page = isRecord(value.page)
    && typeof value.page.url === "string"
    && typeof value.page.title === "string"
      ? { url: value.page.url, title: value.page.title }
      : undefined;
  return {
    action: value.action,
    value: typeof value.value === "string" ? value.value : undefined,
    snapshotRevision: value.snapshot_revision,
    page,
    target: {
      reference: target.ref,
      role: typeof target.role === "string" ? target.role : undefined,
      name: typeof target.name === "string" ? target.name : undefined
    }
  };
}

function browserActionTitle(preview: BrowserActionPreview) {
  const target = compactText(preview.target.name)
    || compactText(preview.target.role)
    || `element ${preview.target.reference}`;
  const page = preview.page ? ` on ${browserPageTitle(preview.page)}` : "";
  switch (preview.action) {
    case "click": return `Click “${target}”${page}`;
    case "fill": return `Fill “${target}”${page}`;
    case "type": return `Type in “${target}”${page}`;
    case "press_key": return `Press a key in “${target}”${page}`;
    case "select_option": return `Choose an option in “${target}”${page}`;
  }
}

function browserPageTitle(page: NonNullable<BrowserActionPreview["page"]>) {
  const title = compactText(page.title);
  if (title) return title;
  try {
    return new URL(page.url).hostname;
  } catch {
    return "Open browser page";
  }
}

function compactText(value?: string) {
  return value?.replace(/\s+/g, " ").trim() ?? "";
}

function sentenceCase(value: string) {
  const normalized = value.toLowerCase().replaceAll("_", " ");
  return normalized.charAt(0).toUpperCase() + normalized.slice(1);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isBrowserAction(value: unknown): value is BrowserActionPreview["action"] {
  return value === "click"
    || value === "fill"
    || value === "type"
    || value === "press_key"
    || value === "select_option";
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

const styles = stylex.create({
  context: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  },
  capability: {
    color: "var(--noema-text-muted)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 12,
    overflowWrap: "anywhere"
  },
  details: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    cursor: "pointer"
  },
  consequence: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  },
  reviewDetails: {
    paddingBlockStart: "var(--spacing-2)"
  },
  arguments: {
    maxHeight: 180,
    marginBlock: "var(--spacing-2)",
    padding: "var(--spacing-2)",
    overflow: "auto",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--noema-text-primary)",
    fontSize: 12,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
    cursor: "text"
  },
  pageTitle: {
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.4,
    overflowWrap: "anywhere"
  },
  pageUrl: {
    color: "var(--noema-text-muted)",
    fontSize: 12,
    lineHeight: 1.35,
    overflowWrap: "anywhere"
  },
  previewValue: {
    maxHeight: 88,
    margin: "var(--spacing-0)",
    padding: "var(--spacing-2)",
    overflow: "auto",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-text-primary)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 12,
    lineHeight: 1.4,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
    cursor: "text"
  },
  technicalDetails: {
    paddingBlockStart: "var(--spacing-1)",
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  assessment: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  },
  assessmentHeading: {
    color: "var(--noema-text-primary)",
    fontWeight: 600
  },
  assessmentSignals: {
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  sessionEnded: {
    color: "var(--noema-text-danger)",
    fontSize: 12,
    lineHeight: 1.45
  }
});
