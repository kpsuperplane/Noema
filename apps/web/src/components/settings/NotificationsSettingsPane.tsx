import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Bell, BellOff } from "lucide-react";
import { useWebPush } from "@/pwa/WebPushContext";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

export function NotificationsSettingsPane() {
  const webPush = useWebPush();
  const enabled = webPush.state === "enabled";
  const actionable = webPush.state === "enabled" || webPush.state === "disabled";

  return (
    <SettingsSection aria-labelledby="notification-settings-title">
      <VStack gap={2}>
        <h2 id="notification-settings-title" {...stylex.props(styles.sectionTitle)}>
          Device notifications
        </h2>
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            mobileEndContentFullWidth
            label={enabled ? "Notifications on" : "Notifications off"}
            description={webPush.detail}
            endContent={
              actionable ? (
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  label={enabled ? "Disable" : "Enable"}
                  icon={enabled
                    ? <BellOff aria-hidden="true" size={14} />
                    : <Bell aria-hidden="true" size={14} />}
                  clickAction={enabled ? webPush.disable : webPush.enable}
                />
              ) : null
            }
          />
        </SettingsList>
        {webPush.error ? (
          <p role="alert" {...stylex.props(styles.errorText)}>{webPush.error}</p>
        ) : null}
      </VStack>
    </SettingsSection>
  );
}

const styles = stylex.create({
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  errorText: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  }
});
