import { Badge } from "@astryxdesign/core/Badge";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import type { TaskModelPoolEntryInput, TaskModelPoolsQuery } from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { TaskModelPoolsSettings } from "./TaskModelPoolsSettings";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import {
  agentBadgeLabel,
  agentDisplayName,
  agentMetadataRows,
  selectedModelWarning
} from "./agentMetadata";

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

  return (
    <VStack gap={3}>
      {saveError ? (
        <p {...stylex.props(styles.saveError)}>
          Noema could not save the model choice.
        </p>
      ) : null}
      {error ? (
        <VStack gap={3} {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>
            Agent metadata could not be loaded.
          </p>
        </VStack>
      ) : agents.length === 0 ? (
        <VStack gap={3} {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>No agents were found.</p>
        </VStack>
      ) : null}
      {error ? null : agents.map((agent) => {
        const displayName = agentDisplayName(agent);
        const badgeLabel = agentBadgeLabel(agent);
        const rows = agentMetadataRows(agent);
        const warning = selectedModelWarning(agent);
        const isTaskExecutor = agent.agentId === TASK_EXECUTOR_AGENT_ID;
        return (
          <VStack
            as="article"
            key={agent.agentId}
            gap={3}
            {...stylex.props(styles.card)}
          >
            <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
              <HStack wrap="wrap" gap={3} vAlign="center">
                <h2 {...stylex.props(styles.cardTitle)}>
                  {displayName}
                </h2>
                {badgeLabel ? <Badge variant="neutral" label={badgeLabel} /> : null}
              </HStack>
              {isTaskExecutor ? null : (
                <ModelPreferenceSelect
                  options={agent.modelOptions ?? []}
                  preference={agent.modelPreference ?? null}
                  useCase={agent.isPrimary ? "PRIMARY" : "TASK_REVIEWER"}
                  saving={saving}
                  ariaLabel={`Model settings for ${displayName}`}
                  onSave={(input) =>
                    onSaveModelPreference({
                      agentId: agent.agentId,
                      ...input
                    })
                  }
                />
              )}
            </HStack>
            <VStack as="dl" gap={2} {...stylex.props(styles.definitionList)}>
              {rows.map((row) => (
                <div
                  key={row.label}
                  {...stylex.props(styles.definitionRow)}
                >
                  <dt {...stylex.props(styles.definitionTerm)}>{row.label}</dt>
                  <dd {...stylex.props(styles.definitionValue)}>
                    {row.value}
                  </dd>
                </div>
              ))}
            </VStack>
            {isTaskExecutor ? (
              <>
                <TaskModelPoolsSettings
                  entries={taskModelPoolEntries}
                  error={taskModelPoolError}
                  loading={taskModelPoolLoading}
                  modelOptions={taskModelPoolModelOptions}
                  onUpdate={onUpdateTaskModelPool}
                  saveError={taskModelPoolSaveError}
                  saving={taskModelPoolSaving}
                />
              </>
            ) : warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
          </VStack>
        );
      })}
    </VStack>
  );
}

const styles = stylex.create({
  mutedText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  card: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: "var(--spacing-4)"
  },
  saveError: {
    margin: "var(--spacing-0)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--destructive) 30%, transparent)",
    borderRadius: 6,
    backgroundColor: "color-mix(in srgb, var(--destructive) 5%, transparent)",
    padding: "var(--spacing-3)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  cardTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  definitionList: {
    margin: "var(--spacing-0)"
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(120px, 180px) 1fr",
    gap: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr",
      gap: "var(--spacing-1)"
    }
  },
  definitionTerm: {
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  definitionValue: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    overflowWrap: "break-word",
    fontFamily: "var(--font-mono)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  warningText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "rgb(180, 83, 9)"
  }
});
