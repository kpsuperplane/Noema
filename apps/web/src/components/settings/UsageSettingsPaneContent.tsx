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
  onSaveToolProgressAuditPreference
}: {
  progressAudit: ToolProgressAuditSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveToolProgressAuditPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const preference = progressAudit?.modelPreference ?? null;
  const warning = progressAudit
    ? selectedPreferenceWarning(preference, progressAudit.modelOptions, "TOOL_PROGRESS_AUDIT")
    : null;
  const unavailable = Boolean(error) || !progressAudit;

  return (
    <VStack as="section" gap={3} {...stylex.props(styles.card)} aria-labelledby="usage-progress-audit-title">
      <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
        <h2 id="usage-progress-audit-title" {...stylex.props(styles.cardTitle)}>
          Tool progress audit
        </h2>
        <ModelPreferenceSelect
          options={progressAudit?.modelOptions ?? []}
          preference={preference}
          useCase="TOOL_PROGRESS_AUDIT"
          saving={saving}
          ariaLabel="Model settings for tool progress audit"
          isDisabled={unavailable}
          onSave={onSaveToolProgressAuditPreference}
        />
      </HStack>
      {loading ? (
        <p {...stylex.props(styles.mutedText)}>Loading progress audit settings...</p>
      ) : error ? (
        <p {...stylex.props(styles.mutedText)}>Progress audit settings could not be loaded.</p>
      ) : (
        <>
          {saveError ? (
            <p {...stylex.props(styles.saveError)}>
              Noema could not save the progress audit model.
            </p>
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
    letterSpacing: 0,
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
