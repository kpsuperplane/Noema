import NoemaAPI
import ApolloAPI
import SwiftUI

struct SettingsRootView: View {
  private let model: NoemaAppModel
  @Environment(NoemaShellCoordinator.self) private var shell
  @State private var settings = SettingsModel()
  @State private var selection: SettingsSection = .agents
  @State private var revocationTarget: PairedClient?

  init(model: NoemaAppModel) {
    self.model = model
  }

  var body: some View {
    SettingsDetail(
      section: selection,
      settings: settings,
      appModel: model,
      onRevoke: { revocationTarget = $0 }
    )
    .task {
      installShellNavigation()
      await settings.load(client: model.graphQLClient?.client)
      installShellNavigation()
    }
    .onChange(of: model.recoveryGeneration) { _, _ in
      Task {
        await settings.load(client: model.graphQLClient?.client)
        installShellNavigation()
      }
    }
    .onAppear { installShellNavigation() }
    .onChange(of: selection) { _, _ in installShellNavigation() }
    .sheet(item: $revocationTarget) { client in
      ClientRevocationSheet(client: client, settings: settings, appModel: model)
    }
  }

  private func installShellNavigation() {
    let actions: [(SettingsSection, String, String)] = [
      (.agents, "Agents", "person.2"),
      (.memory, "Memory", "brain"),
      (.web, "Web", "globe"),
      (.apis, "APIs", "cable.connector"),
      (.mcps, "MCPs", "bolt.horizontal.circle"),
      (.privacy, "Privacy", "hand.raised"),
      (.execution, "Execution", "gauge.with.dots.needle.67percent"),
      (.localModels, "Local Models", "cpu"),
      (.providers, "Providers", "server.rack"),
      (.clients, "Clients", "iphone")
    ]
    var entries: [NoemaSidebarEntry] = []
    for (index, value) in actions.enumerated() {
      if index == 2 { entries.append(.group("Tools")) }
      if index == 5 { entries.append(.group("Safety")) }
      if index == 7 { entries.append(.group("System")) }
      let section = value.0
      entries.append(.item(
        id: "settings-\(section.rawValue)",
        label: value.1,
        symbol: value.2,
        selected: selection == section
      ) {
        selection = section
      })
    }
    shell.show(NoemaSecondaryNavigation(title: selection.title, symbol: selection.symbol, entries: entries))
  }
}

private struct SettingsDetail: View {
  let section: SettingsSection
  let settings: SettingsModel
  let appModel: NoemaAppModel
  let onRevoke: (PairedClient) -> Void

  var body: some View {
    SettingsPage {
      switch section {
      case .agents: AgentsSettings(settings: settings)
      case .memory: MemorySettings(settings: settings)
      case .web: WebSettings(settings: settings)
      case .apis: CapabilitySettings(settings: settings, kind: .api)
      case .mcps: CapabilitySettings(settings: settings, kind: .mcp)
      case .privacy: PrivacySettings(settings: settings)
      case .usage, .execution: ExecutionSettings(settings: settings)
      case .localModels: LocalModelsSettings(settings: settings)
      case .providers: ProvidersSettings(settings: settings)
      case .clients: ClientsSettings(settings: settings, profile: appModel.profile, onRevoke: onRevoke)
      }
    }
    .navigationTitle(section.title)
    .navigationBarTitleDisplayMode(.inline)
  }
}

private struct SettingsPage<Content: View>: View {
  private let content: Content
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  init(@ViewBuilder content: () -> Content) {
    self.content = content()
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        content
      }
      .padding(.horizontal, horizontalSizeClass == .compact ? NoemaSpacing.md : NoemaSpacing.lg)
      .padding(.vertical, NoemaSpacing.lg)
      .frame(maxWidth: 860, alignment: .leading)
      .frame(maxWidth: .infinity, alignment: .center)
    }
    .background(NoemaColor.surface)
    .scrollContentBackground(.hidden)
  }
}

struct SettingsSectionCard<Content: View>: View {
  let title: String?
  let footer: String?
  private let content: Content

  init(_ title: String? = nil, footer: String? = nil, @ViewBuilder content: () -> Content) {
    self.title = title
    self.footer = footer
    self.content = content()
  }

