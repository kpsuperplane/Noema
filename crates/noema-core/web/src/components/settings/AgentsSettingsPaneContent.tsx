import { useMemo, useState } from "react";
import { Settings2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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

type SaveAgentModelPreferenceInput = {
  agentId: string;
  providerAccountId: string;
  modelProfile: string;
};

export function AgentsSettingsPaneContent({
  agents,
  loading,
  error,
  saving,
  saveError,
  onSaveModelPreference
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  const [editingAgentId, setEditingAgentId] = useState<string | null>(null);

  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading agents...</p>;
  }

  if (error) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          Agent metadata could not be loaded.
        </p>
      </div>
    );
  }

  if (agents.length === 0) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">No agents were found.</p>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      {saveError ? (
        <p className="m-0 rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive">
          Noema could not save the model choice.
        </p>
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
            className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4"
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex flex-wrap items-center gap-3">
                <h2 className="m-0 font-heading text-xl leading-tight tracking-normal text-foreground">
                  {displayName}
                </h2>
                {badgeLabel ? <Badge variant="outline">{badgeLabel}</Badge> : null}
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-label={`${editing ? "Close" : "Edit"} model settings for ${displayName}`}
                aria-expanded={editing}
                onClick={() => setEditingAgentId(editing ? null : agent.agentId)}
              >
                <Settings2 className="size-4" aria-hidden="true" />
                Model
              </Button>
            </div>
            <dl className="m-0 grid gap-2">
              {rows.map((row) => (
                <div
                  key={row.label}
                  className="grid grid-cols-[minmax(120px,180px)_1fr] gap-4 max-[760px]:grid-cols-1 max-[760px]:gap-1"
                >
                  <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
                  <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
            {warning ? <p className="m-0 text-sm text-amber-700">{warning}</p> : null}
            {editing ? (
              <AgentModelPreferenceEditor
                agent={agent}
                saving={saving}
                onCancel={() => setEditingAgentId(null)}
                onSave={async (input) => {
                  await onSaveModelPreference(input);
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

function AgentModelPreferenceEditor({
  agent,
  saving,
  onCancel,
  onSave
}: {
  agent: AgentSettingsAgent;
  saving: boolean;
  onCancel: () => void;
  onSave: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  const options = useMemo(() => agent.modelOptions ?? [], [agent.modelOptions]);
  const preferredProvider = agent.modelPreference?.providerAccountId;
  const initialProvider =
    preferredProvider && options.some((option) => option.providerAccountId === preferredProvider)
      ? preferredProvider
      : (options[0]?.providerAccountId ?? "");
  const [providerAccountId, setProviderAccountId] = useState(initialProvider);
  const selectedProvider = useMemo(
    () => options.find((option) => option.providerAccountId === providerAccountId) ?? options[0],
    [options, providerAccountId]
  );
  const preferredProfile = agent.modelPreference?.modelProfile;
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
    for (const option of options) {
      if (option.disabledReason) {
        messages.push(`${option.providerDisplayName}: ${option.disabledReason}`);
      }
      for (const profile of option.profiles) {
        if (profile.disabledReason && profile.disabledReason !== option.disabledReason) {
          messages.push(`${option.providerDisplayName} / ${profile.label}: ${profile.disabledReason}`);
        }
      }
    }
    return Array.from(new Set(messages));
  }, [options]);
  const canSave = Boolean(
    providerAccountId && effectiveProfile && !providerDisabled && !profileDisabled && !saving
  );

  return (
    <form
      className="grid gap-3 border-t border-[var(--border-subtle)] pt-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (canSave) {
          void onSave({
            agentId: agent.agentId,
            providerAccountId,
            modelProfile: effectiveProfile
          });
        }
      }}
    >
      <div className="grid grid-cols-2 gap-3 max-[760px]:grid-cols-1">
        <label className="grid gap-1 text-sm">
          <span className="font-medium text-foreground">Provider</span>
          <select
            className="h-9 min-w-0 rounded-md border border-[var(--border-subtle)] bg-white px-2 text-sm"
            value={providerAccountId}
            onChange={(event) => {
              const nextProvider = options.find(
                (option) => option.providerAccountId === event.target.value
              );
              setProviderAccountId(event.target.value);
              setModelProfile(nextProvider?.profiles[0]?.id ?? "");
            }}
          >
            {options.length === 0 ? (
              <option value="">No providers available</option>
            ) : (
              options.map((option) => (
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
        <label className="grid gap-1 text-sm">
          <span className="font-medium text-foreground">Model</span>
          <select
            className="h-9 min-w-0 rounded-md border border-[var(--border-subtle)] bg-white px-2 text-sm"
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
        <ul className="m-0 grid gap-1 pl-4 text-sm text-amber-700">
          {disabledMessages.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      ) : null}
      <div className="flex flex-wrap justify-end gap-2">
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" size="sm" disabled={!canSave}>
          Save
        </Button>
      </div>
    </form>
  );
}
