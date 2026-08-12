import * as React from "react";
import { useNavigate } from "@tanstack/react-router";
import { useMutation, useQuery } from "@apollo/client/react";
import { KeyRound, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import {
  ContinueMcpServerSetupDocument,
  CapabilityIntegrationsDocument,
  CreateMcpServerDocument,
  DeleteMcpServerDocument,
  McpSettingsDocument,
  StartMcpServerReauthenticationOauthSetupDocument,
  StartMcpServerOauthSetupDocument,
  type CreateMcpServerMutation,
  type ContinueMcpServerSetupMutation,
  type CapabilityIntegrationsQuery,
  type DeleteMcpServerMutation,
  type McpSettingsQuery,
  type StartMcpServerReauthenticationOauthSetupMutation,
  type StartMcpServerOauthSetupMutation
} from "@/generated/graphql";
import { mcpOAuthRedirectUri } from "@/graphql/mcpOAuthCallback";
import { reserveExternalAuthNavigation } from "@/graphql/externalUrls";
import { useMcpOAuthController } from "@/components/mcp/useMcpOAuthController";
import type { McpSettingsServer } from "./mcpMetadata";
import {
  CapabilityIntegrationList,
  type CapabilityIntegration
} from "./CapabilityIntegrationList";
import {
  McpServerSetupFlow,
  type McpServerSetupResult
} from "./McpServerSetupFlow";
import { McpServerReauthenticationDialog } from "./McpServerReauthenticationDialog";
import { McpAddConnectionDialog } from "./McpAddConnectionDialog";
import { CapabilityManagementLayout } from "./CapabilityManagementLayout";
import { DeleteConnectionDialog } from "./DeleteConnectionDialog";
import type { McpSetupContinueSubmission, McpSetupFormSubmission } from "./mcpSetupForm";
import { SettingsSection } from "./SettingsPrimitives";

export function McpSettingsPane({ connectionId }: { connectionId?: string }) {
  const navigate = useNavigate();
  const result = useQuery<McpSettingsQuery>(McpSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const integrationsResult = useQuery<CapabilityIntegrationsQuery>(CapabilityIntegrationsDocument, {
    variables: { kind: "MCP" },
    fetchPolicy: "cache-and-network"
  });
  const {
    startPolling: startIntegrationPolling,
    stopPolling: stopIntegrationPolling
  } = integrationsResult;
  async function refetchManagement() {
    await Promise.all([result.refetch(), integrationsResult.refetch()]);
  }
  const shouldPollToolClassification =
    result.data?.mcpServers.some((server) => server.pendingToolCount > 0) ?? false;
  const { startPolling, stopPolling } = result;
  const [setupResult, setSetupResult] = React.useState<McpServerSetupResult | null>(null);
  const [setupError, setSetupError] = React.useState<string | null>(null);
  const [setupOpen, setSetupOpen] = React.useState(false);
  const [reauthServerId, setReauthServerId] = React.useState<string | null>(null);
  const [reauthResult, setReauthResult] = React.useState<McpServerSetupResult | null>(null);
  const [reauthError, setReauthError] = React.useState<string | null>(null);
  const [deleteError, setDeleteError] = React.useState<string | null>(null);
  const [createMcpServer, createState] =
    useMutation<CreateMcpServerMutation>(CreateMcpServerDocument);
  const [continueMcpServerSetup, continueState] =
    useMutation<ContinueMcpServerSetupMutation>(ContinueMcpServerSetupDocument);
  const [startMcpServerOAuthSetup, oauthStartState] =
    useMutation<StartMcpServerOauthSetupMutation>(StartMcpServerOauthSetupDocument);
  const [startMcpServerReauthenticationOAuthSetup, reauthOauthStartState] =
    useMutation<StartMcpServerReauthenticationOauthSetupMutation>(
      StartMcpServerReauthenticationOauthSetupDocument
    );
  const [deleteMcpServer, deleteState] =
    useMutation<DeleteMcpServerMutation>(DeleteMcpServerDocument);
  const oauth = useMcpOAuthController<{ mode: "setup" | "reauth" }>({
    onCompleted: async (attempt, { mode }) => {
      if (!attempt.setupResult) {
        const message = "Noema completed OAuth but did not return MCP discovery.";
        if (mode === "reauth") setReauthError(message);
        else setSetupError(message);
        return;
      }
      await refetchManagement();
      if (mode === "reauth") {
        setReauthResult(attempt.setupResult);
        if (attempt.setupResult.setupStatus === "ready_for_policy" && attempt.setupResult.server) {
          setReauthServerId(null);
          setReauthResult(null);
        }
        return;
      }
      setSetupResult(attempt.setupResult);
      if (attempt.setupResult.setupStatus === "ready_for_policy" && attempt.setupResult.server) {
        setSetupResult(null);
        setSetupOpen(false);
        void navigate({
          to: "/settings/tools/mcps/$connectionId",
          params: { connectionId: attempt.setupResult.server.mcpServerId }
        });
      }
    },
    onFailed: (message, { mode }) => {
      if (mode === "reauth") setReauthError(message);
      else setSetupError(message);
    }
  });

  async function handleCreateServer(input: McpSetupFormSubmission) {
    setSetupError(null);
    try {
      const response = await createMcpServer({ variables: { input } });
      if (response.data?.createMcpServer) {
        const setup = response.data.createMcpServer;
        setSetupResult(setup);
        await refetchManagement();
        if (setup.setupStatus === "ready_for_policy" && setup.server) {
          setSetupResult(null);
          setSetupOpen(false);
          void navigate({
            to: "/settings/tools/mcps/$connectionId",
            params: { connectionId: setup.server.mcpServerId }
          });
        }
      }
    } catch (error) {
      setSetupError(error instanceof Error ? error.message : "MCP setup failed");
    }
  }

  async function handleStartOAuth(input: McpSetupFormSubmission) {
    const navigation = reserveExternalAuthNavigation();
    setSetupError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startMcpServerOAuthSetup({
        variables: { input: { server: input, redirectUri } }
      });
      const attempt = response.data?.startMcpServerOauthSetup;
      if (!attempt) throw new Error("Noema did not return an MCP OAuth attempt.");
      await oauth.begin(attempt, { mode: "setup" }, navigation);
    } catch (error) {
      navigation.cancel();
      setSetupError(error instanceof Error ? error.message : "MCP OAuth setup failed");
    }
  }

  async function handleDeleteServer(mcpServerId: string) {
    setDeleteError(null);
    try {
      const response = await deleteMcpServer({ variables: { mcpServerId } });
      if (!response.data?.deleteMcpServer) {
        setDeleteError("Noema could not find that MCP server.");
        return false;
      }
      await refetchManagement();
      void navigate({ to: "/settings/tools/mcps" });
      return true;
    } catch {
      setDeleteError("Noema could not delete this MCP server. Try again from Settings.");
      return false;
    }
  }

  async function handleContinueServerSetup(input: McpSetupContinueSubmission) {
    setReauthError(null);
    try {
      const response = await continueMcpServerSetup({ variables: { input } });
      const setup = response.data?.continueMcpServerSetup;
      if (!setup) throw new Error("Noema did not return an MCP setup result.");
      setReauthResult(setup);
      await refetchManagement();
      if (setup.setupStatus === "ready_for_policy" && setup.server) {
        setReauthServerId(null);
        setReauthResult(null);
      }
    } catch (error) {
      setReauthError(error instanceof Error ? error.message : "MCP reauthentication failed");
    }
  }

  async function handleStartReauthenticationOAuth(mcpServerId: string) {
    const navigation = reserveExternalAuthNavigation();
    setReauthError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startMcpServerReauthenticationOAuthSetup({
        variables: { input: { mcpServerId, redirectUri } }
      });
      const attempt = response.data?.startMcpServerReauthenticationOauthSetup;
      if (!attempt) throw new Error("Noema did not return an MCP OAuth attempt.");
      await oauth.begin(attempt, { mode: "reauth" }, navigation);
    } catch (error) {
      navigation.cancel();
      setReauthError(error instanceof Error ? error.message : "MCP OAuth setup failed");
    }
  }

  React.useEffect(() => {
    if (shouldPollToolClassification) {
      startPolling(1500);
      startIntegrationPolling(1500);
    } else {
      stopPolling();
      stopIntegrationPolling();
    }
    return () => {
      stopPolling();
      stopIntegrationPolling();
    };
  }, [
    shouldPollToolClassification,
    startIntegrationPolling,
    startPolling,
    stopIntegrationPolling,
    stopPolling
  ]);

  const servers: readonly McpSettingsServer[] = result.data?.mcpServers ?? [];
  const integrations: readonly CapabilityIntegration[] =
    integrationsResult.data?.capabilityIntegrations ?? [];
  const selectedConnectionId = connectionId;
  const loading =
    (result.loading && !result.data) || (integrationsResult.loading && !integrationsResult.data);
  const error = result.error?.message ?? integrationsResult.error?.message ?? null;
  const setupSubmitting = createState.loading;
  const oauthSubmitting = oauthStartState.loading || oauth.active?.context.mode === "setup";
  const reauthSubmitting = continueState.loading;
  const reauthOauthSubmitting =
    reauthOauthStartState.loading || oauth.active?.context.mode === "reauth";
  const deleteSubmitting = deleteState.loading;
  const onRetry = () => void refetchManagement();
  const onOpenSetup = () => {
    if (
      setupResult?.setupStatus !== "needs_auth" &&
      setupResult?.setupStatus !== "authentication_available"
    ) {
      setSetupResult(null);
    }
    setSetupError(null);
    setSetupOpen(true);
  };
  const onCloseSetup = () => setSetupOpen(false);
  const onCreateServer = (input: McpSetupFormSubmission) => void handleCreateServer(input);
  const onStartOAuth = (input: McpSetupFormSubmission) => void handleStartOAuth(input);
  const onOpenReauth = (mcpServerId: string) => {
    setReauthResult(null);
    setReauthError(null);
    setReauthServerId(mcpServerId);
  };
  const onCloseReauth = () => {
    setReauthServerId(null);
    setReauthResult(null);
    setReauthError(null);
  };
  const onContinueServerSetup = (input: McpSetupContinueSubmission) =>
    void handleContinueServerSetup(input);
  const onStartReauthenticationOAuth = (mcpServerId: string) =>
    void handleStartReauthenticationOAuth(mcpServerId);
  const onConnectionAdded = () => void refetchManagement();
  const onDeleteServer = handleDeleteServer;
  const [deleteTargetId, setDeleteTargetId] = React.useState<string | null>(null);
  const [addTargetId, setAddTargetId] = React.useState<string | null>(null);
  const reauthServer =
    servers.find((server) => server.mcpServerId === reauthServerId) ?? null;
  const selectedServer =
    servers.find((server) => server.mcpServerId === selectedConnectionId) ?? null;
  const selectedIntegration = integrations.find((integration) =>
    integration.connections.some((connection) => connection.connectionId === selectedConnectionId)
  ) ?? null;
  const deleteTarget = servers.find((server) => server.mcpServerId === deleteTargetId) ?? null;
  const addTarget = integrations.find((integration) => integration.definitionId === addTargetId) ?? null;

  if (loading) {
    return <p {...stylex.props(styles.mutedText, styles.pageState)}>Loading connections...</p>;
  }

  if (error) {
    return (
      <SettingsSection {...stylex.props(styles.pageState)}>
        <VStack gap={2}>
          <h2 {...stylex.props(styles.sectionTitle)}>MCP connections</h2>
          <p {...stylex.props(styles.mutedText)}>
            Couldn't load connections.
          </p>
          <Button
            type="button"
            variant="secondary"
            label="Retry"
            icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />}
            {...stylex.props(styles.fitButton)}
            onClick={onRetry}
          />
        </VStack>
      </SettingsSection>
    );
  }

  return (
    <>
      <CapabilityManagementLayout
        kind="MCP"
        connectionId={selectedConnectionId}
        defaultConnectionId={integrations[0]?.connections[0]?.connectionId}
        title="MCPs"
        primaryAction={{ label: "Connect service", onClick: onOpenSetup }}
        serviceName={selectedIntegration?.name}
        list={
          <VStack {...stylex.props(styles.list)}>
            <CapabilityIntegrationList
              integrations={integrations}
              kind="MCP"
              selectedConnectionId={selectedConnectionId}
              emptyMessage="No services connected."
              onAddConnection={(integration) => setAddTargetId(integration.definitionId)}
            />
          </VStack>
        }
        sourceActions={selectedServer && mcpServerNeedsReauth(selectedServer) ? (
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label="Reconnect"
            icon={<KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />}
            onClick={() => onOpenReauth(selectedServer.mcpServerId)}
          />
        ) : null}
        dangerAction={selectedServer ? (
          <Button
            type="button"
            variant="destructive"
            label="Delete connection"
            icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
            isDisabled={deleteSubmitting}
            onClick={() => setDeleteTargetId(selectedServer.mcpServerId)}
          />
        ) : null}
      />
      <McpAddConnectionDialog
        key={addTarget?.definitionId ?? "mcp-add-connection"}
        integration={addTarget}
        open={addTarget !== null}
        onAdded={onConnectionAdded}
        onOpenChange={(open) => {
          if (!open) setAddTargetId(null);
        }}
      />
      <McpSetupDialog
        open={setupOpen}
        setupResult={setupResult}
        setupSubmitting={setupSubmitting}
        oauthSubmitting={oauthSubmitting}
        setupError={setupError}
        onOpenChange={(open) => {
          if (!open) onCloseSetup();
        }}
        onCreateServer={onCreateServer}
        onStartOAuth={onStartOAuth}
      />
      <McpServerReauthenticationDialog
        key={reauthServer?.mcpServerId ?? "mcp-reauthentication"}
        server={reauthServer}
        open={reauthServer !== null}
        submitting={reauthSubmitting}
        oauthSubmitting={reauthOauthSubmitting}
        error={reauthError}
        setupResult={reauthResult}
        onOpenChange={(open) => {
          if (!open) onCloseReauth();
        }}
        onSubmit={onContinueServerSetup}
        onStartBrowserOAuth={onStartReauthenticationOAuth}
      />
      <DeleteConnectionDialog
        connection={deleteTarget ? {
          name: deleteTarget.displayName,
          toolCount: deleteTarget.toolCount
        } : null}
        open={deleteTarget !== null}
        submitting={deleteSubmitting}
        error={deleteError}
        onOpenChange={(open) => {
          if (!open && !deleteSubmitting) setDeleteTargetId(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget) return;
          const deleted = await onDeleteServer(deleteTarget.mcpServerId);
          if (deleted) setDeleteTargetId(null);
        }}
      />
    </>
  );
}

