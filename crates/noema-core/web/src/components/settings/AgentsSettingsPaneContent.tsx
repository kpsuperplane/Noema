import { Badge } from "@/components/ui/badge";
import { agentBadgeLabel, agentDisplayName, agentMetadataRows } from "./agentMetadata";

export type AgentSettingsAgent = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
};

export function AgentsSettingsPaneContent({
  agents,
  loading,
  error
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
}) {
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
      {agents.map((agent) => {
        const badgeLabel = agentBadgeLabel(agent);
        const rows = agentMetadataRows(agent);
        return (
          <article
            key={agent.agentId}
            className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4"
          >
            <div className="flex flex-wrap items-center gap-3">
              <h2 className="m-0 font-heading text-xl leading-tight tracking-normal text-foreground">
                {agentDisplayName(agent)}
              </h2>
              {badgeLabel ? <Badge variant="outline">{badgeLabel}</Badge> : null}
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
          </article>
        );
      })}
    </div>
  );
}
