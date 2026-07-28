import * as React from "react";
import { KeyRound, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import * as stylex from "@stylexjs/stylex";
import type { McpSettingsServer } from "./mcpMetadata";
import {
  CapabilityIntegrationList,
  type CapabilityIntegration
} from "./CapabilityIntegrationList";
import {
  McpServerSetupFlow,
  type McpServerSetupResult
} from "./McpServerSetupFlow";
import { McpServerReauthenticationDialog } from "./McpServerReauthenticationDialog";
import { McpAddConnectionDialog } from "./McpAddConnectionDialog";
import { CapabilityManagementLayout } from "./CapabilityManagementLayout";
import { DeleteConnectionDialog } from "./DeleteConnectionDialog";
import type { McpSetupContinueSubmission, McpSetupFormSubmission } from "./mcpSetupForm";

export function McpSettingsPaneContent({
  servers,
  integrations,
  selectedConnectionId,
  loading,
  error,
  setupResult = null,
  setupOpen = false,
  setupSubmitting = false,
  oauthSubmitting = false,
  setupError = null,
  reauthServerId = null,
  reauthSubmitting = false,
  reauthOauthSubmitting = false,
  reauthError = null,
  reauthResult = null,
  deleteSubmitting = false,
  deleteError = null,
  onOpenSetup = () => {},
  onCloseSetup = () => {},
  onCreateServer = () => {},
  onStartOAuth = () => {},
  onOpenReauth = () => {},
  onCloseReauth = () => {},
  onContinueServerSetup = () => {},
  onStartReauthenticationOAuth = () => {},
  onConnectionAdded = () => {},
  onDeleteServer = async () => false,
  onRetry
}: {
  servers: readonly McpSettingsServer[];
  integrations: readonly CapabilityIntegration[];
  selectedConnectionId?: string;
  loading: boolean;
  error: string | null;
  setupResult?: McpServerSetupResult | null;
  setupOpen?: boolean;
  setupSubmitting?: boolean;
  oauthSubmitting?: boolean;
  setupError?: string | null;
  reauthServerId?: string | null;
  reauthSubmitting?: boolean;
  reauthOauthSubmitting?: boolean;
  reauthError?: string | null;
  reauthResult?: McpServerSetupResult | null;
  deleteSubmitting?: boolean;
  deleteError?: string | null;
  onOpenSetup?: () => void;
  onCloseSetup?: () => void;
  onCreateServer?: (input: McpSetupFormSubmission) => void;
  onStartOAuth?: (input: McpSetupFormSubmission) => void;
  onOpenReauth?: (mcpServerId: string) => void;
  onCloseReauth?: () => void;
  onContinueServerSetup?: (input: McpSetupContinueSubmission) => void;
  onStartReauthenticationOAuth?: (mcpServerId: string) => void;
  onConnectionAdded?: () => void;
  onDeleteServer?: (mcpServerId: string) => Promise<boolean>;
  onRetry: () => void;
}) {
  const [deleteTargetId, setDeleteTargetId] = React.useState<string | null>(null);
  const [addTargetId, setAddTargetId] = React.useState<string | null>(null);
  const reauthServer =
    servers.find((server) => server.mcpServerId === reauthServerId) ?? null;
  const selectedServer =
    servers.find((server) => server.mcpServerId === selectedConnectionId) ?? null;
  const deleteTarget = servers.find((server) => server.mcpServerId === deleteTargetId) ?? null;
  const addTarget = integrations.find((integration) => integration.definitionId === addTargetId) ?? null;

  if (loading) {
    return <p {...stylex.props(styles.mutedText, styles.pageState)}>Loading connections...</p>;
  }

  if (error) {
    return (
      <div {...stylex.props(styles.pageState)}>
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>
            Couldn't load connections.
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
      </div>
    );
  }

  return (
    <>
      <CapabilityManagementLayout
        kind="MCP"
        connectionId={selectedConnectionId}
        list={
          <div {...stylex.props(styles.list)}>
            <CapabilityIntegrationList
              integrations={integrations}
              kind="MCP"
              selectedConnectionId={selectedConnectionId}
              emptyMessage="No services connected."
              primaryAction={{ label: "Connect service", onClick: onOpenSetup }}
              onAddConnection={(integration) => setAddTargetId(integration.definitionId)}
            />
          </div>
        }
        sourceActions={selectedServer && mcpServerNeedsReauth(selectedServer) ? (
          <Button
            type="button"
            variant="secondary"
            label="Reconnect"
            icon={<KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />}
            onClick={() => onOpenReauth(selectedServer.mcpServerId)}
          />
        ) : null}
        dangerAction={selectedServer ? (
          <Button
            type="button"
            variant="destructive"
            label="Delete connection"
            icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
            isDisabled={deleteSubmitting}
            onClick={() => setDeleteTargetId(selectedServer.mcpServerId)}
          />
        ) : null}
      />
      <McpAddConnectionDialog
        key={addTarget?.definitionId ?? "mcp-add-connection"}
        integration={addTarget}
        open={addTarget !== null}
        onAdded={onConnectionAdded}
        onOpenChange={(open) => {
          if (!open) setAddTargetId(null);
        }}
      />
      <McpSetupDialog
        open={setupOpen}
        setupResult={setupResult}
        setupSubmitting={setupSubmitting}
        oauthSubmitting={oauthSubmitting}
        setupError={setupError}
        onOpenChange={(open) => {
          if (!open) onCloseSetup();
        }}
        onCreateServer={onCreateServer}
        onStartOAuth={onStartOAuth}
      />
      <McpServerReauthenticationDialog
        key={reauthServer?.mcpServerId ?? "mcp-reauthentication"}
        server={reauthServer}
        open={reauthServer !== null}
        submitting={reauthSubmitting}
        oauthSubmitting={reauthOauthSubmitting}
        error={reauthError}
        setupResult={reauthResult}
        onOpenChange={(open) => {
          if (!open) onCloseReauth();
        }}
        onSubmit={onContinueServerSetup}
        onStartBrowserOAuth={onStartReauthenticationOAuth}
      />
      <DeleteConnectionDialog
        connection={deleteTarget ? {
          name: deleteTarget.displayName,
          toolCount: deleteTarget.toolCount
        } : null}
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
    </>
  );
}

function mcpServerNeedsReauth(server: McpSettingsServer) {
  return server.healthStatus === "unavailable" || !["none", "authenticated"].includes(server.authStatus);
}

const styles = stylex.create({
  pageState: {
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
    }
  },
  mutedText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  list: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  card: {
    display: "grid",
    gap: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--surface-raised)",
    padding: "var(--spacing-4)"
  },
  titleRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: "var(--spacing-3)"
  },
  cardTitle: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    gap: "var(--spacing-2)"
  },
  definitionList: {
    display: "grid",
    gap: "var(--spacing-2)",
    margin: "var(--spacing-0)"
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(120px, 180px) 1fr",
    gap: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr",
      gap: "var(--spacing-1)"
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
    margin: "var(--spacing-0)",
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

function McpSetupDialog({
  open,
  setupResult,
  setupSubmitting,
  oauthSubmitting,
  setupError,
  onOpenChange,
  onCreateServer,
  onStartOAuth
}: {
  open: boolean;
  setupResult: McpServerSetupResult | null;
  setupSubmitting: boolean;
  oauthSubmitting: boolean;
  setupError: string | null;
  onOpenChange: (open: boolean) => void;
  onCreateServer: (input: McpSetupFormSubmission) => void;
  onStartOAuth: (input: McpSetupFormSubmission) => void;
}) {
  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={680}
      maxHeight="calc(100dvh - var(--spacing-8))"
      aria-label="Connect a service"
    >
      <Layout
        header={
          <DialogHeader
            title="Connect a service"
            subtitle="Use the details provided by the service."
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <McpServerSetupFlow
              setupResult={setupResult}
              setupSubmitting={setupSubmitting}
              oauthSubmitting={oauthSubmitting}
              setupError={setupError}
              onCreateServer={onCreateServer}
              onStartOAuth={onStartOAuth}
            />
          </LayoutContent>
        }
      />
    </Dialog>
  );
}
