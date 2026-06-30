import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import {
  CreateMcpServerDocument,
  DeleteMcpServerDocument,
  McpSettingsDocument,
  type CreateMcpServerMutation,
  type DeleteMcpServerMutation,
  type McpSettingsQuery
} from "@/generated/graphql";
import { McpSettingsPaneContent } from "./McpSettingsPaneContent";
import type { McpServerSetupResult } from "./McpServerSetupFlow";
import type { McpSetupFormSubmission } from "./mcpSetupForm";

export { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export function McpSettingsPane() {
  const result = useQuery<McpSettingsQuery>(McpSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [setupResult, setSetupResult] = React.useState<McpServerSetupResult | null>(null);
  const [setupError, setSetupError] = React.useState<string | null>(null);
  const [setupOpen, setSetupOpen] = React.useState(false);
  const [permissionsServerId, setPermissionsServerId] = React.useState<string | null>(null);
  const [deleteError, setDeleteError] = React.useState<string | null>(null);
  const [createMcpServer, createState] =
    useMutation<CreateMcpServerMutation>(CreateMcpServerDocument);
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

  return (
    <McpSettingsPaneContent
      servers={result.data?.mcpServers ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      setupResult={setupResult}
      setupOpen={setupOpen}
      setupSubmitting={createState.loading}
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
      onOpenPermissions={setPermissionsServerId}
      onClosePermissions={() => setPermissionsServerId(null)}
      onDeleteServer={handleDeleteServer}
      onRetry={() => void result.refetch()}
    />
  );
}
