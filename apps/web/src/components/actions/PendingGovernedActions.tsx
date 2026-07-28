import * as React from "react";
import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { X } from "lucide-react";
import {
  PendingHumanInterventionsDocument,
  ApproveAdapterDefinitionDocument,
  ImportAdapterOauthClientJsonDocument,
  StartAdapterOauthSetupDocument,
  SaveCapabilityConnectionPolicyDocument,
  ConversationEventsDocument,
  ResolveGovernedActionDocument,
  SkipMcpAuthenticationDocument,
  StartMcpAuthenticationDocument,
  type GovernedActionDecision,
  type ExecutionReviewRoute,
  type PendingHumanInterventionsQuery,
  type SaveCapabilityConnectionPolicyMutation
} from "@/generated/graphql";
import { useMcpOAuthController } from "@/components/mcp/useMcpOAuthController";
import {
  CapabilityPolicyChoices,
  type CapabilityDataSharingPolicy,
  type CapabilityUnsafeActionPolicy
} from "@/components/capabilities/CapabilityPolicyChoices";
import { AdapterDefinitionReviewDetails } from "@/components/capabilities/AdapterDefinitionReviewDetails";
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

const dismissedAdapterSetupsKey = "noema.dismissed-adapter-setups";

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
  const [dismissedAdapterSetups, setDismissedAdapterSetups] = React.useState(readDismissedAdapterSetups);
  const allowAdapterSetupDismissal = Boolean(conversationId) && !embedded;
  const visibleInterventions = allowAdapterSetupDismissal
    ? interventions.filter((intervention) => (
        intervention.__typename !== "AdapterDefinition"
        || !intervention.reviewed
        || adapterPolicyPending(intervention)
        || !dismissedAdapterSetups.has(intervention.semanticDigest)
      ))
    : interventions;
  const dismissAdapterSetup = React.useCallback((semanticDigest: string) => {
    setDismissedAdapterSetups((current) => {
      const next = new Set(current).add(semanticDigest);
      writeDismissedAdapterSetups(next);
      return next;
    });
  }, []);
  if (!visibleInterventions.length) return null;
  return (
    <HumanInterventionList
      interventions={visibleInterventions}
      compact={compact}
      embedded={embedded}
      onResolved={() => void result.refetch()}
      onDismissAdapterSetup={allowAdapterSetupDismissal ? dismissAdapterSetup : undefined}
    />
  );
}

export function HumanInterventionList({
  interventions,
  compact = false,
  embedded = false,
  onResolved,
  onDismissAdapterSetup
}: {
  interventions: PendingHumanIntervention[];
  compact?: boolean;
  embedded?: boolean;
  onResolved?: () => void;
  onDismissAdapterSetup?: (semanticDigest: string) => void;
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
            onDismiss={intervention.reviewed && onDismissAdapterSetup
              ? () => onDismissAdapterSetup(intervention.semanticDigest)
              : undefined}
          />
        ))}
    </section>
  );
}

