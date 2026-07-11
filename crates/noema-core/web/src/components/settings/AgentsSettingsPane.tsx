import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  CreateTaskModelPoolEntryDocument,
  DeleteTaskModelPoolEntryDocument,
  SaveAgentModelPreferenceDocument,
  TaskModelPoolsDocument,
  UpdateTaskModelPoolEntryDocument,
  type AgentsQuery,
  type CreateTaskModelPoolEntryMutation,
  type CreateTaskModelPoolEntryMutationVariables,
  type DeleteTaskModelPoolEntryMutation,
  type DeleteTaskModelPoolEntryMutationVariables,
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
  const [savePreference, saveResult] = useMutation<
    SaveAgentModelPreferenceMutation,
    SaveAgentModelPreferenceMutationVariables
  >(SaveAgentModelPreferenceDocument, {
    refetchQueries: [{ query: AgentsDocument }],
    awaitRefetchQueries: true
  });
  const [createPool, createPoolResult] = useMutation<
    CreateTaskModelPoolEntryMutation,
    CreateTaskModelPoolEntryMutationVariables
  >(CreateTaskModelPoolEntryDocument, {
    refetchQueries: [{ query: TaskModelPoolsDocument }],
    awaitRefetchQueries: true
  });
  const [updatePool, updatePoolResult] = useMutation<
    UpdateTaskModelPoolEntryMutation,
    UpdateTaskModelPoolEntryMutationVariables
  >(UpdateTaskModelPoolEntryDocument, {
    refetchQueries: [{ query: TaskModelPoolsDocument }],
    awaitRefetchQueries: true
  });
  const [deletePool, deletePoolResult] = useMutation<
    DeleteTaskModelPoolEntryMutation,
    DeleteTaskModelPoolEntryMutationVariables
  >(DeleteTaskModelPoolEntryDocument, {
    refetchQueries: [{ query: TaskModelPoolsDocument }],
    awaitRefetchQueries: true
  });

  const primaryAgent = agentsResult.data?.agents.find((agent) => agent.isPrimary) ?? agentsResult.data?.agents[0];
  const poolSaving = createPoolResult.loading || updatePoolResult.loading || deletePoolResult.loading;
  const poolSaveError = createPoolResult.error?.message ?? updatePoolResult.error?.message ?? deletePoolResult.error?.message ?? null;

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
      onCreateTaskModelPool={(input: TaskModelPoolEntryInput) => createPool({ variables: { input } })}
      onDeleteTaskModelPool={(poolEntryId) => deletePool({ variables: { poolEntryId } })}
      onUpdateTaskModelPool={(poolEntryId, input: TaskModelPoolEntryInput) => updatePool({ variables: { poolEntryId, input } })}
    />
  );
}
