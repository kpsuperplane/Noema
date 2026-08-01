import * as stylex from "@stylexjs/stylex";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export type ActionReviewerSettings = {
  modelPreference?: ModelPreference | null;
  modelOptions: readonly ModelProviderOption[];
};

export function PrivacySettingsPaneContent({
  reviewer,
  loading,
  error,
  saving,
  saveError,
  onSaveReviewerPreference
}: {
  reviewer: ActionReviewerSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveReviewerPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const preference = reviewer?.modelPreference ?? null;
  const warning = reviewer
    ? selectedPreferenceWarning(preference, reviewer.modelOptions, "ACTION_REVIEWER")
    : null;
  const unavailable = Boolean(error) || !reviewer;

  return (
    <SettingsSection aria-labelledby="privacy-reviewer-title">
      <VStack gap={2}>
        <h2 id="privacy-reviewer-title" {...stylex.props(styles.sectionTitle)}>
          Risky action reviews
        </h2>
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            label="Reviewer model"
            description={
              loading
                ? "Loading reviewer settings..."
                : error
                  ? "Reviewer settings could not be loaded."
                  : undefined
            }
            endContent={
              <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
                <ModelPreferenceSelect
                  options={reviewer?.modelOptions ?? []}
                  preference={preference}
                  useCase="ACTION_REVIEWER"
                  saving={saving}
                  requireExplicitSelection
                  ariaLabel="Reviewer model for write and export actions"
                  isDisabled={unavailable}
                  onSave={onSaveReviewerPreference}
                />
              </HStack>
            }
          />
        </SettingsList>
        {saveError ? (
          <p role="alert" {...stylex.props(styles.saveError)}>
            Noema could not save the reviewer model.
          </p>
        ) : null}
        {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
      </VStack>
    </SettingsSection>
  );
}

const styles = stylex.create({
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
