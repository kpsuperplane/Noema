import NoemaAPI
import SwiftUI

enum SettingsPreferenceKind: Hashable {
  case agent(String)
  case memory
  case webFetch
  case privacy
  case usage
  case defaultModel
}

struct SettingsPreferenceTarget: Identifiable {
  let id: String
  let title: String
  let kind: SettingsPreferenceKind
  let preference: SettingsPreference?
  let options: [SettingsModelOption]
}

private struct SettingsPreferenceDraft {
  var providerAccountID: String
  var selectionMode: String
  var modelProfile: String?
  var reasoningEffort: String?
}

struct SettingsPreferenceEditor: View {
  let target: SettingsPreferenceTarget
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var draft: SettingsPreferenceDraft
  @State private var isSaving = false

  private var isDirty: Bool {
    draft.providerAccountID != (target.preference?.providerAccountId ?? target.options.first?.providerAccountId ?? "")
      || draft.selectionMode != (target.preference?.selectionMode ?? NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue)
      || draft.modelProfile != target.preference?.modelProfile
      || draft.reasoningEffort != target.preference?.reasoningEffort
  }

  init(target: SettingsPreferenceTarget, settings: SettingsModel) {
    self.target = target
    self.settings = settings
    let preference = target.preference
    _draft = State(initialValue: SettingsPreferenceDraft(
      providerAccountID: preference?.providerAccountId ?? target.options.first?.providerAccountId ?? "",
      selectionMode: preference?.selectionMode ?? NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue,
      modelProfile: preference?.modelProfile,
      reasoningEffort: preference?.reasoningEffort
    ))
  }

  var body: some View {
    NavigationStack {
      Form {
        Section("Model") {
          if target.options.isEmpty {
            Text("No model providers are available.")
              .foregroundStyle(NoemaColor.contentSecondary)
          } else {
            Picker("Provider", selection: $draft.providerAccountID) {
              ForEach(target.options) { option in
                Text("\(option.providerDisplayName) · \(option.providerKind)")
                  .tag(option.providerAccountId)
              }
            }
            Picker("Selection", selection: $draft.selectionMode) {
              Text("Noema recommended").tag(NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue)
              Text("Explicit profile").tag(NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue)
            }
            if draft.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue,
               let option = target.options.first(where: { $0.providerAccountId == draft.providerAccountID }),
               !option.profiles.isEmpty {
              Picker("Profile", selection: Binding(
                get: { draft.modelProfile ?? option.profiles.first?.id ?? "" },
                set: { draft.modelProfile = $0 }
              )) {
                ForEach(option.profiles) { profile in
                  Text(profile.label).tag(profile.id)
                }
              }
              let profile = option.profiles.first(where: { $0.id == draft.modelProfile })
              if let profile, !profile.reasoningEfforts.isEmpty {
                Picker("Reasoning", selection: Binding(
                  get: { draft.reasoningEffort ?? profile.defaultReasoningEffort ?? profile.reasoningEfforts[0] },
                  set: { draft.reasoningEffort = $0 }
                )) {
                  ForEach(profile.reasoningEfforts, id: \.self) { effort in
                    Text(effort.replacingOccurrences(of: "_", with: " ").capitalized).tag(effort)
                  }
                }
              }
            }
          }
        }
        if let error = settings.errorMessage, isSaving == false {
          Section { Text(error).foregroundStyle(NoemaColor.danger) }
        }
      }
      .navigationTitle(target.title)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("Cancel") { dismiss() }.disabled(isSaving)
        }
        ToolbarItem(placement: .confirmationAction) {
          Button("Save") { save() }
            .disabled(isSaving || draft.providerAccountID.isEmpty || !settings.canMutate)
        }
      }
    }
    .presentationDetents([.medium, .large])
    .interactiveDismissDisabled(isSaving || isDirty)
  }

  private func save() {
    isSaving = true
    Task {
      let success: Bool
      switch target.kind {
      case .agent(let agentID):
        success = await settings.saveAgentModelPreference(agentID: agentID, providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: draft.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue ? draft.modelProfile : nil, reasoningEffort: draft.reasoningEffort)
      case .memory:
        success = await settings.saveMemoryModelPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: draft.modelProfile, reasoningEffort: draft.reasoningEffort)
      case .webFetch:
        success = await settings.saveWebFetchSummarizerPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: draft.modelProfile, reasoningEffort: draft.reasoningEffort)
      case .privacy:
        success = await settings.saveActionReviewerPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: draft.modelProfile, reasoningEffort: draft.reasoningEffort)
      case .usage:
        success = await settings.saveToolProgressAuditPreference(providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: draft.modelProfile, reasoningEffort: draft.reasoningEffort)
      case .defaultModel:
        guard let option = target.options.first(where: { $0.providerAccountId == draft.providerAccountID }) else { isSaving = false; return }
        success = await settings.saveDefaultModelPreference(providerKind: option.providerKind, providerAccountID: draft.providerAccountID, selectionMode: draft.selectionMode, modelProfile: draft.modelProfile, reasoningEffort: draft.reasoningEffort)
      }
      if success { dismiss() } else { isSaving = false }
    }
  }
}

