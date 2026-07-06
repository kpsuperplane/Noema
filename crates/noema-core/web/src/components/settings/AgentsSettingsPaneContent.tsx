import { useMemo, useState } from "react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Settings2 } from "lucide-react";
import {
  agentBadgeLabel,
  agentDisplayName,
  agentMetadataRows,
  selectedModelWarning
} from "./agentMetadata";

type AgentModelProfileOption = {
  id: string;
  label: string;
  disabledReason?: string | null;
};

type AgentModelProviderOption = {
  providerKind: string;
  providerAccountId: string;
  providerDisplayName: string;
  status: string;
  disabledReason?: string | null;
  profiles: readonly AgentModelProfileOption[];
};

export type AgentSettingsAgent = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
  modelPreference?: {
    providerKind: string;
    providerAccountId: string;
    modelProfile: string;
  } | null;
  modelOptions?: readonly AgentModelProviderOption[];
};

type ModelPreference = {
  providerKind: string;
  providerAccountId: string;
  modelProfile: string;
};

type ModelPreferenceSaveInput = {
  providerAccountId: string;
  modelProfile: string;
};

type SaveAgentModelPreferenceInput = ModelPreferenceSaveInput & {
  agentId: string;
};

export type WebFetchSummarizerSettings = {
  defaultModelProfile: string;
  modelPreference?: ModelPreference | null;
  modelOptions: readonly AgentModelProviderOption[];
};

