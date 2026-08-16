import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/VStack";
import { useCallback, useEffect, useState } from "react";
import * as stylex from "@stylexjs/stylex";
import {
  authenticateWithPasskey,
  enrollPasskey,
  logoutBrowserSessions,
  PasskeyRequestError,
  registeredPasskeys,
  removePasskey,
  type RegisteredPasskey
} from "@/auth/passkey";
import { pwaRuntime } from "@/pwa/runtime";
import { DeleteConfirmationDialog } from "./DeleteConnectionDialog";
import {
  SettingsList,
  SettingsListItem,
  SettingsSection,
  SettingsSectionInset
} from "./SettingsPrimitives";

export function BrowserAccessSettings() {
  const [passkeys, setPasskeys] = useState<RegisteredPasskey[] | null>(null);
  const [busy, setBusy] = useState<"add" | "remove" | "logout" | "logout-all" | "erase" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [removeTarget, setRemoveTarget] = useState<RegisteredPasskey | null>(null);
  const [logoutAllOpen, setLogoutAllOpen] = useState(false);
  const [eraseOpen, setEraseOpen] = useState(false);
  const installed = pwaRuntime.getSnapshot().installed;

  const load = useCallback(async () => {
    try {
      setPasskeys(await registeredPasskeys());
      setError(null);
    } catch {
      setError("Passkeys could not be loaded.");
    }
  }, []);

  useEffect(() => {
    let active = true;
    void registeredPasskeys().then(
      (next) => {
        if (active) setPasskeys(next);
      },
      () => {
        if (active) setError("Passkeys could not be loaded.");
      }
    );
    return () => {
      active = false;
    };
  }, []);

  const withRecentPasskey = async (action: () => Promise<void>) => {
    try {
      await action();
    } catch (caught) {
      if (!(caught instanceof PasskeyRequestError) || caught.status !== 403) throw caught;
      await authenticateWithPasskey();
      await action();
    }
  };

  const add = async () => {
    setBusy("add");
    setError(null);
    pwaRuntime.setCriticalOperation("passkey", true);
    try {
      await withRecentPasskey(enrollPasskey);
      await load();
    } catch (caught) {
      setError(passkeyError(caught, "Noema could not add that passkey."));
    } finally {
      pwaRuntime.setCriticalOperation("passkey", false);
      setBusy(null);
    }
  };

  const remove = async () => {
    if (!removeTarget) return;
    setBusy("remove");
    setError(null);
    pwaRuntime.setCriticalOperation("passkey", true);
    try {
      await withRecentPasskey(() => removePasskey(removeTarget.credentialId));
      const removedCurrent = removeTarget.current;
      setRemoveTarget(null);
      if (removedCurrent) {
        pwaRuntime.requireAuthentication();
        window.location.assign("/");
        return;
      }
      await load();
    } catch (caught) {
      setError(
        caught instanceof PasskeyRequestError && caught.status === 409
          ? "Noema must keep at least one passkey. Add another passkey before removing this one."
          : passkeyError(caught, "Noema could not remove that passkey.")
      );
    } finally {
      pwaRuntime.setCriticalOperation("passkey", false);
      setBusy(null);
    }
  };

  const logout = async (all: boolean) => {
    setBusy(all ? "logout-all" : "logout");
    setError(null);
    try {
      await logoutBrowserSessions(all);
      pwaRuntime.requireAuthentication();
      window.location.assign("/");
    } catch {
      setError(all ? "Noema could not log out all browsers." : "Noema could not log out this browser.");
      setBusy(null);
    }
  };

  const erase = async () => {
    setBusy("erase");
    setError(null);
    try {
      if (navigator.onLine) {
        try {
          await logoutBrowserSessions(false);
        } catch {
          // Continue as an offline erasure. Server authority then expires later.
        }
      }
      await unsubscribeBrowserPush();
      await pwaRuntime.erasePrivateData();
      window.location.assign("/");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Noema could not erase this device.");
      setBusy(null);
    }
  };

  return (
    <VStack gap={4}>
      <SettingsSection
        title="Passkeys"
        titleId="browser-passkeys"
        summary={passkeys ? `${passkeys.length} registered` : undefined}
        action={
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label="Add passkey"
            isDisabled={busy !== null}
            isLoading={busy === "add"}
            onClick={() => void add()}
          />
        }
      >
        {passkeys === null ? (
          <SettingsSectionInset>
            <p {...stylex.props(styles.muted)}>Loading passkeys...</p>
          </SettingsSectionInset>
        ) : (
          <SettingsList density="balanced" hasDividers>
            {passkeys.map((passkey, index) => (
              <SettingsListItem
                key={passkey.credentialId}
                label={`Passkey ${index + 1}`}
                description={passkey.current ? "Used by this browser session" : "Registered access credential"}
                endContent={
                  passkeys.length > 1 ? (
                    <Button
                      type="button"
                      size="sm"
                      variant="destructive"
                      label="Remove"
                      isDisabled={busy !== null}
                      onClick={() => {
                        setError(null);
                        setRemoveTarget(passkey);
                      }}
                    />
                  ) : (
                    <Badge variant="info" label="Required" />
                  )
                }
              />
            ))}
          </SettingsList>
        )}
      </SettingsSection>

      <SettingsSection title="Browser sessions" titleId="browser-sessions">
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            label="This browser"
            description="End this browser session."
            endContent={
              <Button
                type="button"
                size="sm"
                variant="secondary"
                label="Log out"
                isDisabled={busy !== null}
                isLoading={busy === "logout"}
                onClick={() => void logout(false)}
              />
            }
          />
          <SettingsListItem
            label="All browsers"
            description="End every browser session for this server."
            endContent={
              <Button
                type="button"
                size="sm"
                variant="destructive"
                label="Log out all"
                isDisabled={busy !== null}
                onClick={() => {
                  setError(null);
                  setLogoutAllOpen(true);
                }}
              />
            }
          />
        </SettingsList>
      </SettingsSection>

      {installed ? (
        <SettingsSection title="Installed app data" titleId="installed-app-data">
          <SettingsList density="balanced">
            <SettingsListItem
              label="This device"
              description="Log out and remove private offline data from this installed app."
              endContent={
                <Button
                  type="button"
                  size="sm"
                  variant="destructive"
                  label="Log out and erase"
                  isDisabled={busy !== null}
                  onClick={() => {
                    setError(null);
                    setEraseOpen(true);
                  }}
                />
              }
            />
          </SettingsList>
        </SettingsSection>
      ) : null}

      {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
      <DeleteConfirmationDialog
        title="Remove this passkey?"
        message={removeTarget?.current
          ? "This browser will log out. Other registered passkeys will continue to work."
          : "This passkey will stop working. Other registered passkeys will continue to work."}
        open={removeTarget !== null}
        submitting={busy === "remove"}
        error={removeTarget ? error : null}
        confirmLabel="Remove passkey"
        onOpenChange={(open) => {
          if (!open && busy !== "remove") setRemoveTarget(null);
        }}
        onConfirm={() => void remove()}
      />
      <DeleteConfirmationDialog
        title="Log out all browsers?"
        message="Every browser session will end, including this one. Registered passkeys will remain."
        open={logoutAllOpen}
        submitting={busy === "logout-all"}
        error={logoutAllOpen ? error : null}
        confirmLabel="Log out all browsers"
        onOpenChange={(open) => {
          if (!open && busy !== "logout-all") setLogoutAllOpen(false);
        }}
        onConfirm={() => void logout(true)}
      />
      <DeleteConfirmationDialog
        title="Log out and erase this device?"
        message="Noema will remove private offline data and notifications from this installed app."
        open={eraseOpen}
        submitting={busy === "erase"}
        error={eraseOpen ? error : null}
        confirmLabel="Log out and erase"
        onOpenChange={(open) => {
          if (!open && busy !== "erase") setEraseOpen(false);
        }}
        onConfirm={() => void erase()}
      />
    </VStack>
  );
}

async function unsubscribeBrowserPush() {
  if (!("serviceWorker" in navigator)) return;
  const registration = await navigator.serviceWorker.getRegistration("/");
  const subscription = await registration?.pushManager.getSubscription();
  if (subscription && !await subscription.unsubscribe()) {
    throw new Error("Noema could not remove notifications from this device.");
  }
}

function passkeyError(caught: unknown, fallback: string) {
  return caught instanceof DOMException && caught.name === "NotAllowedError"
    ? "The passkey prompt was cancelled or timed out."
    : fallback;
}

const styles = stylex.create({
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13 }
});
