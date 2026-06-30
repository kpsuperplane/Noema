import * as React from "react";
import { RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  mcpEnabledLabel,
  mcpMetadataRows,
  type McpSettingsServer
} from "./mcpMetadata";
import {
  McpServerSetupFlow,
  type McpServerSetupResult
} from "./McpServerSetupFlow";
import type { McpSetupContinueSubmission, McpSetupFormSubmission } from "./mcpSetupForm";

export function McpSettingsPaneContent({
  servers,
  loading,
  error,
  setupResult = null,
  setupSubmitting = false,
  setupError = null,
  onCreateServer = () => {},
  onContinueSetup = () => {},
  onRetry
}: {
  servers: readonly McpSettingsServer[];
  loading: boolean;
  error: string | null;
  setupResult?: McpServerSetupResult | null;
  setupSubmitting?: boolean;
  setupError?: string | null;
  onCreateServer?: (input: McpSetupFormSubmission) => void;
  onContinueSetup?: (input: McpSetupContinueSubmission) => void;
  onRetry: () => void;
}) {
  const [setupOpen, setSetupOpen] = React.useState(false);

  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading MCP servers...</p>;
  }

  if (error) {
    return (
      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          MCP server metadata could not be loaded.
        </p>
        <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
          <RefreshCw className="size-4" aria-hidden="true" />
          Retry
        </Button>
      </div>
    );
  }

  if (servers.length === 0) {
    return (
      <div className="grid gap-4">
        <McpServerSetupFlow
          setupResult={setupResult}
          setupSubmitting={setupSubmitting}
          setupError={setupError}
          onCreateServer={onCreateServer}
          onContinueSetup={onContinueSetup}
        />
        <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
          <p className="m-0 text-sm text-muted-foreground">No MCP servers are configured.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      {setupResult || setupOpen ? (
        <McpServerSetupFlow
          setupResult={setupResult}
          setupSubmitting={setupSubmitting}
          setupError={setupError}
          onCreateServer={onCreateServer}
          onContinueSetup={onContinueSetup}
        />
      ) : (
        <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
          <Button type="button" variant="outline" onClick={() => setSetupOpen(true)}>
            Add MCP server
          </Button>
        </div>
      )}
      {servers.map((server) => {
        const rows = mcpMetadataRows(server);
        return (
          <article
            key={server.mcpServerId}
            className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4"
          >
            <div className="flex flex-wrap items-center gap-3">
              <h2 className="m-0 min-w-0 font-heading text-xl leading-tight tracking-normal text-foreground">
                {server.displayName}
              </h2>
              <Badge variant="outline">{mcpEnabledLabel(server)}</Badge>
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
