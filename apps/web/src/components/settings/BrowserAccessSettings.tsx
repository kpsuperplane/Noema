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
  const [busy, setBusy] = useState<"add" | "remove" | "logout" | "logout-all" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [removeTarget, setRemoveTarget] = useState<RegisteredPasskey | null>(null);
  const [logoutAllOpen, setLogoutAllOpen] = useState(false);

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
    </VStack>
  );
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
