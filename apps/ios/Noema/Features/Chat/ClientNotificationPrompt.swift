import SwiftUI

struct ClientNotificationPrompt: View {
  let notifications: NoemaNotificationService
  let onDismiss: () -> Void

  var body: some View {
    NoemaCard(padding: NoemaSpacing.md) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        HStack(alignment: .top, spacing: NoemaSpacing.sm) {
          Image(systemName: "bell")
            .foregroundStyle(NoemaColor.accent)
            .accessibilityHidden(true)
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("Know when Noema replies")
              .font(NoemaFont.bodyEmphasized)
              .foregroundStyle(NoemaColor.content)
            Text("Get alerts for primary chat replies and items that need you, even when this app is closed.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
              .fixedSize(horizontal: false, vertical: true)
          }
          Spacer(minLength: 0)
          Button(action: onDismiss) {
            Image(systemName: "xmark")
              .font(NoemaFont.captionEmphasized)
              .foregroundStyle(NoemaColor.contentSecondary)
              .frame(width: 28, height: 28)
          }
          .buttonStyle(.plain)
          .accessibilityLabel("Dismiss notification prompt")
        }
        HStack(spacing: NoemaSpacing.sm) {
          Button {
            Task { await notifications.requestAuthorizationAndEnable() }
          } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if notifications.isWorking { ProgressView().controlSize(.small) }
              Text("Enable notifications")
            }
          }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary))
          .disabled(notifications.isWorking)
          Button("Not now", action: onDismiss)
            .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
          Spacer(minLength: 0)
        }
        if let errorMessage = notifications.errorMessage {
          Text(errorMessage)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.danger)
        }
      }
    }
    .frame(maxWidth: 760)
  }
}
