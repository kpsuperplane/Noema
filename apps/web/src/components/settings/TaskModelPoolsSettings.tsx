import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { HStack } from "@astryxdesign/core/HStack";
import { Switch } from "@astryxdesign/core/Switch";
import { VStack } from "@astryxdesign/core/VStack";
import type {
  NoemaModelUseCase,
  TaskComplexity,
  TaskModelPoolEntryInput,
  TaskModelPoolsQuery
} from "@/generated/graphql";
import { ControlledModelPreferenceSelect } from "./ControlledModelPreferenceSelect";
import type { ModelPreferenceSaveInput, ModelProviderOption } from "./modelPreferenceTypes";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

type PoolEntry = TaskModelPoolsQuery["taskModelPools"][number];
const complexities: readonly TaskComplexity[] = ["SIMPLE", "MEDIUM", "DIFFICULT"];

export function TaskModelPoolsSettings({
  entries,
  modelOptions,
  loading,
  error,
  saving,
  saveError,
  onUpdate
}: {
  entries: readonly PoolEntry[];
  modelOptions: readonly ModelProviderOption[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onUpdate: (poolEntryId: string, input: TaskModelPoolEntryInput) => Promise<unknown>;
}) {
  const enabledEntryCount = entries.filter((entry) => entry.enabled).length;

  return (
    <SettingsSection aria-labelledby="task-model-pools-title">
      <VStack gap={2}>
        <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
          <h2 id="task-model-pools-title" {...stylex.props(styles.sectionTitle)}>
            Task models
          </h2>
          <Badge
            variant={enabledEntryCount > 0 ? "success" : "warning"}
            label={`${enabledEntryCount}/3 enabled`}
          />
        </HStack>
        {saveError ? <p role="alert" {...stylex.props(styles.error)}>{saveError}</p> : null}
        {loading && entries.length === 0 ? (
          <p {...stylex.props(styles.muted)}>Loading task model pools...</p>
        ) : error ? (
          <p role="alert" {...stylex.props(styles.muted)}>Task model pools could not be loaded.</p>
        ) : (
          <SettingsList density="balanced" hasDividers>
            {complexities.map((complexity) => {
              const entry = entries.find((candidate) => candidate.complexity === complexity);
              return entry ? (
                <PoolEntryRow
                  key={complexity}
                  entry={entry}
                  modelOptions={modelOptions}
                  saving={saving}
                  onUpdate={onUpdate}
                />
              ) : (
                <SettingsListItem
                  key={complexity}
                  label={complexityLabel(complexity)}
                  description="This task model setting is unavailable."
                />
              );
            })}
          </SettingsList>
        )}
      </VStack>
    </SettingsSection>
  );
}

function PoolEntryRow({
  entry,
  modelOptions,
  saving,
  onUpdate
}: {
  entry: PoolEntry;
  modelOptions: readonly ModelProviderOption[];
  saving: boolean;
  onUpdate: (poolEntryId: string, input: TaskModelPoolEntryInput) => Promise<unknown>;
}) {
  const selection: ModelPreferenceSaveInput = {
    providerAccountId: entry.providerAccountId,
    selectionMode: entry.selectionMode,
    modelProfile: entry.modelProfile,
    reasoningEffort: entry.reasoningEffort
  };

  const save = (next: ModelPreferenceSaveInput, enabled = entry.enabled) => {
    const provider = modelOptions.find((option) => option.providerAccountId === next.providerAccountId);
    void onUpdate(entry.poolEntryId, {
      complexity: entry.complexity,
      // Preserve legacy aliases without exposing the generic field in Settings.
      label: entry.label,
      providerKind: provider?.providerKind ?? entry.providerKind,
      providerAccountId: next.providerAccountId,
      selectionMode: next.selectionMode,
      modelProfile: next.modelProfile ?? null,
      reasoningEffort: next.reasoningEffort ?? null,
      enabled,
      sortOrder: entry.sortOrder
    });
  };

  return (
    <SettingsListItem
      mobileEndContentFullWidth
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <span {...stylex.props(styles.rowLabel)}>{complexityLabel(entry.complexity)}</span>
          <Switch
            label="Enabled"
            aria-label={`Enabled for ${complexityLabel(entry.complexity)} task model`}
            value={entry.enabled}
            isDisabled={saving}
            isLoading={saving}
            onChange={(checked) => save(selection, checked)}
          />
        </HStack>
      }
      endContent={
        <ControlledModelPreferenceSelect
          ariaLabel={`${complexityLabel(entry.complexity)} task model`}
          options={modelOptions}
          selection={selection}
          useCase={complexityUseCase(entry.complexity)}
          disabled={saving}
          onChange={(next) => save(next)}
        />
      }
    />
  );
}

function complexityUseCase(value: TaskComplexity): NoemaModelUseCase {
  return value === "SIMPLE"
    ? "TASK_SIMPLE"
    : value === "DIFFICULT"
      ? "TASK_DIFFICULT"
      : "TASK_MEDIUM";
}

function complexityLabel(value: TaskComplexity): string {
  if (value === "DIFFICULT") return "High";
  return value.charAt(0) + value.slice(1).toLowerCase();
}

const styles = stylex.create({
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  rowLabel: { color: "var(--foreground)", fontWeight: 650 },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});
