import { HStack } from "@astryxdesign/core/HStack";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { VStack } from "@astryxdesign/core/VStack";
import { useEffect, useMemo, useState } from "react";
import * as stylex from "@stylexjs/stylex";
import { ArrowDown, ArrowUp, Plus, RefreshCw, Trash2 } from "lucide-react";
import {
  SaveBrowserProviderRouteDocument,
  SaveWebFetchSummarizerPreferenceDocument,
  SaveWebToolProviderBindingDocument,
  WebFetchSettingsDocument,
  WebToolSettingsDocument,
  type SaveBrowserProviderRouteMutation,
  type SaveBrowserProviderRouteMutationVariables,
  type SaveWebFetchSummarizerPreferenceMutation,
  type SaveWebFetchSummarizerPreferenceMutationVariables,
  type SaveWebToolProviderBindingInput,
  type SaveWebToolProviderBindingMutation,
  type SaveWebToolProviderBindingMutationVariables,
  type WebFetchSettingsQuery,
  type WebToolSettingsQuery
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { selectedPreferenceWarning } from "./modelPreferenceMetadata";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsRowActions,
  SettingsSection,
  SettingsTechnicalDetails
} from "./SettingsPrimitives";
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
  jsRendering: boolean;
  authenticatedContext: boolean;
};

export type WebToolBindingSettings = {
  toolName: string;
  capabilityId: string;
  activeProviderAccountId: string;
  providerRouteAccountIds?: readonly string[];
  providerOptions: readonly WebToolProviderOption[];
};

export type WebToolSettings = {
  search: WebToolBindingSettings;
  fetch: WebToolBindingSettings;
  browse: WebToolBindingSettings;
};

