import { useMutation, useQuery } from "@apollo/client/react";
import {
  SaveToolProgressAuditPreferenceDocument,
  TaskExecutionPolicyDocument,
  UpdateTaskExecutionPolicyDocument,
  UsageSettingsDocument,
  type SaveToolProgressAuditPreferenceMutation,
  type SaveToolProgressAuditPreferenceMutationVariables,
  type TaskExecutionPolicyQuery,
  type UsageSettingsQuery
} from "@/generated/graphql";
import { UsageSettingsPaneContent } from "./UsageSettingsPaneContent";

export { UsageSettingsPaneContent } from "./UsageSettingsPaneContent";

export function UsageSettingsPane() {
  const usageResult = useQuery<UsageSettingsQuery>(UsageSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const policyResult = useQuery<TaskExecutionPolicyQuery>(TaskExecutionPolicyDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveToolProgressAuditPreference, saveResult] = useMutation<
    SaveToolProgressAuditPreferenceMutation,
    SaveToolProgressAuditPreferenceMutationVariables
  >(SaveToolProgressAuditPreferenceDocument, {
    refetchQueries: [{ query: UsageSettingsDocument }],
    awaitRefetchQueries: true
  });
  const [updatePolicy, updatePolicyResult] = useMutation(UpdateTaskExecutionPolicyDocument, {
    refetchQueries: [{ query: TaskExecutionPolicyDocument }],
    awaitRefetchQueries: true
  });

  return (
    <UsageSettingsPaneContent
      progressAudit={usageResult.data?.usageSettings.progressAudit ?? null}
      loading={usageResult.loading && !usageResult.data}
      error={usageResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      taskExecutionPolicy={policyResult.data?.taskExecutionPolicy ?? null}
      taskExecutionPolicyError={policyResult.error?.message ?? null}
      taskExecutionPolicyLoading={policyResult.loading && !policyResult.data}
      taskExecutionPolicySaveError={updatePolicyResult.error?.message ?? null}
      taskExecutionPolicySaving={updatePolicyResult.loading}
      onUpdateTaskExecutionPolicy={async (input) => {
        await updatePolicy({ variables: { input } });
      }}
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
