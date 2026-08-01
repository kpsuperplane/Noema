import SwiftUI

struct ClientRevocationSheet: View {
  let client: PairedClient
  let settings: SettingsModel
  let appModel: NoemaAppModel
  @Environment(\.dismiss) private var dismiss
  @State private var errorMessage: String?

  var body: some View {
    SettingsBottomSheet(
      title: "Revoke \(client.displayName)?",
      subtitle: "This action takes effect immediately.",
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        HStack(alignment: .top, spacing: NoemaSpacing.sm) {
          Image(systemName: "exclamationmark.triangle.fill")
            .foregroundStyle(NoemaColor.danger)
            .accessibilityHidden(true)
          Text(client.isCurrent
            ? "This is the bearer credential used by the current request. Revoking it will end that client's access."
            : "The client will stop authenticating immediately. Past activity and its audit record are kept."
          )
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
        }
        if let errorMessage {
          Text(errorMessage)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.danger)
        }
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { requestDismissal() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .disabled(settings.isMutating)
          Button {
            revoke()
          } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if settings.isMutating { ProgressView().controlSize(.small) }
              Text("Revoke client")
            }
            .frame(minHeight: 32)
            .padding(.horizontal, NoemaSpacing.md)
          }
          .buttonStyle(.plain)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.white)
          .background(NoemaColor.danger, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .opacity(settings.canMutate && !settings.isMutating ? 1 : 0.42)
          .disabled(!settings.canMutate || settings.isMutating)
        }
      }
    }
    .interactiveDismissDisabled(settings.isMutating)
  }

  private func requestDismissal() {
    guard !settings.isMutating else { return }
    dismiss()
  }

  private func revoke() {
    guard settings.canMutate, !settings.isMutating else { return }
    Task {
      do {
        let current = try await settings.revoke(client)
        if current { appModel.disconnect() }
        dismiss()
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }
}

struct SettingsBottomSheet<Content: View>: View {
  let title: String
  let subtitle: String?
  let detent: PresentationDetent
  let onClose: () -> Void
  private let content: Content

  init(title: String, subtitle: String? = nil, detent: PresentationDetent = .medium, onClose: @escaping () -> Void, @ViewBuilder content: () -> Content) {
    self.title = title
    self.subtitle = subtitle
    self.detent = detent
    self.onClose = onClose
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      HStack(alignment: .top, spacing: NoemaSpacing.md) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text(title)
            .font(NoemaFont.mobileTitle)
            .foregroundStyle(NoemaColor.content)
          if let subtitle {
            Text(subtitle)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
        Spacer(minLength: 0)
        Button(action: onClose) {
          Image(systemName: "xmark")
            .font(.system(size: 15, weight: .medium))
            .frame(width: 32, height: 32)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Close")
      }
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.top, NoemaSpacing.md)
      .padding(.bottom, NoemaSpacing.lg)

      ScrollView {
        content
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.lg)
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    .background(NoemaColor.surface)
    .presentationDetents([detent])
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
    .presentationBackground(NoemaColor.surface)
  }
}

struct SettingsConfirmationSheet: View {
  @Environment(\.dismiss) private var dismiss
  let title: String
  let message: String
  let confirmTitle: String
  let cancelTitle: String
  let onConfirm: () -> Void

  init(
    title: String,
    message: String,
    confirmTitle: String,
    cancelTitle: String = "Cancel",
    onConfirm: @escaping () -> Void
  ) {
    self.title = title
    self.message = message
    self.confirmTitle = confirmTitle
    self.cancelTitle = cancelTitle
    self.onConfirm = onConfirm
  }

  var body: some View {
    SettingsBottomSheet(
      title: title,
      subtitle: message,
      detent: .height(194),
      onClose: { dismiss() }
    ) {
      HStack(spacing: NoemaSpacing.sm) {
        Spacer(minLength: 0)
        Button(cancelTitle) { dismiss() }
          .buttonStyle(.plain)
          .font(NoemaFont.body)
        Button(confirmTitle, role: .destructive) {
          onConfirm()
          dismiss()
        }
        .buttonStyle(.plain)
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.white)
        .frame(minHeight: 32)
        .padding(.horizontal, NoemaSpacing.md)
        .background(NoemaColor.danger, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      }
    }
  }
}

struct SettingsSheetField<Content: View>: View {
  let label: String
  private let content: Content

  init(_ label: String, @ViewBuilder content: () -> Content) {
    self.label = label
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label)
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.content)
      content
    }
  }
}

struct PreferenceSummary: View {
  let provider: String
  let account: String
  let profile: String?
  let mode: String

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(profile ?? mode.replacingOccurrences(of: "_", with: " ").capitalized).font(NoemaFont.bodyEmphasized)
      Text("\(provider) · \(account)").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
    }
  }
}

struct ModelOptions<Option>: View {
  let options: [Option]

  var body: some View {
    Text(options.isEmpty ? "No available provider options." : "\(options.count) provider option(s) are available.")
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
  }
}

struct WebBindingRow: View {
  let binding: SettingsWebBinding?

  var body: some View {
    if let binding {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text(binding.toolName).font(NoemaFont.bodyEmphasized)
        Text(binding.activeProviderAccountID.isEmpty ? "No provider selected" : "Account \(binding.activeProviderAccountID)")
          .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
      }
    } else {
      Text("No provider binding available.").foregroundStyle(NoemaColor.contentSecondary)
    }
  }
}

struct SettingsEmpty: View {
  let settings: SettingsModel
  let message: String

  var body: some View {
    if settings.isLoading {
      NoemaInlineState(message: "Loading…", symbol: "arrow.triangle.2.circlepath")
    } else if let error = settings.errorMessage {
      NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
    } else {
      NoemaInlineState(message: message, symbol: "circle.dashed")
    }
  }
}
