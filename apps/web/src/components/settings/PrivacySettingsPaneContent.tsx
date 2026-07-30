import * as stylex from "@stylexjs/stylex";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
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
    <VStack as="section" gap={3} {...stylex.props(styles.card)} aria-labelledby="privacy-reviewer-title">
      <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
        <h2 id="privacy-reviewer-title" {...stylex.props(styles.cardTitle)}>
          Reviewer model
        </h2>
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
      {loading ? (
        <p {...stylex.props(styles.mutedText)}>Loading reviewer settings...</p>
      ) : error ? (
        <p {...stylex.props(styles.mutedText)}>Reviewer settings could not be loaded.</p>
      ) : (
        <>
          <p {...stylex.props(styles.mutedText)}>
            Noema sends the proposed action and bounded authorization context to this model.
            Clear reviews may execute automatically; if no reviewer is selected or it is
            unavailable, the action waits for your approval.
          </p>
          {saveError ? (
            <p {...stylex.props(styles.saveError)}>Noema could not save the reviewer model.</p>
          ) : null}
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
        </>
      )}
    </VStack>
  );
}

const styles = stylex.create({
  card: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: "var(--spacing-4)"
  },
  cardTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    color: "var(--foreground)"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  saveError: {
    margin: "var(--spacing-0)",
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  warningText: {
    margin: "var(--spacing-0)",
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--warning-foreground)"
  }
});
