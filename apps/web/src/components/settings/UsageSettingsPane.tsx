import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { VStack } from "@astryxdesign/core/VStack";
import {
  SaveToolProgressAuditPreferenceDocument,
  UpdateTaskExecutionPolicyDocument,
  UsageSettingsRootDocument,
  type SaveToolProgressAuditPreferenceMutation,
  type SaveToolProgressAuditPreferenceMutationVariables,
  type UsageSettingsRootQuery
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import { TaskExecutionPolicySettings, type TaskExecutionPolicyValue } from "./TaskExecutionPolicySettings";
import { SettingsList, SettingsListItem, SettingsLocalFeedback, SettingsRowActions, SettingsSection } from "./SettingsPrimitives";
import type { ModelPreferenceSaveInput } from "./modelPreferenceTypes";

export function UsageSettingsPane() {
  const rootResult = useQuery<UsageSettingsRootQuery>(UsageSettingsRootDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveToolProgressAuditPreferenceMutation,
    SaveToolProgressAuditPreferenceMutationVariables
  >(SaveToolProgressAuditPreferenceDocument, {
    update(cache, response) {
      const preference = response.data?.saveToolProgressAuditPreference;
      if (!preference) return;
      cache.updateQuery<UsageSettingsRootQuery>(
        { query: UsageSettingsRootDocument },
        (current) => current ? {
          ...current,
          usageSettings: {
            ...current.usageSettings,
            progressAudit: {
              ...current.usageSettings.progressAudit,
              modelPreference: preference
            }
          }
        } : current
      );
    }
  });
  const [updatePolicy, updatePolicyResult] = useMutation(UpdateTaskExecutionPolicyDocument, {
    update(cache, response) {
      const policy = response.data?.updateTaskExecutionPolicy;
      if (!policy) return;
      cache.updateQuery<UsageSettingsRootQuery>(
        { query: UsageSettingsRootDocument },
        (current) => current ? { ...current, taskExecutionPolicy: policy } : current
      );
    }
  });
  const progressAudit = rootResult.data?.usageSettings.progressAudit ?? null;
  const loading = rootResult.loading && !rootResult.data;
  const error = rootResult.error?.message ?? null;
  const saving = saveResult.loading;
  const saveError = saveResult.error?.message ?? null;
  const taskExecutionPolicy = rootResult.data?.taskExecutionPolicy ?? null;
  const taskExecutionPolicyLoading = rootResult.loading && !rootResult.data;
  const taskExecutionPolicyError = rootResult.error?.message ?? null;
  const taskExecutionPolicySaving = updatePolicyResult.loading;
  const taskExecutionPolicySaveError = updatePolicyResult.error?.message ?? null;
  const onUpdateTaskExecutionPolicy = async (input: TaskExecutionPolicyValue) => {
    await updatePolicy({ variables: { input } });
  };
  const onSaveToolProgressAuditPreference = (input: ModelPreferenceSaveInput) => savePreference({
    variables: { input: { ...input, reasoningEffort: input.reasoningEffort ?? null } }
  });
  const preference = progressAudit?.modelPreference ?? null;
  const warning = progressAudit
    ? selectedPreferenceWarning(preference, progressAudit.modelOptions, "TOOL_PROGRESS_AUDIT")
    : null;
  const unavailable = Boolean(error) || !progressAudit;

  return (
    <VStack gap={4} {...stylex.props(styles.stack)}>
      <TaskExecutionPolicySettings
        error={taskExecutionPolicyError}
        loading={taskExecutionPolicyLoading}
        onUpdate={onUpdateTaskExecutionPolicy}
        policy={taskExecutionPolicy}
        saveError={taskExecutionPolicySaveError}
        saving={taskExecutionPolicySaving}
      />
      <SettingsSection title="Progress auditing" titleId="usage-progress-audit-title">
          <SettingsList density="balanced" hasDividers>
            <SettingsListItem
              mobileEndContentFullWidth
              label="Audit model"
              description={
                loading
                  ? "Loading progress audit settings..."
                  : error
                    ? "Progress audit settings could not be loaded."
                    : undefined
              }
              endContent={
                <SettingsRowActions>
                  <ModelPreferenceSelect
                    options={progressAudit?.modelOptions ?? []}
                    preference={preference}
                    useCase="TOOL_PROGRESS_AUDIT"
                    saving={saving}
                    ariaLabel="Model settings for task progress auditing"
                    isDisabled={unavailable}
                    onSave={onSaveToolProgressAuditPreference}
                  />
                </SettingsRowActions>
              }
            />
          </SettingsList>
          {saveError || warning ? <SettingsLocalFeedback>{saveError ? (
            <p role="alert" {...stylex.props(styles.saveError)}>
              Noema could not save the progress audit model.
            </p>
          ) : null}
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}</SettingsLocalFeedback> : null}
      </SettingsSection>
    </VStack>
  );
}

const styles = stylex.create({
  stack: { minWidth: 0 },
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
});