export function AgentsSettingsPaneContent({
  agents,
  webFetchSummarizer,
  loading,
  error,
  webFetchError,
  saving,
  saveError,
  onSaveModelPreference,
  onSaveWebFetchSummarizerPreference
}: {
  agents: readonly AgentSettingsAgent[];
  webFetchSummarizer: WebFetchSummarizerSettings | null;
  loading: boolean;
  error: string | null;
  webFetchError: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
  onSaveWebFetchSummarizerPreference: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const [editingAgentId, setEditingAgentId] = useState<string | null>(null);
  const [editingWebFetch, setEditingWebFetch] = useState(false);

  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading agents...</p>;
  }

  if (error) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>
          Agent metadata could not be loaded.
        </p>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.list)}>
      {saveError ? (
        <p {...stylex.props(styles.saveError)}>
          Noema could not save the model choice.
        </p>
      ) : null}
      <WebFetchSummarizerCard
        settings={webFetchSummarizer}
        error={webFetchError}
        editing={editingWebFetch}
        saving={saving}
        onToggleEditing={() => setEditingWebFetch((current) => !current)}
        onCancel={() => setEditingWebFetch(false)}
        onSave={async (input) => {
          await onSaveWebFetchSummarizerPreference(input);
          setEditingWebFetch(false);
        }}
      />
      {agents.length === 0 ? (
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>No agents were found.</p>
        </div>
      ) : null}
      {agents.map((agent) => {
        const displayName = agentDisplayName(agent);
        const badgeLabel = agentBadgeLabel(agent);
        const rows = agentMetadataRows(agent);
        const warning = selectedModelWarning(agent);
        const editing = editingAgentId === agent.agentId;
        return (
          <article
            key={agent.agentId}
            {...stylex.props(styles.card)}
          >
            <div {...stylex.props(styles.cardHeader)}>
              <div {...stylex.props(styles.titleRow)}>
                <h2 {...stylex.props(styles.cardTitle)}>
                  {displayName}
                </h2>
                {badgeLabel ? <Badge variant="neutral" label={badgeLabel} /> : null}
              </div>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                label="Model"
                icon={<Settings2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                aria-label={`${editing ? "Close" : "Edit"} model settings for ${displayName}`}
                aria-expanded={editing}
                onClick={() => setEditingAgentId(editing ? null : agent.agentId)}
              />
            </div>
            <dl {...stylex.props(styles.definitionList)}>
              {rows.map((row) => (
                <div
                  key={row.label}
                  {...stylex.props(styles.definitionRow)}
                >
                  <dt {...stylex.props(styles.definitionTerm)}>{row.label}</dt>
                  <dd {...stylex.props(styles.definitionValue)}>
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
            {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
            {editing ? (
              <ModelPreferenceEditor
                options={agent.modelOptions ?? []}
                preference={agent.modelPreference ?? null}
                saving={saving}
                onCancel={() => setEditingAgentId(null)}
                onSave={async (input) => {
                  await onSaveModelPreference({
                    agentId: agent.agentId,
                    ...input
                  });
                  setEditingAgentId(null);
                }}
              />
            ) : null}
          </article>
        );
      })}
    </div>
  );
}

function WebFetchSummarizerCard({
  settings,
  error,
  editing,
  saving,
  onToggleEditing,
  onCancel,
  onSave
}: {
  settings: WebFetchSummarizerSettings | null;
  error: string | null;
  editing: boolean;
  saving: boolean;
  onToggleEditing: () => void;
  onCancel: () => void;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const preference = settings?.modelPreference ?? null;
  const rows = webFetchSummarizerRows(settings);
  const warning = settings ? selectedPreferenceWarning(preference, settings.modelOptions) : null;

  return (
    <article {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.cardHeader)}>
        <div {...stylex.props(styles.titleRow)}>
          <h2 {...stylex.props(styles.cardTitle)}>Web fetch summarizer</h2>
          {!preference && settings ? <Badge variant="neutral" label="Default" /> : null}
        </div>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          label="Model"
          icon={<Settings2 {...stylex.props(styles.icon)} aria-hidden="true" />}
          aria-label={`${editing ? "Close" : "Edit"} model settings for web fetch summarizer`}
          aria-expanded={editing}
          onClick={onToggleEditing}
          isDisabled={!settings}
        />
      </div>
      {error ? (
        <p {...stylex.props(styles.mutedText)}>
          Web fetch summarizer settings could not be loaded.
        </p>
      ) : (
        <>
          <dl {...stylex.props(styles.definitionList)}>
            {rows.map((row) => (
              <div
                key={row.label}
                {...stylex.props(styles.definitionRow)}
              >
                <dt {...stylex.props(styles.definitionTerm)}>{row.label}</dt>
                <dd {...stylex.props(styles.definitionValue)}>
                  {row.value}
                </dd>
              </div>
            ))}
          </dl>
          {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
          {editing && settings ? (
            <ModelPreferenceEditor
              options={settings.modelOptions}
              preference={preference}
              defaultModelProfile={settings.defaultModelProfile}
              saving={saving}
              onCancel={onCancel}
              onSave={onSave}
            />
          ) : null}
        </>
      )}
    </article>
  );
}

function ModelPreferenceEditor({
  options,
  preference,
  defaultModelProfile,
  saving,
  onCancel,
  onSave
}: {
  options: readonly AgentModelProviderOption[];
  preference?: ModelPreference | null;
  defaultModelProfile?: string;
  saving: boolean;
  onCancel: () => void;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const providerOptions = useMemo(() => options, [options]);
  const preferredProvider = preference?.providerAccountId;
  const initialProvider =
    preferredProvider &&
    providerOptions.some((option) => option.providerAccountId === preferredProvider)
      ? preferredProvider
      : (providerOptions[0]?.providerAccountId ?? "");
  const [providerAccountId, setProviderAccountId] = useState(initialProvider);
  const selectedProvider = useMemo(
    () =>
      providerOptions.find((option) => option.providerAccountId === providerAccountId) ??
      providerOptions[0],
    [providerOptions, providerAccountId]
  );
  const preferredProfile = preference?.modelProfile ?? defaultModelProfile;
  const initialProfile =
    preferredProfile &&
    selectedProvider?.profiles.some((profile) => profile.id === preferredProfile)
      ? preferredProfile
      : (selectedProvider?.profiles[0]?.id ?? "");
  const [modelProfile, setModelProfile] = useState(initialProfile);

  const profiles = selectedProvider?.profiles ?? [];
  const selectedProfileAvailable = profiles.some((profile) => profile.id === modelProfile);
  const effectiveProfile = selectedProfileAvailable ? modelProfile : profiles[0]?.id ?? "";
  const selectedProfile = profiles.find((profile) => profile.id === effectiveProfile);
  const providerDisabled = Boolean(selectedProvider?.disabledReason);
  const profileDisabled = Boolean(selectedProfile?.disabledReason);
  const disabledMessages = useMemo(() => {
    const messages: string[] = [];
    for (const option of providerOptions) {
      if (option.disabledReason) {
        messages.push(`${option.providerDisplayName}: ${option.disabledReason}`);
      }
      for (const profile of option.profiles) {
        if (profile.disabledReason && profile.disabledReason !== option.disabledReason) {
          messages.push(
            `${option.providerDisplayName} / ${profile.label}: ${profile.disabledReason}`
          );
        }
      }
    }
    return Array.from(new Set(messages));
  }, [providerOptions]);
  const canSave = Boolean(
    providerAccountId && effectiveProfile && !providerDisabled && !profileDisabled && !saving
  );

  return (
    <form
      {...stylex.props(styles.editor)}
      onSubmit={(event) => {
        event.preventDefault();
        if (canSave) {
          void onSave({
            providerAccountId,
            modelProfile: effectiveProfile
          });
        }
      }}
    >
      <div {...stylex.props(styles.editorGrid)}>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>Provider</span>
          <select
            {...stylex.props(styles.select)}
            value={providerAccountId}
            onChange={(event) => {
              const nextProvider = providerOptions.find(
                (option) => option.providerAccountId === event.target.value
              );
              setProviderAccountId(event.target.value);
              setModelProfile(nextProvider?.profiles[0]?.id ?? "");
            }}
          >
            {providerOptions.length === 0 ? (
              <option value="">No providers available</option>
            ) : (
              providerOptions.map((option) => (
                <option
                  key={option.providerAccountId}
                  value={option.providerAccountId}
                  disabled={Boolean(option.disabledReason)}
                >
                  {option.providerDisplayName}
                </option>
              ))
            )}
          </select>
        </label>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>Model</span>
          <select
            {...stylex.props(styles.select)}
            value={effectiveProfile}
            onChange={(event) => setModelProfile(event.target.value)}
            disabled={profiles.length === 0}
          >
            {profiles.length === 0 ? (
              <option value="">No models available</option>
            ) : (
              profiles.map((profile) => (
                <option
                  key={profile.id}
                  value={profile.id}
                  disabled={Boolean(profile.disabledReason)}
                >
                  {profile.label}
                </option>
              ))
            )}
          </select>
        </label>
      </div>
      {disabledMessages.length > 0 ? (
        <ul {...stylex.props(styles.warningList)}>
          {disabledMessages.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      ) : null}
      <div {...stylex.props(styles.actions)}>
        <Button type="button" variant="ghost" size="sm" label="Cancel" onClick={onCancel} />
        <Button
          type="submit"
          size="sm"
          label="Save"
          isDisabled={!canSave}
          isLoading={saving}
        />
      </div>
    </form>
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

function providerPreferenceLabel(
  preference: ModelPreference,
  options: readonly AgentModelProviderOption[]
) {
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  return provider?.providerDisplayName ?? preference.providerKind;
}

function modelPreferenceLabel(
  preference: ModelPreference,
  options: readonly AgentModelProviderOption[]
) {
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  return profile?.label ?? preference.modelProfile;
}

function modelProfileLabel(
  modelProfile: string,
  options: readonly AgentModelProviderOption[]
) {
  for (const option of options) {
    const profile = option.profiles.find((candidate) => candidate.id === modelProfile);
    if (profile) {
      return profile.label;
    }
  }
  return modelProfile;
}

function selectedPreferenceWarning(
  preference: ModelPreference | null,
  options: readonly AgentModelProviderOption[]
) {
  if (!preference) {
    return null;
  }
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  if (!provider) {
    return "Selected provider is not available.";
  }
  if (!profile) {
    return "Selected model is not available.";
  }
  return profile.disabledReason ?? provider.disabledReason ?? null;
}

const styles = stylex.create({
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  list: {
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
  warningText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "rgb(180, 83, 9)"
  },
  editor: {
    display: "grid",
    gap: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  editorGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: 12,
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr"
    }
  },
  field: {
    display: "grid",
    gap: 4,
    fontSize: 14,
    lineHeight: 1.5
  },
  fieldLabel: {
    fontWeight: 500,
    color: "var(--foreground)"
  },
  select: {
    height: 36,
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingInline: 8,
    fontSize: 14
  },
  warningList: {
    display: "grid",
    gap: 4,
    margin: 0,
    paddingLeft: 16,
    fontSize: 14,
    lineHeight: 1.5,
    color: "rgb(180, 83, 9)"
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: 8
  },
  icon: {
    width: 16,
    height: 16
  }
});
