import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
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
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

type NativeMemorySettings = MemorySettingsQuery["memorySettings"];

export function MemorySettingsPane() {
  const settingsResult = useQuery<MemorySettingsQuery>(MemorySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveMemoryModelPreferenceMutation,
    SaveMemoryModelPreferenceMutationVariables
  >(SaveMemoryModelPreferenceDocument, {
    refetchQueries: [{ query: MemorySettingsDocument }],
    awaitRefetchQueries: true
  });
  const settings = settingsResult.data?.memorySettings ?? null;
  const loading = settingsResult.loading && !settingsResult.data;
  const error = settingsResult.error?.message ?? null;
  const saving = saveResult.loading;
  const saveError = saveResult.error?.message ?? null;
  const onSave = (input: ModelPreferenceSaveInput) => savePreference({
    variables: { input: { ...input, reasoningEffort: input.reasoningEffort ?? null } }
  });
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading memory settings...</p>;
  }

  if (error || !settings) {
    return (
      <SettingsSection aria-labelledby="memory-settings-title">
        <VStack gap={2}>
          <h2 id="memory-settings-title" {...stylex.props(styles.sectionTitle)}>
            Background updates
          </h2>
          <p {...stylex.props(styles.mutedText)}>
            The model choices for native memory updates could not be loaded.
          </p>
        </VStack>
      </SettingsSection>
    );
  }

  const preference = toPreference(settings.modelPreference);
  const options = settings.modelOptions.map(toProviderOption);
  const warning = selectedPreferenceWarning(preference, options, "MEMORY_CONSOLIDATION");

  return (
    <SettingsSection aria-labelledby="memory-settings-title">
      <VStack gap={2}>
        <HStack wrap="wrap" gap={2} vAlign="center" hAlign="between">
          <h2 id="memory-settings-title" {...stylex.props(styles.sectionTitle)}>
            Background updates
          </h2>
          <span {...stylex.props(styles.scope)}>Local human only</span>
        </HStack>
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            mobileEndContentFullWidth
            label="Consolidation model"
            endContent={
              <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
                <ModelPreferenceSelect
                  options={options}
                  preference={preference}
                  useCase="MEMORY_CONSOLIDATION"
                  saving={saving}
                  ariaLabel="Model settings for native memory updates"
                  onSave={onSave}
                />
              </HStack>
            }
          />
        </SettingsList>
      {saveError ? (
        <p role="alert" {...stylex.props(styles.saveError)}>
          Noema could not save the memory update model.
        </p>
      ) : null}
      {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
      </VStack>
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
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  scope: {
    color: "var(--muted-foreground)",
    fontSize: 12
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