  var body: some View {
    NoemaOpaqueSurface {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        if let title {
          Text(title)
            .font(NoemaFont.sectionTitle)
            .foregroundStyle(NoemaColor.content)
        }
        content
        if let footer {
          Text(footer)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
      }
      .padding(NoemaSpacing.md)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(NoemaColor.surface)
      .overlay {
        RoundedRectangle(cornerRadius: NoemaRadius.container)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
  }
}

struct SettingsRow<Content: View>: View {
  private let content: Content

  init(@ViewBuilder content: () -> Content) {
    self.content = content()
  }

  var body: some View {
    content
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(.vertical, NoemaSpacing.sm)
  }
}

struct SettingsRowDivider: View {
  var body: some View {
    NoemaDivider()
      .padding(.vertical, NoemaSpacing.xxs)
  }
}

struct SettingsAction: View {
  let title: String
  let symbol: String?
  let role: ButtonRole?
  let disabled: Bool
  let action: () -> Void

  var body: some View {
    Button(role: role, action: action) {
      if let symbol {
        Label(title, systemImage: symbol)
      } else {
        Text(title)
      }
    }
    .font(NoemaFont.captionEmphasized)
    .buttonStyle(.borderless)
    .tint(role == .destructive ? NoemaColor.danger : NoemaColor.accent)
    .disabled(disabled)
  }
}

private struct AgentsSettings: View {
  let settings: SettingsModel

  var body: some View {
    let agents = settings.snapshot?.agents ?? []
    let visibleAgents = agents.filter { $0.agentId != "agent:task-executor" }
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsSectionCard("Registered agents") {
        if settings.isLoading && agents.isEmpty {
          NoemaInlineState(message: "Loading agents…", symbol: "arrow.triangle.2.circlepath")
        } else if let error = settings.errorMessage, agents.isEmpty {
          NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
        } else if visibleAgents.isEmpty {
          NoemaInlineState(message: "No agents are available.", symbol: "person.2")
        } else {
          ForEach(Array(visibleAgents.enumerated()), id: \.element.agentId) { index, agent in
            if index > 0 { SettingsRowDivider() }
            agentRow(agent, title: agent.displayName ?? (agent.isPrimary ? "Primary agent" : "Agent"))
          }
        }
      }
      SettingsSectionCard {
        HStack(spacing: NoemaSpacing.sm) {
          Text("Task models")
            .font(NoemaFont.sectionTitle)
          Spacer(minLength: NoemaSpacing.sm)
          NoemaStatusToken(
            text: "\(settings.taskModelPools.filter(\.enabled).count)/3 enabled",
            tone: settings.taskModelPools.contains(where: \.enabled) ? .success : .warning
          )
        }
        let options = SettingsModel.modelOptions(
          from: agents.first(where: { $0.isPrimary })?.modelOptions ?? []
        )
        if settings.isLoading && settings.taskModelPools.isEmpty {
          NoemaInlineState(message: "Loading task models…", symbol: "arrow.triangle.2.circlepath")
        } else if settings.taskModelPools.isEmpty {
          NoemaInlineState(message: "Task model settings are unavailable.", symbol: "exclamationmark.triangle", tone: .warning)
        } else {
          ForEach(Array(settings.taskModelPools.sorted(by: { $0.sortOrder < $1.sortOrder }).enumerated()), id: \.element.id) { index, pool in
            if index > 0 { SettingsRowDivider() }
            taskPoolRow(pool, options: options)
          }
        }
      }
    }
  }

  private func taskPoolRow(_ pool: SettingsTaskModelPool, options: [SettingsModelOption]) -> some View {
    SettingsRow {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        HStack(spacing: NoemaSpacing.sm) {
          Text(pool.displayName)
            .font(NoemaFont.bodyEmphasized)
          Toggle("", isOn: Binding(
            get: { pool.enabled },
            set: { enabled in Task { await settings.updateTaskModelPool(pool, enabled: enabled) } }
          ))
          .labelsHidden()
          .tint(NoemaColor.clay600)
          .disabled(!settings.canMutate)
          Text("Enabled")
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
          Spacer(minLength: NoemaSpacing.sm)
        }
        SettingsInlineModelControls(
          preference: pool.preference,
          options: options,
          enabled: settings.canMutate
        ) { option, profile, reasoning in
          await settings.updateTaskModelPool(
            pool,
            preference: SettingsPreference(
              providerKind: option.providerKind,
              providerAccountId: option.providerAccountId,
              modelProfile: profile.id,
              reasoningEffort: reasoning,
              selectionMode: NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue
            )
          )
        }
      }
    }
  }

