import SwiftUI

struct ClientNotificationsSettings: View {
  @Bindable var notifications: NoemaNotificationService

  var body: some View {
    SettingsSectionCard("Device notifications") {
      HStack(alignment: .top, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text(notifications.settingsDetail)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .fixedSize(horizontal: false, vertical: true)
        }
        Spacer(minLength: NoemaSpacing.sm)
        action
      }
      if let errorMessage = notifications.errorMessage {
        Text(errorMessage)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.danger)
      }
      if notifications.errorMessage != nil
        || (notifications.status?.available == false && notifications.status?.blocker == nil)
      {
        Button {
          Task { await notifications.refresh() }
        } label: {
          Label("Retry", systemImage: "arrow.clockwise")
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
      }
    }
    .task { await notifications.refresh() }
  }

  @ViewBuilder
  private var action: some View {
    if notifications.authorizationStatus == .denied {
      Button {
        notifications.openSystemSettings()
      } label: {
        Label("Open Settings", systemImage: "gearshape")
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
    } else {
      Toggle(
        "",
        isOn: Binding(
          get: { notifications.status?.enabled == true },
          set: { enabled in
            Task {
              if enabled {
                await notifications.requestAuthorizationAndEnable()
              } else {
                await notifications.disable()
              }
            }
          }
        )
      )
      .labelsHidden()
      .accessibilityLabel("Device notifications")
      .toggleStyle(SettingsCompactToggleStyle())
      .disabled(
        notifications.isWorking
          || (notifications.status?.enabled != true && !notifications.canEnable))
    }
  }
}
