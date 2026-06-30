import { useQuery } from "@apollo/client/react";
import {
  TrustedIdentitySettingsDocument,
  type TrustedIdentitySettingsQuery,
  type TrustedIdentitySettingsQueryVariables
} from "@/generated/graphql";
import { TrustedIdentitiesSettingsPaneContent } from "./TrustedIdentitiesSettingsPaneContent";

export { TrustedIdentitiesSettingsPaneContent } from "./TrustedIdentitiesSettingsPaneContent";

export function TrustedIdentitiesSettingsPane() {
  const result = useQuery<
    TrustedIdentitySettingsQuery,
    TrustedIdentitySettingsQueryVariables
  >(TrustedIdentitySettingsDocument, {
    variables: { ownerScopeId: "human:local" },
    fetchPolicy: "cache-and-network"
  });

  return (
    <TrustedIdentitiesSettingsPaneContent
      selectors={result.data?.trustedIdentitySelectors ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      onRetry={() => void result.refetch()}
    />
  );
}
