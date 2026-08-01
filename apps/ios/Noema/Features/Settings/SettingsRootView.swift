import NoemaAPI
import ApolloAPI
import SwiftUI

struct SettingsRootView: View {
  private let model: NoemaAppModel
  @State private var settings = SettingsModel()
  @State private var selection: SettingsSection? = .agents
  @State private var revocationTarget: PairedClient?

  init(model: NoemaAppModel) {
    self.model = model
  }

  var body: some View {
    GeometryReader { proxy in
      if proxy.size.width >= NoemaBreakpoint.compactMaximum {
        wideSettings
      } else {
        compactSettings
      }
    }
    .task {
      await settings.load(client: model.graphQLClient?.client)
    }
    .onChange(of: model.recoveryGeneration) { _, _ in
      Task { await settings.load(client: model.graphQLClient?.client) }
    }
    .sheet(item: $revocationTarget) { client in
      ClientRevocationSheet(client: client, settings: settings, appModel: model)
    }
  }

  private var wideSettings: some View {
    NavigationSplitView {
      SettingsMenu(selection: $selection)
    } detail: {
      SettingsDetail(
        section: selection ?? .agents,
        settings: settings,
        appModel: model,
        onRevoke: { revocationTarget = $0 }
      )
    }
    .navigationSplitViewStyle(.balanced)
  }

  private var compactSettings: some View {
    NavigationStack {
      List {
        Section("Settings") {
          ForEach(SettingsSection.allCases) { section in
            NavigationLink(value: section) {
              Label(section.title, systemImage: section.symbol)
            }
          }
        }
      }
      .navigationTitle("Settings")
      .navigationDestination(for: SettingsSection.self) { section in
        SettingsDetail(
          section: section,
          settings: settings,
          appModel: model,
          onRevoke: { revocationTarget = $0 }
        )
      }
    }
  }
}

private struct SettingsMenu: View {
  @Binding var selection: SettingsSection?

  var body: some View {
    List(selection: $selection) {
      Section("Settings") {
        ForEach(SettingsSection.allCases) { section in
          Label(section.title, systemImage: section.symbol)
            .tag(section as SettingsSection?)
        }
      }
    }
    .navigationTitle("Settings")
  }
}

private struct SettingsDetail: View {
  let section: SettingsSection
  let settings: SettingsModel
  let appModel: NoemaAppModel
  let onRevoke: (PairedClient) -> Void

  var body: some View {
    Group {
      switch section {
      case .agents: AgentsSettings(settings: settings)
      case .memory: MemorySettings(settings: settings)
      case .web: WebSettings(settings: settings)
      case .apis: CapabilitySettings(settings: settings, kind: .api)
      case .mcps: CapabilitySettings(settings: settings, kind: .mcp)
      case .privacy: PrivacySettings(settings: settings)
      case .usage: UsageSettings(settings: settings)
      case .execution: ExecutionSettings(settings: settings)
      case .localModels: LocalModelsSettings(settings: settings)
      case .providers: ProvidersSettings(settings: settings)
      case .clients: ClientsSettings(settings: settings, onRevoke: onRevoke)
      }
    }
    .navigationTitle(section.title)
    .navigationBarTitleDisplayMode(.inline)
  }
}

private struct AgentsSettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsPreferenceTarget?

  var body: some View {
    List {
      Section {
        if let agents = settings.snapshot?.agents, !agents.isEmpty {
          ForEach(agents, id: \.agentId) { agent in
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              HStack {
                Text(agent.displayName ?? (agent.isPrimary ? "Primary agent" : "Agent"))
                  .font(NoemaFont.bodyEmphasized)
                if agent.isPrimary { Text("Primary").font(NoemaFont.caption).foregroundStyle(NoemaColor.accent) }
              }
              if let preference = agent.modelPreference {
                PreferenceSummary(
                  provider: preference.providerKind,
                  account: preference.providerAccountId,
                  profile: preference.modelProfile,
                  mode: preference.selectionMode.rawValue
                )
              } else {
                Text("No model selected")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
              Text("\(agent.modelOptions.count) provider option(s)")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              Button("Edit model") {
                editor = SettingsPreferenceTarget(
                  id: agent.agentId, title: "Agent model", kind: .agent(agent.agentId),
                  preference: SettingsModel.preference(from: agent.modelPreference),
                  options: SettingsModel.modelOptions(from: agent.modelOptions)
                )
              }
              .buttonStyle(.borderless)
              .disabled(!settings.canMutate)
            }
            .padding(.vertical, NoemaSpacing.xs)
          }
        } else {
          SettingsEmpty(settings: settings, message: "No agents are available.")
        }
      } header: {
        Text("Registered agents")
      }
    }
    .listStyle(.insetGrouped)
    .sheet(item: $editor) { target in
      SettingsPreferenceEditor(target: target, settings: settings)
    }
  }
}

