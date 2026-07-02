import * as React from "react";
import { AlertTriangle, RefreshCw, Settings2, Trash2 } from "lucide-react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
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
  oauthSubmitting = false,
  setupError = null,
  permissionsServerId = null,
  autoAutofillServerId = null,
  deleteSubmitting = false,
  deleteError = null,
  onOpenSetup = () => {},
  onCloseSetup = () => {},
  onCreateServer = () => {},
  onStartOAuth = () => {},
  onOpenPermissions = () => {},
  onClosePermissions = () => {},
  onAutoAutofillComplete = () => {},
  onDeleteServer = async () => false,
  onRetry
}: {
  servers: readonly McpSettingsServer[];
  loading: boolean;
  error: string | null;
  setupResult?: McpServerSetupResult | null;
  setupOpen?: boolean;
  setupSubmitting?: boolean;
  oauthSubmitting?: boolean;
  setupError?: string | null;
  permissionsServerId?: string | null;
  autoAutofillServerId?: string | null;
  deleteSubmitting?: boolean;
  deleteError?: string | null;
  onOpenSetup?: () => void;
  onCloseSetup?: () => void;
  onCreateServer?: (input: McpSetupFormSubmission) => void;
  onStartOAuth?: (input: McpSetupFormSubmission) => void;
  onOpenPermissions?: (mcpServerId: string) => void;
  onClosePermissions?: () => void;
  onAutoAutofillComplete?: () => void;
  onDeleteServer?: (mcpServerId: string) => Promise<boolean>;
  onRetry: () => void;
}) {
  const [deleteTargetId, setDeleteTargetId] = React.useState<string | null>(null);
  const selectedPermissionsServer =
    servers.find((server) => server.mcpServerId === permissionsServerId) ?? null;
  const deleteTarget = servers.find((server) => server.mcpServerId === deleteTargetId) ?? null;

  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading MCP servers...</p>;
  }

  if (error) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>
          MCP server metadata could not be loaded.
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

  if (servers.length === 0) {
    return (
      <div {...stylex.props(styles.list)}>
        <Button
          type="button"
          variant="secondary"
          label="Add MCP server"
          {...stylex.props(styles.fitButton)}
          onClick={onOpenSetup}
        />
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>No MCP servers are configured.</p>
        </div>
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
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.list)}>
      <div {...stylex.props(styles.card)}>
        <Button type="button" variant="secondary" label="Add MCP server" onClick={onOpenSetup} />
      </div>
      {servers.map((server) => {
        const rows = mcpMetadataRows(server);
        return (
          <article
            key={server.mcpServerId}
            {...stylex.props(styles.card)}
          >
            <div {...stylex.props(styles.titleRow)}>
              <h2 {...stylex.props(styles.cardTitle)}>
                {server.displayName}
              </h2>
              <Badge variant="neutral" label={mcpEnabledLabel(server)} />
            </div>
            <div {...stylex.props(styles.actions)}>
              <Button
                type="button"
                variant="secondary"
                label="Configure tools"
                icon={<Settings2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                onClick={() => onOpenPermissions(server.mcpServerId)}
              />
              <Button
                type="button"
                variant="destructive"
                label="Delete"
                icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                isDisabled={deleteSubmitting}
                onClick={() => setDeleteTargetId(server.mcpServerId)}
              />
            </div>
            <dl {...stylex.props(styles.definitionList)}>
              {rows.map((row) => (
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
        );
      })}
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
      {permissionsServerId ? (
        <McpToolPermissionsModal
          open
          serverId={permissionsServerId}
          serverName={selectedPermissionsServer?.displayName ?? null}
          autoAutofill={permissionsServerId === autoAutofillServerId}
          onAutoAutofillComplete={onAutoAutofillComplete}
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
  const subtitle = server
    ? `Delete ${server.displayName}, its stored secrets, ${mcpToolCountLabel(
        server.toolCount
      )}, and any saved tool calibration.`
    : "Delete this MCP server and its stored secrets.";

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={520}
      aria-label="Delete MCP server"
    >
      <div {...stylex.props(styles.dialog)}>
        <DialogHeader
          title="Delete MCP server"
          subtitle={subtitle}
          onOpenChange={onOpenChange}
        />
        <div {...stylex.props(styles.dialogBody)}>
          <p {...stylex.props(styles.warningText)}>
            <AlertTriangle {...stylex.props(styles.warningIcon)} aria-hidden="true" />
            This cannot be undone from Settings. Historical approval and audit records are kept.
          </p>
          {error ? <p {...stylex.props(styles.errorText)}>{error}</p> : null}
          <div {...stylex.props(styles.actions)}>
            <Button
              type="button"
              variant="destructive"
              label="Delete server"
              icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
              isDisabled={submitting}
              isLoading={submitting}
              onClick={onConfirm}
            />
            <Button
              type="button"
              variant="secondary"
              label="Cancel"
              isDisabled={submitting}
              onClick={() => onOpenChange(false)}
            />
          </div>
        </div>
      </div>
    </Dialog>
  );
}

const styles = stylex.create({
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  errorText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  list: {
    display: "grid",
    gap: 12
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
  titleRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 12
  },
  cardTitle: {
    minWidth: 0,
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    gap: 8
  },
  definitionList: {
    display: "grid",
    gap: 8,
    margin: 0
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(120px, 180px) 1fr",
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
  dialog: {
    display: "grid",
    minHeight: 0
  },
  dialogBody: {
    display: "grid",
    gap: 12,
    minHeight: 0,
    overflow: "auto",
    padding: 16
  },
  warningText: {
    display: "flex",
    alignItems: "flex-start",
    gap: 8,
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  warningIcon: {
    width: 16,
    height: 16,
    marginTop: 2,
    color: "var(--destructive)",
    flexShrink: 0
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
      aria-label="Add MCP server"
    >
      <div {...stylex.props(styles.dialog)}>
        <DialogHeader
          title="Add MCP server"
          subtitle="Verify connection and discover tools."
          onOpenChange={onOpenChange}
        />
        <div {...stylex.props(styles.dialogBody)}>
          <McpServerSetupFlow
            setupResult={setupResult}
            setupSubmitting={setupSubmitting}
            oauthSubmitting={oauthSubmitting}
            setupError={setupError}
            onCreateServer={onCreateServer}
            onStartOAuth={onStartOAuth}
          />
        </div>
      </div>
    </Dialog>
  );
}
