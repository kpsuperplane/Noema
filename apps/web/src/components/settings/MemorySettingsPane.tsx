import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { RefreshCw } from "lucide-react";
import {
  MemorySettingsDocument,
  SaveMemoryModelPreferenceDocument,
  type MemorySettingsQuery,
  type SaveMemoryModelPreferenceMutation,
  type SaveMemoryModelPreferenceMutationVariables
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsRowActions,
  SettingsSection,
  SettingsSectionInset
} from "./SettingsPrimitives";

type NativeMemorySettings = MemorySettingsQuery["memorySettings"];

export function MemorySettingsPane() {
  const settingsResult = useQuery<MemorySettingsQuery>(MemorySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveMemoryModelPreferenceMutation,
    SaveMemoryModelPreferenceMutationVariables
  >(SaveMemoryModelPreferenceDocument, {
    update(cache, response) {
      const preference = response.data?.saveMemoryModelPreference;
      if (!preference) return;
      cache.updateQuery<MemorySettingsQuery>(
        { query: MemorySettingsDocument },
        (current) => current ? {
          ...current,
          memorySettings: {
            ...current.memorySettings,
            modelPreference: preference
          }
        } : current
      );
    }
  });
  const settings = settingsResult.data?.memorySettings ?? null;
  const loading = settingsResult.loading && !settingsResult.data;
  const error = settingsResult.error?.message ?? null;
  const saving = saveResult.loading;
  const saveError = saveResult.error?.message ?? null;
  const onSave = (input: ModelPreferenceSaveInput) => savePreference({
    variables: { input: { ...input, reasoningEffort: input.reasoningEffort ?? null } }
  });
  const preference = settings ? toPreference(settings.modelPreference) : null;
  const options = settings?.modelOptions.map(toProviderOption) ?? [];
  const warning = settings ? selectedPreferenceWarning(preference, options, "MEMORY_CONSOLIDATION") : null;

  return (
    <SettingsSection title="Background updates" titleId="memory-settings-title">
      {loading ? <SettingsSectionInset><p {...stylex.props(styles.mutedText)}>Loading memory settings...</p></SettingsSectionInset> : !settings ? (
        <SettingsSectionInset>
          <p role="alert" {...stylex.props(styles.mutedText)}>Memory settings could not be loaded.</p>
          <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void settingsResult.refetch()} />
        </SettingsSectionInset>
      ) : <>
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            mobileEndContentFullWidth
            label="Memory update model"
            endContent={
              <SettingsRowActions>
                <ModelPreferenceSelect
                  options={options}
                  preference={preference}
                  useCase="MEMORY_CONSOLIDATION"
                  saving={saving}
                  ariaLabel="Model settings for native memory updates"
                  onSave={onSave}
                />
              </SettingsRowActions>
            }
          />
      </SettingsList>
      {error || saveError || warning ? <SettingsLocalFeedback>
        {error ? <HStack gap={2} wrap="wrap" vAlign="center">
          <p role="alert" {...stylex.props(styles.mutedText)}>Memory settings could not refresh.</p>
          <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void settingsResult.refetch()} />
        </HStack> : null}
        {saveError ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the memory update model.</p> : null}
        {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
      </SettingsLocalFeedback> : null}
      </>}
    </SettingsSection>
  );
}

function toPreference(value: NonNullable<NativeMemorySettings>["modelPreference"]): ModelPreference | null {
  return value
    ? {
        providerKind: value.providerKind,
        providerAccountId: value.providerAccountId,
        selectionMode: value.selectionMode,
        modelProfile: value.modelProfile,
        reasoningEffort: value.reasoningEffort,
        fastMode: value.fastMode,
      }
    : null;
}

function toProviderOption(value: NonNullable<NativeMemorySettings>["modelOptions"][number]): ModelProviderOption {
  return {
    providerKind: value.providerKind,
    providerAccountId: value.providerAccountId,
    providerDisplayName: value.providerDisplayName,
    status: value.status,
    disabledReason: value.disabledReason,
    profiles: value.profiles.map((profile) => ({
      id: profile.id,
      label: profile.label,
      disabledReason: profile.disabledReason,
      reasoningEfforts: profile.reasoningEfforts,
      defaultReasoningEffort: profile.defaultReasoningEffort
    })),
    recommendations: value.recommendations
  };
}

const styles = stylex.create({
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
