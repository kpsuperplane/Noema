import NoemaAPI
import SwiftUI

struct SettingsWebOption: Identifiable {
  var id: String { providerAccountID }
  let providerAccountID: String
  let providerKind: String
  let displayName: String
}

struct SettingsWebBinding: Identifiable {
  var id: String { toolName }
  let toolName: String
  let capabilityID: String
  let activeProviderAccountID: String
  let options: [SettingsWebOption]
}

struct SettingsProviderAccount: Identifiable {
  var id: String { providerAccountID }
  let providerAccountID: String
  let providerKind: String
  let displayName: String
  let authMethod: String
  let status: String
  let isActive: Bool
  let isDefault: Bool
  let lastError: String?
}

struct SettingsProviderCatalog: Identifiable {
  var id: String { providerKind }
  let providerKind: String
  let displayName: String
  let preferredAuthMethod: String
  let supportedAuthMethods: [String]
}

struct ProviderAccountEditor: View {
  let settings: SettingsModel
  let catalog: [SettingsProviderCatalog]
  @Environment(\.dismiss) private var dismiss
  @State private var providerKind: String
  @State private var displayName = ""
  @State private var secret = ""
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Bool

  init(settings: SettingsModel, catalog: [SettingsProviderCatalog]) {
    self.settings = settings
    self.catalog = catalog
    _providerKind = State(initialValue: catalog.first?.providerKind ?? "")
  }

  private var selected: SettingsProviderCatalog? { catalog.first(where: { $0.providerKind == providerKind }) }
  private var supportsSecret: Bool { selected?.supportedAuthMethods.contains(NoemaAPI.ProviderAuthMethod.secretInput.rawValue) == true }
  private var supportsBrowserAuth: Bool {
    selected?.preferredAuthMethod == NoemaAPI.ProviderAuthMethod.oauthPkce.rawValue
      || selected?.preferredAuthMethod == NoemaAPI.ProviderAuthMethod.oauthDeviceCode.rawValue
  }
  private var isDirty: Bool { !displayName.isEmpty || !secret.isEmpty }

  var body: some View {
    SettingsBottomSheet(
      title: "Add provider account",
      subtitle: "Connect a provider account for models and tools.",
      detent: .large,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Provider") {
          Picker("Provider", selection: $providerKind) {
            ForEach(catalog) { option in Text(option.displayName).tag(option.providerKind) }
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
          .settingsSheetControl()
        }
        SettingsSheetField("Account name") {
          TextField(selected?.displayName ?? "Provider", text: $displayName)
            .textInputAutocapitalization(.words)
            .settingsSheetControl(focused: focusedField)
            .focused($focusedField)
        }
        SettingsSheetField("API key") {
          SecureField(supportsSecret ? "API key" : "Use this provider's connect flow", text: $secret)
            .disabled(!supportsSecret)
            .settingsSheetControl()
        }
        HStack(spacing: NoemaSpacing.sm) {
          if supportsBrowserAuth, let selected {
            Button("Connect \(selected.displayName)") { startAuth(selected) }
              .buttonStyle(.bordered)
              .disabled(isSaving || !settings.canMutate)
          }
          Spacer(minLength: 0)
          if supportsSecret {
            SettingsSheetActions(
              primaryTitle: supportsBrowserAuth ? "Use API key" : "Add account",
              isSaving: isSaving,
              primaryDisabled: isSaving || secret.isEmpty || !settings.canMutate,
              onCancel: requestDismissal,
              onPrimary: create
            )
          }
        }
        if let auth = settings.auth { ProviderAuthStatus(auth: auth, settings: settings) }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        if !supportsSecret {
          HStack {
            Spacer(minLength: 0)
            Button("Cancel") { requestDismissal() }
              .buttonStyle(.plain)
              .font(NoemaFont.body)
              .disabled(isSaving)
          }
        }
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .sheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard provider account?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
    .task { focusedField = true }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func create() {
    guard let selected else { return }
    isSaving = true
    Task {
      await settings.createProviderAccount(providerKind: selected.providerKind, displayName: displayName.isEmpty ? nil : displayName, secret: secret, authMethod: NoemaAPI.ProviderAuthMethod.secretInput.rawValue)
      isSaving = false
      if settings.errorMessage == nil { dismiss() }
    }
  }

  private func startAuth(_ selected: SettingsProviderCatalog) {
    isSaving = true
    Task {
      _ = await settings.startProviderAuth(providerKind: selected.providerKind, providerAccountID: nil, method: selected.preferredAuthMethod)
      isSaving = false
    }
  }
}

private struct ProviderAuthStatus: View {
  let auth: ProviderAuthModel
  let settings: SettingsModel
  @State private var showSafari = false

  var body: some View {
    SettingsSheetField("Sign in") {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        if let instructions = auth.instructions { Text(instructions).font(NoemaFont.caption) }
        if let code = auth.userCode { Text("Code: \(code)").font(NoemaFont.mono).textSelection(.enabled) }
        if auth.verificationURL != nil {
          Button("Open provider sign-in") { showSafari = true }
            .buttonStyle(.bordered)
        }
        if auth.status == "FAILED" || auth.status == "EXPIRED" {
          Text(auth.errorMessage ?? "Sign-in failed.").font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        Button("Cancel sign-in") { Task { _ = await settings.cancelProviderAuth() } }
          .buttonStyle(.plain)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.danger)
          .disabled(settings.isMutating)
      }
    }
    .sheet(isPresented: $showSafari) {
      if let url = auth.verificationURL { SafariView(url: url) }
    }
  }
}

struct ProviderSecretEditor: View {
  let account: SettingsProviderAccount
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var secret = ""
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Bool

