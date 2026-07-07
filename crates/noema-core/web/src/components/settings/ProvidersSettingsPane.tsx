import { useMutation, useQuery } from "@apollo/client/react";
import {
  ClearProviderSecretDocument,
  CreateProviderAccountDocument,
  ProviderAccountsDocument,
  SaveProviderSecretInputDocument,
  WebToolSettingsDocument,
  type ClearProviderSecretMutation,
  type ClearProviderSecretMutationVariables,
  type CreateProviderAccountMutation,
  type CreateProviderAccountMutationVariables,
  type ProviderAccountsQuery,
  type SaveProviderSecretInputMutation,
  type SaveProviderSecretInputMutationVariables
} from "@/generated/graphql";
import { ProvidersSettingsPaneContent } from "./ProvidersSettingsPaneContent";

export { ProvidersSettingsPaneContent } from "./ProvidersSettingsPaneContent";

export function ProvidersSettingsPane() {
  const result = useQuery<ProviderAccountsQuery>(ProviderAccountsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const refetchQueries = [
    { query: ProviderAccountsDocument },
    { query: WebToolSettingsDocument }
  ];
  const [createProviderAccount, createResult] = useMutation<
    CreateProviderAccountMutation,
    CreateProviderAccountMutationVariables
  >(CreateProviderAccountDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [saveProviderSecretInput, saveSecretResult] = useMutation<
    SaveProviderSecretInputMutation,
    SaveProviderSecretInputMutationVariables
  >(SaveProviderSecretInputDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [clearProviderSecret, clearSecretResult] = useMutation<
    ClearProviderSecretMutation,
    ClearProviderSecretMutationVariables
  >(ClearProviderSecretDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });

  return (
    <ProvidersSettingsPaneContent
      catalog={result.data?.providerAccountCatalog ?? []}
      accounts={result.data?.providerAccounts ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      mutationSaving={createResult.loading || saveSecretResult.loading || clearSecretResult.loading}
      mutationError={
        createResult.error?.message ??
        saveSecretResult.error?.message ??
        clearSecretResult.error?.message ??
        null
      }
      onRetry={() => void result.refetch()}
      onCreateProviderAccount={(input) => createProviderAccount({ variables: { input } })}
      onSaveProviderSecret={(input) => saveProviderSecretInput({ variables: { input } })}
      onClearProviderSecret={(input) => clearProviderSecret({ variables: { input } })}
    />
  );
}
