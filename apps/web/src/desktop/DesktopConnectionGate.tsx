import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { TextArea } from "@astryxdesign/core/TextArea";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { AppBootSkeleton } from "@/components/shell/AppBootSkeleton";
import { SetupFrame } from "@/components/shell/SetupFrame";
import { DeleteConfirmationDialog } from "@/components/settings/DeleteConnectionDialog";
import { SettingsEditDialog } from "@/components/settings/SettingsEditDialog";
import { invokeDesktop, listenDesktop } from "@/graphql/desktopBridge";
import { isTauriRuntime } from "@/graphql/transportMode";

export type DesktopConnectionStatus = {
  mode: "local" | "remote";
  state:
    | "ready"
    | "offline"
    | "unauthorized"
    | "unavailable"
    | "credential_unavailable";
  origin: string | null;
  message: string | null;
  pendingPairingOrigin: string | null;
};

type DesktopConnectionContextValue = {
  status: DesktopConnectionStatus | null;
  beginPairing(): void;
  useLocal(): void;
};

const DesktopConnectionContext =
  React.createContext<DesktopConnectionContextValue | null>(null);

export function useDesktopConnection() {
  const value = React.useContext(DesktopConnectionContext);
  if (!value) throw new Error("Desktop connection context is unavailable.");
  return value;
}

export function DesktopConnectionGate({
  children,
}: {
  children: React.ReactNode;
}) {
  const desktop = isTauriRuntime();
  const [status, setStatus] = React.useState<DesktopConnectionStatus | null>(
    null,
  );
  const [pairingOpen, setPairingOpen] = React.useState(false);
  const [pairingOrigin, setPairingOrigin] = React.useState<string | null>(null);
  const [disconnectOpen, setDisconnectOpen] = React.useState(false);
  const [disconnecting, setDisconnecting] = React.useState(false);
  const [connectionError, setConnectionError] = React.useState<string | null>(
    null,
  );
  const [canForget, setCanForget] = React.useState(false);
  const [forgetOpen, setForgetOpen] = React.useState(false);
  const [forgetting, setForgetting] = React.useState(false);

  const refresh = React.useCallback(async (command = "desktop_connection_status") => {
    const next = await invokeDesktop<DesktopConnectionStatus>(
      command,
    );
    setStatus(next);
    if (next.pendingPairingOrigin) {
      setPairingOrigin(next.pendingPairingOrigin);
      setPairingOpen(true);
    }
  }, []);

  React.useEffect(() => {
    if (!desktop) return;
    let active = true;
    const cleanups: Array<() => void> = [];
    void refresh().catch(() => {
      if (active) setStatus(unavailableStatus());
    });
    void listenDesktop("desktop_connection_changed", () => {
      if (active) void refresh();
    }).then((cleanup) => cleanups.push(cleanup));
    void listenDesktop<{ origin: string }>(
      "desktop_pairing_pending",
      (stage) => {
        if (!active) return;
        setPairingOrigin(stage.origin);
        setPairingOpen(true);
      },
    ).then((cleanup) => cleanups.push(cleanup));
    return () => {
      active = false;
      for (const cleanup of cleanups) cleanup();
    };
  }, [desktop, refresh]);

  const beginPairing = React.useCallback(() => {
    setPairingOrigin(null);
    setPairingOpen(true);
  }, []);

  const useLocal = React.useCallback(() => setDisconnectOpen(true), []);

  const disconnectRemote = React.useCallback(async () => {
    setDisconnecting(true);
    setConnectionError(null);
    setCanForget(false);
    try {
      await invokeDesktop("desktop_use_local");
    } catch (error) {
      setConnectionError(errorMessage(error));
      setCanForget(true);
      await refresh().catch(() => undefined);
    } finally {
      setDisconnecting(false);
    }
  }, [refresh]);

  async function forgetRemote() {
    setForgetting(true);
    setConnectionError(null);
    try {
      await invokeDesktop("desktop_forget_remote");
    } catch (error) {
      setConnectionError(errorMessage(error));
      setForgetting(false);
    }
  }

  if (!desktop) return children;
  if (!status) return <AppBootSkeleton />;

  const ready = status.state === "ready";
  const context = { status, beginPairing, useLocal };
  return (
    <DesktopConnectionContext value={context}>
      {ready ? (
        children
      ) : (
        <SetupFrame>
          <VStack as="section" gap={4} {...stylex.props(styles.recovery)}>
            <VStack gap={2}>
              <p {...stylex.props(styles.eyebrow)}>Desktop connection</p>
              <h1 {...stylex.props(styles.title)}>
                Remote Noema is unavailable
              </h1>
              <p {...stylex.props(styles.description)}>
                {status.message ??
                  "Noema could not connect to the saved server."}
              </p>
              {status.origin ? (
                <code {...stylex.props(styles.origin)}>{status.origin}</code>
              ) : null}
            </VStack>
            {connectionError ? (
              <p role="alert" {...stylex.props(styles.error)}>
                {connectionError}
              </p>
            ) : null}
            <VStack gap={2}>
              <Button
                type="button"
                label="Try again"
                onClick={() => void refresh("desktop_retry_remote")}
              >
                Try again
              </Button>
              <Button
                type="button"
                variant="secondary"
                label="Use local Noema"
                isLoading={disconnecting}
                isDisabled={disconnecting}
                onClick={useLocal}
              >
                Use local Noema
              </Button>
              {canForget ? (
                <Button
                  type="button"
                  variant="destructive"
                  label="Forget this server"
                  onClick={() => {
                    setConnectionError(null);
                    setForgetOpen(true);
                  }}
                >
                  Forget this server
                </Button>
              ) : null}
            </VStack>
          </VStack>
        </SetupFrame>
      )}
      <DesktopPairingDialog
        open={pairingOpen}
        origin={pairingOrigin}
        onOriginChange={setPairingOrigin}
        onOpenChange={setPairingOpen}
      />
      <DeleteConfirmationDialog
        title="Use local Noema?"
        message={`Noema will revoke this desktop client on ${status.origin ?? "the remote server"}, remove its credential, and restart.`}
        open={disconnectOpen}
        submitting={disconnecting}
        error={connectionError}
        confirmLabel="Use local Noema"
        onOpenChange={setDisconnectOpen}
        onConfirm={() => void disconnectRemote()}
      />
      <DeleteConfirmationDialog
        title="Forget this server?"
        message="The server is unavailable, so Noema cannot revoke this client. Revoke it later from another signed-in client."
        open={forgetOpen}
        submitting={forgetting}
        error={connectionError}
        confirmLabel="Forget server"
        onOpenChange={setForgetOpen}
        onConfirm={() => void forgetRemote()}
      />
    </DesktopConnectionContext>
  );
}

