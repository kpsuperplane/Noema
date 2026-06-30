import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import {
  ContinueMcpServerSetupDocument,
  CreateMcpServerDocument,
  McpSettingsDocument,
  type ContinueMcpServerSetupMutation,
  type CreateMcpServerMutation,
  type McpSettingsQuery
} from "@/generated/graphql";
import { McpSettingsPaneContent } from "./McpSettingsPaneContent";
import type { McpServerSetupResult } from "./McpServerSetupFlow";
import type { McpSetupContinueSubmission, McpSetupFormSubmission } from "./mcpSetupForm";

export { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export function McpSettingsPane() {
  const result = useQuery<McpSettingsQuery>(McpSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [setupResult, setSetupResult] = React.useState<McpServerSetupResult | null>(null);
  const [setupError, setSetupError] = React.useState<string | null>(null);
  const [createMcpServer, createState] =
    useMutation<CreateMcpServerMutation>(CreateMcpServerDocument);
  const [continueMcpServerSetup, continueState] =
    useMutation<ContinueMcpServerSetupMutation>(ContinueMcpServerSetupDocument);

  async function handleCreateServer(input: McpSetupFormSubmission) {
    setSetupError(null);
    try {
      const response = await createMcpServer({ variables: { input } });
      if (response.data?.createMcpServer) {
        setSetupResult(response.data.createMcpServer);
        void result.refetch();
      }
    } catch (error) {
      setSetupError(error instanceof Error ? error.message : "MCP setup failed");
    }
  }

  async function handleContinueSetup(input: McpSetupContinueSubmission) {
    setSetupError(null);
    try {
      const response = await continueMcpServerSetup({ variables: { input } });
      if (response.data?.continueMcpServerSetup) {
        setSetupResult(response.data.continueMcpServerSetup);
        void result.refetch();
      }
    } catch (error) {
      setSetupError(error instanceof Error ? error.message : "MCP setup retry failed");
    }
  }

  return (
    <McpSettingsPaneContent
      servers={result.data?.mcpServers ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      setupResult={setupResult}
      setupSubmitting={createState.loading || continueState.loading}
      setupError={setupError}
      onCreateServer={(input) => void handleCreateServer(input)}
      onContinueSetup={(input) => void handleContinueSetup(input)}
      onRetry={() => void result.refetch()}
    />
  );
}
