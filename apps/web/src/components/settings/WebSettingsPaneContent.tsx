import { Badge } from "@astryxdesign/core/Badge";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { useMemo } from "react";
import * as stylex from "@stylexjs/stylex";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
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

export type WebToolProviderOption = {
  providerAccountId: string;
  providerKind: string;
  accountKey: string;
  displayName: string;
  capabilityId: string;
  reliabilityContract: string;
  dataFlowClass: string;
  citations: boolean;
  directUrlFetch: boolean;
};

export type WebToolBindingSettings = {
  toolName: string;
  capabilityId: string;
  activeProviderAccountId: string;
  providerOptions: readonly WebToolProviderOption[];
};

export type WebToolSettings = {
  search: WebToolBindingSettings;
  fetch: WebToolBindingSettings;
};

export function WebSettingsPaneContent({
  webFetchSummarizer,
  webToolSettings,
  loading,
  error,
  saving,
  saveError,
  webToolLoading,
  webToolError,
  webToolSaving,
  webToolSaveError,
  onSaveWebFetchSummarizerPreference,
  onSaveWebToolProviderBinding
}: {
  webFetchSummarizer: WebFetchSummarizerSettings | null;
  webToolSettings: WebToolSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  webToolLoading: boolean;
  webToolError: string | null;
  webToolSaving: boolean;
  webToolSaveError: string | null;
  onSaveWebFetchSummarizerPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
  onSaveWebToolProviderBinding: (input: {
    toolName: string;
    capabilityId: string;
    providerAccountId: string;
  }) => Promise<unknown>;
}) {
  return (
    <VStack gap={3}>
      <VStack as="section" gap={3} {...stylex.props(styles.card)} aria-labelledby="web-search-settings-title">
        <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
          <HStack wrap="wrap" gap={3} vAlign="center">
            <h2 id="web-search-settings-title" {...stylex.props(styles.cardTitle)}>
              Search
            </h2>
            <Badge variant="neutral" label="Enabled" />
          </HStack>
        </HStack>
        <VStack as="dl" gap={2} {...stylex.props(styles.definitionList)}>
          <MetadataRow label="Tool" value="web.search" />
          <MetadataRow
            label="Provider"
            value={activeProviderLabel(webToolSettings?.search ?? null, webToolLoading, webToolError)}
          />
        </VStack>
        <WebProviderBindingCard
          title="Search provider"
          ariaLabel="Provider for web search"
          settings={webToolSettings?.search ?? null}
          loading={webToolLoading}
          error={webToolError}
          saveError={webToolSaveError}
          saving={webToolSaving}
          onSave={onSaveWebToolProviderBinding}
        />
      </VStack>

      <VStack as="section" gap={3} {...stylex.props(styles.card)} aria-labelledby="web-fetch-settings-title">
        <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
          <HStack wrap="wrap" gap={3} vAlign="center">
            <h2 id="web-fetch-settings-title" {...stylex.props(styles.cardTitle)}>
              Fetch
            </h2>
            <Badge variant="neutral" label="Enabled" />
          </HStack>
        </HStack>
        <VStack as="dl" gap={2} {...stylex.props(styles.definitionList)}>
          <MetadataRow label="Tool" value="web.fetch" />
          <MetadataRow
            label="Provider"
            value={activeProviderLabel(webToolSettings?.fetch ?? null, webToolLoading, webToolError)}
          />
          <MetadataRow
            label="Contract"
            value={activeProviderContract(webToolSettings?.fetch ?? null, webToolLoading, webToolError)}
          />
          <MetadataRow label="Extraction" value="readabilityrs markdown" />
          <MetadataRow
            label="Safety"
            value="Public HTTP(S), checked redirects, private/local targets blocked, response size caps."
          />
        </VStack>
        <WebProviderBindingCard
          title="Fetch provider"
          ariaLabel="Provider settings for web fetch"
          settings={webToolSettings?.fetch ?? null}
          loading={webToolLoading}
          error={webToolError}
          saveError={webToolSaveError}
          saving={webToolSaving}
          onSave={onSaveWebToolProviderBinding}
        />
        <FetchSummarizerCard
          settings={webFetchSummarizer}
          loading={loading}
          error={error}
          saveError={saveError}
          saving={saving}
          onSave={onSaveWebFetchSummarizerPreference}
        />
      </VStack>
    </VStack>
  );
}

