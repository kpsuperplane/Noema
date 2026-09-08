import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { Badge } from "@astryxdesign/core/Badge";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import {
  AgentsSettingsRootDocument,
  SaveAgentModelPreferenceDocument,
  UpdateTaskModelPoolEntryDocument,
  type AgentsSettingsRootQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables,
  type TaskModelPoolEntryInput,
  type UpdateTaskModelPoolEntryMutation,
  type UpdateTaskModelPoolEntryMutationVariables
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { TaskModelPoolsSettings } from "./TaskModelPoolsSettings";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import { agentDisplayName, selectedModelWarning } from "./agentMetadata";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsSection,
  SettingsSectionInset
} from "./SettingsPrimitives";

export type AgentSettingsAgent = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
  modelPreference?: ModelPreference | null;
  modelOptions?: readonly ModelProviderOption[];
};

type SaveAgentModelPreferenceInput = ModelPreferenceSaveInput & {
  agentId: string;
};

const TASK_EXECUTOR_AGENT_ID = "agent:task-executor";

export function AgentsSettingsPane() {
  const rootResult = useQuery<AgentsSettingsRootQuery>(AgentsSettingsRootDocument, { fetchPolicy: "cache-and-network" });
  const [savePreference, saveResult] = useMutation<SaveAgentModelPreferenceMutation, SaveAgentModelPreferenceMutationVariables>(SaveAgentModelPreferenceDocument, {
    update(cache, response, options) {
      const preference = response.data?.saveAgentModelPreference;
      const agentId = options.variables?.input.agentId;
      if (!preference || !agentId) return;
      cache.updateQuery<AgentsSettingsRootQuery>(
        { query: AgentsSettingsRootDocument },
        (current) => current ? {
          ...current,
          agents: current.agents.map((agent) => agent.agentId === agentId
            ? { ...agent, modelPreference: preference }
            : agent)
        } : current
      );
    }
  });
  const [updatePool, updatePoolResult] = useMutation<UpdateTaskModelPoolEntryMutation, UpdateTaskModelPoolEntryMutationVariables>(UpdateTaskModelPoolEntryDocument);
  const agents = rootResult.data?.agents ?? [];
  const loading = rootResult.loading && !rootResult.data;
  const error = rootResult.error?.message ?? null;
  const saving = saveResult.loading;
  const saveError = saveResult.error?.message ?? null;
  const onSaveModelPreference = (input: SaveAgentModelPreferenceInput) => savePreference({ variables: { input } });
  const taskModelPoolEntries = rootResult.data?.taskModelPools ?? [];
  const primaryAgent = agents.find((agent) => agent.isPrimary) ?? agents[0];
  const taskModelPoolModelOptions = primaryAgent?.modelOptions ?? [];
  const taskModelPoolLoading = rootResult.loading && !rootResult.data;
  const taskModelPoolError = rootResult.error?.message ?? null;
  const taskModelPoolSaving = updatePoolResult.loading;
  const taskModelPoolSaveError = updatePoolResult.error?.message ?? null;
  const onUpdateTaskModelPool = (poolEntryId: string, input: TaskModelPoolEntryInput) => updatePool({ variables: { poolEntryId, input } });
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading agents...</p>;
  }

  const taskExecutor = agents.find((agent) => agent.agentId === TASK_EXECUTOR_AGENT_ID);
  const visibleAgents = agents.filter((agent) => agent.agentId !== TASK_EXECUTOR_AGENT_ID);

  return (
    <VStack gap={4} {...stylex.props(styles.stack)}>
      <SettingsSection title="Agent models" titleId="registered-agents-title">
          {error ? (
            <SettingsSectionInset><p role="alert" {...stylex.props(styles.mutedText)}>Agent metadata could not be loaded.</p></SettingsSectionInset>
          ) : visibleAgents.length === 0 ? (
            <SettingsSectionInset><p {...stylex.props(styles.mutedText)}>No agents were found.</p></SettingsSectionInset>
          ) : (
            <SettingsList density="balanced" hasDividers>
              {visibleAgents.map((agent) => (
                <AgentRow key={agent.agentId} agent={agent} saving={saving} onSave={onSaveModelPreference} />
              ))}
            </SettingsList>
          )}
          {saveError ? <SettingsLocalFeedback>
            <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the model choice.</p>
          </SettingsLocalFeedback> : null}
      </SettingsSection>
      {error || !taskExecutor ? null : (
        <TaskModelPoolsSettings
          entries={taskModelPoolEntries}
          error={taskModelPoolError}
          loading={taskModelPoolLoading}
          modelOptions={taskModelPoolModelOptions}
          onUpdate={onUpdateTaskModelPool}
          saveError={taskModelPoolSaveError}
          saving={taskModelPoolSaving}
        />
      )}
    </VStack>
  );
}

function AgentRow({
  agent,
  saving,
  onSave
}: {
  agent: AgentSettingsAgent;
  saving: boolean;
  onSave: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  const warning = selectedModelWarning(agent);
  const label = (
    <HStack gap={2} vAlign="center" wrap="wrap">
      <span {...stylex.props(styles.rowLabel)}>{agentDisplayName(agent)}</span>
      {agent.isPrimary ? <Badge variant="neutral" label="Primary" /> : null}
    </HStack>
  );
  const description = warning ? <span {...stylex.props(styles.warningText)}>{warning}</span> : undefined;

  return (
    <SettingsListItem
      label={<VStack gap={2} {...stylex.props(styles.stack)}>
        {label}
        {description}
        <ModelPreferenceSelect
          options={agent.modelOptions ?? []}
          preference={agent.modelPreference ?? null}
          useCase={agent.isPrimary ? "PRIMARY" : "TASK_REVIEWER"}
          saving={saving}
          ariaLabel={`Model settings for ${agentDisplayName(agent)}`}
          onSave={(input) => onSave({ agentId: agent.agentId, ...input })}
        />
      </VStack>}
    />
  );
}

const styles = stylex.create({
  stack: { minWidth: 0 },
  rowLabel: {
    color: "var(--foreground)",
    fontWeight: 650,
    overflowWrap: "anywhere"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  warningText: {
    color: "var(--warning-foreground)",
    fontSize: 12,
    lineHeight: 1.4
  },
  saveError: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
});
