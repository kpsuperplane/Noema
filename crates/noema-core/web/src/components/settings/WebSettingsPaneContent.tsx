import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import {
  modelPreferenceLabel,
  modelProfileLabel,
  providerPreferenceLabel,
  selectedPreferenceWarning
} from "./modelPreferenceMetadata";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export type WebFetchSummarizerSettings = {
  defaultModelProfile: string;
  modelPreference?: ModelPreference | null;
  modelOptions: readonly ModelProviderOption[];
};

export function WebSettingsPaneContent({
  webFetchSummarizer,
  loading,
  error,
  saving,
  saveError,
  onSaveWebFetchSummarizerPreference
}: {
  webFetchSummarizer: WebFetchSummarizerSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveWebFetchSummarizerPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  return (
    <div {...stylex.props(styles.stack)}>
      <section {...stylex.props(styles.card)} aria-labelledby="web-search-settings-title">
        <div {...stylex.props(styles.cardHeader)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2 id="web-search-settings-title" {...stylex.props(styles.cardTitle)}>Search</h2>
            <Badge variant="neutral" label="Enabled" />
          </div>
        </div>
        <dl {...stylex.props(styles.definitionList)}>
          <MetadataRow label="Tool" value="web.search" />
          <MetadataRow label="Provider" value="DuckDuckGo public adapter" />
          <MetadataRow label="Contract" value="Search results only; pages are not fetched or read." />
        </dl>
      </section>

      <section {...stylex.props(styles.card)} aria-labelledby="web-fetch-settings-title">
        <div {...stylex.props(styles.cardHeader)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2 id="web-fetch-settings-title" {...stylex.props(styles.cardTitle)}>Fetch</h2>
            <Badge variant="neutral" label="Enabled" />
          </div>
        </div>
        <dl {...stylex.props(styles.definitionList)}>
          <MetadataRow label="Tool" value="web.fetch" />
          <MetadataRow label="Provider" value="Direct HTTP" />
          <MetadataRow label="Extraction" value="readabilityrs markdown" />
          <MetadataRow
            label="Safety"
            value="Public HTTP(S), checked redirects, private/local targets blocked, response size caps."
          />
        </dl>
        <FetchSummarizerCard
          settings={webFetchSummarizer}
          loading={loading}
          error={error}
          saveError={saveError}
          saving={saving}
          onSave={onSaveWebFetchSummarizerPreference}
        />
      </section>
    </div>
  );
}

function FetchSummarizerCard({
  settings,
  loading,
  error,
  saveError,
  saving,
  onSave
}: {
  settings: WebFetchSummarizerSettings | null;
  loading: boolean;
  error: string | null;
  saveError: string | null;
  saving: boolean;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const preference = settings?.modelPreference ?? null;
  const rows = webFetchSummarizerRows(settings);
  const warning = settings ? selectedPreferenceWarning(preference, settings.modelOptions) : null;
  const unavailable = Boolean(error) || !settings;

  return (
    <div {...stylex.props(styles.subcard)}>
      <div {...stylex.props(styles.cardHeader)}>
        <div {...stylex.props(styles.titleRow)}>
          <h3 {...stylex.props(styles.subcardTitle)}>Fetch summarizer</h3>
          {!preference && settings ? <Badge variant="neutral" label="Default" /> : null}
        </div>
        <ModelPreferenceSelect
          options={settings?.modelOptions ?? []}
          preference={preference}
          defaultModelProfile={settings?.defaultModelProfile}
          saving={saving}
          ariaLabel="Model settings for fetch summarizer"
          isDisabled={unavailable}
          onSave={onSave}
        />
      </div>
      {loading ? (
        <p {...stylex.props(styles.mutedText)}>Loading fetch summarizer settings...</p>
      ) : error ? (
        <p {...stylex.props(styles.mutedText)}>Fetch summarizer settings could not be loaded.</p>
      ) : (
        <>
          {saveError ? (
            <p {...stylex.props(styles.saveError)}>
              Noema could not save the fetch summarizer model.
            </p>
          ) : null}
          <dl {...stylex.props(styles.definitionList)}>
            {rows.map((row) => (
              <MetadataRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
        </>
      )}
    </div>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div {...stylex.props(styles.definitionRow)}>
      <dt {...stylex.props(styles.definitionTerm)}>{label}</dt>
      <dd {...stylex.props(styles.definitionValue)}>{value}</dd>
    </div>
  );
}

function webFetchSummarizerRows(settings: WebFetchSummarizerSettings | null) {
  if (!settings) {
    return [{ label: "Model", value: "Unavailable" }];
  }
  const preference = settings.modelPreference ?? null;
  if (!preference) {
    return [
      { label: "Provider", value: "Default provider" },
      {
        label: "Model",
        value: `${modelProfileLabel(settings.defaultModelProfile, settings.modelOptions)} (default)`
      }
    ];
  }
  return [
    { label: "Provider", value: providerPreferenceLabel(preference, settings.modelOptions) },
    { label: "Model", value: modelPreferenceLabel(preference, settings.modelOptions) }
  ];
}

const styles = stylex.create({
  stack: {
    display: "grid",
    gap: 12
  },
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
  subcard: {
    display: "grid",
    gap: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  cardHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12
  },
  titleRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
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
  subcardTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  definitionList: {
    display: "grid",
    gap: 8,
    margin: 0
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(110px, 0.35fr) minmax(0, 1fr)",
    gap: 12,
    "@media (max-width: 560px)": {
      gridTemplateColumns: "1fr",
      gap: 2
    }
  },
  definitionTerm: {
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.45
  },
  definitionValue: {
    margin: 0,
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.45
  },
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  saveError: {
    margin: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--destructive) 30%, transparent)",
    borderRadius: 6,
    backgroundColor: "color-mix(in srgb, var(--destructive) 5%, transparent)",
    padding: 12,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  warningText: {
    margin: 0,
    fontSize: 13,
    lineHeight: 1.45,
    color: "var(--muted-foreground)"
  }
});