function WebProviderBindingCard({
  title,
  ariaLabel,
  settings,
  loading,
  error,
  saveError,
  saving,
  onSave
}: {
  title: string;
  ariaLabel: string;
  settings: WebToolBindingSettings | null;
  loading: boolean;
  error: string | null;
  saveError: string | null;
  saving: boolean;
  onSave: (input: {
    toolName: string;
    capabilityId: string;
    providerAccountId: string;
  }) => Promise<unknown>;
}) {
  const unavailable = Boolean(error) || !settings;

  return (
    <VStack gap={3} {...stylex.props(styles.subcard)}>
      <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
        <HStack wrap="wrap" gap={3} vAlign="center">
          <h3 {...stylex.props(styles.subcardTitle)}>{title}</h3>
        </HStack>
        <WebProviderSelect
          settings={settings}
          saving={saving}
          isDisabled={unavailable}
          ariaLabel={ariaLabel}
          onSave={onSave}
        />
      </HStack>
      {loading ? (
        <p {...stylex.props(styles.mutedText)}>Loading provider settings...</p>
      ) : error ? (
        <p {...stylex.props(styles.mutedText)}>Provider settings could not be loaded.</p>
      ) : saveError ? (
        <p {...stylex.props(styles.saveError)}>Noema could not save the provider binding.</p>
      ) : null}
    </VStack>
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
  const warning = settings ? selectedPreferenceWarning(preference, settings.modelOptions) : null;
  const unavailable = Boolean(error) || !settings;

  return (
    <VStack gap={3} {...stylex.props(styles.subcard)}>
      <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
        <h3 {...stylex.props(styles.subcardTitle)}>Fetch summarizer</h3>
        <ModelPreferenceSelect
          options={settings?.modelOptions ?? []}
          preference={preference}
          defaultModelProfile={settings?.defaultModelProfile}
          saving={saving}
          ariaLabel="Model settings for fetch summarizer"
          isDisabled={unavailable}
          onSave={onSave}
        />
      </HStack>
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
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
        </>
      )}
    </VStack>
  );
}

function WebProviderSelect({
  settings,
  saving,
  isDisabled = false,
  ariaLabel,
  onSave
}: {
  settings: WebToolBindingSettings | null;
  saving: boolean;
  isDisabled?: boolean;
  ariaLabel: string;
  onSave: (input: {
    toolName: string;
    capabilityId: string;
    providerAccountId: string;
  }) => Promise<unknown>;
}) {
  const providerOptions = useMemo(() => settings?.providerOptions ?? [], [settings]);
  const selectorOptions = useMemo<SelectorOptionType[]>(
    () =>
      providerOptions.map((option) => ({
        value: option.providerAccountId,
        label: option.displayName
      })),
    [providerOptions]
  );
  const selectedValue = useMemo(() => {
    if (!settings) {
      return "";
    }
    const hasActiveOption = providerOptions.some(
      (option) => option.providerAccountId === settings.activeProviderAccountId
    );
    if (hasActiveOption) {
      return settings.activeProviderAccountId;
    }
    return providerOptions[0]?.providerAccountId ?? "";
  }, [providerOptions, settings]);

  return (
    <HStack wrap="wrap" gap={2} vAlign="center" hAlign="end">
      <span {...stylex.props(styles.fieldLabel)}>Provider</span>
      <Selector
        isLabelHidden
        label={ariaLabel}
        options={selectorOptions}
        placeholder={providerOptions.length === 0 ? "No providers available" : "Select provider"}
        value={selectedValue || undefined}
        isDisabled={isDisabled || saving || providerOptions.length === 0}
        onChange={(value) => {
          if (!settings || value === selectedValue) {
            return;
          }
          void onSave({
            toolName: settings.toolName,
            capabilityId: settings.capabilityId,
            providerAccountId: value
          });
        }}
      />
    </HStack>
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

function activeProviderLabel(
  settings: WebToolBindingSettings | null,
  loading: boolean,
  error: string | null
) {
  if (loading) {
    return "Loading...";
  }
  if (error) {
    return "Unavailable";
  }
  return activeProviderOption(settings)?.displayName ?? "No provider configured";
}

function activeProviderContract(
  settings: WebToolBindingSettings | null,
  loading: boolean,
  error: string | null
) {
  if (loading) {
    return "Loading...";
  }
  if (error) {
    return "Unavailable";
  }
  return activeProviderOption(settings)?.reliabilityContract ?? "No provider configured";
}

function activeProviderOption(settings: WebToolBindingSettings | null) {
  if (!settings) {
    return null;
  }
  return (
    settings.providerOptions.find(
      (option) => option.providerAccountId === settings.activeProviderAccountId
    ) ?? settings.providerOptions[0] ?? null
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
  subcard: {
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: "var(--spacing-3)"
  },
  cardTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  subcardTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  definitionList: {
    margin: "var(--spacing-0)"
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(110px, 0.35fr) minmax(0, 1fr)",
    gap: "var(--spacing-3)",
    "@media (max-width: 560px)": {
      gridTemplateColumns: "1fr",
      gap: "var(--spacing-0-5)"
    }
  },
  definitionTerm: {
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.45
  },
  definitionValue: {
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.45
  },
  fieldLabel: {
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  saveError: {
    margin: "var(--spacing-0)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--destructive) 30%, transparent)",
    borderRadius: 6,
    backgroundColor: "color-mix(in srgb, var(--destructive) 5%, transparent)",
    padding: "var(--spacing-3)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  warningText: {
    margin: "var(--spacing-0)",
    fontSize: 13,
    lineHeight: 1.45,
    color: "var(--muted-foreground)"
  }
});
