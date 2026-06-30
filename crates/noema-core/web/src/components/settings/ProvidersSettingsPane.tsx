import { useQuery } from "@apollo/client/react";
import { ProviderAccountsDocument } from "@/generated/graphql";
import { ProvidersSettingsPaneContent } from "./ProvidersSettingsPaneContent";

export { ProvidersSettingsPaneContent } from "./ProvidersSettingsPaneContent";

export function ProvidersSettingsPane() {
  const result = useQuery(ProviderAccountsDocument, { fetchPolicy: "cache-and-network" });
  return (
    <ProvidersSettingsPaneContent
      accounts={result.data?.providerAccounts ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      onRetry={() => void result.refetch()}
    />
  );
}
