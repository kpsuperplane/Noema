import * as stylex from "@stylexjs/stylex";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import { TaskExecutionPolicySettings, type TaskExecutionPolicyValue } from "./TaskExecutionPolicySettings";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export type ToolProgressAuditSettings = {
  modelPreference?: ModelPreference | null;
  modelOptions: readonly ModelProviderOption[];
};

export function UsageSettingsPaneContent({
  progressAudit,
  loading,
  error,
  saving,
  saveError,
  onSaveToolProgressAuditPreference,
  taskExecutionPolicy,
  taskExecutionPolicyLoading,
  taskExecutionPolicyError,
  taskExecutionPolicySaving,
  taskExecutionPolicySaveError,
  onUpdateTaskExecutionPolicy
}: {
  progressAudit: ToolProgressAuditSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveToolProgressAuditPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
  taskExecutionPolicy: TaskExecutionPolicyValue | null;
  taskExecutionPolicyLoading: boolean;
  taskExecutionPolicyError: string | null;
  taskExecutionPolicySaving: boolean;
  taskExecutionPolicySaveError: string | null;
  onUpdateTaskExecutionPolicy: (input: TaskExecutionPolicyValue) => Promise<unknown>;
}) {
  const preference = progressAudit?.modelPreference ?? null;
  const warning = progressAudit
    ? selectedPreferenceWarning(preference, progressAudit.modelOptions, "TOOL_PROGRESS_AUDIT")
    : null;
  const unavailable = Boolean(error) || !progressAudit;

  return (
    <VStack gap={6} {...stylex.props(styles.stack)}>
      <TaskExecutionPolicySettings
        error={taskExecutionPolicyError}
        loading={taskExecutionPolicyLoading}
        onUpdate={onUpdateTaskExecutionPolicy}
        policy={taskExecutionPolicy}
        saveError={taskExecutionPolicySaveError}
        saving={taskExecutionPolicySaving}
      />
      <SettingsSection aria-labelledby="usage-progress-audit-title">
        <VStack gap={2}>
          <h2 id="usage-progress-audit-title" {...stylex.props(styles.sectionTitle)}>
            Progress auditing
          </h2>
          <SettingsList density="balanced" hasDividers>
            <SettingsListItem
              label="Audit model"
              description={
                loading
                  ? "Loading progress audit settings..."
                  : error
                    ? "Progress audit settings could not be loaded."
                    : undefined
              }
              endContent={
                <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
                  <ModelPreferenceSelect
                    options={progressAudit?.modelOptions ?? []}
                    preference={preference}
                    useCase="TOOL_PROGRESS_AUDIT"
                    saving={saving}
                    ariaLabel="Model settings for task progress auditing"
                    isDisabled={unavailable}
                    onSave={onSaveToolProgressAuditPreference}
                  />
                </HStack>
              }
            />
          </SettingsList>
          {saveError ? (
            <p role="alert" {...stylex.props(styles.saveError)}>
              Noema could not save the progress audit model.
            </p>
          ) : null}
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
        </VStack>
      </SettingsSection>
    </VStack>
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
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  saveError: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
  warningText: {
    margin: "var(--spacing-0)",
    color: "var(--warning-foreground)",
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
