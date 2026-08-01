import { AlertTriangle, Trash2 } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import * as stylex from "@stylexjs/stylex";

type DeletableConnection = {
  name: string;
  toolCount: number;
};

export function DeleteConnectionDialog({
  connection,
  open,
  submitting,
  error,
  onOpenChange,
  onConfirm
}: {
  connection: DeletableConnection | null;
  open: boolean;
  submitting: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
  const title = connection ? `Delete ${connection.name}?` : "Delete this connection?";
  const toolCount = connection
    ? `${connection.toolCount} ${connection.toolCount === 1 ? "tool" : "tools"}`
    : null;
  const consequence = toolCount
    ? `Removes the connection, sign-in details, ${toolCount}, and tool settings.`
    : "Removes the connection and sign-in details.";

  return (
    <DeleteConfirmationDialog
      title={title}
      message={`${consequence} You can't undo this. Past activity is kept.`}
      open={open}
      submitting={submitting}
      error={error}
      onOpenChange={onOpenChange}
      onConfirm={onConfirm}
    />
  );
}

export function DeleteServiceDialog({
  service,
  open,
  submitting,
  error,
  onOpenChange,
  onConfirm
}: {
  service: { name: string; connectionCount: number } | null;
  open: boolean;
  submitting: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
  const title = service ? `Delete ${service.name}?` : "Delete this API service?";
  const connectionCount = service?.connectionCount ?? 0;
  const canDelete = connectionCount === 0;
  const message = canDelete
    ? "Removes the API definition and its tool setup. You can't undo this. Past activity is kept."
    : `Remove ${connectionCount} connected ${connectionCount === 1 ? "account" : "accounts"} before deleting this service.`;

  return (
    <DeleteConfirmationDialog
      title={title}
      message={message}
      open={open}
      submitting={submitting}
      error={error}
      canConfirm={canDelete}
      onOpenChange={onOpenChange}
      onConfirm={onConfirm}
    />
  );
}

export function DeleteConfirmationDialog({
  title,
  message,
  open,
  submitting,
  error,
  canConfirm = true,
  confirmLabel = "Delete",
  onOpenChange,
  onConfirm
}: {
  title: string;
  message: string;
  open: boolean;
  submitting: boolean;
  error: string | null;
  canConfirm?: boolean;
  confirmLabel?: string;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
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
            <div {...stylex.props(styles.body)}>
              <p {...stylex.props(styles.warning)}>
                <AlertTriangle {...stylex.props(styles.warningIcon)} aria-hidden="true" />
                <span>{message}</span>
              </p>
              {error ? <p {...stylex.props(styles.error)}>{error}</p> : null}
              <div {...stylex.props(styles.actions)}>
                <Button
                  type="button"
                  variant="secondary"
                  label={canConfirm ? "Cancel" : "Close"}
                  isDisabled={submitting}
                  onClick={() => onOpenChange(false)}
                />
                {canConfirm ? (
                  <Button
                    type="button"
                    variant="destructive"
                    label={confirmLabel}
                    icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                    isDisabled={submitting}
                    isLoading={submitting}
                    onClick={onConfirm}
                  />
                ) : null}
              </div>
            </div>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

const styles = stylex.create({
  body: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  warning: {
    display: "flex",
    alignItems: "flex-start",
    gap: "var(--spacing-2)",
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  warningIcon: {
    width: 16,
    height: 16,
    marginTop: "var(--spacing-0-5)",
    color: "var(--destructive)",
    flexShrink: 0
  },
  error: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: "var(--spacing-2)",
    paddingTop: "var(--spacing-1)"
  },
  icon: {
    width: 16,
    height: 16
  }
});