private struct MemorySettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsPreferenceTarget?

  var body: some View {
    List {
      Section("Background updates") {
        if let preference = settings.snapshot?.memorySettings.modelPreference {
          PreferenceSummary(
            provider: preference.providerKind,
            account: preference.providerAccountId,
            profile: preference.modelProfile,
            mode: preference.selectionMode.rawValue
          )
        } else {
          Text("No memory model selected")
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        Text("Memory updates use the selected provider after new conversation source messages arrive.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      Section("Available models") {
        let options = settings.snapshot?.memorySettings.modelOptions ?? []
        Button("Edit model") {
          editor = SettingsPreferenceTarget(
            id: "memory", title: "Memory model", kind: .memory,
            preference: SettingsModel.preference(from: settings.snapshot?.memorySettings.modelPreference),
            options: SettingsModel.modelOptions(from: options)
          )
        }
        .disabled(!settings.canMutate || options.isEmpty)
        if options.isEmpty { ModelOptions(options: options) }
        else { Text("\(options.count) provider option(s) are available.").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary) }
      }
    }
    .listStyle(.insetGrouped)
    .sheet(item: $editor) { target in
      SettingsPreferenceEditor(target: target, settings: settings)
    }
  }
}

private struct WebSettings: View {
  let settings: SettingsModel
  @State private var bindingEditor: SettingsWebBinding?
  @State private var preferenceEditor: SettingsPreferenceTarget?

  var body: some View {
    List {
      Section("Search") {
        if let snapshot = settings.snapshot {
          let binding = searchBinding(snapshot.webToolSettings.search)
          WebBindingRow(binding: binding)
          Button("Edit provider") { bindingEditor = binding }.disabled(!settings.canMutate || binding.options.isEmpty)
        } else {
          WebBindingRow(binding: nil)
        }
      }
      Section("Fetch") {
        if let snapshot = settings.snapshot {
          let binding = fetchBinding(snapshot.webToolSettings.fetch)
          WebBindingRow(binding: binding)
          Button("Edit provider") { bindingEditor = binding }.disabled(!settings.canMutate || binding.options.isEmpty)
          if let preference = snapshot.webFetchSettings.summarizer.modelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          }
          let target = SettingsPreferenceTarget(
            id: "web-fetch", title: "Web summarizer", kind: .webFetch,
            preference: SettingsModel.preference(from: snapshot.webFetchSettings.summarizer.modelPreference),
            options: SettingsModel.modelOptions(from: snapshot.webFetchSettings.summarizer.modelOptions)
          )
          Button("Edit summarizer model") { preferenceEditor = target }.disabled(!settings.canMutate || target.options.isEmpty)
        }
      }
    }
    .listStyle(.insetGrouped)
    .sheet(item: $bindingEditor) { binding in WebBindingEditor(binding: binding, settings: settings) }
    .sheet(item: $preferenceEditor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }

  private func searchBinding(_ value: NoemaAPI.SettingsSnapshotQuery.Data.WebToolSettings.Search) -> SettingsWebBinding {
    SettingsWebBinding(toolName: value.toolName, capabilityID: value.capabilityId, activeProviderAccountID: value.activeProviderAccountId, options: value.providerOptions.map { SettingsWebOption(providerAccountID: $0.providerAccountId, providerKind: $0.providerKind, displayName: $0.displayName) })
  }

  private func fetchBinding(_ value: NoemaAPI.SettingsSnapshotQuery.Data.WebToolSettings.Fetch) -> SettingsWebBinding {
    SettingsWebBinding(toolName: value.toolName, capabilityID: value.capabilityId, activeProviderAccountID: value.activeProviderAccountId, options: value.providerOptions.map { SettingsWebOption(providerAccountID: $0.providerAccountId, providerKind: $0.providerKind, displayName: $0.displayName) })
  }
}

