import * as React from "react";
import { useNavigate } from "@tanstack/react-router";
import { useMutation, useQuery } from "@apollo/client/react";
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
import { useMcpOAuthController } from "@/components/mcp/useMcpOAuthController";
import { McpSettingsPaneContent } from "./McpSettingsPaneContent";
import type { McpServerSetupResult } from "./McpServerSetupFlow";
import type { McpSetupContinueSubmission, McpSetupFormSubmission } from "./mcpSetupForm";

export { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export function McpSettingsPane() {
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
        } else if (
          setup.setupStatus === "needs_auth" &&
          setup.auth?.oauthAuthorizationSupported
        ) {
          await handleStartOAuth(input);
        }
      }
    } catch (error) {
      setSetupError(error instanceof Error ? error.message : "MCP setup failed");
    }
  }

  async function handleStartOAuth(input: McpSetupFormSubmission) {
    setSetupError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startMcpServerOAuthSetup({
        variables: { input: { server: input, redirectUri } }
      });
      const attempt = response.data?.startMcpServerOauthSetup;
      if (!attempt) {
        throw new Error("Noema did not return an MCP OAuth attempt.");
      }
      await oauth.begin(attempt, { mode: "setup" });
    } catch (error) {
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
      if (!setup) {
        throw new Error("Noema did not return an MCP setup result.");
      }
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
    setReauthError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startMcpServerReauthenticationOAuthSetup({
        variables: { input: { mcpServerId, redirectUri } }
      });
      const attempt = response.data?.startMcpServerReauthenticationOauthSetup;
      if (!attempt) {
        throw new Error("Noema did not return an MCP OAuth attempt.");
      }
      await oauth.begin(attempt, { mode: "reauth" });
    } catch (error) {
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

  return (
    <McpSettingsPaneContent
      servers={result.data?.mcpServers ?? []}
      integrations={integrationsResult.data?.capabilityIntegrations ?? []}
      loading={(result.loading && !result.data) || (integrationsResult.loading && !integrationsResult.data)}
      error={result.error?.message ?? integrationsResult.error?.message ?? null}
      setupResult={setupResult}
      setupOpen={setupOpen}
      setupSubmitting={createState.loading}
      oauthSubmitting={oauthStartState.loading || oauth.active?.context.mode === "setup"}
      setupError={setupError}
      reauthServerId={reauthServerId}
      reauthSubmitting={continueState.loading}
      reauthOauthSubmitting={
        reauthOauthStartState.loading || oauth.active?.context.mode === "reauth"
      }
      reauthError={reauthError}
      reauthResult={reauthResult}
      deleteSubmitting={deleteState.loading}
      deleteError={deleteError}
      onOpenSetup={() => {
        if (
          setupResult?.setupStatus !== "needs_auth" &&
          setupResult?.setupStatus !== "authentication_available"
        ) {
          setSetupResult(null);
        }
        setSetupError(null);
        setSetupOpen(true);
      }}
      onCloseSetup={() => setSetupOpen(false)}
      onCreateServer={(input) => void handleCreateServer(input)}
      onStartOAuth={(input) => void handleStartOAuth(input)}
      onOpenReauth={(mcpServerId) => {
        setReauthResult(null);
        setReauthError(null);
        setReauthServerId(mcpServerId);
      }}
      onCloseReauth={() => {
        setReauthServerId(null);
        setReauthResult(null);
        setReauthError(null);
      }}
      onContinueServerSetup={(input) => void handleContinueServerSetup(input)}
      onStartReauthenticationOAuth={(mcpServerId) =>
        void handleStartReauthenticationOAuth(mcpServerId)
      }
      onConnectionAdded={() => void refetchManagement()}
      onDeleteServer={handleDeleteServer}
      onRetry={() => void refetchManagement()}
    />
  );
}
