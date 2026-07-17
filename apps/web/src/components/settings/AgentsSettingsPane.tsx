import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  SaveAgentModelPreferenceDocument,
  TaskExecutionPolicyDocument,
  TaskModelPoolsDocument,
  UpdateTaskExecutionPolicyDocument,
  UpdateTaskModelPoolEntryDocument,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables,
  type TaskModelPoolEntryInput,
  type TaskModelPoolsQuery,
  type UpdateTaskModelPoolEntryMutation,
  type UpdateTaskModelPoolEntryMutationVariables
} from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const poolsResult = useQuery<TaskModelPoolsQuery>(TaskModelPoolsDocument, { fetchPolicy: "cache-and-network" });
  const policyResult = useQuery(TaskExecutionPolicyDocument, { fetchPolicy: "cache-and-network" });
  const [savePreference, saveResult] = useMutation<
    SaveAgentModelPreferenceMutation,
    SaveAgentModelPreferenceMutationVariables
  >(SaveAgentModelPreferenceDocument, {
    refetchQueries: [{ query: AgentsDocument }],
    awaitRefetchQueries: true
  });
  const [updatePool, updatePoolResult] = useMutation<
    UpdateTaskModelPoolEntryMutation,
    UpdateTaskModelPoolEntryMutationVariables
  >(UpdateTaskModelPoolEntryDocument, {
    refetchQueries: [{ query: TaskModelPoolsDocument }],
    awaitRefetchQueries: true
  });
  const [updatePolicy, updatePolicyResult] = useMutation(UpdateTaskExecutionPolicyDocument, {
    refetchQueries: [{ query: TaskExecutionPolicyDocument }],
    awaitRefetchQueries: true
  });

  const primaryAgent = agentsResult.data?.agents.find((agent) => agent.isPrimary) ?? agentsResult.data?.agents[0];
  const poolSaving = updatePoolResult.loading;
  const poolSaveError = updatePoolResult.error?.message ?? null;

  return (
    <AgentsSettingsPaneContent
      agents={agentsResult.data?.agents ?? []}
      loading={agentsResult.loading && !agentsResult.data}
      error={agentsResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSaveModelPreference={(input) => savePreference({ variables: { input } })}
      taskModelPoolEntries={poolsResult.data?.taskModelPools ?? []}
      taskModelPoolError={poolsResult.error?.message ?? null}
      taskModelPoolLoading={poolsResult.loading && !poolsResult.data}
      taskModelPoolModelOptions={primaryAgent?.modelOptions ?? []}
      taskModelPoolSaveError={poolSaveError}
      taskModelPoolSaving={poolSaving}
      onUpdateTaskModelPool={(poolEntryId, input: TaskModelPoolEntryInput) => updatePool({ variables: { poolEntryId, input } })}
      taskExecutionPolicy={policyResult.data?.taskExecutionPolicy ?? null}
      taskExecutionPolicyError={policyResult.error?.message ?? null}
      taskExecutionPolicyLoading={policyResult.loading && !policyResult.data}
      taskExecutionPolicySaveError={updatePolicyResult.error?.message ?? null}
      taskExecutionPolicySaving={updatePolicyResult.loading}
      onUpdateTaskExecutionPolicy={async (input) => {
        await updatePolicy({ variables: { input } });
      }}
    />
  );
}
