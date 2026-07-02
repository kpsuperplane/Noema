import { RefreshCw } from "lucide-react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
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
    return <p {...stylex.props(styles.mutedText)}>Loading MCP approvals...</p>;
  }

  if (error) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>
          MCP approval requests could not be loaded.
        </p>
        <Button
          type="button"
          variant="secondary"
          label="Retry"
          icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
          onClick={onRetry}
        />
      </div>
    );
  }

  if (approvals.length === 0) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>No MCP approvals are pending.</p>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.panel)}>
      <div {...stylex.props(styles.list)}>
        {approvals.map((approval) => (
          <article key={approval.approvalId} {...stylex.props(styles.item)}>
            <div {...stylex.props(styles.titleRow)}>
              <h2 {...stylex.props(styles.title)}>
                {approval.actionSummary}
              </h2>
              <Badge variant="neutral" label={approval.status} />
            </div>
            <dl {...stylex.props(styles.definitionList)}>
              {approvalRows(approval).map((row) => (
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
    { label: "Invocation", value: approval.toolInvocationId },
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

const styles = stylex.create({
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
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
  panel: {
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white"
  },
  list: {
    display: "grid",
    minWidth: 0
  },
  item: {
    display: "grid",
    gap: 12,
    padding: 16,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    ":first-child": {
      borderTopWidth: 0
    }
  },
  titleRow: {
    display: "flex",
    minWidth: 0,
    flexWrap: "wrap",
    alignItems: "center",
    gap: 12
  },
  title: {
    minWidth: 0,
    margin: 0,
    overflowWrap: "break-word",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
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
    gridTemplateColumns: "minmax(110px, 160px) 1fr",
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
  fitButton: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  }
});
