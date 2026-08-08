import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  AcpAgentsDocument,
  AuthenticateAcpAgentDocument,
  CreateAcpAgentDocument,
  SaveAgentModelPreferenceDocument,
  TaskModelPoolsDocument,
  UpdateTaskModelPoolEntryDocument,
  TestAcpAgentDocument,
  UpdateAcpAgentDocument,
  type AcpAgentsQuery,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables,
  type TaskModelPoolEntryInput,
  type TaskModelPoolsQuery,
  type UpdateTaskModelPoolEntryMutation,
  type UpdateTaskModelPoolEntryMutationVariables
} from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const acpAgentsResult = useQuery<AcpAgentsQuery>(AcpAgentsDocument, { fetchPolicy: "cache-and-network" });
  const poolsResult = useQuery<TaskModelPoolsQuery>(TaskModelPoolsDocument, { fetchPolicy: "cache-and-network" });
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
  const mutationOptions = {
    refetchQueries: [{ query: AcpAgentsDocument }],
    awaitRefetchQueries: true
  };
  const [createAcpAgent, createAcpState] = useMutation(CreateAcpAgentDocument, mutationOptions);
  const [updateAcpAgent, updateAcpState] = useMutation(UpdateAcpAgentDocument, mutationOptions);
  const [testAcpAgent, testAcpState] = useMutation(TestAcpAgentDocument, mutationOptions);
  const [authenticateAcpAgent, authenticateAcpState] = useMutation(AuthenticateAcpAgentDocument, mutationOptions);
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
      acpAgents={acpAgentsResult.data?.acpAgents ?? []}
      acpLoading={acpAgentsResult.loading && !acpAgentsResult.data}
      acpError={acpAgentsResult.error?.message ?? createAcpState.error?.message ?? updateAcpState.error?.message ?? testAcpState.error?.message ?? authenticateAcpState.error?.message ?? null}
      acpBusy={createAcpState.loading || updateAcpState.loading || testAcpState.loading || authenticateAcpState.loading}
      onCreateAcpAgent={(input) => createAcpAgent({ variables: { input } })}
      onUpdateAcpAgent={(input) => updateAcpAgent({ variables: { input } })}
      onTestAcpAgent={(input) => testAcpAgent({ variables: { input } })}
      onAuthenticateAcpAgent={(input) => authenticateAcpAgent({ variables: { input } })}
    />
  );
}
