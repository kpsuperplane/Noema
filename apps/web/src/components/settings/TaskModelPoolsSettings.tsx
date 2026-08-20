import * as stylex from "@stylexjs/stylex";
import { HStack } from "@astryxdesign/core/HStack";
import { Switch } from "@astryxdesign/core/Switch";
import type {
  NoemaModelUseCase,
  TaskComplexity,
  TaskModelPoolEntryFieldsFragment,
  TaskModelPoolEntryInput,
} from "@/generated/graphql";
import { ControlledModelPreferenceSelect } from "./ControlledModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import type { ModelPreferenceSaveInput, ModelProviderOption } from "./modelPreferenceTypes";
import { SettingsList, SettingsListItem, SettingsLocalFeedback, SettingsSection, SettingsSectionInset } from "./SettingsPrimitives";

type PoolEntry = TaskModelPoolEntryFieldsFragment;
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
  const hasUsableEntry = entries.some((entry) => entry.enabled && !selectedPreferenceWarning({
    providerKind: entry.providerKind,
    providerAccountId: entry.providerAccountId,
    selectionMode: entry.selectionMode,
    modelProfile: entry.modelProfile,
    reasoningEffort: entry.reasoningEffort,
    fastMode: entry.fastMode
  }, modelOptions, complexityUseCase(entry.complexity)));

  return (
    <SettingsSection title="Task models" titleId="task-model-pools-title">
        {loading && entries.length === 0 ? (
          <SettingsSectionInset><p {...stylex.props(styles.muted)}>Loading task models...</p></SettingsSectionInset>
        ) : error ? (
          <SettingsSectionInset><p role="alert" {...stylex.props(styles.muted)}>Task models could not be loaded.</p></SettingsSectionInset>
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
        {saveError || !hasUsableEntry ? <SettingsLocalFeedback>
          {saveError ? <p role="alert" {...stylex.props(styles.error)}>{saveError}</p> : null}
          {!hasUsableEntry ? <p {...stylex.props(styles.warning)}>No usable task model is enabled.</p> : null}
        </SettingsLocalFeedback> : null}
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
    reasoningEffort: entry.reasoningEffort,
    fastMode: entry.fastMode
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
      fastMode: next.fastMode,
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
  rowLabel: { color: "var(--foreground)", fontWeight: 650 },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 },
  warning: { margin: "var(--spacing-0)", color: "var(--warning-foreground)", fontSize: 13, lineHeight: 1.45 }
});