function mcpServerNeedsReauth(server: McpSettingsServer) {
  return server.healthStatus === "unavailable" || !["none", "authenticated"].includes(server.authStatus);
}

const styles = stylex.create({
  pageState: {
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
    }
  },
  mutedText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  list: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  fitButton: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  }
});

function McpSetupDialog({
  open,
  setupResult,
  setupSubmitting,
  oauthSubmitting,
  setupError,
  onOpenChange,
  onCreateServer,
  onStartOAuth
}: {
  open: boolean;
  setupResult: McpServerSetupResult | null;
  setupSubmitting: boolean;
  oauthSubmitting: boolean;
  setupError: string | null;
  onOpenChange: (open: boolean) => void;
  onCreateServer: (input: McpSetupFormSubmission) => void;
  onStartOAuth: (input: McpSetupFormSubmission) => void;
}) {
  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={680}
      maxHeight="calc(100dvh - var(--spacing-8))"
      aria-label="Connect a service"
    >
      <Layout
        header={
          <DialogHeader
            title="Connect a service"
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <McpServerSetupFlow
              setupResult={setupResult}
              setupSubmitting={setupSubmitting}
              oauthSubmitting={oauthSubmitting}
              setupError={setupError}
              onCreateServer={onCreateServer}
              onStartOAuth={onStartOAuth}
            />
          </LayoutContent>
        }
      />
    </Dialog>
  );
}