  @ViewBuilder
  private func agentRow(
    _ agent: NoemaAPI.SettingsSnapshotQuery.Data.Agent,
    title: String
  ) -> some View {
    SettingsRow {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        HStack(spacing: NoemaSpacing.sm) {
          Text(title).font(NoemaFont.bodyEmphasized)
          if agent.isPrimary { NoemaStatusToken(text: "Primary", tone: .success) }
          Spacer(minLength: NoemaSpacing.sm)
        }
        let preference = SettingsModel.preference(from: agent.modelPreference)
        SettingsInlineModelControls(
          preference: preference,
          options: SettingsModel.modelOptions(from: agent.modelOptions),
          enabled: settings.canMutate
        ) { option, profile, reasoning in
          await settings.saveAgentModelPreference(
            agentID: agent.agentId,
            providerAccountID: option.providerAccountId,
            selectionMode: NoemaAPI.ModelPreferenceSelectionMode.explicitProfile.rawValue,
            modelProfile: profile.id,
            reasoningEffort: reasoning
          )
        }
      }
    }
  }
}

private struct SettingsInlineModelControls: View {
  let preference: SettingsPreference?
  let options: [SettingsModelOption]
  let enabled: Bool
  let save: (SettingsModelOption, SettingsModelProfile, String?) async -> Bool

  private var option: SettingsModelOption? {
    options.first { $0.providerAccountId == preference?.providerAccountId } ?? options.first
  }

  private var profile: SettingsModelProfile? {
    guard let option else { return nil }
    return option.profiles.first { $0.id == preference?.modelProfile } ?? option.profiles.first
  }

  private var reasoning: String? {
    preference?.reasoningEffort ?? profile?.defaultReasoningEffort ?? profile?.reasoningEfforts.first
  }

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Menu {
        ForEach(options) { option in
          ForEach(option.profiles.filter { $0.disabledReason == nil }) { profile in
            Button(profile.label) {
              Task { _ = await save(option, profile, profile.defaultReasoningEffort ?? profile.reasoningEfforts.first) }
            }
          }
        }
      } label: {
        HStack(spacing: NoemaSpacing.sm) {
          Image(systemName: "sparkles")
            .foregroundStyle(NoemaColor.clay600)
          Text(profile?.label ?? "No model available")
            .foregroundStyle(NoemaColor.content)
            .lineLimit(1)
          Spacer(minLength: NoemaSpacing.xs)
          Image(systemName: "chevron.down")
            .font(NoemaFont.metadata)
            .foregroundStyle(NoemaColor.contentTertiary)
        }
        .padding(.horizontal, NoemaSpacing.md)
        .frame(maxWidth: .infinity, minHeight: 34)
        .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
        .overlay {
          RoundedRectangle(cornerRadius: NoemaRadius.element)
            .stroke(NoemaColor.separator, lineWidth: 1)
        }
      }
      .buttonStyle(.plain)
      .disabled(!enabled || profile == nil)

      Menu {
        if let option, let profile {
          ForEach(profile.reasoningEfforts, id: \.self) { effort in
            Button(effort.replacingOccurrences(of: "_", with: " ").capitalized) {
              Task { _ = await save(option, profile, effort) }
            }
          }
        }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          Text(reasoning?.replacingOccurrences(of: "_", with: " ").capitalized ?? "Default")
            .lineLimit(1)
          Spacer(minLength: 0)
          Image(systemName: "chevron.down")
            .font(NoemaFont.metadata)
        }
        .foregroundStyle(NoemaColor.contentTertiary)
        .padding(.horizontal, NoemaSpacing.md)
        .frame(width: 108)
        .frame(minHeight: 34)
        .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
        .overlay {
          RoundedRectangle(cornerRadius: NoemaRadius.element)
            .stroke(NoemaColor.separator, lineWidth: 1)
        }
      }
      .buttonStyle(.plain)
      .disabled(
        !enabled || profile?.reasoningEfforts.isEmpty != false
          || preference?.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue
      )
      .opacity(preference?.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue ? 0.62 : 1)
    }
    .font(NoemaFont.body)
  }
}

