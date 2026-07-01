type AgentLike = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean | null;
  modelPreference?: {
    providerKind: string;
    providerAccountId: string;
    modelProfile: string;
  } | null;
  modelOptions?: readonly {
    providerKind: string;
    providerAccountId: string;
    providerDisplayName: string;
    status: string;
    disabledReason?: string | null;
    profiles: readonly {
      id: string;
      label: string;
      disabledReason?: string | null;
    }[];
  }[];
};

export type AgentMetadataRow = {
  label: string;
  value: string;
};

export function agentDisplayName(agent: Pick<AgentLike, "displayName">) {
  return agent.displayName?.trim() || "Unnamed agent";
}

export function agentBadgeLabel(agent: Pick<AgentLike, "isPrimary">) {
  return agent.isPrimary ? "Primary" : null;
}

export function selectedModelLabel(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return "System default";
  }
  const provider = agent.modelOptions?.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  return profile?.label ?? preference.modelProfile;
}

export function selectedProviderLabel(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return "System default";
  }
  const provider = agent.modelOptions?.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  return provider?.providerDisplayName ?? preference.providerKind;
}

export function selectedModelWarning(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return null;
  }
  const provider = agent.modelOptions?.find(
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

export function agentMetadataRows(agent: AgentLike): AgentMetadataRow[] {
  return [
    { label: "Agent id", value: agent.agentId },
    { label: "Provider", value: selectedProviderLabel(agent) },
    { label: "Model", value: selectedModelLabel(agent) }
  ];
}