struct SettingsIntegrationConnection: Identifiable {
  let id: String
  let kind: NoemaAPI.CapabilityIntegrationKind
  let definitionId: String
  let name: String
  let connectionLabel: String?
  let sourceRevision: String
  let connectionRevision: String
  let policyRevision: Int
  let authStatus: String
  let dataSharingPolicy: String?
  let unsafeActionPolicy: String?
  let toolCount: Int
  let availableToolCount: Int
}

private struct SettingsIntegration: Identifiable {
  let id: String
  let name: String
  let sourceRevision: String
  let reviewed: Bool
  let sourceSummary: String
  let connections: [SettingsIntegrationConnection]
}

private struct CapabilitySettings: View {
  let settings: SettingsModel
  let kind: NoemaAPI.CapabilityIntegrationKind
  @State private var editor: SettingsIntegrationConnection?

  var body: some View {
    Group {
      if kind == .api {
        capabilityList(integrations(settings.snapshot?.apis ?? []))
      } else {
        capabilityList(integrations(settings.snapshot?.mcps ?? []))
      }
    }
    .sheet(item: $editor) { connection in
      CapabilityConnectionEditor(connection: connection, settings: settings)
    }
  }

  @ViewBuilder
  private func capabilityList(_ integrations: [SettingsIntegration]) -> some View {
    List {
      if integrations.isEmpty {
        SettingsEmpty(settings: settings, message: "No \(kind.rawValue) definitions are available.")
      }
      ForEach(integrations) { integration in
        Section {
          ForEach(integration.connections) { connection in
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              Text(connection.connectionLabel ?? connection.name)
                .font(NoemaFont.bodyEmphasized)
              Text("\(connection.authStatus) · \(connection.availableToolCount)/\(connection.toolCount) tools")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              Text("Source \(connection.sourceRevision) · Connection \(connection.connectionRevision) · Policy v\(connection.policyRevision)")
                .font(NoemaFont.mono)
                .foregroundStyle(NoemaColor.contentTertiary)
              if let sharing = connection.dataSharingPolicy, let unsafe = connection.unsafeActionPolicy {
                Text("Sharing: \(sharing) · Unsafe actions: \(unsafe)")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
              Button("Edit connection policy") { editor = connection }
                .buttonStyle(.borderless)
                .disabled(!settings.canMutate)
            }
            .padding(.vertical, NoemaSpacing.xs)
          }
          if integration.connections.isEmpty {
            Text("No connection added to this definition.")
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        } header: {
          HStack {
            Text(integration.name)
            Spacer()
            Text(integration.reviewed ? "Reviewed" : "Needs review")
              .font(NoemaFont.caption)
              .foregroundStyle(integration.reviewed ? NoemaColor.success : NoemaColor.warning)
          }
        } footer: {
          Text("Definition \(integration.sourceRevision) · \(integration.sourceSummary)")
        }
      }
      if kind == .mcp, let servers = settings.snapshot?.mcpServers, !servers.isEmpty {
        Section("Server health") {
          ForEach(servers, id: \.mcpServerId) { server in
            LabeledContent(server.displayName) {
              Text("\(server.healthStatus) · \(server.authStatus)")
                .foregroundStyle(NoemaColor.contentSecondary)
            }
            Text("Connection \(server.connectionRevision) · Policy v\(server.policyRevision)")
              .font(NoemaFont.mono)
              .foregroundStyle(NoemaColor.contentTertiary)
          }
        }
      }
    }
    .listStyle(.insetGrouped)
  }

  private func integrations(_ values: [NoemaAPI.SettingsSnapshotQuery.Data.Api]) -> [SettingsIntegration] {
    values.map { value in
      SettingsIntegration(
        id: value.definitionId, name: value.name, sourceRevision: value.sourceRevision,
        reviewed: value.reviewed, sourceSummary: value.sourceSummary,
        connections: value.connections.map { connection in
          SettingsIntegrationConnection(
            id: connection.connectionId,
            kind: .api,
            definitionId: connection.definitionId,
            name: connection.name,
            connectionLabel: connection.connectionLabel,
            sourceRevision: connection.sourceRevision,
            connectionRevision: connection.connectionRevision,
            policyRevision: connection.policyRevision,
            authStatus: connection.authStatus,
            dataSharingPolicy: connection.dataSharingPolicy,
            unsafeActionPolicy: connection.unsafeActionPolicy,
            toolCount: connection.toolCount,
            availableToolCount: connection.availableToolCount
          )
        }
      )
    }
  }

  private func integrations(_ values: [NoemaAPI.SettingsSnapshotQuery.Data.Mcp]) -> [SettingsIntegration] {
    values.map { value in
      SettingsIntegration(
        id: value.definitionId, name: value.name, sourceRevision: value.sourceRevision,
        reviewed: value.reviewed, sourceSummary: value.sourceSummary,
        connections: value.connections.map { connection in
          SettingsIntegrationConnection(
            id: connection.connectionId,
            kind: .mcp,
            definitionId: connection.definitionId,
            name: connection.name,
            connectionLabel: connection.connectionLabel,
            sourceRevision: connection.sourceRevision,
            connectionRevision: connection.connectionRevision,
            policyRevision: connection.policyRevision,
            authStatus: connection.authStatus,
            dataSharingPolicy: connection.dataSharingPolicy,
            unsafeActionPolicy: connection.unsafeActionPolicy,
            toolCount: connection.toolCount,
            availableToolCount: connection.availableToolCount
          )
        }
      )
    }
  }
}

private struct PrivacySettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsPreferenceTarget?

