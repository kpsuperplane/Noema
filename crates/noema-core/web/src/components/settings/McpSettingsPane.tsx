import { useQuery } from "@apollo/client/react";
import { McpSettingsDocument, type McpSettingsQuery } from "@/generated/graphql";
import { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export { McpSettingsPaneContent } from "./McpSettingsPaneContent";

export function McpSettingsPane() {
  const result = useQuery<McpSettingsQuery>(McpSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });

  return (
    <McpSettingsPaneContent
      servers={result.data?.mcpServers ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      onRetry={() => void result.refetch()}
    />
  );
}
