import { useMutation, useQuery } from "@apollo/client/react";
import { useNavigate } from "@tanstack/react-router";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Trash2 } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import { useRef, useState, type ReactNode } from "react";
import {
  AdapterDefinitionsDocument,
  ApproveAdapterDefinitionDocument,
  CapabilityIntegrationsDocument,
  DeleteAdapterConnectionDocument,
  DeleteAdapterServiceDocument,
  ImportAdapterOauthClientJsonDocument,
  type AdapterDefinitionsQuery,
  type ApproveAdapterDefinitionMutation,
  type CapabilityIntegrationsQuery,
  type DeleteAdapterConnectionMutation,
  type DeleteAdapterServiceMutation,
  type ImportAdapterOauthClientJsonMutation
} from "@/generated/graphql";
import { CapabilityIntegrationList } from "./CapabilityIntegrationList";
import { CapabilityManagementLayout } from "./CapabilityManagementLayout";
import { DeleteConnectionDialog, DeleteServiceDialog } from "./DeleteConnectionDialog";
import { AdapterDefinitionReviewDetails } from "@/components/capabilities/AdapterDefinitionReviewDetails";

export function AdapterSettingsPane({ connectionId }: { connectionId?: string }) {
  const navigate = useNavigate();
  const clientJsonInputRef = useRef<HTMLInputElement>(null);
  const importRevisionRef = useRef<string | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleteServiceTargetId, setDeleteServiceTargetId] = useState<string | null>(null);
  const [deleteServiceError, setDeleteServiceError] = useState<string | null>(null);
  const result = useQuery<AdapterDefinitionsQuery>(AdapterDefinitionsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const integrationsResult = useQuery<CapabilityIntegrationsQuery>(CapabilityIntegrationsDocument, {
    variables: { kind: "API" },
    fetchPolicy: "cache-and-network"
  });
  const [approve, approval] = useMutation<ApproveAdapterDefinitionMutation>(
    ApproveAdapterDefinitionDocument
  );
  const [importClient, importing] = useMutation<ImportAdapterOauthClientJsonMutation>(
    ImportAdapterOauthClientJsonDocument
  );
  const [deleteConnection, deletingConnection] = useMutation<DeleteAdapterConnectionMutation>(
    DeleteAdapterConnectionDocument
  );
  const [deleteService, deletingService] = useMutation<DeleteAdapterServiceMutation>(
    DeleteAdapterServiceDocument
  );
  const definitions = (result.data?.adapterDefinitions ?? []).filter(
    (definition) => !definition.superseded
  );
  const pendingDefinitions = definitions.filter((definition) => !definition.reviewed);
  const integrations = integrationsResult.data?.capabilityIntegrations ?? [];
  const displayIntegrations = integrations.map((integration) => ({
    ...integration,
    name: definitions.find((definition) => definition.semanticDigest === integration.sourceRevision)
      ?.displayName ?? integration.name
  }));
  const selectedConnection = displayIntegrations
    .flatMap((integration) => integration.connections)
    .find((connection) => connection.connectionId === connectionId) ?? null;
  const selectedDefinition = selectedConnection
    ? definitions.find((definition) => definition.semanticDigest === selectedConnection.sourceRevision) ?? null
    : null;
  const deleteServiceTarget = displayIntegrations.find(
    (integration) => integration.definitionId === deleteServiceTargetId
  ) ?? null;

  if ((result.loading && !result.data) || (integrationsResult.loading && !integrationsResult.data)) {
    return <p {...stylex.props(styles.muted, styles.pageState)}>Loading discovered definitions...</p>;
  }
  if (result.error || integrationsResult.error) {
    return (
      <div {...stylex.props(styles.stack, styles.pageState)}>
        <p {...stylex.props(styles.muted)}>Couldn't load discovered definitions.</p>
        <Button
          type="button"
          variant="secondary"
          label="Retry"
          {...stylex.props(styles.fit)}
          onClick={() => void Promise.all([result.refetch(), integrationsResult.refetch()])}
        />
      </div>
    );
  }
  async function approveDefinition(semanticDigest: string) {
    await approve({ variables: { input: { semanticDigest } } });
    await Promise.all([result.refetch(), integrationsResult.refetch()]);
  }

  async function addConnection(semanticDigest: string, file: File | undefined) {
    if (!file || file.size === 0 || file.size > 32 * 1024) return;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await importClient({ variables: {
        input: { semanticDigest, clientJsonBase64: encodeBase64(bytes) }
      } });
      await Promise.all([result.refetch(), integrationsResult.refetch()]);
    } catch {
      // Apollo exposes the safe error state below the list.
    }
  }

  async function deleteSelectedConnection() {
    if (!selectedConnection) return;
    setDeleteError(null);
    try {
      const response = await deleteConnection({ variables: { input: {
        connectionId: selectedConnection.connectionId,
        expectedConnectionRevision: Number(selectedConnection.connectionRevision)
      } } });
      if (!response.data?.deleteAdapterConnection) {
        setDeleteError("Noema could not find that API connection.");
        return;
      }
      await Promise.all([result.refetch(), integrationsResult.refetch()]);
      setDeleteOpen(false);
      void navigate({ to: "/settings/tools/apis" });
    } catch {
      setDeleteError("This API connection changed or could not be deleted. Reload it and try again.");
    }
  }

  async function deleteSelectedService() {
    if (!deleteServiceTarget) return;
    setDeleteServiceError(null);
    try {
      const response = await deleteService({ variables: { input: {
        definitionId: deleteServiceTarget.definitionId,
        expectedSourceRevision: deleteServiceTarget.sourceRevision
      } } });
      if (!response.data?.deleteAdapterService) {
        setDeleteServiceError("Noema could not find that API service.");
        return;
      }
      await Promise.all([result.refetch(), integrationsResult.refetch()]);
      setDeleteServiceTargetId(null);
    } catch {
      setDeleteServiceError("This API service changed or is still in use. Reload it and try again.");
    }
  }

  return (
    <>
      <CapabilityManagementLayout
        kind="API"
        connectionId={connectionId}
        list={
          <div {...stylex.props(styles.stack)}>
          {!connectionId
            ? pendingDefinitions.map((definition) => (
              <article key={definition.semanticDigest} {...stylex.props(styles.card)}>
              <div {...stylex.props(styles.titleRow)}>
                <h2 {...stylex.props(styles.title)}>{definition.displayName}</h2>
                <Badge
                  variant="neutral"
                  label={definition.reviewed ? "Reviewed" : "Pending review"}
                />
              </div>

              <dl {...stylex.props(styles.details)}>
                <DefinitionRow label="Source">
                  {isHttpsUrl(definition.sourceReference) ? (
                    <a
                      href={definition.sourceReference}
                      target="_blank"
                      rel="noreferrer"
                      {...stylex.props(styles.link)}
                    >
                      {definition.sourceReference}
                    </a>
                  ) : (
                    definition.sourceReference
                  )}
                </DefinitionRow>
                <DefinitionRow label="API origin">{definition.origin}</DefinitionRow>
                <DefinitionRow label="Authentication">
                  {humanize(definition.authenticationMode)}
                </DefinitionRow>
                <DefinitionRow label="Scopes">
                  {definition.scopes.length > 0 ? definition.scopes.join(", ") : "None"}
                </DefinitionRow>
                <DefinitionRow label="Revision">{definition.definitionRevision}</DefinitionRow>
              </dl>

              <div {...stylex.props(styles.operations)}>
                <h3 {...stylex.props(styles.subheading)}>Proposed tools</h3>
                {definition.operations.map((operation) => (
                  <div key={operation.operationId} {...stylex.props(styles.operation)}>
                    <code {...stylex.props(styles.code)}>
                      {operation.method} {operation.path}
                    </code>
                    <span {...stylex.props(styles.muted)}>
                      {operation.operationId} · {operation.readOnly ? "Read only" : "Can make changes"} ·{" "}
                      {operation.idempotent ? "Safe to repeat" : "Do not retry"}
                    </span>
                  </div>
                ))}
              </div>

              <AdapterDefinitionReviewDetails
                operations={definition.operations}
                accountIdentityOperationId={definition.accountIdentityOperationId}
                authenticationMode={definition.authenticationMode}
              />

              {definition.reviewed ? (
                <p {...stylex.props(styles.muted)}>
                  Definition approved. Account setup will use these exact operations and scopes.
                </p>
              ) : (
                <Button
                  type="button"
                  label="Approve definition"
                  isDisabled={approval.loading}
                  {...stylex.props(styles.fit)}
                  onClick={() => void approveDefinition(definition.semanticDigest)}
                />
              )}
              {approval.error ? (
                <p role="alert" {...stylex.props(styles.error)}>
                  {approval.error.message}
                </p>
              ) : null}
              </article>
            ))
            : null}
          <CapabilityIntegrationList
            integrations={displayIntegrations}
            kind="API"
            selectedConnectionId={connectionId}
            emptyMessage="No API definitions discovered yet. Ask Momo to connect a service and it can research the official API."
            isAddingConnection={importing.loading}
            integrationAction={(integration) => (
              <Button
                type="button"
                size="sm"
                variant="destructive"
                label="Delete service"
                icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                isDisabled={deletingService.loading}
                onClick={() => {
                  setDeleteServiceError(null);
                  setDeleteServiceTargetId(integration.definitionId);
                }}
              />
            )}
            onAddConnection={(integration) => {
              importRevisionRef.current = integration.sourceRevision;
              clientJsonInputRef.current?.click();
            }}
          />
          <input
            ref={clientJsonInputRef}
            type="file"
            accept="application/json,.json"
            disabled={importing.loading}
            {...stylex.props(styles.hiddenInput)}
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              const sourceRevision = importRevisionRef.current;
              event.currentTarget.value = "";
              importRevisionRef.current = null;
              if (sourceRevision) void addConnection(sourceRevision, file);
            }}
          />
          {importing.error ? (
            <p role="alert" {...stylex.props(styles.error)}>
              The OAuth client JSON could not be imported.
            </p>
          ) : null}
          </div>
        }
        definitionDetails={selectedDefinition ? (
          <div {...stylex.props(styles.definitionInspection)}>
            <AdapterDefinitionReviewDetails
              operations={selectedDefinition.operations}
              accountIdentityOperationId={selectedDefinition.accountIdentityOperationId}
              authenticationMode={selectedDefinition.authenticationMode}
            />
            <details>
              <summary {...stylex.props(styles.inspectSummary)}>Canonical definition</summary>
              <pre {...stylex.props(styles.manifest)}>{selectedDefinition.manifestJson}</pre>
            </details>
          </div>
        ) : null}
        dangerAction={selectedConnection ? (
          <Button
            type="button"
            variant="destructive"
            label="Delete connection"
            icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
            isDisabled={deletingConnection.loading}
            onClick={() => {
              setDeleteError(null);
              setDeleteOpen(true);
            }}
          />
        ) : null}
      />
      <DeleteConnectionDialog
        connection={selectedConnection ? {
          name: selectedConnection.name,
          toolCount: selectedConnection.toolCount
        } : null}
        open={deleteOpen && selectedConnection !== null}
        submitting={deletingConnection.loading}
        error={deleteError}
        onOpenChange={(open) => {
          if (!open && !deletingConnection.loading) setDeleteOpen(false);
        }}
        onConfirm={() => void deleteSelectedConnection()}
      />
      <DeleteServiceDialog
        service={deleteServiceTarget ? {
          name: deleteServiceTarget.name,
          connectionCount: deleteServiceTarget.connections.length
        } : null}
        open={deleteServiceTarget !== null}
        submitting={deletingService.loading}
        error={deleteServiceError}
        onOpenChange={(open) => {
          if (!open && !deletingService.loading) setDeleteServiceTargetId(null);
        }}
        onConfirm={() => void deleteSelectedService()}
      />
    </>
  );
}

function DefinitionRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div {...stylex.props(styles.definitionRow)}>
      <dt {...stylex.props(styles.term)}>{label}</dt>
      <dd {...stylex.props(styles.value)}>{children}</dd>
    </div>
  );
}

function humanize(value: string) {
  return value.replaceAll("_", " ");
}

function isHttpsUrl(value: string) {
  try {
    return new URL(value).protocol === "https:";
  } catch {
    return false;
  }
}

const styles = stylex.create({
  stack: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  pageState: {
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
    }
  },
  card: {
    display: "grid",
    gap: "var(--spacing-3)",
    padding: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--surface-raised)"
  },
  titleRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)"
  },
  title: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    fontWeight: 600
  },
  details: {
    display: "grid",
    margin: "var(--spacing-0)"
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "9rem minmax(0, 1fr)",
    gap: "var(--spacing-2)",
    paddingBlock: "var(--spacing-1)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr",
      gap: "var(--spacing-0)"
    }
  },
  term: {
    color: "var(--muted-foreground)",
    fontSize: 13
  },
  value: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    overflowWrap: "anywhere",
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  operations: {
    display: "grid",
    gap: "var(--spacing-2)"
  },
  subheading: {
    margin: "var(--spacing-0)",
    fontSize: 13,
    fontWeight: 600
  },
  operation: {
    display: "grid",
    gap: "var(--spacing-1)"
  },
  code: {
    overflowWrap: "anywhere",
    fontFamily: "var(--font-mono)",
    fontSize: 13
  },
  muted: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  link: {
    color: "var(--text-accent)"
  },
  error: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13
  },
  fit: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  },
  hiddenInput: {
    position: "absolute",
    width: 1,
    height: 1,
    overflow: "hidden",
    clip: "rect(0 0 0 0)"
  },
  definitionInspection: {
    display: "grid",
    gap: "var(--spacing-2)"
  },
  inspectSummary: {
    cursor: "pointer",
    fontSize: 12,
    fontWeight: 600
  },
  manifest: {
    maxHeight: 280,
    margin: "var(--spacing-2) 0 0",
    padding: "var(--spacing-2)",
    overflow: "auto",
    borderRadius: 6,
    backgroundColor: "var(--noema-surface-subtle)",
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
    cursor: "text"
  }
});

function encodeBase64(bytes: Uint8Array) {
  let value = "";
  for (const byte of bytes) value += String.fromCharCode(byte);
  return btoa(value);
}