export function WebSettingsPane() {
  const webFetchResult = useQuery<WebFetchSettingsQuery>(WebFetchSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const webToolResult = useQuery<WebToolSettingsQuery>(WebToolSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveWebFetchPreference, saveWebFetchResult] = useMutation<
    SaveWebFetchSummarizerPreferenceMutation,
    SaveWebFetchSummarizerPreferenceMutationVariables
  >(SaveWebFetchSummarizerPreferenceDocument, {
    refetchQueries: [{ query: WebFetchSettingsDocument }],
    awaitRefetchQueries: true
  });
  const [saveWebToolBinding, saveWebToolResult] = useMutation<
    SaveWebToolProviderBindingMutation,
    SaveWebToolProviderBindingMutationVariables
  >(SaveWebToolProviderBindingDocument, {
    refetchQueries: [{ query: WebToolSettingsDocument }],
    awaitRefetchQueries: true
  });
  const [savingToolName, setSavingToolName] = useState<string | null>(null);
  const webFetchSummarizer = webFetchResult.data?.webFetchSettings.summarizer ?? null;
  const webToolSettings = webToolResult.data?.webToolSettings ?? null;
  const loading = webFetchResult.loading && !webFetchResult.data;
  const error = webFetchResult.error?.message ?? null;
  const saving = saveWebFetchResult.loading;
  const saveError = saveWebFetchResult.error?.message ?? null;
  const webToolLoading = webToolResult.loading && !webToolResult.data;
  const webToolError = webToolResult.error?.message ?? null;
  const webToolSaving = saveWebToolResult.loading;
  const webToolSaveError = saveWebToolResult.error?.message ?? null;
  const onSaveWebFetchSummarizerPreference = (input: ModelPreferenceSaveInput) => saveWebFetchPreference({ variables: { input } });
  const onSaveWebToolProviderBinding = (input: SaveWebToolProviderBindingInput) => {
    setSavingToolName(input.toolName);
    return saveWebToolBinding({ variables: { input } });
  };
  const search = webToolSettings?.search ?? null;
  const fetch = webToolSettings?.fetch ?? null;
  const browse = webToolSettings?.browse ?? null;
  const fetchPreference = webFetchSummarizer?.modelPreference ?? null;
  const fetchWarning = webFetchSummarizer
    ? selectedPreferenceWarning(
        fetchPreference,
        webFetchSummarizer.modelOptions,
        "WEB_FETCH_SUMMARIZER"
      )
    : null;
  const failedToolName = savingToolName;

  return (
    <VStack gap={4} {...stylex.props(styles.stack)}>
      <SettingsSection title="Search" titleId="web-search-settings-title">
          <SettingsList density="balanced" hasDividers>
            <WebProviderRow
              settings={search}
              loading={webToolLoading}
              error={webToolError}
              saving={webToolSaving}
              onSave={onSaveWebToolProviderBinding}
            />
          </SettingsList>
          {webToolError || (webToolSaveError && failedToolName === "web.search") ? <SettingsLocalFeedback>
            {webToolError ? <HStack gap={2} wrap="wrap" vAlign="center">
              <p role="alert" {...stylex.props(styles.mutedText)}>Search provider settings could not refresh.</p>
              <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void webToolResult.refetch()} />
            </HStack> : null}
            {webToolSaveError && failedToolName === "web.search" ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the search provider.</p> : null}
          </SettingsLocalFeedback> : null}
          <SettingsTechnicalDetails>
            <SettingsList density="compact">
              <SettingsListItem label="Tool" description="web.search" />
              <SettingsListItem label="Data flow" description={providerDataFlow(search)} />
              <SettingsListItem label="Citations" description={providerCapability(search, "citations")} />
            </SettingsList>
          </SettingsTechnicalDetails>
      </SettingsSection>

      <SettingsSection title="Fetch" titleId="web-fetch-settings-title">
          <SettingsList density="balanced" hasDividers>
            <WebProviderRow
              settings={fetch}
              loading={webToolLoading}
              error={webToolError}
              saving={webToolSaving}
              onSave={onSaveWebToolProviderBinding}
            />
            <SettingsListItem
              mobileEndContentFullWidth
              label="Summarizer model"
              description={
                loading
                  ? "Loading fetch summarizer settings..."
                  : undefined
              }
              endContent={
                <SettingsRowActions>
                  <ModelPreferenceSelect
                    options={webFetchSummarizer?.modelOptions ?? []}
                    preference={fetchPreference}
                    useCase="WEB_FETCH_SUMMARIZER"
                    saving={saving}
                    ariaLabel="Model settings for fetch summarizer"
                    isDisabled={!webFetchSummarizer}
                    onSave={onSaveWebFetchSummarizerPreference}
                  />
                </SettingsRowActions>
              }
            />
          </SettingsList>
          {webToolError || error || saveError || (webToolSaveError && failedToolName === "web.fetch") || fetchWarning ? <SettingsLocalFeedback>
            {webToolError || error ? <HStack gap={2} wrap="wrap" vAlign="center">
              <p role="alert" {...stylex.props(styles.mutedText)}>Fetch settings could not refresh.</p>
              <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void Promise.all([webToolResult.refetch(), webFetchResult.refetch()])} />
            </HStack> : null}
            {webToolSaveError && failedToolName === "web.fetch" ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the fetch provider.</p> : null}
            {saveError ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the fetch summarizer model.</p> : null}
            {fetchWarning ? <p {...stylex.props(styles.warningText)}>{fetchWarning}</p> : null}
          </SettingsLocalFeedback> : null}
          <SettingsTechnicalDetails>
            <SettingsList density="compact">
              <SettingsListItem label="Tool" description="web.fetch" />
              <SettingsListItem label="Contract" description={activeProviderContract(fetch, webToolLoading, webToolError)} />
              <SettingsListItem label="Extraction" description="readabilityrs Markdown" />
              <SettingsListItem
                label="Safety"
                description="Public HTTP(S), checked redirects, private/local targets blocked, response size caps."
              />
              <SettingsListItem label="Data flow" description={providerDataFlow(fetch)} />
            </SettingsList>
          </SettingsTechnicalDetails>
      </SettingsSection>

      <SettingsSection title="Browse" titleId="web-browse-settings-title">
          <SettingsList density="balanced" hasDividers>
            <BrowserProviderRouteRow
              settings={browse}
              loading={webToolLoading}
              error={webToolError}
            />
          </SettingsList>
          {webToolError || (webToolSaveError && failedToolName === "web.browse") ? <SettingsLocalFeedback>
            {webToolError ? <HStack gap={2} wrap="wrap" vAlign="center">
              <p role="alert" {...stylex.props(styles.mutedText)}>Browse provider settings could not refresh.</p>
              <Button type="button" size="sm" variant="secondary" label="Retry" icon={<RefreshCw size={14} aria-hidden="true" />} onClick={() => void webToolResult.refetch()} />
            </HStack> : null}
            {webToolSaveError && failedToolName === "web.browse" ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the browse provider.</p> : null}
          </SettingsLocalFeedback> : null}
          <SettingsTechnicalDetails><SettingsList density="compact">
            <SettingsListItem
              label="Session"
              description="Execution-scoped; closes after 15 minutes without successful activity."
            />
            <SettingsListItem
              label="Safety"
              description="Public HTTP(S) only; private and local network targets are blocked. Browser page content is untrusted."
            />
            <SettingsListItem
              label="Capabilities"
              description={`${providerCapability(browse, "jsRendering")} JavaScript rendering; ${providerCapability(browse, "authenticatedContext")} session cookies.`}
            />
          </SettingsList></SettingsTechnicalDetails>
      </SettingsSection>
    </VStack>
  );
}