function AdapterDefinitionCard({
  definition,
  compact,
  embedded,
  onResolved,
  onDismiss
}: {
  definition: PendingAdapterDefinition;
  compact: boolean;
  embedded: boolean;
  onResolved?: () => void;
  onDismiss?: () => void;
}) {
  const [approveDefinition, approval] = useMutation(ApproveAdapterDefinitionDocument);
  const [importClientJson, credentialImport] = useMutation(ImportAdapterOauthClientJsonDocument);
  const [startOauth, oauthStart] = useMutation(StartAdapterOauthSetupDocument);
  const [savePolicy, policySave] = useMutation<SaveCapabilityConnectionPolicyMutation>(
    SaveCapabilityConnectionPolicyDocument
  );
  const fileInput = React.useRef<HTMLInputElement>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [authorizing, setAuthorizing] = React.useState(false);
  const [authorizationExpiry, setAuthorizationExpiry] = React.useState<number | null>(null);
  const connection = definition.connections.find(
    (candidate) => candidate.status === "authentication_required"
  );
  const policyConnection = definition.connections.find(
    (candidate) => candidate.status === "active" && !candidate.policyConfigured
  );
  const [sharing, setSharing] = React.useState<CapabilityDataSharingPolicy>("allow_automatically");
  const [unsafeActions, setUnsafeActions] = React.useState<CapabilityUnsafeActionPolicy>("reviewer_may_approve");
  const [policyStep, setPolicyStep] = React.useState<"sharing" | "unsafe_actions">("sharing");
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
  const submitPolicy = async () => {
    if (!policyConnection) return;
    setError(null);
    try {
      await savePolicy({ variables: { input: {
        kind: "API",
        connectionId: policyConnection.connectionId,
        expectedConnectionRevision: String(policyConnection.connectionRevision),
        expectedPolicyRevision: policyConnection.policyRevision,
        dataSharingPolicy: sharing,
        unsafeActionPolicy: unsafeActions
      } } });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The tool policy could not be saved.");
    }
  };
  React.useEffect(() => {
    if (!authorizing || authorizationExpiry === null || policyConnection) return;
    const timeout = window.setTimeout(() => {
      setAuthorizing(false);
      setAuthorizationExpiry(null);
      setError("Authorization expired. You can try again.");
    }, Math.max(0, authorizationExpiry * 1000 - Date.now()));
    return () => window.clearTimeout(timeout);
  }, [authorizationExpiry, authorizing, policyConnection]);
  const operationSummary = definition.operations
    .map((operation) => `${operation.method} ${operation.path}`)
    .join("\n");
  const operationCount = definition.operations.length;
  const scopeCount = definition.scopes.length;
  const isReadOnly = definition.operations.every((operation) => operation.readOnly === true);
  const scopePhrase = scopeCount === 0
    ? "without OAuth scopes"
    : `using ${countLabel(scopeCount, "OAuth scope")}`;
  const accessSummary = isReadOnly
    ? `Noema is proposing ${countLabel(operationCount, "read-only API operation")} ${scopePhrase}.`
    : `Noema is proposing ${countLabel(operationCount, "API operation")}, including access that can make changes, ${scopePhrase}.`;
  const sourceIsHttps = definition.sourceReference.startsWith("https://");
  const setupUrl = definition.clientSetupUrl;
  const oauthSetupUnavailable = definition.reviewed
    && definition.acceptsOauthClientJson
    && !definition.oauthRedirectUri;
  return (
    <InterventionCardShell
      compact={compact}
      embedded={embedded}
      elevatedPanel
      dismissLabel="Hide OAuth setup from chat"
      onDismiss={policyConnection ? undefined : onDismiss}
      copy={
        <div {...stylex.props(styles.copy, styles.adapterCopy)}>
          <div {...stylex.props(styles.eyebrow)}>
            <span>{definition.reviewed ? (policyConnection ? `Tool permissions · ${policyStep === "sharing" ? "1" : "2"} of 2` : connection ? "Authorization" : "OAuth setup") : "Connection review"}</span>
            <span {...stylex.props(styles.accessBadge)}>{readOnlyLabel(definition.operations)}</span>
          </div>
          <div {...stylex.props(styles.adapterHeading)}>
            <strong {...stylex.props(styles.summary, styles.adapterSummary)}>
              {definition.reviewed
                ? policyConnection
                  ? `Enable ${definition.displayName}`
                  : connection
                  ? `Connect ${definition.displayName}`
                  : `Add credentials for ${definition.displayName}`
                : `Review ${definition.displayName}`}
            </strong>
            <span {...stylex.props(styles.context)}>
              {definition.reviewed
                ? policyConnection
                  ? policyStep === "sharing"
                    ? "Your account is connected. Choose when Noema may share relevant conversation details."
                    : "Choose who may approve calls that can change, delete, or send information."
                  : oauthSetupUnavailable
                  ? "This connection's reviewed OAuth callback modes do not match this Noema app. Ask Noema to propose a compatible definition."
                  : connection
                  ? "Noema has the OAuth client details. Continue in your browser to grant the reviewed access."
                  : "Open the provider's developer tools in another tab, create an OAuth client, download its JSON, then choose that file here. Noema keeps only the declared client fields."
                : accessSummary}
            </span>
          </div>
          {policyConnection ? (
            <CapabilityPolicyChoices
              serviceName={definition.displayName}
              step={policyStep}
              dataSharingPolicy={sharing}
              unsafeActionPolicy={unsafeActions}
              onChange={(policy) => {
                setSharing(policy.dataSharingPolicy);
                setUnsafeActions(policy.unsafeActionPolicy);
              }}
            />
          ) : null}
          {!definition.reviewed ? (
            <span {...stylex.props(styles.setupNote)}>
              Approving this plan confirms the definition only. It does not connect your account or grant access yet.
            </span>
          ) : null}
          {definition.reviewed && !connection && definition.oauthRedirectUri ? (
            <div {...stylex.props(styles.redirectUri)}>
              <span {...stylex.props(styles.context)}>
                <b>Authorized redirect URI</b><br />
                Copy this exact value into the provider's OAuth client form.
              </span>
              <code {...stylex.props(styles.redirectUriValue)}>{definition.oauthRedirectUri}</code>
            </div>
          ) : null}
          <details {...stylex.props(styles.details)}>
            <summary>Review access details</summary>
            <div {...stylex.props(styles.reviewDetails)}>
              <div {...stylex.props(styles.detailGroup)}>
                <strong {...stylex.props(styles.detailHeading)}>OAuth access</strong>
                <span>{scopeCount ? definition.scopes.join("\n") : "No OAuth scopes requested"}</span>
              </div>
              <div {...stylex.props(styles.detailGroup)}>
                <strong {...stylex.props(styles.detailHeading)}>API operations</strong>
                <span>{countLabel(operationCount, isReadOnly ? "read-only operation" : "operation")}</span>
                <pre {...stylex.props(styles.arguments)}>{operationSummary || "No operations requested"}</pre>
                <AdapterDefinitionReviewDetails
                  operations={definition.operations}
                  accountIdentityOperationId={definition.accountIdentityOperationId}
                />
              </div>
              {sourceIsHttps ? (
                <a href={definition.sourceReference} target="_blank" rel="noreferrer" {...stylex.props(styles.sourceLink)}>
                  Open source documentation in another tab
                </a>
              ) : (
                <span>Source: {definition.sourceReference}</span>
              )}
              <details {...stylex.props(styles.manifestDetails)}>
                <summary>Technical definition</summary>
                <div {...stylex.props(styles.technicalDetails)}>
                  <span><b>API origin</b><br />{definition.origin}</span>
                  {definition.clientSetupUrl ? (
                    <span><b>OAuth client setup</b><br />{definition.clientSetupUrl}</span>
                  ) : null}
                  <span><b>Revision</b><br />{definition.definitionRevision}</span>
                  <details {...stylex.props(styles.manifestDetails)}>
                    <summary>Canonical manifest</summary>
                    <pre {...stylex.props(styles.arguments)}>{definition.manifestJson}</pre>
                  </details>
                </div>
              </details>
            </div>
          </details>
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
        </div>
      }
      actions={
        <div {...stylex.props(styles.actions)}>
          {definition.reviewed && !policyConnection && !oauthSetupUnavailable && setupUrl ? (
            <Button
              size="sm"
              variant="ghost"
              label="Open developer tools"
              isDisabled={approval.loading || credentialImport.loading || oauthStart.loading || authorizing}
              onClick={() => void openUrl(setupUrl)}
            />
          ) : definition.reviewed && sourceIsHttps ? (
            <Button
              size="sm"
              variant="ghost"
              label={definition.reviewed ? "Open official source" : "Review source"}
              isDisabled={approval.loading}
              onClick={() => void openUrl(definition.sourceReference)}
            />
          ) : null}
          {policyConnection ? (
            policyStep === "sharing" ? (
              <Button
                size="sm"
                variant="primary"
                label="Continue"
                onClick={() => setPolicyStep("unsafe_actions")}
              />
            ) : (
              <>
                <Button
                  size="sm"
                  variant="ghost"
                  label="Back"
                  isDisabled={policySave.loading}
                  onClick={() => setPolicyStep("sharing")}
                />
                <Button
                  size="sm"
                  variant="primary"
                  label={`Enable ${definition.displayName}`}
                  isLoading={policySave.loading}
                  isDisabled={policySave.loading}
                  onClick={() => void submitPolicy()}
                />
              </>
            )
          ) : oauthSetupUnavailable ? null : definition.reviewed && connection ? (
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
              label="Approve access plan"
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
          <span>{reviewLabel(action.reviewRoute, action.behavior?.readOnly)}</span>
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
  elevatedPanel = false,
  copy,
  actions,
  dismissLabel,
  onDismiss
}: {
  compact: boolean;
  embedded: boolean;
  elevatedPanel?: boolean;
  copy: React.ReactNode;
  actions: React.ReactNode;
  dismissLabel?: string;
  onDismiss?: () => void;
}) {
  const showElevatedPanel = elevatedPanel && !embedded;
  return (
    <article {...stylex.props(
      styles.card,
      compact && styles.compactCard,
      showElevatedPanel && styles.elevatedPanel,
      embedded && styles.embeddedCard,
      onDismiss && styles.dismissibleCard
    )}>
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
      {copy}
      {actions}
    </article>
  );
}

function readDismissedAdapterSetups() {
  if (typeof window === "undefined") return new Set<string>();
  try {
    const stored = JSON.parse(window.sessionStorage.getItem(dismissedAdapterSetupsKey) ?? "[]");
    return new Set<string>(Array.isArray(stored) ? stored.filter((value): value is string => typeof value === "string") : []);
  } catch {
    return new Set<string>();
  }
}

function writeDismissedAdapterSetups(digests: Set<string>) {
  try {
    window.sessionStorage.setItem(dismissedAdapterSetupsKey, JSON.stringify([...digests]));
  } catch {
    // Storage can be unavailable in privacy-restricted browser contexts; local state still dismisses the card.
  }
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

function readOnlyLabel(operations: PendingAdapterDefinition["operations"]) {
  return operations.every((operation) => operation.readOnly === true) ? "Read only" : "Can make changes";
}

function adapterPolicyPending(intervention: PendingHumanIntervention) {
  return intervention.__typename === "AdapterDefinition"
    && intervention.connections.some(
      (connection) => connection.status === "active" && !connection.policyConfigured
    );
}

function countLabel(count: number, singular: string) {
  return `${count} ${singular}${count === 1 ? "" : "s"}`;
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
    position: "relative",
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
  elevatedPanel: {
    width: "min(520px, 100%)",
    alignItems: "stretch",
    flexDirection: "column",
    marginInline: "auto",
    gap: "var(--spacing-3)",
    padding: "var(--spacing-4)",
    borderColor: "var(--noema-border-default)",
    borderRadius: "var(--radius-container)",
    backgroundColor: "var(--color-background-popover)",
    boxShadow: "var(--shadow-low)"
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
  dismissibleCard: {
    paddingInlineEnd: "calc(var(--spacing-8) + var(--spacing-2))"
  },
  dismiss: {
    position: "absolute",
    insetBlockStart: "var(--spacing-1)",
    insetInlineEnd: "var(--spacing-1)"
  },
  copy: {
    display: "grid",
    gap: "var(--spacing-1)",
    minWidth: 0
  },
  adapterCopy: {
    gap: "var(--spacing-3)"
  },
  adapterHeading: {
    display: "grid",
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
    letterSpacing: "0.045em",
    whiteSpace: "nowrap"
  },
  accessBadge: {
    borderRadius: "var(--radius-full)",
    backgroundColor: "var(--noema-surface-sunken)",
    paddingBlock: "var(--spacing-0-5)",
    paddingInline: "var(--spacing-1)",
    color: "var(--noema-text-secondary)",
    fontSize: 10,
    letterSpacing: 0,
    textTransform: "none"
  },
  summary: {
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.35
  },
  adapterSummary: {
    fontSize: 14
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
  setupNote: {
    color: "var(--noema-text-muted)",
    fontSize: 11,
    lineHeight: 1.4
  },
  redirectUri: {
    display: "grid",
    gap: "var(--spacing-1)"
  },
  redirectUriValue: {
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--noema-text-primary)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 11,
    lineHeight: 1.4,
    overflowWrap: "anywhere",
    userSelect: "all",
    cursor: "text"
  },
  reviewDetails: {
    display: "grid",
    gap: "var(--spacing-2)",
    marginTop: "var(--spacing-2)",
    lineHeight: 1.4,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere"
  },
  detailGroup: {
    display: "grid",
    gap: "var(--spacing-1)"
  },
  detailHeading: {
    color: "var(--noema-text-primary)",
    fontSize: 11
  },
  technicalDetails: {
    display: "grid",
    gap: "var(--spacing-2)",
    marginTop: "var(--spacing-2)"
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
    justifyContent: "flex-end",
    gap: "var(--spacing-1)"
  },
  error: {
    color: "var(--noema-text-danger)",
    fontSize: 12
  }
});