  var body: some View {
    SettingsBottomSheet(
      title: "Replace API key for \(account.displayName)",
      subtitle: "The new key replaces the stored provider credential.",
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("API key") {
          SecureField("API key", text: $secret)
            .settingsSheetControl(focused: focusedField)
            .focused($focusedField)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(
          primaryTitle: "Save key",
          isSaving: isSaving,
          primaryDisabled: secret.isEmpty || isSaving || !settings.canMutate,
          onCancel: requestDismissal,
          onPrimary: save
        )
      }
    }
    .interactiveDismissDisabled(isSaving || !secret.isEmpty)
    .sheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard replacement key?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
    .task { focusedField = true }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if !secret.isEmpty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    guard !secret.isEmpty, settings.canMutate else { return }
    isSaving = true
    Task {
      await settings.saveProviderSecret(providerAccountID: account.providerAccountID, secret: secret)
      isSaving = false
      dismiss()
    }
  }
}

struct LocalModelImportEditor: View {
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var name = ""
  @State private var sourceKind = NoemaAPI.LocalModelSourceKind.localFile.rawValue
  @State private var localPath = ""
  @State private var repo = ""
  @State private var revision = ""
  @State private var file = ""
  @State private var sha256 = ""
  @State private var license = ""
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Bool

  private var isDirty: Bool {
    !name.isEmpty || !localPath.isEmpty || !repo.isEmpty || !revision.isEmpty || !file.isEmpty || !sha256.isEmpty || !license.isEmpty
  }

  private var canSave: Bool {
    !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !isSaving && settings.canMutate
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Advanced GGUF import",
      subtitle: "Imported models are selectable, but Noema only recommends models from its bundled catalog.",
      detent: .large,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Model name") {
          TextField("Model name", text: $name)
            .textInputAutocapitalization(.words)
            .settingsSheetControl(focused: focusedField)
            .focused($focusedField)
        }
        SettingsSheetField("Source") {
          Picker("Source", selection: $sourceKind) {
            Text("Local file").tag(NoemaAPI.LocalModelSourceKind.localFile.rawValue)
            Text("Public Hugging Face GGUF").tag(NoemaAPI.LocalModelSourceKind.publicGguf.rawValue)
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
          .settingsSheetControl()
        }
        if sourceKind == NoemaAPI.LocalModelSourceKind.localFile.rawValue {
          SettingsSheetField("GGUF file path") {
            TextField("/path/to/model.gguf", text: $localPath).settingsSheetControl()
          }
        } else {
          SettingsSheetField("Repository") { TextField("owner/repository", text: $repo).settingsSheetControl() }
          SettingsSheetField("Pinned revision") { TextField("40-character commit", text: $revision).settingsSheetControl() }
          SettingsSheetField("GGUF filename") { TextField("model.gguf", text: $file).settingsSheetControl() }
          SettingsSheetField("SHA-256") { TextField("64-character digest", text: $sha256).settingsSheetControl() }
        }
        SettingsSheetField("License") { TextField("Optional", text: $license).settingsSheetControl() }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(
          primaryTitle: "Import model",
          isSaving: isSaving,
          primaryDisabled: !canSave,
          onCancel: requestDismissal,
          onPrimary: save
        )
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .sheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard model import?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
    .task { focusedField = true }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    guard canSave else { return }
    isSaving = true
    Task {
      await settings.importLocalModel(name: name, sourceKind: sourceKind, localPath: localPath.isEmpty ? nil : localPath, repo: repo.isEmpty ? nil : repo, revision: revision.isEmpty ? nil : revision, file: file.isEmpty ? nil : file, sha256: sha256.isEmpty ? nil : sha256, license: license.isEmpty ? nil : license)
      isSaving = false
      if settings.errorMessage == nil { dismiss() }
    }
  }
}