function BrowserProviderRouteRow({
  settings,
  loading,
  error
}: {
  settings: WebToolBindingSettings | null;
  loading: boolean;
  error: string | null;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [route, setRoute] = useState<string[]>([]);
  const [addProvider, setAddProvider] = useState("");
  const [saveRoute, saveResult] = useMutation<
    SaveBrowserProviderRouteMutation,
    SaveBrowserProviderRouteMutationVariables
  >(SaveBrowserProviderRouteDocument, {
    refetchQueries: [{ query: WebToolSettingsDocument }],
    awaitRefetchQueries: true
  });
  const options = useMemo(() => settings?.providerOptions ?? [], [settings]);
  const selectorOptions = useMemo<SelectorOptionType[]>(
    () => options.map((option) => ({ value: option.providerAccountId, label: option.displayName })),
    [options]
  );
  const addOptions = options
    .filter((option) => !route.includes(option.providerAccountId))
    .map((option) => ({ value: option.providerAccountId, label: option.displayName }));
  const openEditor = () => {
    if (!settings) return;
    setRoute([...(settings.providerRouteAccountIds ?? [settings.activeProviderAccountId])]);
    setAddProvider("");
    setIsOpen(true);
  };
  useEffect(() => {
    if (!isOpen) return;
    requestAnimationFrame(() => {
      document.querySelector<HTMLElement>('[aria-label="Preferred browser provider"]')?.focus();
    });
  }, [isOpen]);
  const updatePreferred = (providerAccountId: string) => {
    setRoute((current) => [providerAccountId, ...current.filter((id) => id !== providerAccountId)]);
  };
  const move = (index: number, offset: -1 | 1) => {
    setRoute((current) => {
      const target = index + offset;
      if (target < 0 || target >= current.length) return current;
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  };
  const providerName = (providerAccountId: string) =>
    options.find((option) => option.providerAccountId === providerAccountId)?.displayName ?? "Unavailable provider";
  const summary = loading
    ? "Loading provider route..."
    : error
      ? "Provider route could not be loaded."
      : routeLabel(settings);

  return (
    <>
      <SettingsListItem
        label="Provider route"
        description={summary}
        endContent={
          <SettingsRowActions>
            <Button label="Edit" variant="secondary" size="sm" isDisabled={!settings} onClick={openEditor} />
          </SettingsRowActions>
        }
      />
      <Dialog isOpen={isOpen} onOpenChange={setIsOpen} purpose="form" width={520}>
        <Layout
          header={
            <DialogHeader
              title="Edit browser provider route"
              subtitle="The agent starts with the preferred provider and decides when to switch."
              onOpenChange={setIsOpen}
            />
          }
          content={
            <LayoutContent>
              <VStack gap={4}>
                <Selector
                  label="Preferred browser provider"
                  options={selectorOptions}
                  value={route[0]}
                  onChange={updatePreferred}
                  isDisabled={saveResult.loading || selectorOptions.length === 0}
                />
                <VStack gap={2}>
                  <SettingsList density="compact" hasDividers>
                    {route.map((providerAccountId, index) => (
                      <SettingsListItem
                        key={providerAccountId}
                        label={providerName(providerAccountId)}
                        description={index === 0 ? "Preferred" : `Fallback ${index}`}
                        endContent={
                          <SettingsRowActions>
                            <Button label="Up" variant="ghost" size="sm" icon={<ArrowUp size={14} aria-hidden="true" />} isDisabled={index === 0 || saveResult.loading} onClick={() => move(index, -1)} />
                            <Button label="Down" variant="ghost" size="sm" icon={<ArrowDown size={14} aria-hidden="true" />} isDisabled={index === route.length - 1 || saveResult.loading} onClick={() => move(index, 1)} />
                            <Button label="Remove" variant="ghost" size="sm" icon={<Trash2 size={14} aria-hidden="true" />} isDisabled={route.length === 1 || saveResult.loading} onClick={() => setRoute((current) => current.filter((id) => id !== providerAccountId))} />
                          </SettingsRowActions>
                        }
                      />
                    ))}
                  </SettingsList>
                </VStack>
                <HStack gap={2} vAlign="center">
                  <Selector
                    label="Add provider"
                    options={addOptions}
                    value={addProvider || undefined}
                    placeholder={addOptions.length === 0 ? "All providers added" : "Choose provider"}
                    isDisabled={addOptions.length === 0 || saveResult.loading}
                    onChange={setAddProvider}
                  />
                  <Button
                    label="Add"
                    variant="secondary"
                    icon={<Plus size={14} aria-hidden="true" />}
                    isDisabled={!addProvider || saveResult.loading}
                    onClick={() => {
                      setRoute((current) => [...current, addProvider]);
                      setAddProvider("");
                    }}
                  />
                </HStack>
                <p {...stylex.props(styles.mutedText)}>Each successful switch creates a fresh session. Cookies, storage, history, and element references do not transfer.</p>
                <p {...stylex.props(styles.warningText)}>Hosted providers receive the selected public URL and page activity. Their usage can create provider charges.</p>
                {saveResult.error ? <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the browser provider route.</p> : null}
              </VStack>
            </LayoutContent>
          }
          footer={
            <LayoutFooter>
              <HStack gap={2} hAlign="end">
                <Button label="Cancel" variant="secondary" isDisabled={saveResult.loading} onClick={() => setIsOpen(false)} />
                <Button
                  label="Save"
                  variant="primary"
                  isLoading={saveResult.loading}
                  isDisabled={route.length === 0}
                  onClick={() => {
                    void saveRoute({ variables: { input: { providerAccountIds: route } } }).then(() => setIsOpen(false));
                  }}
                />
              </HStack>
            </LayoutFooter>
          }
        />
      </Dialog>
    </>
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
  const unavailable = !settings;
  return (
    <SettingsListItem
      mobileEndContentFullWidth
      label="Provider"
      description={providerDescription(settings, loading, error)}
      endContent={
        <WebProviderSelect
          settings={settings}
          saving={saving}
          isDisabled={unavailable}
          ariaLabel={settings ? `Provider for ${settings.toolName}` : "Web provider"}
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
    <SettingsRowActions>
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
    </SettingsRowActions>
  );
}

function providerDescription(
  settings: WebToolBindingSettings | null,
  loading: boolean,
  error: string | null
) {
  if (loading) return "Loading provider settings...";
  if (settings) return activeProviderLabel(settings, false, null);
  if (error) return "Provider settings could not be loaded.";
  return activeProviderLabel(settings, false, null);
}

function routeLabel(settings: WebToolBindingSettings | null) {
  if (!settings) return "No provider route configured";
  const labels = (settings.providerRouteAccountIds ?? [settings.activeProviderAccountId]).map(
    (id) => settings.providerOptions.find((option) => option.providerAccountId === id)?.displayName ?? "Unavailable"
  );
  return labels.join(" → ");
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

function providerCapability(
  settings: WebToolBindingSettings | null,
  capability: "citations" | "jsRendering" | "authenticatedContext"
) {
  const option = activeProviderOption(settings);
  if (!option) return "Unavailable";
  return option[capability] ? "Supported" : "Not provided";
}

const styles = stylex.create({
  stack: { minWidth: 0 },
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
  }
});
