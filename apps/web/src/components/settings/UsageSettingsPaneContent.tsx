import * as stylex from "@stylexjs/stylex";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export type ToolProgressAuditSettings = {
  defaultModelProfile: string;
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
    ? selectedPreferenceWarning(preference, progressAudit.modelOptions)
    : null;
  const unavailable = Boolean(error) || !progressAudit;

  return (
    <section {...stylex.props(styles.card)} aria-labelledby="usage-progress-audit-title">
      <div {...stylex.props(styles.cardHeader)}>
        <h2 id="usage-progress-audit-title" {...stylex.props(styles.cardTitle)}>
          Tool progress audit
        </h2>
        <ModelPreferenceSelect
          options={progressAudit?.modelOptions ?? []}
          preference={preference}
          defaultModelProfile={progressAudit?.defaultModelProfile}
          saving={saving}
          ariaLabel="Model settings for tool progress audit"
          isDisabled={unavailable}
          onSave={onSaveToolProgressAuditPreference}
        />
      </div>
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
    </section>
  );
}

const styles = stylex.create({
  card: {
    display: "grid",
    gap: 12,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 16
  },
  cardHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12
  },
  cardTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  mutedText: {
    margin: 0,
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  saveError: {
    margin: 0,
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  warningText: {
    margin: 0,
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--warning-foreground)"
  }
});