struct WebBindingEditor: View {
  let binding: SettingsWebBinding
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var accountID: String
  @State private var isSaving = false

  private var isDirty: Bool { accountID != binding.activeProviderAccountID }

  init(binding: SettingsWebBinding, settings: SettingsModel) {
    self.binding = binding
    self.settings = settings
    _accountID = State(initialValue: binding.activeProviderAccountID)
  }

  var body: some View {
    NavigationStack {
      Form {
        Section(binding.toolName) {
          Picker("Provider", selection: $accountID) {
            ForEach(binding.options) { option in
              Text("\(option.displayName) · \(option.providerKind)").tag(option.providerAccountID)
            }
          }
        }
        if let error = settings.errorMessage, !isSaving { Section { Text(error).foregroundStyle(NoemaColor.danger) } }
      }
      .navigationTitle("Web provider")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(isSaving) }
        ToolbarItem(placement: .confirmationAction) {
          Button("Save") {
            isSaving = true
            Task {
              let ok = await settings.saveWebToolProviderBinding(toolName: binding.toolName, capabilityID: binding.capabilityID, providerAccountID: accountID)
              if ok { dismiss() } else { isSaving = false }
            }
          }
          .disabled(isSaving || accountID.isEmpty || !settings.canMutate)
        }
      }
    }
    .presentationDetents([.medium])
    .interactiveDismissDisabled(isSaving || isDirty)
  }
}

struct ExecutionPolicyEditor: View {
  let settings: SettingsModel
  let policy: NoemaAPI.SettingsSnapshotQuery.Data.TaskExecutionPolicy
  @Environment(\.dismiss) private var dismiss
  @State private var values: [String]
  @State private var isSaving = false
  @FocusState private var focusedField: Bool

  private var isDirty: Bool {
    values != [policy.maxProviderContinuations, policy.maxToolCalls, policy.maxActiveMinutes, policy.progressAuditInterval].map(String.init)
  }

  init(settings: SettingsModel, policy: NoemaAPI.SettingsSnapshotQuery.Data.TaskExecutionPolicy) {
    self.settings = settings
    self.policy = policy
    _values = State(initialValue: [policy.maxProviderContinuations, policy.maxToolCalls, policy.maxActiveMinutes, policy.progressAuditInterval].map(String.init))
  }

