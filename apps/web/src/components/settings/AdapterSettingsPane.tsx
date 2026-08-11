import { useMutation, useQuery } from "@apollo/client/react";
import { useNavigate } from "@tanstack/react-router";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import { VStack } from "@astryxdesign/core/VStack";
import { Trash2 } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import {
  AdapterDefinitionsDocument,
  ApproveAdapterDefinitionDocument,
  CapabilityIntegrationsDocument,
  DeleteAdapterConnectionDocument,
  DeleteAdapterServiceDocument,
  SetupAdapterConnectionDocument,
  type AdapterDefinitionsQuery,
  type ApproveAdapterDefinitionMutation,
  type CapabilityIntegrationsQuery,
  type DeleteAdapterConnectionMutation,
  type DeleteAdapterServiceMutation,
  type SetupAdapterConnectionMutation
} from "@/generated/graphql";
import { CapabilityIntegrationList } from "./CapabilityIntegrationList";
import { CapabilityManagementLayout } from "./CapabilityManagementLayout";
import { DeleteConnectionDialog, DeleteServiceDialog } from "./DeleteConnectionDialog";
import { AdapterDefinitionReviewDetails } from "@/components/capabilities/AdapterDefinitionReviewDetails";
import { AdapterCredentialSetupDialog, type AdapterCredentialSubmission } from "@/components/capabilities/AdapterCredentialSetupDialog";

type AdapterDefinition = AdapterDefinitionsQuery["adapterDefinitions"][number];

export function AdapterSettingsPane({ connectionId }: { connectionId?: string }) {
  const navigate = useNavigate();
  const [setupRevision, setSetupRevision] = useState<string | null>(null);
  const [setupError, setSetupError] = useState<string | null>(null);
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
  const [setupConnection, settingUp] = useMutation<SetupAdapterConnectionMutation>(
    SetupAdapterConnectionDocument
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
  const setupDefinition = definitions.find((definition) => definition.semanticDigest === setupRevision) ?? null;

  if ((result.loading && !result.data) || (integrationsResult.loading && !integrationsResult.data)) {
    return <p {...stylex.props(styles.muted, styles.pageState)}>Loading discovered definitions...</p>;
  }
  if (result.error || integrationsResult.error) {
    return (
      <SettingsSection {...stylex.props(styles.pageState)}>
        <VStack gap={2}>
          <h2 {...stylex.props(styles.sectionTitle)}>API definitions</h2>
          <p {...stylex.props(styles.muted)}>Couldn't load discovered definitions.</p>
        <Button
          type="button"
          variant="secondary"
          label="Retry"
          {...stylex.props(styles.fit)}
          onClick={() => void Promise.all([result.refetch(), integrationsResult.refetch()])}
        />
        </VStack>
      </SettingsSection>
    );
  }
  async function approveDefinition(semanticDigest: string) {
    await approve({ variables: { input: { semanticDigest } } });
    await Promise.all([result.refetch(), integrationsResult.refetch()]);
  }

  async function addConnection(semanticDigest: string, submission: AdapterCredentialSubmission) {
    setSetupError(null);
    try {
      const documentBase64 = submission.document
        ? encodeBase64(new Uint8Array(await submission.document.arrayBuffer()))
        : null;
      await setupConnection({ variables: {
        input: { semanticDigest, fieldValues: submission.fieldValues, documentBase64 }
      } });
      await Promise.all([result.refetch(), integrationsResult.refetch()]);
      setSetupRevision(null);
    } catch (caught: unknown) {
      setSetupError(caught instanceof Error ? caught.message : "The reviewed credentials could not be added.");
      throw caught;
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
          <VStack gap={4} {...stylex.props(styles.stack)}>
            {!connectionId && pendingDefinitions.length > 0 ? (
              <SettingsSection aria-labelledby="api-definition-review-title">
                <VStack gap={2}>
                  <h2 id="api-definition-review-title" {...stylex.props(styles.sectionTitle)}>
                    Definition review
                  </h2>
                  <SettingsList density="balanced" hasDividers>
                    {pendingDefinitions.map((definition) => (
                      <PendingDefinitionRow
                        key={definition.semanticDigest}
                        definition={definition}
                        approving={approval.loading}
                        onApprove={() => void approveDefinition(definition.semanticDigest)}
                      />
                    ))}
                  </SettingsList>
                  {approval.error ? (
                    <p role="alert" {...stylex.props(styles.error)}>{approval.error.message}</p>
                  ) : null}
                </VStack>
              </SettingsSection>
            ) : null}
            <CapabilityIntegrationList
              integrations={displayIntegrations}
              kind="API"
              selectedConnectionId={connectionId}
              emptyMessage="No API definitions discovered yet. Ask Momo to connect a service and it can research the official API."
              isAddingConnection={settingUp.loading}
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
                setSetupError(null);
                setSetupRevision(integration.sourceRevision);
              }}
            />
            {setupDefinition && !setupDefinition.credentialSetup ? (
              <p role="alert" {...stylex.props(styles.error)}>
                This definition has no reviewed credential setup compatible with this Noema app. Ask Noema to propose a compatible definition.
              </p>
            ) : null}
          </VStack>
        }
        definitionDetails={selectedDefinition ? (
          <VStack gap={2} {...stylex.props(styles.definitionInspection)}>
            <AdapterDefinitionReviewDetails definition={selectedDefinition} />
            <details>
              <summary {...stylex.props(styles.inspectSummary)}>Canonical definition</summary>
              <pre {...stylex.props(styles.manifest)}>{selectedDefinition.manifestJson}</pre>
            </details>
          </VStack>
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
      <AdapterCredentialSetupDialog
        serviceName={setupDefinition?.displayName ?? "API"}
        setup={setupDefinition?.credentialSetup}
        scopes={setupDefinition?.scopes ?? []}
        open={setupDefinition?.credentialSetup != null}
        submitting={settingUp.loading}
        error={setupError}
        onOpenChange={(open) => {
          if (!open && !settingUp.loading) setSetupRevision(null);
        }}
        onSubmit={(submission) => setupDefinition
          ? addConnection(setupDefinition.semanticDigest, submission)
          : Promise.resolve()}
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

function PendingDefinitionRow({
  definition,
  approving,
  onApprove
}: {
  definition: AdapterDefinition;
  approving: boolean;
  onApprove: () => void;
}) {
  return (
    <SettingsListItem
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <span {...stylex.props(styles.rowLabel)}>{definition.displayName}</span>
          <Badge variant={definition.reviewed ? "success" : "warning"} label={definition.reviewed ? "Reviewed" : "Pending review"} />
        </HStack>
      }
      description={
        <VStack gap={1}>
          <span>{humanize(definition.authenticationMode)} · {definition.origin}</span>
          <VStack as="details" gap={1} {...stylex.props(styles.definitionDetails)}>
            <summary {...stylex.props(styles.inspectSummary)}>Review definition</summary>
            <AdapterDefinitionReviewDetails definition={definition} />
          </VStack>
        </VStack>
      }
      endContent={
        definition.reviewed ? (
          <span {...stylex.props(styles.muted)}>Approved</span>
        ) : (
          <Button
            type="button"
            size="sm"
            label="Approve definition"
            isDisabled={approving}
            onClick={onApprove}
          />
        )
      }
    />
  );
}

function humanize(value: string) {
  return value.replaceAll("_", " ");
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
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    fontWeight: 600
  },
  rowLabel: { color: "var(--foreground)", fontWeight: 650, overflowWrap: "anywhere" },
  definitionDetails: { minWidth: 0 },
  muted: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
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
    fontSize: 12,
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