function DesktopPairingDialog({
  open,
  origin,
  onOriginChange,
  onOpenChange,
}: {
  open: boolean;
  origin: string | null;
  onOriginChange: (origin: string | null) => void;
  onOpenChange: (open: boolean) => void;
}) {
  const [pairingLink, setPairingLink] = React.useState("");
  const [displayName, setDisplayName] = React.useState("Noema Desktop");
  const [working, setWorking] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  async function close() {
    if (working) return;
    await invokeDesktop("desktop_cancel_pairing").catch(() => undefined);
    setPairingLink("");
    setError(null);
    onOriginChange(null);
    onOpenChange(false);
  }

  async function continuePairing() {
    setWorking(true);
    setError(null);
    try {
      const stage = await invokeDesktop<{ origin: string }>(
        "desktop_stage_pairing",
        { pairingUri: pairingLink },
      );
      setPairingLink("");
      onOriginChange(stage.origin);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setWorking(false);
    }
  }

  async function connect() {
    setWorking(true);
    setError(null);
    try {
      await invokeDesktop("desktop_complete_pairing", { displayName });
    } catch (caught) {
      setError(errorMessage(caught));
      setWorking(false);
    }
  }

  return (
    <SettingsEditDialog
      title={
        origin ? "Connect to this Noema server?" : "Connect to a Noema server"
      }
      open={open}
      saving={working}
      saveLabel={origin ? "Connect" : "Continue"}
      saveDisabled={
        origin
          ? displayName.trim().length === 0
          : pairingLink.trim().length === 0
      }
      error={error}
      onOpenChange={(next) => {
        if (!next) void close();
      }}
      onSave={origin ? connect : continuePairing}
    >
      {origin ? (
        <>
          <p {...stylex.props(styles.dialogCopy)}>
            This desktop will use the data and settings from this server.
          </p>
          <code {...stylex.props(styles.dialogOrigin)}>{origin}</code>
          <TextInput
            label="Client name"
            value={displayName}
            isRequired
            hasAutoFocus
            onChange={setDisplayName}
          />
        </>
      ) : (
        <TextArea
          label="Pairing link"
          description="Create a ten-minute pairing link from the server's Client settings."
          placeholder="noema://pair?…"
          value={pairingLink}
          rows={3}
          isRequired
          hasAutoFocus
          hasSpellCheck={false}
          onChange={setPairingLink}
        />
      )}
    </SettingsEditDialog>
  );
}

function unavailableStatus(): DesktopConnectionStatus {
  return {
    mode: "remote",
    state: "unavailable",
    origin: null,
    message: "Noema could not read its desktop connection state.",
    pendingPairingOrigin: null,
  };
}

function errorMessage(error: unknown) {
  if (typeof error === "string") return error;
  return error instanceof Error
    ? error.message
    : "Noema could not complete that connection change.";
}

const styles = stylex.create({
  recovery: {
    width: "min(480px, 100%)",
    minHeight: "100%",
    marginInline: "auto",
    padding: "var(--spacing-6)",
    justifyContent: "center",
  },
  eyebrow: {
    margin: "var(--spacing-0)",
    color: "var(--text-accent)",
    fontFamily: "var(--font-mono)",
    fontSize: 12,
    letterSpacing: "0.12em",
    textTransform: "uppercase",
  },
  title: {
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 28,
    lineHeight: 1.15,
  },
  description: { margin: "var(--spacing-0)", color: "var(--muted-foreground)" },
  origin: { overflowWrap: "anywhere", color: "var(--foreground)" },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)" },
  dialogCopy: { margin: "var(--spacing-0)", color: "var(--muted-foreground)" },
  dialogOrigin: {
    padding: "var(--spacing-2)",
    overflowWrap: "anywhere",
    borderRadius: "var(--radius-control)",
    backgroundColor: "var(--muted)",
  },
});
