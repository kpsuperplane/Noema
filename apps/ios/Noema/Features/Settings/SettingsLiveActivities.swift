import SwiftUI

struct ClientLiveActivitiesSettings: View {
  @Bindable var liveActivities: NoemaLiveActivityService

  var body: some View {
    SettingsSectionCard("Task Live Activities") {
      HStack(alignment: .top, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
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
    if !liveActivities.activitiesEnabled {
      Button {
        liveActivities.openSystemSettings()
      } label: {
        Label("Open Settings", systemImage: "gearshape")
      }
      .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
    } else {
      Toggle(
        "Task Live Activities",
        isOn: Binding(
          get: { liveActivities.isEnabled },
          set: { enabled in
            Task {
              if enabled {
                await liveActivities.enable()
              } else {
                await liveActivities.disable()
              }
            }
          }
        )
      )
      .labelsHidden()
      .toggleStyle(SettingsCompactToggleStyle())
      .disabled(
        liveActivities.isWorking
          || (!liveActivities.isEnabled && !liveActivities.canEnable))
    }
  }
}