  var body: some View {
    List {
      Section("Governed actions") {
        if let preference = settings.snapshot?.privacySettings.reviewer.modelPreference {
          PreferenceSummary(
            provider: preference.providerKind,
            account: preference.providerAccountId,
            profile: preference.modelProfile,
            mode: preference.selectionMode.rawValue
          )
        } else {
          Text("Human approval is required when no reviewer model is configured.")
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        if let snapshot = settings.snapshot {
          let target = SettingsPreferenceTarget(
            id: "privacy", title: "Reviewer model", kind: .privacy,
            preference: SettingsModel.preference(from: snapshot.privacySettings.reviewer.modelPreference),
            options: SettingsModel.modelOptions(from: snapshot.privacySettings.reviewer.modelOptions)
          )
          Button("Edit reviewer model") { editor = target }.disabled(!settings.canMutate || target.options.isEmpty)
          Text("\(target.options.count) provider option(s) are available.")
            .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
      }
    }
    .listStyle(.insetGrouped)
    .sheet(item: $editor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }
}

private struct UsageSettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsPreferenceTarget?

  var body: some View {
    List {
      if let snapshot = settings.snapshot {
        let target = SettingsPreferenceTarget(
          id: "usage-progress-audit", title: "Progress-audit model", kind: .usage,
          preference: SettingsModel.preference(from: snapshot.usageSettings.progressAudit.modelPreference),
          options: SettingsModel.modelOptions(from: snapshot.usageSettings.progressAudit.modelOptions)
        )
        Section("Progress audits") {
          if let preference = snapshot.usageSettings.progressAudit.modelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          } else {
            Text("No progress-audit model selected.").foregroundStyle(NoemaColor.contentSecondary)
          }
          Button("Edit progress-audit model") { editor = target }.disabled(!settings.canMutate || target.options.isEmpty)
          Text("\(target.options.count) provider option(s) are available.").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
      } else {
        SettingsEmpty(settings: settings, message: "Usage settings are unavailable.")
      }
    }
    .listStyle(.insetGrouped)
    .sheet(item: $editor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }
}

private struct ExecutionSettings: View {
  let settings: SettingsModel
  @State private var editor = false

  var body: some View {
    List {
      if let policy = settings.snapshot?.taskExecutionPolicy {
        Section("Run limits") {
          LabeledContent("Provider continuations", value: "\(policy.maxProviderContinuations)")
          LabeledContent("Tool calls", value: "\(policy.maxToolCalls)")
          LabeledContent("Active minutes", value: "\(policy.maxActiveMinutes)")
          LabeledContent("Progress-audit interval", value: "\(policy.progressAuditInterval)")
          LabeledContent("Automatic retries", value: "\(policy.maxAutomaticRetries)")
          LabeledContent("Review rounds", value: "\(policy.maxReviewRounds)")
          Button("Edit limits") { editor = true }.disabled(!settings.canMutate)
        }
      } else {
        SettingsEmpty(settings: settings, message: "Execution policy is unavailable.")
      }
    }
    .listStyle(.insetGrouped)
    .sheet(isPresented: $editor) {
      if let policy = settings.snapshot?.taskExecutionPolicy { ExecutionPolicyEditor(settings: settings, policy: policy) }
    }
  }
}

private struct LocalModelsSettings: View {
  let settings: SettingsModel
  @State private var importPresented = false

  var body: some View {
    List {
      if let setup = settings.snapshot?.localModelSetup {
        Section("Runtime") {
          LabeledContent("Status", value: setup.isReady ? "Ready" : setup.runtimeStatus.rawValue)
          if !setup.isReady {
            Button("Retry runtime") { Task { await settings.retryLocalModelRuntime() } }
              .disabled(!settings.canMutate)
          }
          if let recommendation = setup.recommendedModel {
            LabeledContent("Recommended", value: recommendation.name)
            if let fit = recommendation.hardwareFit {
              Text(fit.explanation)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
            }
            if setup.installation == nil {
              Button("Install \(recommendation.name)") {
                Task { await settings.installLocalModel(modelID: recommendation.modelId, file: recommendation.selectedBuild?.file) }
              }
              .disabled(!settings.canMutate)
            }
          }
        }
      }
      Section("Installed") {
        let installations = settings.snapshot?.localModelInstallations ?? []
        if installations.isEmpty {
          Text("No local models installed.")
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          ForEach(installations, id: \.installationId) { installation in
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              HStack {
                Text(installation.name).font(NoemaFont.bodyEmphasized)
                Spacer()
                if installation.isActive { Text("Active").font(NoemaFont.caption).foregroundStyle(NoemaColor.success) }
              }
              Text("\(installation.status.rawValue) · \(installation.file)")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              HStack {
                if !installation.isActive && installation.status.rawValue == "INSTALLED" {
                  Button("Use") {
                    Task { await settings.activateLocalModel(installationID: installation.installationId) }
                  }
                  .disabled(!settings.canMutate)
                }
                if ["QUEUED", "DOWNLOADING", "VERIFYING"].contains(installation.status.rawValue) {
                  Button("Cancel", role: .destructive) {
                    Task { await settings.cancelLocalModelInstall(installationID: installation.installationId) }
                  }
                }
                Button("Remove", role: .destructive) {
                  Task { await settings.removeLocalModel(installationID: installation.installationId) }
                }
                .disabled(!settings.canMutate || installation.isActive)
              }
              .buttonStyle(.borderless)
            }
            .padding(.vertical, NoemaSpacing.xs)
          }
        }
      }
      Section {
        Button("Import local model") { importPresented = true }
          .disabled(!settings.canMutate)
      }
    }
    .listStyle(.insetGrouped)
    .sheet(isPresented: $importPresented) { LocalModelImportEditor(settings: settings) }
  }
}

private struct ProvidersSettings: View {
  let settings: SettingsModel
  @State private var addPresented = false
  @State private var secretAccount: SettingsProviderAccount?
  @State private var clearAccount: SettingsProviderAccount?
  @State private var deleteAccount: SettingsProviderAccount?
  @State private var defaultEditor: SettingsPreferenceTarget?

  var body: some View {
    List {
      Section("Connected accounts") {
        let accounts = settings.snapshot?.providerAccounts ?? []
        if accounts.isEmpty {
          Text("No provider accounts are connected.")
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          ForEach(accounts, id: \.providerAccountId) { account in
            let local = providerAccount(account)
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              HStack {
                Text(account.displayName).font(NoemaFont.bodyEmphasized)
                Spacer()
                Text(account.status.rawValue)
                  .font(NoemaFont.caption)
                  .foregroundStyle(account.isActive ? NoemaColor.success : NoemaColor.warning)
              }
              Text("\(account.providerKind) · \(account.authMethod) · \(account.accountKey)")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              if let error = account.lastErrorMessage {
                Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.warning)
              }
              HStack {
                if account.authMethod == NoemaAPI.ProviderAuthMethod.secretInput.rawValue {
                  Button("Replace key") { secretAccount = local }
                  Button("Clear key", role: .destructive) { clearAccount = local }
                }
                if !account.isDefault { Button("Delete", role: .destructive) { deleteAccount = local } }
              }
              .buttonStyle(.borderless)
            }
          }
        }
      }
      Section("Default model") {
        if let snapshot = settings.snapshot {
          let options = defaultModelOptions(snapshot)
          if let preference = snapshot.defaultModelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          } else {
            Text("No default model selected.").foregroundStyle(NoemaColor.contentSecondary)
          }
          let target = SettingsPreferenceTarget(id: "default-model", title: "Default model", kind: .defaultModel, preference: SettingsModel.preference(from: snapshot.defaultModelPreference), options: options)
          Button("Edit default model") { defaultEditor = target }.disabled(!settings.canMutate || options.isEmpty)
        }
      }
      Section("Available providers") {
        Button("Add provider account") { addPresented = true }.disabled(!settings.canMutate)
        ForEach(settings.snapshot?.providerAccountCatalog ?? [], id: \.providerKind) { provider in
          LabeledContent(provider.displayName, value: provider.preferredAuthMethod.rawValue)
        }
      }
    }
    .listStyle(.insetGrouped)
    .sheet(isPresented: $addPresented) {
      ProviderAccountEditor(settings: settings, catalog: providerCatalog)
    }
    .sheet(item: $secretAccount) { account in ProviderSecretEditor(account: account, settings: settings) }
    .sheet(item: $defaultEditor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
    .confirmationDialog("Clear provider secret?", isPresented: Binding(get: { clearAccount != nil }, set: { if !$0 { clearAccount = nil } })) {
      Button("Clear secret", role: .destructive) {
        if let account = clearAccount { Task { await settings.clearProviderSecret(providerAccountID: account.providerAccountID) } }
        clearAccount = nil
      }
      Button("Cancel", role: .cancel) { clearAccount = nil }
    }
    .confirmationDialog("Delete provider account?", isPresented: Binding(get: { deleteAccount != nil }, set: { if !$0 { deleteAccount = nil } })) {
      Button("Delete account", role: .destructive) {
        if let account = deleteAccount { Task { await settings.deleteProviderAccount(providerAccountID: account.providerAccountID) } }
        deleteAccount = nil
      }
      Button("Cancel", role: .cancel) { deleteAccount = nil }
    }
  }

