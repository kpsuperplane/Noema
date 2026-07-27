import * as React from "react";
import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  PendingHumanInterventionsDocument,
  ApproveAdapterDefinitionDocument,
  ImportAdapterOauthClientJsonDocument,
  StartAdapterOauthSetupDocument,
  ConversationEventsDocument,
  ResolveGovernedActionDocument,
  SkipMcpAuthenticationDocument,
  StartMcpAuthenticationDocument,
  type GovernedActionDecision,
  type GovernedActionEffect,
  type PendingHumanInterventionsQuery
} from "@/generated/graphql";
import { useMcpOAuthController } from "@/components/mcp/useMcpOAuthController";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import { mcpOAuthRedirectUri } from "@/graphql/mcpOAuthCallback";
import { WorkTaskRuntimeEventsDocument } from "@/graphql/workOperations";

export type PendingHumanIntervention = PendingHumanInterventionsQuery["pendingHumanInterventions"][number];
export type PendingGovernedAction = Extract<PendingHumanIntervention, { __typename: "GovernedAction" }>;
type PendingMcpAuthentication = Extract<PendingHumanIntervention, { __typename: "McpAuthenticationIntervention" }>;
type PendingAdapterDefinition = Extract<PendingHumanIntervention, { __typename: "AdapterDefinition" }>;

type Scope = {
  conversationId?: string | null;
  taskId?: string;
};

export function usePendingHumanInterventions(scope: Scope = {}) {
  const result = useQuery(PendingHumanInterventionsDocument, {
    variables: {
      conversationId: scope.conversationId ?? undefined,
      taskId: scope.taskId,
      first: 50
    },
    skip: scope.conversationId === null,
    fetchPolicy: "cache-and-network"
  });
  useSubscription(ConversationEventsDocument, {
    variables: { conversationId: scope.conversationId ?? "" },
    skip: !scope.conversationId,
    onData: () => void result.refetch()
  });
  useSubscription(WorkTaskRuntimeEventsDocument, {
    variables: { taskId: scope.taskId ?? "" },
    skip: !scope.taskId,
    onData: () => void result.refetch()
  });
  return result;
}

export function PendingHumanInterventions({
  conversationId,
  taskId,
  compact = false,
  embedded = false
}: Scope & { compact?: boolean; embedded?: boolean }) {
  const result = usePendingHumanInterventions({ conversationId, taskId });
  const interventions = result.data?.pendingHumanInterventions ?? [];
  if (!interventions.length) return null;
  return (
    <HumanInterventionList
      interventions={interventions}
      compact={compact}
      embedded={embedded}
      onResolved={() => void result.refetch()}
    />
  );
}

export function HumanInterventionList({
  interventions,
  compact = false,
  embedded = false,
  onResolved
}: {
  interventions: PendingHumanIntervention[];
  compact?: boolean;
  embedded?: boolean;
  onResolved?: () => void;
}) {
  return (
    <section aria-label="Items waiting for you" {...stylex.props(styles.list, embedded && styles.embeddedList)}>
      {interventions.map((intervention) => intervention.__typename === "GovernedAction" ? (
          <GovernedActionCard
            action={intervention}
            compact={compact}
            embedded={embedded}
            key={`${intervention.actionId}:${intervention.revision}`}
            onResolved={onResolved}
          />
        ) : intervention.__typename === "McpAuthenticationIntervention" ? (
          <McpAuthenticationCard
            request={intervention}
            compact={compact}
            embedded={embedded}
            key={`${intervention.requestId}:${intervention.revision}`}
            onResolved={onResolved}
          />
        ) : (
          <AdapterDefinitionCard
            definition={intervention}
            compact={compact}
            embedded={embedded}
            key={intervention.semanticDigest}
            onResolved={onResolved}
          />
        ))}
    </section>
  );
}

