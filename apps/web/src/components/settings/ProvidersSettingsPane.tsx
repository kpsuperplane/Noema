import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { useState } from "react";
import {
  ClearProviderSecretDocument,
  CancelProviderAuthAttemptDocument,
  CreateProviderAccountDocument,
  DeleteProviderAccountDocument,
  ProviderAccountsDocument,
  ProviderAuthAttemptEventsDocument,
  SaveProviderSecretInputDocument,
  StartProviderAuthAttemptDocument,
  WebToolSettingsDocument,
  type ClearProviderSecretMutation,
  type ClearProviderSecretMutationVariables,
  type CreateProviderAccountMutation,
  type CreateProviderAccountMutationVariables,
  type DeleteProviderAccountMutation,
  type DeleteProviderAccountMutationVariables,
  type ProviderAccountsQuery,
  type SaveProviderSecretInputMutation,
  type SaveProviderSecretInputMutationVariables,
  type StartProviderAuthAttemptMutation
} from "@/generated/graphql";
import { ProvidersSettingsPaneContent } from "./ProvidersSettingsPaneContent";

export function ProvidersSettingsPane() {
  const [authAttempt, setAuthAttempt] = useState<
    StartProviderAuthAttemptMutation["startProviderAuthAttempt"] | null
  >(null);
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
  const [deleteProviderAccount, deleteResult] = useMutation<
    DeleteProviderAccountMutation,
    DeleteProviderAccountMutationVariables
  >(DeleteProviderAccountDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [startProviderAuthAttempt, startAuthResult] = useMutation(
    StartProviderAuthAttemptDocument
  );
  const [cancelProviderAuthAttempt] = useMutation(CancelProviderAuthAttemptDocument);
  useSubscription(ProviderAuthAttemptEventsDocument, {
    variables: { attemptId: authAttempt?.attemptId ?? "" },
    skip: !authAttempt,
    onData: ({ data }) => {
      const next = data.data?.providerAuthAttemptEvents;
      if (!next) return;
      setAuthAttempt(next);
      if (next.status === "COMPLETED") void result.refetch();
    }
  });

  return (
    <ProvidersSettingsPaneContent
      catalog={result.data?.providerAccountCatalog ?? []}
      accounts={result.data?.providerAccounts ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      mutationSaving={
        createResult.loading ||
        saveSecretResult.loading ||
        clearSecretResult.loading ||
        deleteResult.loading ||
        startAuthResult.loading
      }
      mutationError={
        createResult.error?.message ??
        saveSecretResult.error?.message ??
        clearSecretResult.error?.message ??
        startAuthResult.error?.message ??
        null
      }
      deleteError={deleteResult.error?.message ?? null}
      authAttempt={authAttempt}
      onRetry={() => void result.refetch()}
      onCreateProviderAccount={(input) => createProviderAccount({ variables: { input } })}
      onConnectProvider={async (providerKind, method) => {
        const response = await startProviderAuthAttempt({
          variables: { input: { providerKind, method } }
        });
        if (response.data?.startProviderAuthAttempt) {
          setAuthAttempt(response.data.startProviderAuthAttempt);
        }
      }}
      onCancelProviderAuth={async () => {
        if (!authAttempt) return;
        await cancelProviderAuthAttempt({
          variables: { input: { attemptId: authAttempt.attemptId } }
        });
        setAuthAttempt(null);
      }}
      onSaveProviderSecret={(input) => saveProviderSecretInput({ variables: { input } })}
      onClearProviderSecret={(input) => clearProviderSecret({ variables: { input } })}
      onDeleteProviderAccount={(input) => deleteProviderAccount({ variables: { input } })}
    />
  );
}
