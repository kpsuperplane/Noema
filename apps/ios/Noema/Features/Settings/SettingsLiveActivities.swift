import SwiftUI

struct ClientLiveActivitiesSettings: View {
  @Bindable var liveActivities: NoemaLiveActivityService

  var body: some View {
    SettingsSectionCard("Task Live Activities") {
      HStack(alignment: .top, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text(liveActivities.status?.enabled == true ? "Live Activities on" : "Live Activities off")
            .font(NoemaFont.taskTitle)
            .foregroundStyle(NoemaColor.content)
          Text(liveActivities.settingsDetail)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .fixedSize(horizontal: false, vertical: true)
        }
        Spacer(minLength: NoemaSpacing.sm)
        action
      }
      if liveActivities.status?.registered == true {
        Text("This device is registered for task updates.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      if let errorMessage = liveActivities.errorMessage {
        Text(errorMessage)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.danger)
      }
      if liveActivities.errorMessage != nil || liveActivities.status?.available == false {
        Button {
          Task { await liveActivities.refresh() }
        } label: {
          Label("Retry", systemImage: "arrow.clockwise")
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
      }
    }
    .task { await liveActivities.refresh() }
  }

  @ViewBuilder
  private var action: some View {
    if liveActivities.status?.enabled == true {
      Button {
        Task { await liveActivities.disable() }
      } label: {
        Label("Disable", systemImage: "rectangle.badge.xmark")
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
      .disabled(liveActivities.isWorking)
    } else if !liveActivities.activitiesEnabled {
      Button {
        liveActivities.openSystemSettings()
      } label: {
        Label("Open Settings", systemImage: "gearshape")
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
    } else if liveActivities.canEnable {
      Button {
        Task { await liveActivities.enable() }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          if liveActivities.isWorking { ProgressView().controlSize(.small) }
          Label("Enable", systemImage: "rectangle.inset.filled.and.person.filled")
        }
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .primary))
      .disabled(liveActivities.isWorking)
    }
  }
}
