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
      detent: .height(client.isCurrent ? 214 : 190),
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
        if current { appModel.disconnect(registrationsAlreadyRemoved: true) }
        dismiss()
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }
}

struct SettingsBottomSheet<Content: View>: View {
  let title: String?
  let subtitle: String?
  let detent: PresentationDetent
  let onClose: () -> Void
  private let content: Content
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  init(title: String? = nil, subtitle: String? = nil, detent: PresentationDetent = .medium, onClose: @escaping () -> Void, @ViewBuilder content: () -> Content) {
    self.title = title
    self.subtitle = subtitle
    self.detent = detent
    self.onClose = onClose
    self.content = content()
  }

  var body: some View {
    if horizontalSizeClass == .compact {
      sheetSurface
        .presentationDetents([detent])
        .presentationDragIndicator(.visible)
    } else {
      sheetSurface
        .frame(minWidth: 460, idealWidth: 520, maxWidth: 580, minHeight: 320, idealHeight: 520, maxHeight: 680)
        .presentationSizing(.form)
    }
  }

  private var sheetSurface: some View {
    NoemaNativeSheet(title: title ?? "", onDismiss: onClose) {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          if let subtitle {
            Text(subtitle)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
              .fixedSize(horizontal: false, vertical: true)
          }
          content
        }
        .padding(NoemaSpacing.lg)
      }
    }
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
      detent: .height(194),
      onClose: { dismiss() }
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        HStack(alignment: .top, spacing: NoemaSpacing.sm) {
          Image(systemName: "exclamationmark.triangle.fill")
            .foregroundStyle(NoemaColor.danger)
            .accessibilityHidden(true)
          Text(message)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
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
}

struct SettingsMutationConfirmationSheet: View {
  @Environment(\.dismiss) private var dismiss
  let title: String
  let message: String
  let confirmTitle: String
  var confirmDisabled = false
  let action: () async -> Bool
  @State private var isSaving = false
  @State private var errorMessage: String?

  var body: some View {
    SettingsBottomSheet(title: title, detent: .height(errorMessage == nil ? 210 : 242), onClose: requestDismissal) {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        HStack(alignment: .top, spacing: NoemaSpacing.sm) {
          Image(systemName: "exclamationmark.triangle.fill")
            .foregroundStyle(NoemaColor.danger)
            .accessibilityHidden(true)
          Text(message)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        if let errorMessage {
          Text(errorMessage).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel", action: requestDismissal).buttonStyle(.plain).disabled(isSaving)
          Button(confirmTitle, role: .destructive) {
            isSaving = true
            Task {
              if await action() { dismiss() }
              else { errorMessage = "Noema could not complete that change. Try again." }
              isSaving = false
            }
          }
          .buttonStyle(.plain)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.white)
          .frame(minHeight: 32)
          .padding(.horizontal, NoemaSpacing.md)
          .background(NoemaColor.danger.opacity(confirmDisabled ? 0.45 : 1), in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .disabled(isSaving || confirmDisabled)
        }
      }
    }
    .interactiveDismissDisabled(isSaving)
  }

  private func requestDismissal() {
    if !isSaving { dismiss() }
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
    if settings.isLoading && settings.snapshot == nil {
      NoemaInlineState(message: "Loading…", symbol: "arrow.triangle.2.circlepath")
    } else if let error = settings.errorMessage {
      NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
      SettingsAction(title: "Retry", symbol: "arrow.clockwise", role: nil, disabled: settings.client == nil) {
        Task { await settings.load(client: settings.client) }
      }
    } else {
      NoemaInlineState(message: message, symbol: "circle.dashed")
    }
  }
}
