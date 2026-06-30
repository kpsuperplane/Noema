import { RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { McpApprovalSettingsQuery } from "@/generated/graphql";

type McpApprovalRequest = McpApprovalSettingsQuery["mcpApprovalRequests"][number];

type ApprovalsSettingsPaneContentProps = {
  approvals?: readonly McpApprovalRequest[];
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
};

export function ApprovalsSettingsPaneContent({
  approvals = [],
  loading = false,
  error = null,
  onRetry = () => {}
}: ApprovalsSettingsPaneContentProps = {}) {
  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading MCP approvals...</p>;
  }

  if (error) {
    return (
      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          MCP approval requests could not be loaded.
        </p>
        <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
          <RefreshCw className="size-4" aria-hidden="true" />
          Retry
        </Button>
      </div>
    );
  }

  if (approvals.length === 0) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">No MCP approvals are pending.</p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-md border border-[var(--border-subtle)] bg-white">
      <div className="grid min-w-0 divide-y divide-[var(--border-subtle)]">
        {approvals.map((approval) => (
          <article key={approval.approvalId} className="grid gap-3 p-4">
            <div className="flex min-w-0 flex-wrap items-center gap-3">
              <h2 className="m-0 min-w-0 break-words font-heading text-lg leading-tight tracking-normal text-foreground">
                {approval.actionSummary}
              </h2>
              <Badge variant="outline">{approval.status}</Badge>
            </div>
            <dl className="m-0 grid gap-2">
              {approvalRows(approval).map((row) => (
                <div
                  key={row.label}
                  className="grid grid-cols-[minmax(110px,160px)_1fr] gap-4 max-[760px]:grid-cols-1 max-[760px]:gap-1"
                >
                  <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
                  <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
          </article>
        ))}
      </div>
    </div>
  );
}

function approvalRows(approval: McpApprovalRequest): { label: string; value: string }[] {
  return [
    { label: "Destination", value: approval.destinationSummary },
    { label: "Data source", value: approval.dataSourceSummary },
    { label: "Export", value: approval.exportSummary },
    {
      label: "Source owner",
      value: `${approval.sourceOwnerIdentity} (${approval.sourceOwnerTrust})`
    },
    {
      label: "Destination owner",
      value: `${approval.destinationOwnerIdentity} (${approval.destinationOwnerTrust})`
    },
    { label: "Scope", value: approval.activeScopeId },
    { label: "Requester", value: approval.requesterActorId },
    { label: "Owner scope", value: approval.ownerScopeId },
    { label: "Invocation", value: approval.toolInvocationId ?? "Unlinked" },
    { label: "Tool", value: approval.mcpToolId ?? "Unknown tool" },
    { label: "Server", value: approval.mcpServerId ?? "Unknown server" },
    { label: "Payload", value: formatPayloadPreview(approval.payloadPreview) }
  ];
}

function formatPayloadPreview(value: unknown): string {
  if (value === null || value === undefined) {
    return "None";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  try {
    return JSON.stringify(value);
  } catch {
    return "Unavailable";
  }
}
