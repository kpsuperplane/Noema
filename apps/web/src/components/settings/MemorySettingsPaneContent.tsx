import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import type { MemorySettingsQuery } from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

type NativeMemorySettings = MemorySettingsQuery["memorySettings"];

export function MemorySettingsPaneContent({
  settings,
  loading,
  error,
  saving,
  saveError,
  onSave
}: {
  settings: NativeMemorySettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading memory settings...</p>;
  }

  if (error || !settings) {
    return (
      <section {...stylex.props(styles.card)} aria-labelledby="memory-settings-title">
        <h2 id="memory-settings-title" {...stylex.props(styles.cardTitle)}>
          Memory updates
        </h2>
        <p {...stylex.props(styles.mutedText)}>
          The model choices for native memory updates could not be loaded.
        </p>
      </section>
    );
  }

  const preference = toPreference(settings.modelPreference);
  const options = settings.modelOptions.map(toProviderOption);
  const warning = selectedPreferenceWarning(preference, options);

  return (
    <section {...stylex.props(styles.card)} aria-labelledby="memory-settings-title">
      <div {...stylex.props(styles.cardHeader)}>
        <div {...stylex.props(styles.titleRow)}>
          <h2 id="memory-settings-title" {...stylex.props(styles.cardTitle)}>
            Background memory updates
          </h2>
          {!preference ? <Badge variant="neutral" label="Default" /> : null}
        </div>
        <span {...stylex.props(styles.scope)}>Local human only</span>
      </div>
      <p {...stylex.props(styles.mutedText)}>
        Choose the model Noema uses to consolidate completed conversation items into native Markdown pages.
        Updates run in the background and keep the last successful checkpoint when a run fails.
      </p>
      <ModelPreferenceSelect
        options={options}
        preference={preference}
        saving={saving}
        ariaLabel="Model settings for native memory updates"
        onSave={onSave}
      />
      {saveError ? (
        <p role="alert" {...stylex.props(styles.saveError)}>
          Noema could not save the memory update model.
        </p>
      ) : null}
      {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
    </section>
  );
}

function toPreference(value: NonNullable<NativeMemorySettings>["modelPreference"]): ModelPreference | null {
  return value
    ? {
        providerKind: value.providerKind,
        providerAccountId: value.providerAccountId,
        modelProfile: value.modelProfile,
        reasoningEffort: value.reasoningEffort
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
    defaultModelProfile: value.defaultModelProfile,
    profiles: value.profiles.map((profile) => ({
      id: profile.id,
      label: profile.label,
      disabledReason: profile.disabledReason,
      reasoningEfforts: profile.reasoningEfforts,
      defaultReasoningEffort: profile.defaultReasoningEffort
    }))
  };
}

const styles = stylex.create({
  card: {
    display: "grid",
    gap: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--surface-raised)",
    padding: "var(--spacing-4)"
  },
  cardHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)"
  },
  titleRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: "var(--spacing-2)"
  },
  cardTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  scope: {
    color: "var(--muted-foreground)",
    fontSize: 12
  },
  mutedText: {
    margin: 0,
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  saveError: {
    margin: 0,
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
  warningText: {
    margin: 0,
    color: "var(--warning-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  }
});