  private var providerCatalog: [SettingsProviderCatalog] {
    (settings.snapshot?.providerAccountCatalog ?? []).map {
      SettingsProviderCatalog(providerKind: $0.providerKind, displayName: $0.displayName, preferredAuthMethod: $0.preferredAuthMethod.rawValue, supportedAuthMethods: $0.supportedAuthMethods.map(\.rawValue))
    }
  }

  private func providerAccount(_ value: NoemaAPI.SettingsSnapshotQuery.Data.ProviderAccount) -> SettingsProviderAccount {
    SettingsProviderAccount(providerAccountID: value.providerAccountId, providerKind: value.providerKind, displayName: value.displayName, authMethod: value.authMethod, status: value.status.rawValue, isActive: value.isActive, isDefault: value.isDefault, lastError: value.lastErrorMessage)
  }

  private func defaultModelOptions(_ snapshot: NoemaAPI.SettingsSnapshotQuery.Data) -> [SettingsModelOption] {
    var values = snapshot.agents.flatMap { SettingsModel.modelOptions(from: $0.modelOptions) }
    if values.isEmpty { values = SettingsModel.modelOptions(from: snapshot.memorySettings.modelOptions) }
    var seen = Set<String>()
    return values.filter { seen.insert($0.providerAccountId).inserted }
  }
}

private struct ClientsSettings: View {
  let settings: SettingsModel
  let onRevoke: (PairedClient) -> Void

  var body: some View {
    List {
      Section {
        if settings.clients.isEmpty {
          SettingsEmpty(settings: settings, message: "No paired clients found.")
        } else {
          ForEach(settings.clients) { client in
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              HStack {
                Text(client.displayName).font(NoemaFont.bodyEmphasized)
                if client.isCurrent {
                  Text("This device").font(NoemaFont.caption).foregroundStyle(NoemaColor.accent)
                }
                Spacer()
                if client.isRevoked {
                  Text("Revoked").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
                } else {
                  Button("Revoke", role: .destructive) { onRevoke(client) }
                    .disabled(!settings.canMutate)
                }
              }
              Text("Added \(client.createdAt)")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              Text(client.id)
                .font(NoemaFont.mono)
                .foregroundStyle(NoemaColor.contentTertiary)
            }
            .padding(.vertical, NoemaSpacing.xs)
          }
        }
      } header: {
        Text("Paired clients")
      } footer: {
        Text("Revoking a client closes its active sessions. The current iPhone disconnects only after Noema confirms the revocation.")
      }
    }
    .listStyle(.insetGrouped)
  }
}
