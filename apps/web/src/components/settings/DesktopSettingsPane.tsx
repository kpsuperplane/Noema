import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { useDesktopConnection } from "@/desktop/DesktopConnectionGate";
import {
  SettingsList,
  SettingsListItem,
  SettingsRowActions,
  SettingsSection
} from "./SettingsPrimitives";

export function DesktopSettingsPane() {
  const { status, beginConnection, useLocal } = useDesktopConnection();
  const remote = status?.mode === "remote";

  return (
    <SettingsSection title="Connection" titleId="desktop-connection-title">
      <SettingsList aria-label="Desktop connection">
        <SettingsListItem
          label={remote ? (status?.origin ?? "Remote server") : "This computer"}
          description={
            remote
              ? "The desktop app uses this server's data, settings, and agent runtime."
              : "Noema runs its embedded service and stores data on this computer."
          }
          endContent={
            <SettingsRowActions>
              <HStack gap={1} vAlign="center">
                <StatusDot variant="success" label="Connected" />
                <span>Connected</span>
              </HStack>
              {remote ? (
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  label="Use local Noema"
                  onClick={useLocal}
                >
                  Use local Noema
                </Button>
              ) : (
                <Button
                  type="button"
                  size="sm"
                  label="Connect to server"
                  onClick={beginConnection}
                >
                  Connect to server
                </Button>
              )}
            </SettingsRowActions>
          }
          mobileEndContentFullWidth
        />
      </SettingsList>
    </SettingsSection>
  );
}