private struct MemorySettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsPreferenceTarget?

  var body: some View {
    let options = settings.snapshot?.memorySettings.modelOptions ?? []
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsSectionCard("Background updates") {
        if let preference = settings.snapshot?.memorySettings.modelPreference {
          PreferenceSummary(
            provider: preference.providerKind,
            account: preference.providerAccountId,
            profile: preference.modelProfile,
            mode: preference.selectionMode.rawValue
          )
        } else {
          NoemaInlineState(message: "No memory model selected", symbol: "circle.dashed")
        }
        Text("Memory updates use the selected provider after new conversation source messages arrive.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      SettingsSectionCard("Available models") {
        SettingsAction(title: "Edit model", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || options.isEmpty) {
          editor = SettingsPreferenceTarget(
            id: "memory", title: "Memory model", kind: .memory,
            preference: SettingsModel.preference(from: settings.snapshot?.memorySettings.modelPreference),
            options: SettingsModel.modelOptions(from: options)
          )
        }
        ModelOptions(options: SettingsModel.modelOptions(from: options))
      }
    }
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
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsSectionCard("Search") {
        if let snapshot = settings.snapshot {
          let binding = searchBinding(snapshot.webToolSettings.search)
          WebBindingRow(binding: binding)
          SettingsAction(title: "Edit provider", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || binding.options.isEmpty) {
            bindingEditor = binding
          }
        } else {
          WebBindingRow(binding: nil)
        }
      }
      SettingsSectionCard("Fetch") {
        if let snapshot = settings.snapshot {
          let binding = fetchBinding(snapshot.webToolSettings.fetch)
          WebBindingRow(binding: binding)
          SettingsAction(title: "Edit provider", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || binding.options.isEmpty) {
            bindingEditor = binding
          }
          if let preference = snapshot.webFetchSettings.summarizer.modelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          }
          let target = SettingsPreferenceTarget(
            id: "web-fetch", title: "Web summarizer", kind: .webFetch,
            preference: SettingsModel.preference(from: snapshot.webFetchSettings.summarizer.modelPreference),
            options: SettingsModel.modelOptions(from: snapshot.webFetchSettings.summarizer.modelOptions)
          )
          SettingsAction(title: "Edit summarizer model", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || target.options.isEmpty) {
            preferenceEditor = target
          }
        } else {
          NoemaInlineState(message: "Web settings are unavailable.", symbol: "wifi.slash", tone: .warning)
        }
      }
      NoemaInlineState(message: "Search and fetch providers can be changed independently.", symbol: "info.circle")
    }
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
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if integrations.isEmpty {
        SettingsSectionCard {
          if settings.isLoading {
            NoemaInlineState(message: "Loading \(kind.rawValue)…", symbol: "arrow.triangle.2.circlepath")
          } else if let error = settings.errorMessage {
            NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
          } else {
            NoemaInlineState(message: "No \(kind.rawValue) definitions are available.", symbol: "cable.connector")
          }
        }
      }
      ForEach(integrations) { integration in
        SettingsSectionCard(
          integration.name,
          footer: "Definition \(integration.sourceRevision) · \(integration.sourceSummary)"
        ) {
          HStack(spacing: NoemaSpacing.sm) {
            NoemaStatusToken(text: integration.reviewed ? "Reviewed" : "Needs review", tone: integration.reviewed ? .success : .warning)
            Spacer(minLength: NoemaSpacing.sm)
            Text("\(integration.connections.count) connection(s)")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
          if integration.connections.isEmpty {
            SettingsRowDivider()
            NoemaInlineState(message: "No connection added to this definition.", symbol: "link.badge.plus")
          } else {
            ForEach(Array(integration.connections.enumerated()), id: \.element.id) { index, connection in
              if index > 0 { SettingsRowDivider() }
              SettingsRow {
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  HStack(spacing: NoemaSpacing.sm) {
                    Text(connection.connectionLabel ?? connection.name)
                      .font(NoemaFont.bodyEmphasized)
                    Spacer(minLength: NoemaSpacing.sm)
                    SettingsAction(title: "Edit", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate) {
                      editor = connection
                    }
                  }
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
                }
              }
            }
          }
        }
      }
      if kind == .mcp, let servers = settings.snapshot?.mcpServers, !servers.isEmpty {
        SettingsSectionCard("Server health") {
          ForEach(Array(servers.enumerated()), id: \.element.mcpServerId) { index, server in
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                Text(server.displayName).font(NoemaFont.bodyEmphasized)
                Text("\(server.healthStatus) · \(server.authStatus)")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                Text("Connection \(server.connectionRevision) · Policy v\(server.policyRevision)")
                  .font(NoemaFont.mono)
                  .foregroundStyle(NoemaColor.contentTertiary)
              }
            }
          }
        }
      }
    }
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
    SettingsSectionCard("Governed actions", footer: "When no reviewer model is configured, Noema keeps human approval as the safe default.") {
      if let preference = settings.snapshot?.privacySettings.reviewer.modelPreference {
        PreferenceSummary(
          provider: preference.providerKind,
          account: preference.providerAccountId,
          profile: preference.modelProfile,
          mode: preference.selectionMode.rawValue
        )
      } else {
        NoemaInlineState(message: "Human approval is required when no reviewer model is configured.", symbol: "hand.raised")
      }
      if let snapshot = settings.snapshot {
        let target = SettingsPreferenceTarget(
          id: "privacy", title: "Reviewer model", kind: .privacy,
          preference: SettingsModel.preference(from: snapshot.privacySettings.reviewer.modelPreference),
          options: SettingsModel.modelOptions(from: snapshot.privacySettings.reviewer.modelOptions)
        )
        SettingsAction(title: "Edit reviewer model", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || target.options.isEmpty) {
          editor = target
        }
        Text("\(target.options.count) provider option(s) are available.")
          .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
      }
    }
    .sheet(item: $editor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }
}

