import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import {
  ActivateLocalModelDocument,
  CancelLocalModelInstallDocument,
  ImportLocalModelDocument,
  InstallLocalModelDocument,
  LocalModelEventsDocument,
  LocalModelsSettingsDocument,
  RemoveLocalModelDocument,
  RetryLocalModelRuntimeDocument,
  type ImportLocalModelInput,
  type LocalModelsSettingsQuery
} from "@/generated/graphql";
import { LocalModelsSettingsPaneContent } from "./LocalModelsSettingsPaneContent";

export function LocalModelsSettingsPane() {
  const result = useQuery<LocalModelsSettingsQuery>(LocalModelsSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const refetchQueries = [{ query: LocalModelsSettingsDocument }];
  const [install, installResult] = useMutation(InstallLocalModelDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [importModel, importResult] = useMutation(ImportLocalModelDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [cancel, cancelResult] = useMutation(CancelLocalModelInstallDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [remove, removeResult] = useMutation(RemoveLocalModelDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [activate, activateResult] = useMutation(ActivateLocalModelDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });
  const [retryRuntime, retryResult] = useMutation(RetryLocalModelRuntimeDocument, {
    refetchQueries,
    awaitRefetchQueries: true
  });

  useSubscription(LocalModelEventsDocument, {
    onData: () => void result.refetch()
  });

  const mutationResults = [
    installResult,
    importResult,
    cancelResult,
    removeResult,
    activateResult,
    retryResult
  ];

  return (
    <LocalModelsSettingsPaneContent
      setup={result.data?.localModelSetup ?? null}
      catalog={result.data?.localModelCatalog ?? []}
      installations={result.data?.localModelInstallations ?? []}
      defaultPreference={result.data?.defaultModelPreference ?? null}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
      saving={mutationResults.some((mutation) => mutation.loading)}
      saveError={mutationResults.find((mutation) => mutation.error)?.error?.message ?? null}
      onRetry={() => void result.refetch()}
      onInstall={(modelId, file) => install({ variables: { input: { modelId, file } } })}
      onImport={(input: ImportLocalModelInput) => importModel({ variables: { input } })}
      onCancel={(installationId) => cancel({ variables: { installationId } })}
      onRemove={(installationId) => remove({ variables: { installationId } })}
      onActivate={(installationId) => activate({ variables: { installationId } })}
      onRetryRuntime={() => retryRuntime()}
    />
  );
}
