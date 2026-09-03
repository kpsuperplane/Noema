import NoemaAPI
import SwiftUI

struct SettingsWebOption: Identifiable {
  var id: String { providerAccountID }
  let providerAccountID: String
  let providerKind: String
  let displayName: String
  let reliabilityContract: String
  let dataFlowClass: String
  let citations: Bool
  let directURLFetch: Bool
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
  let capabilities: [String]
}

struct ProviderAccountEditor: View {
  let settings: SettingsModel
  let catalog: [SettingsProviderCatalog]
  let profile: NoemaProfile?
  @Environment(\.dismiss) private var dismiss
  @State private var providerKind: String?
  @State private var displayName = ""
  @State private var secret = ""
  @State private var secretSetupPresented = false
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Bool

  init(settings: SettingsModel, catalog: [SettingsProviderCatalog], profile: NoemaProfile?) {
    self.settings = settings
    self.catalog = catalog
    self.profile = profile
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
      title: selected.map { "Add \($0.displayName)" } ?? "Add provider",
      subtitle: selected == nil ? "Choose a provider for models or web tools." : nil,
      detent: .large,
      onClose: requestDismissal
    ) {
      if selected == nil {
        providerChoices
      } else {
        providerSetup
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .noemaSheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard provider account?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
    .onChange(of: secretSetupPresented) { _, presented in
      if presented { focusedField = true }
    }
  }

  private var providerChoices: some View {
    LazyVStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      ForEach(catalog) { option in
        Button {
          choose(option)
        } label: {
          HStack(spacing: NoemaSpacing.sm) {
            ZStack {
              Image(systemName: "externaldrive.connected.to.line.below")
                .font(NoemaFont.bodyEmphasized)
                .foregroundStyle(NoemaColor.contentSecondary)
              NoemaFaviconImage(hostname: providerHostname(option.providerKind), profile: profile)
            }
            .frame(width: 32, height: 32)
            VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
              Text(option.displayName)
                .font(NoemaFont.bodyEmphasized)
                .foregroundStyle(NoemaColor.content)
              Text(providerCapabilityDescription(option))
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
                .multilineTextAlignment(.leading)
            }
            Spacer(minLength: NoemaSpacing.sm)
            Image(systemName: "chevron.right")
              .font(NoemaFont.compactEmphasized)
              .foregroundStyle(NoemaColor.contentTertiary)
          }
          .padding(NoemaSpacing.md)
          .frame(maxWidth: .infinity, alignment: .leading)
          .background(NoemaColor.surfaceSecondary, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
        }
        .buttonStyle(.plain)
        .disabled(isSaving || !settings.canMutate)
      }
    }
  }

  @ViewBuilder private var providerSetup: some View {
    if let auth = settings.auth {
      ProviderAuthStatus(auth: auth, settings: settings)
    } else if let selected {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        Button("Back to providers", systemImage: "arrow.left") { returnToProviders() }
          .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
          .disabled(isSaving)
        if supportsBrowserAuth {
          Button("Connect \(selected.displayName)") { startAuth(selected) }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
            .disabled(isSaving || !settings.canMutate)
        }
        if supportsBrowserAuth && supportsSecret {
          Button(secretSetupPresented ? "Hide API key setup" : "Use an API key instead") {
            secretSetupPresented.toggle()
          }
          .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
          .disabled(isSaving)
        }
        if supportsSecret && (!supportsBrowserAuth || secretSetupPresented) {
          secretSetup(selected)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
      }
    }
  }

  private func secretSetup(_ selected: SettingsProviderCatalog) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsSheetField("API key") {
        SecureField("API key", text: $secret)
          .settingsSheetControl(focused: focusedField)
          .focused($focusedField)
      }
      SettingsSheetField("Account name (optional)") {
        TextField(selected.displayName, text: $displayName)
          .textInputAutocapitalization(.words)
          .settingsSheetControl()
      }
      SettingsSheetActions(
        primaryTitle: supportsBrowserAuth ? "Add with API key" : "Add account",
        isSaving: isSaving,
        primaryDisabled: isSaving || secret.isEmpty || !settings.canMutate,
        onCancel: requestDismissal,
        onPrimary: create
      )
    }
    .task { focusedField = true }
  }

  private func choose(_ option: SettingsProviderCatalog) {
    providerKind = option.providerKind
    displayName = ""
    secret = ""
    secretSetupPresented = false
  }

  private func returnToProviders() {
    providerKind = nil
    displayName = ""
    secret = ""
    secretSetupPresented = false
    settings.errorMessage = nil
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

  private func providerCapabilityDescription(_ option: SettingsProviderCatalog) -> String {
    let capabilities = Set(option.capabilities)
    if capabilities.contains("model.generate") { return "Models for chat and tasks." }
    if capabilities.contains("web.browse") { return "Interactive browsing for web tools." }
    if capabilities.contains("web.search") && capabilities.contains("web.fetch") {
      return "Search and page reading for web tools."
    }
    return "Provider access for Noema."
  }

  private func providerHostname(_ providerKind: String) -> String {
    switch providerKind {
    case "codex": "chatgpt.com"
    case "openrouter": "openrouter.ai"
    case "exa": "exa.ai"
    case "kernel": "kernel.sh"
    case "tinyfish": "docs.tinyfish.ai"
    case "firecrawl": "firecrawl.dev"
    default: providerKind
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
            .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
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
    .noemaSheet(isPresented: $showSafari) {
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
      detent: .height(300),
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
    .noemaSheet(isPresented: $discardPresented) {
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
      let ok = await settings.saveProviderSecret(providerAccountID: account.providerAccountID, secret: secret)
      if ok { dismiss() } else { isSaving = false }
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
    guard !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !isSaving, settings.canMutate else { return false }
    if sourceKind == NoemaAPI.LocalModelSourceKind.localFile.rawValue {
      return !localPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
    return !repo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && !revision.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && !file.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && !sha256.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Advanced GGUF import",
      subtitle: "Imported models are selectable, but Noema only recommends models from its bundled catalog.",
      detent: .height(430),
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Source") {
          Picker("Source", selection: $sourceKind) {
            Text("Local file").tag(NoemaAPI.LocalModelSourceKind.localFile.rawValue)
            Text("Public Hugging Face GGUF").tag(NoemaAPI.LocalModelSourceKind.publicGguf.rawValue)
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
          .settingsSheetControl()
        }
        SettingsSheetField("Model name") {
          TextField("Model name", text: $name)
            .textInputAutocapitalization(.words)
            .settingsSheetControl(focused: focusedField)
            .focused($focusedField)
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
        DisclosureGroup("Advanced metadata") {
          SettingsSheetField("License") { TextField("Optional", text: $license).settingsSheetControl() }
            .padding(.top, NoemaSpacing.sm)
        }
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
    .noemaSheet(isPresented: $discardPresented) {
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
