import * as React from "react";
import { AlertTriangle, RefreshCw, Settings2, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog";
import {
  mcpEnabledLabel,
  mcpMetadataRows,
  mcpToolCountLabel,
  type McpSettingsServer
} from "./mcpMetadata";
import {
  McpServerSetupFlow,
  type McpServerSetupResult
} from "./McpServerSetupFlow";
import { McpToolPermissionsModal } from "./McpToolPermissionsModal";
import type { McpSetupFormSubmission } from "./mcpSetupForm";

export function McpSettingsPaneContent({
  servers,
  loading,
  error,
  setupResult = null,
  setupOpen = false,
  setupSubmitting = false,
  setupError = null,
  permissionsServerId = null,
  deleteSubmitting = false,
  deleteError = null,
  onOpenSetup = () => {},
  onCloseSetup = () => {},
  onCreateServer = () => {},
  onOpenPermissions = () => {},
  onClosePermissions = () => {},
  onDeleteServer = async () => false,
  onRetry
}: {
  servers: readonly McpSettingsServer[];
  loading: boolean;
  error: string | null;
  setupResult?: McpServerSetupResult | null;
  setupOpen?: boolean;
  setupSubmitting?: boolean;
  setupError?: string | null;
  permissionsServerId?: string | null;
  deleteSubmitting?: boolean;
  deleteError?: string | null;
  onOpenSetup?: () => void;
  onCloseSetup?: () => void;
  onCreateServer?: (input: McpSetupFormSubmission) => void;
  onOpenPermissions?: (mcpServerId: string) => void;
  onClosePermissions?: () => void;
  onDeleteServer?: (mcpServerId: string) => Promise<boolean>;
  onRetry: () => void;
}) {
  const [deleteTargetId, setDeleteTargetId] = React.useState<string | null>(null);
  const selectedPermissionsServer =
    servers.find((server) => server.mcpServerId === permissionsServerId) ?? null;
  const deleteTarget = servers.find((server) => server.mcpServerId === deleteTargetId) ?? null;

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
        <Button type="button" variant="outline" className="w-fit" onClick={onOpenSetup}>
          Add MCP server
        </Button>
        <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
          <p className="m-0 text-sm text-muted-foreground">No MCP servers are configured.</p>
        </div>
        <McpSetupDialog
          open={setupOpen}
          setupResult={setupResult}
          setupSubmitting={setupSubmitting}
          setupError={setupError}
          onOpenChange={(open) => {
            if (!open) onCloseSetup();
          }}
          onCreateServer={onCreateServer}
        />
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <Button type="button" variant="outline" onClick={onOpenSetup}>
          Add MCP server
        </Button>
      </div>
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
            <div className="flex flex-wrap gap-2">
              <Button type="button" variant="outline" onClick={() => onOpenPermissions(server.mcpServerId)}>
                <Settings2 className="size-4" aria-hidden="true" />
                Configure tools
              </Button>
              <Button
                type="button"
                variant="destructive"
                disabled={deleteSubmitting}
                onClick={() => setDeleteTargetId(server.mcpServerId)}
              >
                <Trash2 className="size-4" aria-hidden="true" />
                Delete
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
          </article>
        );
      })}
      <McpSetupDialog
        open={setupOpen}
        setupResult={setupResult}
        setupSubmitting={setupSubmitting}
        setupError={setupError}
        onOpenChange={(open) => {
          if (!open) onCloseSetup();
        }}
        onCreateServer={onCreateServer}
      />
      {permissionsServerId ? (
        <McpToolPermissionsModal
          open
          serverId={permissionsServerId}
          serverName={selectedPermissionsServer?.displayName ?? null}
          onOpenChange={(open) => {
            if (!open) onClosePermissions();
          }}
        />
      ) : null}
      <DeleteMcpServerDialog
        server={deleteTarget}
        open={deleteTarget !== null}
        submitting={deleteSubmitting}
        error={deleteError}
        onOpenChange={(open) => {
          if (!open && !deleteSubmitting) setDeleteTargetId(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget) return;
          const deleted = await onDeleteServer(deleteTarget.mcpServerId);
          if (deleted) setDeleteTargetId(null);
        }}
      />
    </div>
  );
}

function DeleteMcpServerDialog({
  server,
  open,
  submitting,
  error,
  onOpenChange,
  onConfirm
}: {
  server: McpSettingsServer | null;
  open: boolean;
  submitting: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Delete MCP server</DialogTitle>
          <DialogDescription>
            {server
              ? `Delete ${server.displayName}, its stored secrets, ${mcpToolCountLabel(
                  server.toolCount
                )}, and any saved tool calibration.`
              : "Delete this MCP server and its stored secrets."}
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="grid gap-3">
          <p className="m-0 flex items-start gap-2 text-sm text-muted-foreground">
            <AlertTriangle className="mt-0.5 size-4 text-destructive" aria-hidden="true" />
            This cannot be undone from Settings. Historical approval and audit records are kept.
          </p>
          {error ? <p className="m-0 text-sm text-destructive">{error}</p> : null}
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="destructive"
              disabled={submitting}
              onClick={onConfirm}
            >
              <Trash2 className="size-4" aria-hidden="true" />
              Delete server
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={submitting}
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
          </div>
        </DialogBody>
      </DialogContent>
    </Dialog>
  );
}

function McpSetupDialog({
  open,
  setupResult,
  setupSubmitting,
  setupError,
  onOpenChange,
  onCreateServer
}: {
  open: boolean;
  setupResult: McpServerSetupResult | null;
  setupSubmitting: boolean;
  setupError: string | null;
  onOpenChange: (open: boolean) => void;
  onCreateServer: (input: McpSetupFormSubmission) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add MCP server</DialogTitle>
          <DialogDescription>Verify connection and discover tools.</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <McpServerSetupFlow
            setupResult={setupResult}
            setupSubmitting={setupSubmitting}
            setupError={setupError}
            onCreateServer={onCreateServer}
          />
        </DialogBody>
      </DialogContent>
    </Dialog>
  );
}