private struct UsageSettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsPreferenceTarget?

  var body: some View {
    Group {
      if let snapshot = settings.snapshot {
        let target = SettingsPreferenceTarget(
          id: "usage-progress-audit", title: "Progress-audit model", kind: .usage,
          preference: SettingsModel.preference(from: snapshot.usageSettings.progressAudit.modelPreference),
          options: SettingsModel.modelOptions(from: snapshot.usageSettings.progressAudit.modelOptions)
        )
        SettingsSectionCard("Progress audits") {
          if let preference = snapshot.usageSettings.progressAudit.modelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          } else {
            NoemaInlineState(message: "No progress-audit model selected.", symbol: "circle.dashed")
          }
          SettingsAction(title: "Edit progress-audit model", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || target.options.isEmpty) {
            editor = target
          }
          Text("\(target.options.count) provider option(s) are available.").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
      } else {
        SettingsSectionCard {
          SettingsEmpty(settings: settings, message: "Usage settings are unavailable.")
        }
      }
    }
    .sheet(item: $editor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }
}

private struct ExecutionSettings: View {
  let settings: SettingsModel
  @State private var editor = false
  @State private var progressEditor: SettingsPreferenceTarget?

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if let policy = settings.snapshot?.taskExecutionPolicy {
        SettingsSectionCard("Run limits", footer: "These ceilings apply across Work task execution and review.") {
          SettingsMetricRow(label: "Provider continuations", value: "\(policy.maxProviderContinuations)")
          SettingsMetricRow(label: "Tool calls", value: "\(policy.maxToolCalls)")
          SettingsMetricRow(label: "Active minutes", value: "\(policy.maxActiveMinutes)")
          SettingsMetricRow(label: "Progress-audit interval", value: "\(policy.progressAuditInterval)")
          SettingsMetricRow(label: "Automatic retries", value: "\(policy.maxAutomaticRetries)")
          SettingsMetricRow(label: "Review rounds", value: "\(policy.maxReviewRounds)")
          SettingsAction(title: "Edit limits", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate) { editor = true }
        }
      } else {
        SettingsSectionCard {
          SettingsEmpty(settings: settings, message: "Execution policy is unavailable.")
        }
      }
      if let snapshot = settings.snapshot {
        let target = SettingsPreferenceTarget(
          id: "usage-progress-audit", title: "Progress-audit model", kind: .usage,
          preference: SettingsModel.preference(from: snapshot.usageSettings.progressAudit.modelPreference),
          options: SettingsModel.modelOptions(from: snapshot.usageSettings.progressAudit.modelOptions)
        )
        SettingsSectionCard("Progress audits") {
          if let preference = snapshot.usageSettings.progressAudit.modelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          } else {
            NoemaInlineState(message: "No progress-audit model selected.", symbol: "circle.dashed")
          }
          SettingsAction(title: "Edit progress-audit model", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || target.options.isEmpty) { progressEditor = target }
        }
      }
    }
    .sheet(isPresented: $editor) {
      if let policy = settings.snapshot?.taskExecutionPolicy { ExecutionPolicyEditor(settings: settings, policy: policy) }
    }
    .sheet(item: $progressEditor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }
}
