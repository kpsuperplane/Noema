import * as React from "react";
import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  PendingHumanInterventionsDocument,
  ApproveAdapterDefinitionDocument,
  ImportAdapterOauthClientJsonDocument,
  StartAdapterOauthSetupDocument,
  SaveCapabilityConnectionPolicyDocument,
  ConversationEventsDocument,
  type PendingHumanInterventionsQuery,
  type SaveCapabilityConnectionPolicyMutation
} from "@/generated/graphql";
import {
  CapabilityPolicyChoices,
  type CapabilityDataSharingPolicy,
  type CapabilityUnsafeActionPolicy
} from "@/components/capabilities/CapabilityPolicyChoices";
import { AdapterDefinitionReviewDetails } from "@/components/capabilities/AdapterDefinitionReviewDetails";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import { WorkTaskRuntimeEventsDocument } from "@/graphql/workOperations";
import { HumanInterventionCard } from "./HumanInterventionCard";
import {
  AdapterAuthenticationCard,
  GovernedActionCard,
  McpAuthenticationCard,
  TaskGateInterventionCard
} from "./HumanInterventionDecisionCards";

export type PendingHumanIntervention = PendingHumanInterventionsQuery["pendingHumanInterventions"][number];
type PendingAdapterDefinition = Extract<PendingHumanIntervention, { __typename: "AdapterDefinition" }>;

type Scope = {
  conversationId?: string | null;
  taskId?: string;
  projectId?: string;
};

export type HumanInterventionPlacement = "chat" | "dock" | "queue";

const dismissedAdapterSetupsKey = "noema.dismissed-adapter-setups";

export function usePendingHumanInterventions(scope: Scope = {}) {
  const result = useQuery(PendingHumanInterventionsDocument, {
    variables: {
      conversationId: scope.conversationId ?? undefined,
      taskId: scope.taskId,
      projectId: scope.projectId,
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
  projectId,
  placement = "chat"
}: Scope & { placement?: HumanInterventionPlacement }) {
  const result = usePendingHumanInterventions({ conversationId, taskId, projectId });
  const interventions = result.data?.pendingHumanInterventions ?? [];
  const [dismissedAdapterSetups, setDismissedAdapterSetups] = React.useState(readDismissedAdapterSetups);
  const allowAdapterSetupDismissal = Boolean(conversationId) && placement === "chat";
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
      placement={placement}
      onResolved={() => void result.refetch()}
      onDismissAdapterSetup={allowAdapterSetupDismissal ? dismissAdapterSetup : undefined}
    />
  );
}

export function HumanInterventionList({
  interventions,
  placement = "chat",
  onResolved,
  onDismissAdapterSetup
}: {
  interventions: PendingHumanIntervention[];
  placement?: HumanInterventionPlacement;
  onResolved?: () => void;
  onDismissAdapterSetup?: (semanticDigest: string) => void;
}) {
  return (
    <section aria-label="Items waiting for you" {...stylex.props(styles.list, placement !== "chat" && styles.fullWidthList)}>
      {interventions.map((intervention) => intervention.__typename === "TaskAttention" ? (
          <TaskGateInterventionCard
            attention={intervention}
            key={`${intervention.task.taskId}:${intervention.gate?.gateId ?? intervention.kind}`}
            onResolved={onResolved}
          />
        ) : intervention.__typename === "GovernedAction" ? (
          <GovernedActionCard
            action={intervention}
            key={`${intervention.actionId}:${intervention.revision}`}
            onResolved={onResolved}
          />
        ) : intervention.__typename === "McpAuthenticationIntervention" ? (
          <McpAuthenticationCard
            request={intervention}
            key={`${intervention.requestId}:${intervention.revision}`}
            onResolved={onResolved}
          />
        ) : intervention.__typename === "AdapterAuthenticationIntervention" ? (
          <AdapterAuthenticationCard
            request={intervention}
            key={`${intervention.requestId}:${intervention.revision}`}
            onResolved={onResolved}
          />
        ) : (
          <AdapterDefinitionCard
            definition={intervention}
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
  onResolved,
  onDismiss
}: {
  definition: PendingAdapterDefinition;
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
                  authenticationMode={definition.authenticationMode}
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

function InterventionCardShell({
  copy,
  actions,
  dismissLabel,
  onDismiss
}: {
  elevatedPanel?: boolean;
  copy: React.ReactNode;
  actions: React.ReactNode;
  dismissLabel?: string;
  onDismiss?: () => void;
}) {
  return (
    <HumanInterventionCard
      content={copy}
      actions={actions}
      dismissLabel={dismissLabel}
      onDismiss={onDismiss}
    />
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
  fullWidthList: {
    gap: "var(--spacing-3)",
    width: "100%",
    maxWidth: "none",
    marginInline: 0
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
