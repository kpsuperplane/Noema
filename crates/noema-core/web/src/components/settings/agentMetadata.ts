type AgentLike = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
};

export type AgentMetadataRow = {
  label: string;
  value: string;
};

export function agentDisplayName(agent: Pick<AgentLike, "displayName">) {
  const displayName = agent.displayName?.trim();
  return displayName ? displayName : "Unnamed agent";
}

export function agentBadgeLabel(agent: Pick<AgentLike, "isPrimary">) {
  return agent.isPrimary ? "Primary" : null;
}

export function agentMetadataRows(agent: Pick<AgentLike, "agentId">): AgentMetadataRow[] {
  return [{ label: "Agent id", value: agent.agentId }];
}