  var body: some View {
    NavigationStack {
      Form {
        Section("Run limits") {
          NumberField(title: "Provider continuations", text: $values[0]).focused($focusedField)
          NumberField(title: "Tool calls", text: $values[1])
          NumberField(title: "Active minutes", text: $values[2])
          NumberField(title: "Progress-audit interval", text: $values[3])
        }
        if let error = settings.errorMessage, !isSaving { Section { Text(error).foregroundStyle(NoemaColor.danger) } }
      }
      .navigationTitle("Edit execution limits")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(isSaving) }
        ToolbarItem(placement: .confirmationAction) {
          Button("Save") { save() }.disabled(isSaving || values.contains(where: { Int($0) == nil || Int($0)! < 1 }) || !settings.canMutate)
        }
      }
      .task { focusedField = true }
    }
    .presentationDetents([.medium])
    .interactiveDismissDisabled(isSaving || isDirty)
  }

  private func save() {
    guard values.count == 4,
          let first = Int(values[0]), let second = Int(values[1]),
          let third = Int(values[2]), let fourth = Int(values[3]) else { return }
    let parsed = [first, second, third, fourth]
    isSaving = true
    Task {
      let ok = await settings.updateExecutionPolicy(maxProviderContinuations: parsed[0], maxToolCalls: parsed[1], maxActiveMinutes: parsed[2], progressAuditInterval: parsed[3])
      if ok { dismiss() } else { isSaving = false }
    }
  }
}

private struct NumberField: View {
  let title: String
  @Binding var text: String
  var body: some View { TextField(title, text: $text).keyboardType(.numberPad) }
}

struct CapabilityConnectionEditor: View {
  let connection: SettingsIntegrationConnection
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var label: String
  @State private var sharing: String
  @State private var unsafeActions: String
  @State private var detail: SettingsCapabilityDetail?
  @State private var editingTool: SettingsCapabilityTool?
  @State private var isSaving = false
  @FocusState private var focusedField: Bool

  private var isDirty: Bool {
    label != (connection.connectionLabel ?? "")
      || sharing != (connection.dataSharingPolicy ?? "")
      || unsafeActions != (connection.unsafeActionPolicy ?? "")
  }

  init(connection: SettingsIntegrationConnection, settings: SettingsModel) {
    self.connection = connection
    self.settings = settings
    _label = State(initialValue: connection.connectionLabel ?? "")
    _sharing = State(initialValue: connection.dataSharingPolicy ?? "")
    _unsafeActions = State(initialValue: connection.unsafeActionPolicy ?? "")
  }

  var body: some View {
    NavigationStack {
      Form {
        Section("Connection") {
          TextField("Label", text: $label).focused($focusedField)
          Text("Revision \(connection.connectionRevision) · policy v\(connection.policyRevision)").font(NoemaFont.mono)
          Button("Save label") { saveLabel() }.disabled(isSaving || !settings.canMutate)
        }
        Section("Policy") {
          TextField("Data sharing policy", text: $sharing)
          TextField("Unsafe action policy", text: $unsafeActions)
          Button("Save policy") { savePolicy() }.disabled(isSaving || sharing.isEmpty || unsafeActions.isEmpty || !settings.canMutate)
        }
        Section("Tools") {
          if let detail {
            ForEach(detail.tools) { tool in
              HStack {
                VStack(alignment: .leading) { Text(tool.name); Text(tool.status).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary) }
                Spacer()
                Toggle("Enabled", isOn: Binding(get: { tool.enabled }, set: { _ in editingTool = tool })).labelsHidden()
              }
              .contentShape(Rectangle())
              .onTapGesture { editingTool = tool }
            }
          } else { ProgressView("Loading tools…") }
        }
        if let error = settings.errorMessage, !isSaving { Section { Text(error).foregroundStyle(NoemaColor.danger) } }
      }
      .navigationTitle(connection.connectionLabel ?? connection.name)
      .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() }.disabled(isSaving) } }
      .task {
        await settings.loadCapabilityDetail(kind: connection.kind, connectionID: connection.id)
        detail = settings.capabilityDetails[connection.id]
      }
      .task { focusedField = true }
      .sheet(item: $editingTool) { tool in
        CapabilityToolEditor(connection: connection, tool: tool, settings: settings)
      }
    }
    .presentationDetents([.large])
    .interactiveDismissDisabled(isSaving || isDirty)
  }

  private func saveLabel() {
    isSaving = true
    Task {
      let ok = await settings.saveCapabilityConnectionLabel(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, expectedConnectionLabel: connection.connectionLabel, connectionLabel: label.isEmpty ? nil : label)
      isSaving = !ok
    }
  }

  private func savePolicy() {
    isSaving = true
    Task {
      let ok = await settings.saveCapabilityConnectionPolicy(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, expectedPolicyRevision: connection.policyRevision, dataSharingPolicy: sharing, unsafeActionPolicy: unsafeActions)
      isSaving = !ok
    }
  }
}

