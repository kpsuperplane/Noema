import SwiftUI

struct ClientNotificationsSettings: View {
  @Bindable var notifications: NoemaNotificationService

  var body: some View {
    SettingsSectionCard("Device notifications") {
      HStack(alignment: .top, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text(notifications.status?.enabled == true ? "Notifications on" : "Notifications off")
            .font(NoemaFont.taskTitle)
            .foregroundStyle(NoemaColor.content)
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
        || (notifications.status?.available == false && notifications.status?.blocker == nil) {
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
    if notifications.status?.enabled == true {
      Button {
        Task { await notifications.disable() }
      } label: {
        Label("Disable", systemImage: "bell.slash")
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
      .disabled(notifications.isWorking)
    } else if notifications.authorizationStatus == .denied {
      Button {
        notifications.openSystemSettings()
      } label: {
        Label("Open Settings", systemImage: "gearshape")
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
    } else if notifications.canEnable {
      Button {
        Task { await notifications.requestAuthorizationAndEnable() }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          if notifications.isWorking { ProgressView().controlSize(.small) }
          Label("Enable", systemImage: "bell")
        }
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .primary))
      .disabled(notifications.isWorking)
    }
  }
}
