import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import type { TaskModelPoolEntryInput, TaskModelPoolsQuery } from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { TaskModelPoolsSettings } from "./TaskModelPoolsSettings";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import { agentDisplayName, selectedModelWarning } from "./agentMetadata";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

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

export function AgentsSettingsPaneContent({
  agents,
  loading,
  error,
  saving,
  saveError,
  onSaveModelPreference,
  taskModelPoolEntries,
  taskModelPoolModelOptions,
  taskModelPoolLoading,
  taskModelPoolError,
  taskModelPoolSaving,
  taskModelPoolSaveError,
  onUpdateTaskModelPool
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
  taskModelPoolEntries: readonly TaskModelPoolsQuery["taskModelPools"][number][];
  taskModelPoolModelOptions: readonly ModelProviderOption[];
  taskModelPoolLoading: boolean;
  taskModelPoolError: string | null;
  taskModelPoolSaving: boolean;
  taskModelPoolSaveError: string | null;
  onUpdateTaskModelPool: (poolEntryId: string, input: TaskModelPoolEntryInput) => Promise<unknown>;
}) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading agents...</p>;
  }

  const taskExecutor = agents.find((agent) => agent.agentId === TASK_EXECUTOR_AGENT_ID);
  const visibleAgents = agents.filter((agent) => agent.agentId !== TASK_EXECUTOR_AGENT_ID);

  return (
    <VStack gap={6} {...stylex.props(styles.stack)}>
      {saveError ? (
        <p role="alert" {...stylex.props(styles.saveError)}>
          Noema could not save the model choice.
        </p>
      ) : null}
      <SettingsSection aria-labelledby="registered-agents-title">
        <VStack gap={2}>
          <h2 id="registered-agents-title" {...stylex.props(styles.sectionTitle)}>
            Registered agents
          </h2>
          {error ? (
            <p role="alert" {...stylex.props(styles.mutedText)}>
              Agent metadata could not be loaded.
            </p>
          ) : visibleAgents.length === 0 ? (
            <p {...stylex.props(styles.mutedText)}>No agents were found.</p>
          ) : (
            <SettingsList density="balanced" hasDividers>
              {visibleAgents.map((agent) => (
                <AgentRow key={agent.agentId} agent={agent} saving={saving} onSave={onSaveModelPreference} />
              ))}
            </SettingsList>
          )}
        </VStack>
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
      label={label}
      description={description}
      endContent={
        <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
          <ModelPreferenceSelect
            options={agent.modelOptions ?? []}
            preference={agent.modelPreference ?? null}
            useCase={agent.isPrimary ? "PRIMARY" : "TASK_REVIEWER"}
            saving={saving}
            ariaLabel={`Model settings for ${agentDisplayName(agent)}`}
            onSave={(input) => onSave({ agentId: agent.agentId, ...input })}
          />
        </HStack>
      }
    />
  );
}

const styles = stylex.create({
  stack: { minWidth: 0 },
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
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
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  }
});