private struct CapabilityToolEditor: View {
  let connection: SettingsIntegrationConnection
  let tool: SettingsCapabilityTool
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var readOnly: Bool
  @State private var idempotent: Bool
  @State private var destructive: Bool
  @State private var openWorld: Bool
  @State private var isSaving = false

  private var isDirty: Bool {
    readOnly != (tool.readOnly ?? false) || idempotent != (tool.idempotent ?? false)
      || destructive != (tool.destructive ?? false) || openWorld != (tool.openWorld ?? false)
  }

  init(connection: SettingsIntegrationConnection, tool: SettingsCapabilityTool, settings: SettingsModel) {
    self.connection = connection; self.tool = tool; self.settings = settings
    _readOnly = State(initialValue: tool.readOnly ?? false); _idempotent = State(initialValue: tool.idempotent ?? false)
    _destructive = State(initialValue: tool.destructive ?? false); _openWorld = State(initialValue: tool.openWorld ?? false)
  }

  var body: some View {
    NavigationStack {
      Form {
        Section(tool.name) {
          Toggle("Enabled", isOn: Binding(get: { tool.enabled }, set: { value in Task { _ = await settings.setCapabilityToolEnabled(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision, enabled: value); dismiss() } }))
          Toggle("Read-only", isOn: $readOnly); Toggle("Idempotent", isOn: $idempotent); Toggle("Destructive", isOn: $destructive); Toggle("Open-world", isOn: $openWorld)
          Button("Save tool override") { save() }.disabled(isSaving || !settings.canMutate)
          Button("Reset to source policy", role: .destructive) {
            isSaving = true
            Task { _ = await settings.resetCapabilityToolPolicy(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision); dismiss() }
          }.disabled(isSaving || !settings.canMutate)
        }
      }
      .navigationTitle("Tool policy")
      .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(isSaving) } }
    }
    .presentationDetents([.medium])
    .interactiveDismissDisabled(isSaving || isDirty)
  }

  private func save() {
    isSaving = true
    Task {
      _ = await settings.saveCapabilityToolOverride(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision, readOnly: readOnly, idempotent: idempotent, destructive: destructive, openWorld: openWorld)
      dismiss()
    }
  }
}

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
  @FocusState private var focusedField: Bool

  init(settings: SettingsModel, catalog: [SettingsProviderCatalog]) {
    self.settings = settings; self.catalog = catalog
    _providerKind = State(initialValue: catalog.first?.providerKind ?? "")
  }

  private var selected: SettingsProviderCatalog? { catalog.first(where: { $0.providerKind == providerKind }) }
  private var supportsSecret: Bool { selected?.supportedAuthMethods.contains(NoemaAPI.ProviderAuthMethod.secretInput.rawValue) == true }

  var body: some View {
    NavigationStack {
      Form {
        Section("Provider") {
          Picker("Provider", selection: $providerKind) { ForEach(catalog) { Text($0.displayName).tag($0.providerKind) } }
          TextField("Account name", text: $displayName).focused($focusedField)
          SecureField("API key or secret", text: $secret).disabled(!supportsSecret)
          if !supportsSecret, let selected {
            Button("Connect \(selected.displayName)") { startAuth(selected) }.disabled(isSaving || !settings.canMutate)
          }
          if let auth = settings.auth { ProviderAuthStatus(auth: auth, settings: settings) }
          if supportsSecret {
            Button("Add account") { create() }.disabled(isSaving || secret.isEmpty || !settings.canMutate)
          }
        }
        if let error = settings.errorMessage, !isSaving { Section { Text(error).foregroundStyle(NoemaColor.danger) } }
      }
      .navigationTitle("Add provider account")
      .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(isSaving) } }
      .task { focusedField = true }
    }
    .presentationDetents([.medium, .large])
    .interactiveDismissDisabled(isSaving || !displayName.isEmpty || !secret.isEmpty)
  }

  private func create() {
    guard let selected else { return }; isSaving = true
    Task { await settings.createProviderAccount(providerKind: selected.providerKind, displayName: displayName.isEmpty ? nil : displayName, secret: secret, authMethod: NoemaAPI.ProviderAuthMethod.secretInput.rawValue); isSaving = false; if settings.errorMessage == nil { dismiss() } }
  }

  private func startAuth(_ selected: SettingsProviderCatalog) {
    isSaving = true
    Task { _ = await settings.startProviderAuth(providerKind: selected.providerKind, providerAccountID: nil, method: selected.preferredAuthMethod); isSaving = false }
  }
}

