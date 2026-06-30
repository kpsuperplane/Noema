import { useQuery } from "@apollo/client/react";
import {
  McpApprovalSettingsDocument,
  type McpApprovalSettingsQuery,
  type McpApprovalSettingsQueryVariables
} from "@/generated/graphql";
import { ApprovalsSettingsPaneContent } from "./ApprovalsSettingsPaneContent";

export { ApprovalsSettingsPaneContent } from "./ApprovalsSettingsPaneContent";

export function ApprovalsSettingsPane() {
  const result = useQuery<McpApprovalSettingsQuery, McpApprovalSettingsQueryVariables>(
    McpApprovalSettingsDocument,
    {
      variables: { status: null },
      fetchPolicy: "cache-and-network"
    }
  );

  return (
    <ApprovalsSettingsPaneContent
      approvals={result.data?.mcpApprovalRequests ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      onRetry={() => void result.refetch()}
    />
  );
}
