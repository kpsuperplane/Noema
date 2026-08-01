import SwiftUI

struct ClientRevocationSheet: View {
  let client: PairedClient
  let settings: SettingsModel
  let appModel: NoemaAppModel
  @Environment(\.dismiss) private var dismiss
  @State private var errorMessage: String?

  var body: some View {
    NavigationStack {
      Form {
        Section {
          Text(client.isCurrent ? "This revokes the credential used by this device. Noema will disconnect after the server confirms the revocation." : "The client will lose access to Noema immediately.")
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        if let errorMessage { Section { Text(errorMessage).foregroundStyle(NoemaColor.danger) } }
      }
      .navigationTitle("Revoke client?")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(settings.isMutating) }
        ToolbarItem(placement: .confirmationAction) {
          Button("Revoke", role: .destructive) {
            Task {
              do {
                let current = try await settings.revoke(client)
                if current { appModel.disconnect() }
                dismiss()
              } catch { errorMessage = error.localizedDescription }
            }
          }
          .disabled(!settings.canMutate)
        }
      }
      .interactiveDismissDisabled(settings.isMutating)
    }
    .presentationDetents([.medium])
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
