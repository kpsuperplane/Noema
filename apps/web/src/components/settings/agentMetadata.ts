type AgentLike = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean | null;
  modelPreference?: {
    providerKind: string;
    providerAccountId: string;
    selectionMode: string;
    modelProfile?: string | null;
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
    recommendations: readonly {
      useCase: string;
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

export function selectedModelWarning(agent: AgentLike) {
  const preference = agent.modelPreference;
  if (!preference) {
    return null;
  }
  const provider = agent.modelOptions?.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  if (!provider) {
    return "Selected provider is not available.";
  }
  if (preference.selectionMode === "NOEMA_RECOMMENDED") {
    const useCase = agent.isPrimary ? "PRIMARY" : "TASK_REVIEWER";
    const recommendation = provider.recommendations.find(
      (candidate) => candidate.useCase === useCase
    );
    return recommendation?.disabledReason ?? provider.disabledReason ??
      (recommendation ? null : "Noema has no recommendation for this agent.");
  }
  const profile = provider.profiles.find((candidate) => candidate.id === preference.modelProfile);
  if (!profile) {
    return "Selected model is not available.";
  }
  return profile.disabledReason ?? provider.disabledReason ?? null;
}

export function agentMetadataRows(agent: AgentLike): AgentMetadataRow[] {
  return [
    { label: "Agent id", value: agent.agentId }
  ];
}
