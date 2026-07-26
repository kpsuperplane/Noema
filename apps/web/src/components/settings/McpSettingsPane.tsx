import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import {
  ContinueMcpServerSetupDocument,
  CreateMcpServerDocument,
  DeleteMcpServerDocument,
  McpSettingsDocument,
  StartMcpServerReauthenticationOauthSetupDocument,
  StartMcpServerOauthSetupDocument,
  type CreateMcpServerMutation,
  type ContinueMcpServerSetupMutation,
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
  const result = useQuery<McpSettingsQuery>(McpSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const shouldPollToolClassification =
    result.data?.mcpServers.some((server) => server.pendingToolCount > 0) ?? false;
  const { startPolling, stopPolling } = result;
  const [setupResult, setSetupResult] = React.useState<McpServerSetupResult | null>(null);
  const [setupError, setSetupError] = React.useState<string | null>(null);
  const [setupOpen, setSetupOpen] = React.useState(false);
  const [permissionsServerId, setPermissionsServerId] = React.useState<string | null>(null);
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
      await result.refetch();
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
        setPermissionsServerId(attempt.setupResult.server.mcpServerId);
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
        await result.refetch();
        if (setup.setupStatus === "ready_for_policy" && setup.server) {
          setSetupResult(null);
          setSetupOpen(false);
          setPermissionsServerId(setup.server.mcpServerId);
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
      if (permissionsServerId === mcpServerId) {
        setPermissionsServerId(null);
      }
      await result.refetch();
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
      await result.refetch();
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
    } else {
      stopPolling();
    }
    return () => stopPolling();
  }, [shouldPollToolClassification, startPolling, stopPolling]);

  return (
    <McpSettingsPaneContent
      servers={result.data?.mcpServers ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      setupResult={setupResult}
      setupOpen={setupOpen}
      setupSubmitting={createState.loading}
      oauthSubmitting={oauthStartState.loading || oauth.active?.context.mode === "setup"}
      setupError={setupError}
      permissionsServerId={permissionsServerId}
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
      onOpenPermissions={(mcpServerId) => {
        setPermissionsServerId(mcpServerId);
      }}
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
      onClosePermissions={() => {
        setPermissionsServerId(null);
      }}
      onPolicySaved={() => void result.refetch()}
      onDeleteServer={handleDeleteServer}
      onRetry={() => void result.refetch()}
    />
  );
}
