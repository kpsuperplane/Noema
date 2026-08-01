import { HStack } from "@astryxdesign/core/HStack";
import { List, ListItem } from "@astryxdesign/core/List";
import { Section } from "@astryxdesign/core/Section";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { StatusDot } from "@astryxdesign/core/StatusDot";
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
  const search = webToolSettings?.search ?? null;
  const fetch = webToolSettings?.fetch ?? null;
  const fetchPreference = webFetchSummarizer?.modelPreference ?? null;
  const fetchWarning = webFetchSummarizer
    ? selectedPreferenceWarning(
        fetchPreference,
        webFetchSummarizer.modelOptions,
        "WEB_FETCH_SUMMARIZER"
      )
    : null;

  return (
    <VStack gap={6} {...stylex.props(styles.stack)}>
      <Section variant="transparent" padding={0} aria-labelledby="web-search-settings-title">
        <VStack gap={2}>
          <HStack wrap="wrap" gap={2} vAlign="center" hAlign="between">
            <h2 id="web-search-settings-title" {...stylex.props(styles.sectionTitle)}>
              Search
            </h2>
            <EnabledStatus />
          </HStack>
          <List density="balanced" hasDividers>
            <WebProviderRow
              settings={search}
              loading={webToolLoading}
              error={webToolError}
              saving={webToolSaving}
              onSave={onSaveWebToolProviderBinding}
            />
          </List>
          {webToolSaveError ? (
            <p role="alert" {...stylex.props(styles.saveError)}>
              Noema could not save the search provider binding.
            </p>
          ) : null}
          <VStack as="details" gap={2} {...stylex.props(styles.details)}>
            <summary {...stylex.props(styles.summary)}>Technical details</summary>
            <List density="compact">
              <ListItem label="Tool" description="web.search" />
              <ListItem label="Data flow" description={providerDataFlow(search)} />
              <ListItem label="Citations" description={providerCapability(search, "citations")} />
            </List>
          </VStack>
        </VStack>
      </Section>

      <Section variant="transparent" padding={0} aria-labelledby="web-fetch-settings-title">
        <VStack gap={2}>
          <HStack wrap="wrap" gap={2} vAlign="center" hAlign="between">
            <h2 id="web-fetch-settings-title" {...stylex.props(styles.sectionTitle)}>
              Fetch
            </h2>
            <EnabledStatus />
          </HStack>
          <List density="balanced" hasDividers>
            <WebProviderRow
              settings={fetch}
              loading={webToolLoading}
              error={webToolError}
              saving={webToolSaving}
              onSave={onSaveWebToolProviderBinding}
            />
            <ListItem
              label="Summarizer model"
              description={
                loading
                  ? "Loading fetch summarizer settings..."
                  : undefined
              }
              endContent={
                <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
                  <ModelPreferenceSelect
                    options={webFetchSummarizer?.modelOptions ?? []}
                    preference={fetchPreference}
                    useCase="WEB_FETCH_SUMMARIZER"
                    saving={saving}
                    ariaLabel="Model settings for fetch summarizer"
                    isDisabled={Boolean(error) || !webFetchSummarizer}
                    onSave={onSaveWebFetchSummarizerPreference}
                  />
                </HStack>
              }
            />
          </List>
          {webToolSaveError || saveError ? (
            <p role="alert" {...stylex.props(styles.saveError)}>
              {webToolSaveError
                ? "Noema could not save the fetch provider binding."
                : "Noema could not save the fetch summarizer model."}
            </p>
          ) : null}
          {webToolError || error ? (
            <p {...stylex.props(styles.mutedText)}>
              {webToolError
                ? "Provider settings could not be loaded."
                : "Fetch summarizer settings could not be loaded."}
            </p>
          ) : null}
          {fetchWarning ? <p {...stylex.props(styles.warningText)}>{fetchWarning}</p> : null}
          <VStack as="details" gap={2} {...stylex.props(styles.details)}>
            <summary {...stylex.props(styles.summary)}>Technical details</summary>
            <List density="compact">
              <ListItem label="Tool" description="web.fetch" />
              <ListItem label="Contract" description={activeProviderContract(fetch, webToolLoading, webToolError)} />
              <ListItem label="Extraction" description="readabilityrs Markdown" />
              <ListItem
                label="Safety"
                description="Public HTTP(S), checked redirects, private/local targets blocked, response size caps."
              />
              <ListItem label="Data flow" description={providerDataFlow(fetch)} />
            </List>
          </VStack>
        </VStack>
      </Section>
    </VStack>
  );
}