function AdapterDefinitionCard({
  definition,
  compact,
  embedded,
  onResolved
}: {
  definition: PendingAdapterDefinition;
  compact: boolean;
  embedded: boolean;
  onResolved?: () => void;
}) {
  const [approveDefinition, approval] = useMutation(ApproveAdapterDefinitionDocument);
  const [importClientJson, credentialImport] = useMutation(ImportAdapterOauthClientJsonDocument);
  const [startOauth, oauthStart] = useMutation(StartAdapterOauthSetupDocument);
  const fileInput = React.useRef<HTMLInputElement>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [authorizing, setAuthorizing] = React.useState(false);
  const [authorizationExpiry, setAuthorizationExpiry] = React.useState<number | null>(null);
  const connection = definition.connections.find(
    (candidate) => candidate.status === "authentication_required"
  );
  const approve = async () => {
    setError(null);
    try {
      await approveDefinition({
        variables: { input: { semanticDigest: definition.semanticDigest } }
      });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The connection definition could not be approved.");
    }
  };
  const openUrl = async (url: string) => {
    const handled = await openExternalUrlForAuth(url);
    if (!handled) window.open(url, "_blank", "noopener,noreferrer");
  };
  const importCredentials = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file) return;
    setError(null);
    if (file.size === 0 || file.size > 32 * 1024) {
      setError("Choose a non-empty OAuth client JSON file smaller than 32 KB.");
      return;
    }
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await importClientJson({
        variables: {
          input: {
            semanticDigest: definition.semanticDigest,
            clientJsonBase64: encodeBase64(bytes)
          }
        }
      });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The OAuth client JSON could not be imported.");
    }
  };
  const authorize = async () => {
    if (!connection) return;
    setError(null);
    try {
      const response = await startOauth({
        variables: {
          input: {
            connectionId: connection.connectionId,
            expectedConnectionRevision: connection.connectionRevision,
            expectedCredentialRevision: connection.credentialRevision,
            expectedGrantRevision: connection.grantRevision,
            expectedPolicyRevision: connection.policyRevision
          }
        }
      });
      const attempt = response.data?.startAdapterOauthSetup;
      if (!attempt) throw new Error("Noema did not return an OAuth attempt.");
      setAuthorizationExpiry(attempt.expiresAtEpochSeconds);
      setAuthorizing(true);
      await openUrl(attempt.authorizationUrl);
    } catch (caught: unknown) {
      setAuthorizing(false);
      setAuthorizationExpiry(null);
      setError(caught instanceof Error ? caught.message : "Authorization could not be started.");
    }
  };
  React.useEffect(() => {
    if (!authorizing || authorizationExpiry === null) return;
    const refetch = () => onResolved?.();
    const returned = () => {
      refetch();
      setAuthorizing(false);
      setAuthorizationExpiry(null);
    };
    const interval = window.setInterval(refetch, 1500);
    const timeout = window.setTimeout(() => {
      setAuthorizing(false);
      setAuthorizationExpiry(null);
      setError("Authorization expired. You can try again.");
    }, Math.max(0, authorizationExpiry * 1000 - Date.now()));
    window.addEventListener("focus", returned, { once: true });
    return () => {
      window.clearInterval(interval);
      window.clearTimeout(timeout);
      window.removeEventListener("focus", returned);
    };
  }, [authorizationExpiry, authorizing, onResolved]);
  const operationSummary = definition.operations
    .map((operation) => `${operation.method} ${operation.path}`)
    .join("\n");
  const sourceIsHttps = definition.sourceReference.startsWith("https://");
  const setupUrl = definition.clientSetupUrl;
  return (
    <InterventionCardShell
      compact={compact}
      embedded={embedded}
      copy={
        <div {...stylex.props(styles.copy)}>
          <div {...stylex.props(styles.eyebrow)}>
            <span>{definition.reviewed ? (connection ? "Authorization" : "OAuth setup") : "Connection review"}</span>
            <span>{readOnlyLabel(definition.operations)}</span>
          </div>
          <strong {...stylex.props(styles.summary)}>
            {definition.reviewed
              ? connection
                ? `Connect ${definition.displayName}`
                : `Add credentials for ${definition.displayName}`
              : `Allow ${definition.displayName}`}
          </strong>
          <span {...stylex.props(styles.context)}>
            {definition.reviewed
              ? connection
                ? "Noema has the OAuth client details. Continue in your browser to grant the reviewed access."
                : "Open the provider's developer tools in another tab, create an OAuth client, download its JSON, then choose that file here. Noema keeps only the declared client fields."
              : "Noema researched this API definition. Approving it allows only the operations and OAuth scopes shown here."}
          </span>
          <details {...stylex.props(styles.details)}>
            <summary>Review access and definition</summary>
            <div {...stylex.props(styles.reviewDetails)}>
              <span><b>OAuth scopes</b><br />{definition.scopes.length ? definition.scopes.join("\n") : "None"}</span>
              <span><b>Operations</b></span>
              <pre {...stylex.props(styles.arguments)}>{operationSummary}</pre>
              <span><b>API origin</b><br />{definition.origin}</span>
              {definition.clientSetupUrl ? (
                <span><b>OAuth client setup</b><br />{definition.clientSetupUrl}</span>
              ) : null}
              <span><b>Revision</b><br />{definition.definitionRevision}</span>
              {sourceIsHttps ? (
                <a href={definition.sourceReference} target="_blank" rel="noreferrer" {...stylex.props(styles.sourceLink)}>
                  Open official source in another tab
                </a>
              ) : (
                <span>Source: {definition.sourceReference}</span>
              )}
              <details {...stylex.props(styles.manifestDetails)}>
                <summary>Canonical manifest</summary>
                <pre {...stylex.props(styles.arguments)}>{definition.manifestJson}</pre>
              </details>
            </div>
          </details>
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
        </div>
      }
      actions={
        <div {...stylex.props(styles.actions)}>
          {definition.reviewed && setupUrl ? (
            <Button
              size="sm"
              variant="ghost"
              label="Open developer tools"
              isDisabled={approval.loading || credentialImport.loading || oauthStart.loading || authorizing}
              onClick={() => void openUrl(setupUrl)}
            />
          ) : sourceIsHttps ? (
            <Button
              size="sm"
              variant="ghost"
              label={definition.reviewed ? "Open official source" : "Review source"}
              isDisabled={approval.loading}
              onClick={() => void openUrl(definition.sourceReference)}
            />
          ) : null}
          {definition.reviewed && connection ? (
            <Button
              size="sm"
              variant="primary"
              label="Continue in browser"
              isLoading={oauthStart.loading || authorizing}
              isDisabled={oauthStart.loading || authorizing}
              onClick={() => void authorize()}
            />
          ) : definition.reviewed ? (
            <>
              <input
                ref={fileInput}
                type="file"
                accept="application/json,.json"
                hidden
                onChange={(event) => void importCredentials(event)}
              />
              <Button
                size="sm"
                variant="primary"
                label="Choose OAuth client JSON"
                isLoading={credentialImport.loading}
                isDisabled={credentialImport.loading || oauthStart.loading}
                onClick={() => fileInput.current?.click()}
              />
            </>
          ) : (
            <Button
              size="sm"
              variant="primary"
              label="Approve definition"
              isLoading={approval.loading}
              isDisabled={approval.loading}
              onClick={() => void approve()}
            />
          )}
        </div>
      }
    />
  );
}

