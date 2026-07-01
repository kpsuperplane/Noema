import * as React from "react";
import { useApolloClient, useMutation, useQuery } from "@apollo/client/react";
import {
  CreateMcpServerDocument,
  DeleteMcpServerDocument,
  McpOauthSetupAttemptDocument,
  McpSettingsDocument,
  StartMcpServerOauthSetupDocument,
  type CreateMcpServerMutation,
  type DeleteMcpServerMutation,
  type McpOauthSetupAttemptQuery,
  type McpSettingsQuery,
  type StartMcpServerOauthSetupMutation
} from "@/generated/graphql";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import { mcpOAuthRedirectUri } from "@/graphql/mcpOAuthCallback";
import { McpSettingsPaneContent } from "./McpSettingsPaneContent";
import type { McpServerSetupResult } from "./McpServerSetupFlow";
import type { McpSetupFormSubmission } from "./mcpSetupForm";

export { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export function McpSettingsPane() {
  const client = useApolloClient();
  const result = useQuery<McpSettingsQuery>(McpSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [setupResult, setSetupResult] = React.useState<McpServerSetupResult | null>(null);
  const [setupError, setSetupError] = React.useState<string | null>(null);
  const [setupOpen, setSetupOpen] = React.useState(false);
  const [permissionsServerId, setPermissionsServerId] = React.useState<string | null>(null);
  const [deleteError, setDeleteError] = React.useState<string | null>(null);
  const [oauthAttemptId, setOauthAttemptId] = React.useState<string | null>(null);
  const [createMcpServer, createState] =
    useMutation<CreateMcpServerMutation>(CreateMcpServerDocument);
  const [startMcpServerOAuthSetup, oauthStartState] =
    useMutation<StartMcpServerOauthSetupMutation>(StartMcpServerOauthSetupDocument);
  const [deleteMcpServer, deleteState] =
    useMutation<DeleteMcpServerMutation>(DeleteMcpServerDocument);

  async function handleCreateServer(input: McpSetupFormSubmission) {
    setSetupError(null);
    try {
      const response = await createMcpServer({ variables: { input } });
      if (response.data?.createMcpServer) {
        const setup = response.data.createMcpServer;
        setSetupResult(setup);
        await result.refetch();
        if (setup.setupStatus === "ready_for_calibration" && setup.server) {
          setSetupResult(null);
          setSetupOpen(false);
          setPermissionsServerId(setup.server.mcpServerId);
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
      setOauthAttemptId(attempt.attemptId);
      if (attempt.authorizationUrl) {
        const handled = await openExternalUrlForAuth(attempt.authorizationUrl);
        if (!handled) {
          window.open(attempt.authorizationUrl, "_blank", "noopener,noreferrer");
        }
      }
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

  React.useEffect(() => {
    if (!oauthAttemptId) return;
    let cancelled = false;
    let timeout: number | null = null;
    const poll = () => {
      timeout = window.setTimeout(() => {
        client
          .query<McpOauthSetupAttemptQuery>({
            query: McpOauthSetupAttemptDocument,
            variables: { attemptId: oauthAttemptId },
            fetchPolicy: "network-only"
          })
          .then(async (response) => {
            if (cancelled) return;
            const attempt = response.data?.mcpOauthSetupAttempt;
            if (!attempt) {
              setOauthAttemptId(null);
              setSetupError("Noema could not find that MCP OAuth setup attempt.");
              return;
            }
            if (attempt.status === "completed" && attempt.setupResult) {
              setOauthAttemptId(null);
              setSetupResult(attempt.setupResult);
              await result.refetch();
              if (
                attempt.setupResult.setupStatus === "ready_for_calibration" &&
                attempt.setupResult.server
              ) {
                setSetupResult(null);
                setSetupOpen(false);
                setPermissionsServerId(attempt.setupResult.server.mcpServerId);
              }
              return;
            }
            if (attempt.status === "failed") {
              setOauthAttemptId(null);
              setSetupError(
                attempt.errorMessage ?? "Noema could not complete MCP OAuth setup."
              );
              return;
            }
            poll();
          })
          .catch((error) => {
            if (cancelled) return;
            setOauthAttemptId(null);
            setSetupError(error instanceof Error ? error.message : "MCP OAuth setup failed");
          });
      }, 1500);
    };
    poll();
    return () => {
      cancelled = true;
      if (timeout !== null) window.clearTimeout(timeout);
    };
  }, [client, oauthAttemptId, result]);

  return (
    <McpSettingsPaneContent
      servers={result.data?.mcpServers ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      setupResult={setupResult}
      setupOpen={setupOpen}
      setupSubmitting={createState.loading}
      oauthSubmitting={oauthStartState.loading || oauthAttemptId !== null}
      setupError={setupError}
      permissionsServerId={permissionsServerId}
      deleteSubmitting={deleteState.loading}
      deleteError={deleteError}
      onOpenSetup={() => {
        if (setupResult?.setupStatus !== "needs_auth") {
          setSetupResult(null);
        }
        setSetupError(null);
        setSetupOpen(true);
      }}
      onCloseSetup={() => setSetupOpen(false)}
      onCreateServer={(input) => void handleCreateServer(input)}
      onStartOAuth={(input) => void handleStartOAuth(input)}
      onOpenPermissions={setPermissionsServerId}
      onClosePermissions={() => setPermissionsServerId(null)}
      onDeleteServer={handleDeleteServer}
      onRetry={() => void result.refetch()}
    />
  );
}
