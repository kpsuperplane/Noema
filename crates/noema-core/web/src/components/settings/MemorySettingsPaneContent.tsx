import * as stylex from "@stylexjs/stylex";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import { memoryStatusLabel } from "./memorySettingsModel";
import type { MemorySettingsQuery, SaveMemoryServiceSettingsInput } from "@/generated/graphql";
import type { ModelPreferenceSaveInput } from "./modelPreferenceTypes";

type MemorySettings = MemorySettingsQuery["memorySettings"];

export type MemorySettingsPaneContentProps = {
  settings: MemorySettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  checking: boolean;
  saveError: string | null;
  onSave: (input: SaveMemoryServiceSettingsInput) => Promise<unknown>;
  onCheck: () => Promise<unknown>;
};

export function MemorySettingsPaneContent({
  settings,
  loading,
  error,
  saving,
  checking,
  saveError,
  onSave,
  onCheck
}: MemorySettingsPaneContentProps) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading memory settings...</p>;
  }

  if (error || !settings) {
    return (
      <section {...stylex.props(styles.card)} aria-labelledby="memory-settings-title">
        <h2 id="memory-settings-title" {...stylex.props(styles.cardTitle)}>
          Supermemory
        </h2>
        <p {...stylex.props(styles.mutedText)}>Memory settings could not be loaded.</p>
      </section>
    );
  }

  const warning = selectedPreferenceWarning(
    settings.modelPreference ?? null,
    settings.modelOptions
  );

  return (
    <section {...stylex.props(styles.card)} aria-labelledby="memory-settings-title">
      <div {...stylex.props(styles.cardHeader)}>
        <div {...stylex.props(styles.titleStack)}>
          <h2 id="memory-settings-title" {...stylex.props(styles.cardTitle)}>
            Supermemory
          </h2>
          <p {...stylex.props(styles.mutedText)}>
            {memoryStatusLabel(settings.status.status)}
          </p>
        </div>
        <button
          type="button"
          {...stylex.props(styles.button)}
          disabled={checking}
          onClick={() => void onCheck()}
        >
          {checking ? "Checking..." : "Check connection"}
        </button>
      </div>

      <dl {...stylex.props(styles.definitionList)}>
        <MetadataRow label="Mode" value={memoryModeLabel(settings.mode)} />
        <MetadataRow label="Base URL" value={settings.baseUrl} />
        {settings.port ? <MetadataRow label="Port" value={String(settings.port)} /> : null}
        {settings.status.checkedAt ? (
          <MetadataRow label="Last checked" value={settings.status.checkedAt} />
        ) : null}
        {settings.status.lastErrorCode ? (
          <MetadataRow label="Last error" value={settings.status.lastErrorCode} />
        ) : null}
      </dl>

      <ModelPreferenceSelect
        options={settings.modelOptions}
        preference={settings.modelPreference ?? null}
        saving={saving}
        ariaLabel="Model settings for Supermemory extraction"
        onSave={(input) => onSave(memorySaveInputFromSelection(settings, input))}
      />

      {saveError ? <p {...stylex.props(styles.saveError)}>{saveError}</p> : null}
      {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
    </section>
  );
}

function memorySaveInputFromSelection(
  settings: MemorySettings,
  input: ModelPreferenceSaveInput
): SaveMemoryServiceSettingsInput {
  return {
    mode: settings.mode,
    baseUrl: settings.baseUrl,
    port: settings.port ?? null,
    providerAccountId: input.providerAccountId,
    modelProfile: input.modelProfile,
    reasoningEffort: input.reasoningEffort ?? null
  };
}

function memoryModeLabel(mode: MemorySettings["mode"]) {
  switch (mode) {
    case "MANAGED":
      return "Managed local";
    case "EXTERNAL":
      return "External service";
  }
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div {...stylex.props(styles.definitionRow)}>
      <dt {...stylex.props(styles.definitionTerm)}>{label}</dt>
      <dd {...stylex.props(styles.definitionValue)}>{value}</dd>
    </div>
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
  titleStack: {
    display: "grid",
    gap: 4
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
  button: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: "7px 10px",
    fontSize: 13,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--foreground)",
    cursor: "pointer",
    ":disabled": {
      cursor: "not-allowed",
      opacity: 0.6
    }
  },
  definitionList: {
    display: "grid",
    gap: 8,
    margin: 0
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(120px, 180px) 1fr",
    gap: 16,
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr",
      gap: 4
    }
  },
  definitionTerm: {
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  definitionValue: {
    minWidth: 0,
    margin: 0,
    overflowWrap: "break-word",
    fontFamily: "var(--font-mono)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--foreground)"
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