function GovernedActionCard({
  action,
  compact,
  embedded,
  onResolved
}: {
  action: PendingGovernedAction;
  compact: boolean;
  embedded: boolean;
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
    <InterventionCardShell
      compact={compact}
      embedded={embedded}
      copy={
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
      }
      actions={
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
      }
    />
  );
}

function McpAuthenticationCard({
  request,
  compact,
  embedded,
  onResolved
}: {
  request: PendingMcpAuthentication;
  compact: boolean;
  embedded: boolean;
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
    setError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startAuthentication({
        variables: {
          input: {
            requestId: request.requestId,
            expectedRevision: request.revision,
            redirectUri
          }
        }
      });
      const attempt = response.data?.startMcpAuthentication;
      if (!attempt) throw new Error("Noema did not return an MCP OAuth attempt.");
      await oauth.begin(attempt, request.requestId);
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "Sign-in could not be started.");
    }
  };
  const skip = async () => {
    setError(null);
    try {
      await skipAuthentication({
        variables: {
          input: {
            requestId: request.requestId,
            expectedRevision: request.revision
          }
        }
      });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The tool call could not be skipped.");
    }
  };
  const authorizing = startState.loading || oauth.active?.context === request.requestId;
  return (
    <InterventionCardShell
      compact={compact}
      embedded={embedded}
      copy={
        <div {...stylex.props(styles.copy)}>
          <div {...stylex.props(styles.eyebrow)}>
            <span>Sign-in required</span>
            <span>{request.taskId ? "Task" : "Chat"}</span>
          </div>
          <strong {...stylex.props(styles.summary)}>Sign in to {request.serverDisplayName}</strong>
          <span {...stylex.props(styles.context)}>
            {request.failureCode
              ? "The previous sign-in did not finish. Try again to continue."
              : request.taskId
                ? "This task is paused until you sign in."
                : "Your request is paused until you sign in."}
          </span>
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
        </div>
      }
      actions={
        <div {...stylex.props(styles.actions)}>
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
        </div>
      }
    />
  );
}

function InterventionCardShell({
  compact,
  embedded,
  copy,
  actions
}: {
  compact: boolean;
  embedded: boolean;
  copy: React.ReactNode;
  actions: React.ReactNode;
}) {
  return (
    <article {...stylex.props(styles.card, compact && styles.compactCard, embedded && styles.embeddedCard)}>
      {copy}
      {actions}
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

function readOnlyLabel(operations: PendingAdapterDefinition["operations"]) {
  return operations.every((operation) => operation.effect === "read_only") ? "Read only" : "Can make changes";
}

function formatArguments(value: unknown) {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "Arguments could not be displayed.";
  }
}

function encodeBase64(bytes: Uint8Array) {
  let value = "";
  for (let index = 0; index < bytes.length; index += 1) {
    value += String.fromCharCode(bytes[index] ?? 0);
  }
  return btoa(value);
}

const styles = stylex.create({
  list: {
    display: "grid",
    gap: "var(--spacing-2)",
    width: "min(640px, var(--chat-column-width, 100%))",
    maxWidth: "100%",
    marginInline: "auto"
  },
  embeddedList: {
    gap: "var(--spacing-3)",
    width: "100%",
    maxWidth: "none",
    marginInline: 0
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
  embeddedCard: {
    borderWidth: 0,
    borderRadius: 0,
    backgroundColor: "transparent",
    boxShadow: "none",
    padding: 0
  },
  copy: {
    display: "grid",
    gap: "var(--spacing-1)",
    minWidth: 0
  },
  eyebrow: {
    display: "flex",
    flexWrap: "nowrap",
    gap: 8,
    color: "var(--noema-text-muted)",
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: "0.045em",
    whiteSpace: "nowrap"
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
  context: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.4
  },
  reviewDetails: {
    display: "grid",
    gap: "var(--spacing-2)",
    marginTop: "var(--spacing-2)",
    lineHeight: 1.4,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere"
  },
  sourceLink: {
    color: "var(--noema-text-link)",
    textDecoration: "underline"
  },
  manifestDetails: {
    marginTop: "var(--spacing-1)"
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
