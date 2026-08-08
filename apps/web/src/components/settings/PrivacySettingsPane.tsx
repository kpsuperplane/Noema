import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import {
  PrivacySettingsDocument,
  SaveActionReviewerPreferenceDocument,
  type PrivacySettingsQuery,
  type SaveActionReviewerPreferenceMutation,
  type SaveActionReviewerPreferenceMutationVariables
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import type { ModelPreferenceSaveInput } from "./modelPreferenceTypes";

export function PrivacySettingsPane() {
  const privacyResult = useQuery<PrivacySettingsQuery>(PrivacySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveActionReviewerPreferenceMutation,
    SaveActionReviewerPreferenceMutationVariables
  >(SaveActionReviewerPreferenceDocument, {
    refetchQueries: [{ query: PrivacySettingsDocument }],
    awaitRefetchQueries: true
  });
  const reviewer = privacyResult.data?.privacySettings.reviewer ?? null;
  const loading = privacyResult.loading && !privacyResult.data;
  const error = privacyResult.error?.message ?? null;
  const saving = saveResult.loading;
  const saveError = saveResult.error?.message ?? null;
  const onSaveReviewerPreference = (input: ModelPreferenceSaveInput) => savePreference({
    variables: { input: { ...input, reasoningEffort: input.reasoningEffort ?? null } }
  });
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
            mobileEndContentFullWidth
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