private struct ProviderAuthStatus: View {
  let auth: ProviderAuthModel
  let settings: SettingsModel
  @State private var showSafari = false
  var body: some View {
    Section("Sign in") {
      if let instructions = auth.instructions { Text(instructions).font(NoemaFont.caption) }
      if let code = auth.userCode { Text("Code: \(code)").font(NoemaFont.mono).textSelection(.enabled) }
      if auth.verificationURL != nil { Button("Open provider sign-in") { showSafari = true } }
      if auth.status == "FAILED" || auth.status == "EXPIRED" { Text(auth.errorMessage ?? "Sign-in failed.").foregroundStyle(NoemaColor.danger) }
      Button("Cancel sign-in") { Task { _ = await settings.cancelProviderAuth() } }.disabled(settings.isMutating)
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
  @FocusState private var focusedField: Bool
  var body: some View {
    NavigationStack {
      Form { SecureField("API key or secret", text: $secret).focused($focusedField) }
        .navigationTitle("Replace \(account.displayName)")
        .toolbar {
          ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
          ToolbarItem(placement: .confirmationAction) {
            Button("Save") { isSaving = true; Task { await settings.saveProviderSecret(providerAccountID: account.providerAccountID, secret: secret); isSaving = false; dismiss() } }
              .disabled(secret.isEmpty || isSaving || !settings.canMutate)
          }
        }
        .task { focusedField = true }
    }
    .presentationDetents([.medium])
    .interactiveDismissDisabled(isSaving || !secret.isEmpty)
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
  @FocusState private var focusedField: Bool

  private var isDirty: Bool {
    !name.isEmpty || !localPath.isEmpty || !repo.isEmpty || !revision.isEmpty || !file.isEmpty || !sha256.isEmpty || !license.isEmpty
  }

  var body: some View {
    NavigationStack {
      Form {
        Section("Model") {
          TextField("Name", text: $name).focused($focusedField)
          Picker("Source", selection: $sourceKind) {
            Text("Local file").tag(NoemaAPI.LocalModelSourceKind.localFile.rawValue)
            Text("Public GGUF").tag(NoemaAPI.LocalModelSourceKind.publicGguf.rawValue)
          }
          if sourceKind == NoemaAPI.LocalModelSourceKind.localFile.rawValue { TextField("Local path", text: $localPath) }
          else { TextField("Repository", text: $repo); TextField("Revision", text: $revision); TextField("Filename", text: $file); TextField("SHA-256", text: $sha256) }
          TextField("License", text: $license)
        }
      }
      .navigationTitle("Import local model")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(isSaving) }
        ToolbarItem(placement: .confirmationAction) { Button("Import") { save() }.disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || isSaving || !settings.canMutate) }
      }
      .task { focusedField = true }
    }
    .presentationDetents([.large])
    .interactiveDismissDisabled(isSaving || isDirty)
  }

  private func save() {
    isSaving = true
    Task {
      await settings.importLocalModel(name: name, sourceKind: sourceKind, localPath: localPath.isEmpty ? nil : localPath, repo: repo.isEmpty ? nil : repo, revision: revision.isEmpty ? nil : revision, file: file.isEmpty ? nil : file, sha256: sha256.isEmpty ? nil : sha256, license: license.isEmpty ? nil : license)
      isSaving = false; if settings.errorMessage == nil { dismiss() }
    }
  }
}
