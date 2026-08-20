import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { RefreshCw } from "lucide-react";
import {
  PrivacySettingsDocument,
  SaveActionReviewerPreferenceDocument,
  type PrivacySettingsQuery,
  type SaveActionReviewerPreferenceMutation,
  type SaveActionReviewerPreferenceMutationVariables
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsRowActions,
  SettingsSection,
  SettingsSectionInset
} from "./SettingsPrimitives";
import type { ModelPreferenceSaveInput } from "./modelPreferenceTypes";

export function PrivacySettingsPane() {
  const privacyResult = useQuery<PrivacySettingsQuery>(PrivacySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveActionReviewerPreferenceMutation,
    SaveActionReviewerPreferenceMutationVariables
  >(SaveActionReviewerPreferenceDocument, {
    update(cache, response) {
      const preference = response.data?.saveActionReviewerPreference;
      if (!preference) return;
      cache.updateQuery<PrivacySettingsQuery>(
        { query: PrivacySettingsDocument },
        (current) => current ? {
          ...current,
          privacySettings: {
            ...current.privacySettings,
            reviewer: {
              ...current.privacySettings.reviewer,
              modelPreference: preference
            }
          }
        } : current
      );
    }
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
  const unavailable = !reviewer;

  return (
    <SettingsSection title="Risky action reviews" titleId="privacy-reviewer-title">
      {!reviewer && !loading ? <SettingsSectionInset>
        <p role="alert" {...stylex.props(styles.mutedText)}>Reviewer settings could not be loaded.</p>
        <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void privacyResult.refetch()} />
      </SettingsSectionInset> :
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            mobileEndContentFullWidth
            label="Reviewer model"
            description={!reviewer && loading ? "Loading reviewer settings..." : undefined}
            endContent={
              <SettingsRowActions>
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
              </SettingsRowActions>
            }
          />
        </SettingsList>}
        {reviewer && (error || saveError || warning) ? <SettingsLocalFeedback>
          {error ? <HStack gap={2} wrap="wrap" vAlign="center">
            <p role="alert" {...stylex.props(styles.mutedText)}>Reviewer settings could not refresh.</p>
            <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void privacyResult.refetch()} />
          </HStack> : null}
          {saveError ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the reviewer model.</p> : null}
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
        </SettingsLocalFeedback> : null}
    </SettingsSection>
  );
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
