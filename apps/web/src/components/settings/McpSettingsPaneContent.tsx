import * as React from "react";
import { AlertTriangle, KeyRound, RefreshCw, Settings2, Trash2 } from "lucide-react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
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
import { McpServerReauthenticationDialog } from "./McpServerReauthenticationDialog";
import { McpToolPermissionsModal } from "./McpToolPermissionsModal";
import type { McpSetupContinueSubmission, McpSetupFormSubmission } from "./mcpSetupForm";

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
  onOpenPermissions = () => {},
  onOpenReauth = () => {},
  onCloseReauth = () => {},
  onContinueServerSetup = () => {},
  onStartReauthenticationOAuth = () => {},
  onClosePermissions = () => {},
  onPolicySaved = () => {},
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
  onOpenPermissions?: (mcpServerId: string) => void;
  onOpenReauth?: (mcpServerId: string) => void;
  onCloseReauth?: () => void;
  onContinueServerSetup?: (input: McpSetupContinueSubmission) => void;
  onStartReauthenticationOAuth?: (mcpServerId: string) => void;
  onClosePermissions?: () => void;
  onPolicySaved?: () => void;
  onDeleteServer?: (mcpServerId: string) => Promise<boolean>;
  onRetry: () => void;
}) {
  const [deleteTargetId, setDeleteTargetId] = React.useState<string | null>(null);
  const selectedPermissionsServer =
    servers.find((server) => server.mcpServerId === permissionsServerId) ?? null;
  const reauthServer =
    servers.find((server) => server.mcpServerId === reauthServerId) ?? null;
  const deleteTarget = servers.find((server) => server.mcpServerId === deleteTargetId) ?? null;

  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading connections...</p>;
  }

  if (error) {
    return (
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
    );
  }

  if (servers.length === 0) {
    return (
      <div {...stylex.props(styles.list)}>
        <Button
          type="button"
          variant="secondary"
          label="Connect a service"
          {...stylex.props(styles.fitButton)}
          onClick={onOpenSetup}
        />
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>No services connected.</p>
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
        <Button type="button" variant="secondary" label="Connect a service" onClick={onOpenSetup} />
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
                label="Sharing & approvals"
                icon={<Settings2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                onClick={() => onOpenPermissions(server.mcpServerId)}
              />
              {mcpServerNeedsReauth(server) ? (
                <Button
                  type="button"
                  variant="secondary"
                  label="Reconnect"
                  icon={<KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />}
                  onClick={() => onOpenReauth(server.mcpServerId)}
                />
              ) : null}
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
          key={`${selectedPermissionsServer?.mcpServerId ?? permissionsServerId}:${selectedPermissionsServer?.policyRevision ?? 0}`}
          open
          server={selectedPermissionsServer}
          onSaved={onPolicySaved}
          onOpenChange={(open) => {
            if (!open) onClosePermissions();
          }}
        />
      ) : null}
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

function mcpServerNeedsReauth(server: McpSettingsServer) {
  return server.healthStatus === "unavailable" || !["none", "authenticated"].includes(server.authStatus);
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
  const title = server ? `Delete ${server.displayName}?` : "Delete this connection?";
  const consequence = server
    ? `Removes the connection, sign-in details, ${mcpToolCountLabel(
        server.toolCount
      )}, and tool settings.`
    : "Removes the connection and sign-in details.";

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={480}
      aria-label={title}
    >
      <Layout
        height="auto"
        header={<DialogHeader title={title} onOpenChange={onOpenChange} />}
        content={
          <LayoutContent>
            <div {...stylex.props(styles.dialogBody)}>
              <p {...stylex.props(styles.warningText)}>
                <AlertTriangle {...stylex.props(styles.warningIcon)} aria-hidden="true" />
                <span>{consequence} You can't undo this. Past activity is kept.</span>
              </p>
              {error ? <p {...stylex.props(styles.errorText)}>{error}</p> : null}
              <div {...stylex.props(styles.dialogActions)}>
                <Button
                  type="button"
                  variant="secondary"
                  label="Cancel"
                  isDisabled={submitting}
                  onClick={() => onOpenChange(false)}
                />
                <Button
                  type="button"
                  variant="destructive"
                  label="Delete"
                  icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                  isDisabled={submitting}
                  isLoading={submitting}
                  onClick={onConfirm}
                />
              </div>
            </div>
          </LayoutContent>
        }
      />
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
  dialogBody: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  dialogActions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: "var(--spacing-2)",
    paddingTop: "var(--spacing-1)"
  },
  warningText: {
    display: "flex",
    alignItems: "flex-start",
    gap: "var(--spacing-2)",
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
