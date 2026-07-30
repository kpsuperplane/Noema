import { useMutation, useQuery } from "@apollo/client/react";
import {
  SaveToolProgressAuditPreferenceDocument,
  UsageSettingsDocument,
  type SaveToolProgressAuditPreferenceMutation,
  type SaveToolProgressAuditPreferenceMutationVariables,
  type UsageSettingsQuery
} from "@/generated/graphql";
import { UsageSettingsPaneContent } from "./UsageSettingsPaneContent";

export { UsageSettingsPaneContent } from "./UsageSettingsPaneContent";

export function UsageSettingsPane() {
  const usageResult = useQuery<UsageSettingsQuery>(UsageSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveToolProgressAuditPreference, saveResult] = useMutation<
    SaveToolProgressAuditPreferenceMutation,
    SaveToolProgressAuditPreferenceMutationVariables
  >(SaveToolProgressAuditPreferenceDocument, {
    refetchQueries: [{ query: UsageSettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <UsageSettingsPaneContent
      progressAudit={usageResult.data?.usageSettings.progressAudit ?? null}
      loading={usageResult.loading && !usageResult.data}
      error={usageResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSaveToolProgressAuditPreference={(input) =>
        saveToolProgressAuditPreference({
          variables: {
            input: {
              providerAccountId: input.providerAccountId,
              selectionMode: input.selectionMode,
              modelProfile: input.modelProfile,
              reasoningEffort: input.reasoningEffort ?? null
            }
          }
        })
      }
    />
  );
}