function WebProviderRow({
  settings,
  loading,
  error,
  saving,
  onSave
}: {
  settings: WebToolBindingSettings | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  onSave: (input: {
    toolName: string;
    capabilityId: string;
    providerAccountId: string;
  }) => Promise<unknown>;
}) {
  const unavailable = Boolean(error) || !settings;
  return (
    <ListItem
      label="Provider"
      description={providerDescription(settings, loading, error)}
      endContent={
        <WebProviderSelect
          settings={settings}
          saving={saving}
          isDisabled={unavailable}
          ariaLabel="Provider for web tooling"
          onSave={onSave}
        />
      }
    />
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
    () => providerOptions.map((option) => ({ value: option.providerAccountId, label: option.displayName })),
    [providerOptions]
  );
  const selectedValue = useMemo(() => {
    if (!settings) return "";
    return providerOptions.some((option) => option.providerAccountId === settings.activeProviderAccountId)
      ? settings.activeProviderAccountId
      : providerOptions[0]?.providerAccountId ?? "";
  }, [providerOptions, settings]);

  return (
    <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
      <Selector
        isLabelHidden
        label={ariaLabel}
        options={selectorOptions}
        placeholder={providerOptions.length === 0 ? "No providers available" : "Select provider"}
        value={selectedValue || undefined}
        isDisabled={isDisabled || saving || providerOptions.length === 0}
        onChange={(value) => {
          if (!settings || value === selectedValue) return;
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

function EnabledStatus() {
  return (
    <HStack gap={1} vAlign="center" {...stylex.props(styles.status)}>
      <StatusDot variant="success" label="Enabled" />
      <span>Enabled</span>
    </HStack>
  );
}

function providerDescription(
  settings: WebToolBindingSettings | null,
  loading: boolean,
  error: string | null
) {
  if (loading) return "Loading provider settings...";
  if (error) return "Provider settings could not be loaded.";
  return activeProviderLabel(settings, false, null);
}

function activeProviderLabel(
  settings: WebToolBindingSettings | null,
  loading: boolean,
  error: string | null
) {
  if (loading) return "Loading...";
  if (error) return "Unavailable";
  return activeProviderOption(settings)?.displayName ?? "No provider configured";
}

function activeProviderContract(
  settings: WebToolBindingSettings | null,
  loading: boolean,
  error: string | null
) {
  if (loading) return "Loading...";
  if (error) return "Unavailable";
  return activeProviderOption(settings)?.reliabilityContract ?? "No provider configured";
}

function activeProviderOption(settings: WebToolBindingSettings | null) {
  if (!settings) return null;
  return settings.providerOptions.find(
    (option) => option.providerAccountId === settings.activeProviderAccountId
  ) ?? settings.providerOptions[0] ?? null;
}

function providerDataFlow(settings: WebToolBindingSettings | null) {
  return activeProviderOption(settings)?.dataFlowClass ?? "Unavailable";
}

function providerCapability(settings: WebToolBindingSettings | null, capability: "citations") {
  const option = activeProviderOption(settings);
  if (!option) return "Unavailable";
  return option[capability] ? "Supported" : "Not provided";
}

const styles = stylex.create({
  stack: { minWidth: 0 },
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
  status: {
    flexShrink: 0,
    color: "var(--muted-foreground)",
    fontSize: 12,
    fontWeight: 600
  },
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  },
  details: {
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: "var(--spacing-3)"
  },
  summary: {
    cursor: "pointer",
    color: "var(--muted-foreground)",
    fontSize: 12,
    fontWeight: 600
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
  }
});
