import NoemaAPI
import ApolloAPI
import SwiftUI

struct SettingsRootView: View {
  private let model: NoemaAppModel
  @Environment(NoemaShellCoordinator.self) private var shell
  @State private var settings: SettingsModel
  @State private var selection: SettingsSection = .agents
  @State private var disconnectPresented = false

  init(model: NoemaAppModel, settings: SettingsModel) {
    self.model = model
    _settings = State(initialValue: settings)
  }

  var body: some View {
    SettingsDetail(
      section: selection,
      settings: settings,
      appModel: model,
      notifications: model.notifications,
      liveActivities: model.liveActivities,
      onDisconnect: { disconnectPresented = true }
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
    .noemaSheet(isPresented: $disconnectPresented) {
      SettingsConfirmationSheet(
        title: "Disconnect this app?",
        message: "Noema will revoke this client and remove the saved connection from this device.",
        confirmTitle: "Disconnect"
      ) {
        model.disconnect()
      }
    }
    .alert("Could not disconnect", isPresented: Binding(
      get: { model.disconnectError != nil },
      set: { if !$0 { model.clearDisconnectError() } }
    )) {
      Button("Cancel", role: .cancel) { model.clearDisconnectError() }
      Button("Unpair Anyway", role: .destructive) { model.disconnectLocally() }
    } message: {
      Text("\(model.disconnectError ?? "The server could not be updated.") Unpairing anyway removes this app’s saved connection and offline cache. The server can continue to send notifications until you revoke this client there.")
    }
  }

  private func installShellNavigation() {
    let sections: [SettingsSection] = [
      .agents,
      .memory,
      .web,
      .apis,
      .mcps,
      .privacy,
      .execution,
      .localModels,
      .providers,
      .notifications,
      .clients
    ]
    var entries: [NoemaSidebarEntry] = []
    for (index, section) in sections.enumerated() {
      if index == 2 { entries.append(.group("Tools")) }
      if index == 5 { entries.append(.group("Safety")) }
      if index == 7 { entries.append(.group("System")) }
      entries.append(.item(
        id: "settings-\(section.rawValue)",
        label: section.title,
        icon: section.icon,
        selected: selection == section
      ) {
        selection = section
      })
    }
    shell.show(
      NoemaSecondaryNavigation(title: selection.title, icon: selection.icon, entries: entries),
      for: .settings
    )
  }
}

private struct SettingsDetail: View {
  let section: SettingsSection
  let settings: SettingsModel
  let appModel: NoemaAppModel
  let notifications: NoemaNotificationService
  let liveActivities: NoemaLiveActivityService
  let onDisconnect: () -> Void

  var body: some View {
    SettingsPage {
      switch section {
      case .agents: AgentsSettings(settings: settings)
      case .memory: MemorySettings(settings: settings)
      case .web: WebSettings(settings: settings)
      case .apis: CapabilitySettings(settings: settings, kind: .api, appModel: appModel)
      case .mcps: CapabilitySettings(settings: settings, kind: .mcp, appModel: appModel)
      case .privacy: PrivacySettings(settings: settings)
      case .usage, .execution: ExecutionSettings(settings: settings)
      case .localModels: LocalModelsSettings(settings: settings)
      case .providers: ProvidersSettings(settings: settings)
      case .notifications:
        ClientNotificationsSettings(notifications: notifications)
        ClientLiveActivitiesSettings(liveActivities: liveActivities)
      case .clients:
        ClientsSettings(
          settings: settings,
          profile: appModel.profile,
          appModel: appModel,
          onDisconnect: onDisconnect
        )
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
      .padding(.top, NoemaSpacing.sm)
      .padding(.bottom, NoemaSpacing.lg)
      .frame(maxWidth: 860, alignment: .leading)
      .frame(maxWidth: .infinity, alignment: .center)
    }
    .tracksNoemaSurfaceTop(for: .settings)
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
      VStack(alignment: .leading, spacing: 0) {
        if let title {
          Text(title)
            .font(NoemaFont.sectionTitle)
            .foregroundStyle(NoemaColor.content)
            .padding(.horizontal, NoemaSpacing.md)
            .padding(.vertical, NoemaSpacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
          NoemaDivider()
        }
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          content
          if let footer {
            Text(footer)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
        .padding(NoemaSpacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(NoemaColor.surface)
      .overlay {
        NoemaSuperellipse(cornerRadius: NoemaRadius.container, treatment: .container)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
  }
}

struct SettingsGroupTitle: View {
  let title: String
  let actionTitle: String?
  let actionDisabled: Bool
  let action: (() -> Void)?

  init(
    _ title: String,
    actionTitle: String? = nil,
    actionDisabled: Bool = false,
    action: (() -> Void)? = nil
  ) {
    self.title = title
    self.actionTitle = actionTitle
    self.actionDisabled = actionDisabled
    self.action = action
  }

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Text(title)
        .font(NoemaFont.sectionTitle)
        .foregroundStyle(NoemaColor.content)
      Spacer(minLength: NoemaSpacing.sm)
      if let actionTitle, let action {
        SettingsAction(title: actionTitle, symbol: "plus", role: nil, disabled: actionDisabled, action: action)
      }
    }
    .padding(.horizontal, NoemaSpacing.xs)
  }
}

struct SettingsListCard: View {
  let title: String
  let detail: String?
  let symbol: String
  let status: String?
  let statusTone: NoemaStatusToken.Tone
  let action: () -> Void

  init(
    title: String,
    detail: String? = nil,
    symbol: String,
    status: String? = nil,
    statusTone: NoemaStatusToken.Tone = .neutral,
    action: @escaping () -> Void
  ) {
    self.title = title
    self.detail = detail
    self.symbol = symbol
    self.status = status
    self.statusTone = statusTone
    self.action = action
  }

  var body: some View {
    Button(action: action) {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: symbol)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.contentSecondary)
          .frame(width: 32, height: 32)
          .background(NoemaColor.surfaceSecondary, in: NoemaSuperellipse(cornerRadius: NoemaRadius.inner))
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(title)
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.content)
            .multilineTextAlignment(.leading)
          if let detail {
            Text(detail)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
              .multilineTextAlignment(.leading)
          }
        }
        Spacer(minLength: NoemaSpacing.sm)
        if let status {
          NoemaStatusToken(text: status, tone: statusTone)
        }
        Image(systemName: "chevron.right")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentTertiary)
      }
      .padding(NoemaSpacing.md)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(NoemaColor.surface)
      .overlay {
        NoemaSuperellipse(cornerRadius: NoemaRadius.container, treatment: .container)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
    .buttonStyle(.plain)
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
      .padding(.vertical, NoemaSpacing.xs)
  }
}

struct SettingsRowDivider: View {
  var verticalPadding: CGFloat = NoemaSpacing.xxs

  var body: some View {
    NoemaDivider()
      .padding(.vertical, verticalPadding)
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
        if settings.isLoading && settings.snapshot == nil {
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
      AcpWorkExecutorsSettings(settings: settings)
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
        if settings.isLoadingTaskModelPools && !settings.hasLoadedTaskModelPools {
          NoemaInlineState(message: "Loading task models…", symbol: "arrow.triangle.2.circlepath")
        } else if let error = settings.taskModelPoolsErrorMessage, settings.taskModelPools.isEmpty {
          NoemaInlineState(message: error, symbol: "exclamationmark.triangle", tone: .warning)
          SettingsAction(title: "Retry", symbol: "arrow.clockwise", role: nil, disabled: settings.isOffline) {
            Task { await settings.loadTaskModelPools() }
          }
        } else if settings.taskModelPools.isEmpty {
          NoemaInlineState(message: "Task model settings are unavailable.", symbol: "exclamationmark.triangle", tone: .warning)
        } else {
          let complexities = [NoemaAPI.TaskComplexity.simple, .medium, .difficult]
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            ForEach(Array(complexities.enumerated()), id: \.element) { index, complexity in
              if index > 0 { SettingsRowDivider() }
              if let pool = settings.taskModelPools.first(where: { $0.complexity == complexity }) {
                taskPoolRow(pool, options: options)
              } else {
                SettingsRow {
                  VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                    Text(taskComplexityLabel(complexity)).font(NoemaFont.bodyEmphasized)
                    Text("This task model setting is unavailable.")
                      .font(NoemaFont.caption)
                      .foregroundStyle(NoemaColor.contentSecondary)
                  }
                }
              }
            }
          }
        }
        if let error = settings.taskModelPoolsErrorMessage, !settings.taskModelPools.isEmpty {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
      }
      if let error = settings.errorMessage, settings.snapshot != nil, !settings.isLoading, !settings.isMutating {
        NoemaInlineState(message: error, symbol: "exclamationmark.triangle", tone: .warning)
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
          .toggleStyle(SettingsCompactToggleStyle())
          .accessibilityLabel("Enabled")
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
          useCase: taskModelUseCase(pool.complexity),
          enabled: settings.canMutate
        ) { option, profile, reasoning, selectionMode, fastMode in
          await settings.updateTaskModelPool(
            pool,
            preference: SettingsPreference(
              providerKind: option.providerKind,
              providerAccountId: option.providerAccountId,
              modelProfile: profile,
              reasoningEffort: reasoning,
              selectionMode: selectionMode.rawValue,
              fastMode: fastMode
            )
          )
        }
      }
    }
  }

  private func taskComplexityLabel(_ complexity: NoemaAPI.TaskComplexity) -> String {
    switch complexity {
    case .simple: "Simple"
    case .medium: "Medium"
    case .difficult: "High"
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
          useCase: agent.isPrimary ? .primary : .taskReviewer,
          enabled: settings.canMutate
        ) { option, profile, reasoning, selectionMode, fastMode in
          await settings.saveAgentModelPreference(
            agentID: agent.agentId,
            providerAccountID: option.providerAccountId,
            selectionMode: selectionMode.rawValue,
            modelProfile: profile,
            reasoningEffort: reasoning,
            fastMode: fastMode
          )
        }
      }
    }
  }

  private func taskModelUseCase(_ complexity: NoemaAPI.TaskComplexity) -> NoemaAPI.NoemaModelUseCase {
    switch complexity {
    case .simple: .taskSimple
    case .medium: .taskMedium
    case .difficult: .taskDifficult
    }
  }
}

private struct MemorySettings: View {
  let settings: SettingsModel
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @State private var saveError: String?

  var body: some View {
    let preference = SettingsModel.preference(from: settings.snapshot?.memorySettings.modelPreference)
    let options = SettingsModel.modelOptions(from: settings.snapshot?.memorySettings.modelOptions ?? [])
    SettingsSectionCard {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        Text("Background updates")
          .font(NoemaFont.sectionTitle)
          .foregroundStyle(NoemaColor.content)
        Spacer(minLength: NoemaSpacing.sm)
        Text("Local human only")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }

      SettingsRow {
        if horizontalSizeClass == .compact {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            Text("Consolidation model").font(NoemaFont.body)
            modelControls(preference: preference, options: options)
          }
        } else {
          HStack(spacing: NoemaSpacing.md) {
            Text("Consolidation model").font(NoemaFont.body)
            Spacer(minLength: NoemaSpacing.md)
            modelControls(preference: preference, options: options)
              .frame(maxWidth: 380)
          }
        }
      }

      if let saveError {
        Text(saveError)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.danger)
          .accessibilityLabel("Error: \(saveError)")
      }
      if let warning = preferenceWarning(preference: preference, options: options) {
        Text(warning)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.warning)
      }
    }
  }

  private func modelControls(
    preference: SettingsPreference?,
    options: [SettingsModelOption]
  ) -> some View {
    SettingsInlineModelControls(
      preference: preference,
      options: options,
      useCase: .memoryConsolidation,
      enabled: settings.canMutate
    ) { option, profile, reasoning, selectionMode, fastMode in
      let saved = await settings.saveMemoryModelPreference(
        providerAccountID: option.providerAccountId,
        selectionMode: selectionMode.rawValue,
        modelProfile: profile,
        reasoningEffort: reasoning,
        fastMode: fastMode
      )
      saveError = saved ? nil : "Noema could not save the memory update model."
      return saved
    }
  }

  private func preferenceWarning(
    preference: SettingsPreference?,
    options: [SettingsModelOption]
  ) -> String? {
    guard let preference else { return nil }
    guard let provider = options.first(where: { $0.providerAccountId == preference.providerAccountId }) else {
      return "Selected provider is not available."
    }
    if preference.selectionMode == NoemaAPI.ModelPreferenceSelectionMode.noemaRecommended.rawValue {
      let recommendation = provider.recommendations.first { $0.useCase == .memoryConsolidation }
      return recommendation?.disabledReason ?? provider.disabledReason
        ?? (recommendation == nil ? "Noema has no recommendation for this setting." : nil)
    }
    guard let profile = provider.profiles.first(where: { $0.id == preference.modelProfile }) else {
      return "Selected model is not available."
    }
    if let effort = preference.reasoningEffort, !profile.reasoningEfforts.contains(effort) {
      return "Selected reasoning effort is not available."
    }
    if preference.reasoningEffort == nil, !profile.reasoningEfforts.isEmpty {
      return "Selected model requires a reasoning effort."
    }
    return profile.disabledReason ?? provider.disabledReason
  }
}

private struct WebSettings: View {
  let settings: SettingsModel
  @State private var searchSaveError: String?
  @State private var fetchSaveError: String?
  @State private var browseSaveError: String?

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsSectionCard {
        webSectionHeader("Search")
        if let snapshot = settings.snapshot {
          let binding = searchBinding(snapshot.webToolSettings.search)
          WebProviderInlineRow(binding: binding, settings: settings, saveError: $searchSaveError)
          if let searchSaveError {
            Text(searchSaveError).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
          }
          technicalDetails([
            ("Tool", "web.search"),
            ("Data flow", activeOption(binding)?.dataFlowClass ?? "Unavailable"),
            ("Citations", activeOption(binding)?.citations == true ? "Supported" : "Unavailable")
          ])
        } else {
          SettingsEmpty(settings: settings, message: "Search settings are unavailable.")
        }
      }
      SettingsSectionCard {
        webSectionHeader("Fetch")
        if let snapshot = settings.snapshot {
          let binding = fetchBinding(snapshot.webToolSettings.fetch)
          WebProviderInlineRow(binding: binding, settings: settings, saveError: $fetchSaveError)
          SettingsRowDivider()
          SettingsRow {
            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              Text("Summarizer model").font(NoemaFont.bodyEmphasized)
              let preference = SettingsModel.preference(from: snapshot.webFetchSettings.summarizer.modelPreference)
              let options = SettingsModel.modelOptions(from: snapshot.webFetchSettings.summarizer.modelOptions)
              SettingsInlineModelControls(
                preference: preference,
                options: options,
                useCase: .webFetchSummarizer,
                enabled: settings.canMutate
              ) { option, profile, reasoning, selectionMode, fastMode in
                let saved = await settings.saveWebFetchSummarizerPreference(
                  providerAccountID: option.providerAccountId,
                  selectionMode: selectionMode.rawValue,
                  modelProfile: profile,
                  reasoningEffort: reasoning,
                  fastMode: fastMode
                )
                fetchSaveError = saved ? nil : "Noema could not save the fetch summarizer model."
                return saved
              }
            }
          }
          if let fetchSaveError {
            Text(fetchSaveError).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
          }
          technicalDetails([
            ("Tool", "web.fetch"),
            ("Contract", activeOption(binding)?.reliabilityContract ?? "No provider configured"),
            ("Extraction", "readabilityrs Markdown"),
            ("Safety", "Public HTTP(S), checked redirects, private/local targets blocked, response size caps."),
            ("Data flow", activeOption(binding)?.dataFlowClass ?? "Unavailable")
          ])
        } else {
          SettingsEmpty(settings: settings, message: "Fetch settings are unavailable.")
        }
      }
      SettingsSectionCard {
        webSectionHeader("Browse")
        if let snapshot = settings.snapshot {
          let binding = browseBinding(snapshot.webToolSettings.browse)
          WebProviderInlineRow(binding: binding, settings: settings, saveError: $browseSaveError)
          if let browseSaveError {
            Text(browseSaveError).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
          }
          technicalDetails([
            ("Tool", "web.browse"),
            ("Contract", activeOption(binding)?.reliabilityContract ?? "No provider configured"),
            ("Session", "Execution-scoped with 15-minute idle expiry"),
            ("Safety", "Public network only"),
            ("Data flow", activeOption(binding)?.dataFlowClass ?? "Unavailable")
          ])
        } else {
          SettingsEmpty(settings: settings, message: "Browse settings are unavailable.")
        }
      }
    }
  }

  private func webSectionHeader(_ title: String) -> some View {
    HStack(spacing: NoemaSpacing.sm) {
      Text(title).font(NoemaFont.sectionTitle)
      Spacer(minLength: NoemaSpacing.sm)
      NoemaStatusToken(text: "Enabled", tone: .success)
    }
  }

  private func technicalDetails(_ rows: [(String, String)]) -> some View {
    DisclosureGroup("Technical details") {
      VStack(spacing: NoemaSpacing.xs) {
        ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
          if index > 0 { SettingsRowDivider() }
          SettingsMetricRow(label: row.0, value: row.1)
        }
      }
      .padding(.top, NoemaSpacing.xs)
    }
    .font(NoemaFont.caption)
    .foregroundStyle(NoemaColor.contentSecondary)
  }

  private func activeOption(_ binding: SettingsWebBinding) -> SettingsWebOption? {
    binding.options.first { $0.providerAccountID == binding.activeProviderAccountID } ?? binding.options.first
  }

  private func searchBinding(_ value: NoemaAPI.SettingsSnapshotQuery.Data.WebToolSettings.Search) -> SettingsWebBinding {
    SettingsWebBinding(toolName: value.toolName, capabilityID: value.capabilityId, activeProviderAccountID: value.activeProviderAccountId, options: value.providerOptions.map { SettingsWebOption(providerAccountID: $0.providerAccountId, providerKind: $0.providerKind, displayName: $0.displayName, reliabilityContract: $0.reliabilityContract, dataFlowClass: $0.dataFlowClass, citations: $0.citations, directURLFetch: $0.directUrlFetch) })
  }

  private func fetchBinding(_ value: NoemaAPI.SettingsSnapshotQuery.Data.WebToolSettings.Fetch) -> SettingsWebBinding {
    SettingsWebBinding(toolName: value.toolName, capabilityID: value.capabilityId, activeProviderAccountID: value.activeProviderAccountId, options: value.providerOptions.map { SettingsWebOption(providerAccountID: $0.providerAccountId, providerKind: $0.providerKind, displayName: $0.displayName, reliabilityContract: $0.reliabilityContract, dataFlowClass: $0.dataFlowClass, citations: $0.citations, directURLFetch: $0.directUrlFetch) })
  }

  private func browseBinding(_ value: NoemaAPI.SettingsSnapshotQuery.Data.WebToolSettings.Browse) -> SettingsWebBinding {
    SettingsWebBinding(toolName: value.toolName, capabilityID: value.capabilityId, activeProviderAccountID: value.activeProviderAccountId, options: value.providerOptions.map { SettingsWebOption(providerAccountID: $0.providerAccountId, providerKind: $0.providerKind, displayName: $0.displayName, reliabilityContract: $0.reliabilityContract, dataFlowClass: $0.dataFlowClass, citations: $0.citations, directURLFetch: $0.directUrlFetch) })
  }
}

private struct WebProviderInlineRow: View {
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  let binding: SettingsWebBinding
  let settings: SettingsModel
  @Binding var saveError: String?

  var body: some View {
    SettingsRow {
      Group {
        if horizontalSizeClass == .compact {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) { label; providerMenu }
        } else {
          HStack(spacing: NoemaSpacing.md) { label; Spacer(minLength: NoemaSpacing.sm); providerMenu.frame(maxWidth: 260) }
        }
      }
    }
  }

  private var label: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
      Text("Provider").font(NoemaFont.bodyEmphasized)
      Text(activeOption?.displayName ?? "No provider configured")
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
    }
  }

  private var providerMenu: some View {
    Menu {
      ForEach(binding.options) { option in
        Button(option.displayName) {
          guard option.providerAccountID != binding.activeProviderAccountID else { return }
          Task {
            let saved = await settings.saveWebToolProviderBinding(
              toolName: binding.toolName,
              capabilityID: binding.capabilityID,
              providerAccountID: option.providerAccountID
            )
            saveError = saved ? nil : "Noema could not save the \(binding.toolName) provider binding."
          }
        }
      }
    } label: {
      HStack(spacing: NoemaSpacing.sm) {
        Text(activeOption?.displayName ?? (binding.options.isEmpty ? "No providers available" : "Select provider"))
          .lineLimit(1)
        Spacer(minLength: NoemaSpacing.xs)
        Image(systemName: "chevron.down").font(NoemaFont.metadata)
      }
      .padding(.horizontal, NoemaSpacing.md)
      .frame(maxWidth: .infinity, minHeight: 34)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .overlay { NoemaSuperellipse(cornerRadius: NoemaRadius.element).stroke(NoemaColor.separator, lineWidth: 1) }
    }
    .buttonStyle(.plain)
    .disabled(!settings.canMutate || binding.options.isEmpty)
  }

  private var activeOption: SettingsWebOption? {
    binding.options.first { $0.providerAccountID == binding.activeProviderAccountID } ?? binding.options.first
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

struct SettingsIntegration: Identifiable {
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
  let appModel: NoemaAppModel
  @Environment(NoemaShellCoordinator.self) private var shell
  @State private var editor: SettingsIntegrationConnection?
  @State private var addTarget: SettingsIntegration?
  @State private var pendingPolicyConnectionID: String?
  @State private var startPolicyConnectionID: String?
  @State private var deleteTarget: SettingsIntegration?
  @State private var labelGrant: SettingsAdapterOAuthGrant?
  @State private var labelDraft = ""
  @State private var disconnectGrant: SettingsAdapterOAuthGrant?
  @State private var replaceApplication: SettingsAdapterOAuthApplication?
  @State private var setupPresented = false

  var body: some View {
    Group {
      if kind == .api {
        apiAccountList(integrations(settings.snapshot?.apis ?? []))
      } else {
        capabilityList(integrations(settings.snapshot?.mcps ?? []))
      }
    }
    .noemaSheet(item: $editor, onDismiss: { startPolicyConnectionID = nil }) { connection in
      CapabilityConnectionEditor(
        connection: connection,
        settings: settings,
        appModel: appModel,
        startPolicyEditing: startPolicyConnectionID == connection.id
      )
    }
    .noemaSheet(item: $addTarget, onDismiss: openPendingPolicy) { integration in
      if kind == .api {
        APIConnectionSheet(
          integration: integration,
          settings: settings,
          onAttached: { pendingPolicyConnectionID = $0 }
        )
      } else {
        MCPConnectionSheet(integration: integration, settings: settings)
      }
    }
    .noemaSheet(item: $deleteTarget) { integration in
      let connectionCount = integration.connections.count
      let connectionNoun = connectionCount == 1 ? "account" : "accounts"
      SettingsMutationConfirmationSheet(
        title: "Delete \(integration.name)?",
        message: connectionCount == 0
          ? "Removes the API definition and its tool setup. You can't undo this. Past activity is kept."
          : "Remove \(connectionCount) connected \(connectionNoun) before deleting this service.",
        confirmTitle: "Delete",
        confirmDisabled: connectionCount > 0
      ) {
        await settings.deleteAdapterService(integration)
      }
    }
    .noemaSheet(isPresented: $setupPresented) {
      MCPSetupSheet(settings: settings, appModel: appModel)
    }
    .noemaSheet(item: $labelGrant) { grant in
      SettingsBottomSheet(title: "Label account", subtitle: grant.providerName, detent: .medium, onClose: { labelGrant = nil }) {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          SettingsSheetField("Account label") { TextField("Account label", text: $labelDraft).settingsSheetControl() }
          HStack(spacing: NoemaSpacing.sm) {
            Spacer(minLength: 0)
            Button("Cancel") { labelGrant = nil }.buttonStyle(NoemaActionButtonStyle(variant: .ghost))
            Button("Save label") {
              Task {
                if await settings.labelAdapterGrant(grant, label: labelDraft.nilIfBlank) {
                  await settings.loadAdapterOAuthState(); labelGrant = nil
                }
              }
            }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
          }
        }
      }
    }
    .noemaSheet(item: $disconnectGrant) { grant in
      SettingsMutationConfirmationSheet(
        title: "Disconnect \(grant.accountLabel ?? "account")?",
        message: "This removes account tokens and disables dependent APIs. API definitions and OAuth client setup stay available.",
        confirmTitle: "Disconnect"
      ) {
        let result = await settings.disconnectAdapterGrant(grant)
        if result { await settings.loadAdapterOAuthState() }
        return result
      }
    }
    .noemaSheet(item: $replaceApplication) { application in
      if let setup = settings.adapterOAuthState?.profiles.first(where: { $0.profileDigest == application.profileDigest })?.credentialSetup {
        AdapterCredentialSetupSheet(
          serviceName: application.providerName,
          title: "Replace \(application.providerName) OAuth client",
          setup: setup,
          scopes: [],
          introduction: "Replace this OAuth client document. \(application.grantCount) grants across \(application.accountCount) accounts will use the new credential.",
          submitTitle: "Replace OAuth client",
          onClose: { replaceApplication = nil },
          onSubmit: { submission in
            guard let document = submission.document,
                  await settings.replaceAdapterOAuthApplication(application, document: document) else {
              throw SettingsError.server(settings.errorMessage ?? "The OAuth client could not be replaced.")
            }
            await settings.loadAdapterOAuthState()
          }
        )
      }
    }
  }

  @ViewBuilder
  private func apiAccountList(_ integrations: [SettingsIntegration]) -> some View {
    let pending = settings.adapterDefinitions.filter { !$0.reviewed && !$0.superseded }
    let reviewed = settings.adapterDefinitions.filter { $0.reviewed && !$0.superseded }
    let available = integrations.filter { $0.connections.isEmpty }
    let oauth = settings.adapterOAuthState
    let grants = oauth?.grants ?? []
    let grantConnectionIDs = Set(grants.flatMap(\.connectionIDs))
    let directIntegrations = integrations.compactMap { integration -> SettingsIntegration? in
      let connections = integration.connections.filter { !grantConnectionIDs.contains($0.id) }
      return connections.isEmpty ? nil : SettingsIntegration(
        id: integration.id,
        name: integration.name,
        sourceRevision: integration.sourceRevision,
        reviewed: integration.reviewed,
        sourceSummary: integration.sourceSummary,
        connections: connections
      )
    }
    let providerGroups = Dictionary(grouping: grants, by: \.providerName)
    let providerNames = providerGroups.keys.sorted()
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if !pending.isEmpty {
        SettingsSectionCard("Review before connecting") {
          ForEach(Array(pending.enumerated()), id: \.element.id) { index, definition in
            if index > 0 { SettingsRowDivider() }
            SettingsAdapterDefinitionReview(definition: definition, settings: settings)
          }
        }
      }
      if pending.isEmpty && reviewed.isEmpty {
        SettingsSectionCard("No APIs set up") {
          Text("Ask Noema in Chat to add Gmail, Google Calendar, or another API. You will review access before it connects.")
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
          Button("Open Chat") { shell.requestedDestination = .chat }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
        }
      }
      if !providerGroups.isEmpty {
        ForEach(providerNames, id: \.self) { providerName in
          let providerIntegration = integrations.first { integration in
            guard let definition = settings.adapterDefinitions.first(where: { $0.semanticDigest == integration.sourceRevision }),
                  let profileDigest = definition.oauthProfileDigest else { return false }
            return oauth?.profiles.first { $0.profileDigest == profileDigest }?.displayName == providerName
              && !definition.connectionActions.isEmpty
          }
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            SettingsGroupTitle(
              providerName,
              actionTitle: providerIntegration == nil ? nil : "Add account",
              actionDisabled: !settings.canMutate,
              action: providerIntegration.map { integration in { addTarget = integration } }
            )
            let providerGrants = providerGroups[providerName] ?? []
            let accountGroups = Dictionary(grouping: providerGrants) { $0.accountID ?? $0.grantID }
            ForEach(accountGroups.keys.sorted(), id: \.self) { accountID in
              let accountGrants = accountGroups[accountID] ?? []
              if let account = accountGrants.first {
                let connectionIDs = Set(accountGrants.flatMap(\.connectionIDs))
                let connections = integrations.flatMap(\.connections).filter { connectionIDs.contains($0.id) }
                VStack(alignment: .leading, spacing: NoemaSpacing.compact) {
                  HStack(spacing: NoemaSpacing.sm) {
                    Image(systemName: "person.crop.circle")
                      .foregroundStyle(NoemaColor.contentSecondary)
                    Text(account.accountLabel ?? "Unlabeled account").font(NoemaFont.taskTitle)
                    Spacer(minLength: NoemaSpacing.sm)
                    if accountGrants.contains(where: { $0.status != "active" }) {
                      NoemaStatusToken(
                        text: "Action required",
                        tone: .warning
                      )
                    }
                    Menu {
                      Button("Rename") {
                        labelDraft = account.accountLabel ?? ""
                        labelGrant = account
                      }
                      Button("Disconnect", role: .destructive) { disconnectGrant = account }
                    } label: {
                      Image(systemName: "ellipsis")
                        .font(NoemaFont.bodyEmphasized)
                        .foregroundStyle(NoemaColor.contentSecondary)
                        .frame(width: 28, height: 28)
                    }
                    .disabled(!settings.canMutate)
                    .accessibilityLabel("Actions for \(account.accountLabel ?? "unlabeled account")")
                  }
                  .padding(.horizontal, NoemaSpacing.xs)
                  ForEach(connections) { connection in
                    SettingsListCard(
                      title: connection.name,
                      detail: connection.authStatus.lowercased() == "active"
                        ? nil
                        : connection.authStatus.replacingOccurrences(of: "_", with: " ").lowercased().capitalized,
                      symbol: apiServiceSymbol(connection.definitionId),
                      status: nil
                    ) { editor = connection }
                  }
                }
              }
            }
          }
        }
      }
      ForEach(directIntegrations) { integration in
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          SettingsGroupTitle(integration.name)
          ForEach(integration.connections) { connection in
            SettingsListCard(
              title: connection.connectionLabel ?? connection.name,
              detail: connection.authStatus.lowercased() == "active"
                ? nil
                : connection.authStatus.replacingOccurrences(of: "_", with: " ").lowercased().capitalized,
              symbol: apiServiceSymbol(connection.definitionId)
            ) { editor = connection }
          }
        }
      }
      if !available.isEmpty {
        SettingsSectionCard("Connect an API") {
          ForEach(Array(available.enumerated()), id: \.element.id) { index, integration in
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              HStack(spacing: NoemaSpacing.sm) {
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  Text(integration.name).font(NoemaFont.bodyEmphasized)
                  Text(apiActionDescription(integration)).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
                }
                Spacer(minLength: NoemaSpacing.sm)
                SettingsAction(title: apiActionTitle(integration), symbol: "plus", role: nil, disabled: !settings.canMutate) {
                  addTarget = integration
                }
              }
            }
          }
        }
      }
      if let applications = oauth?.applications, !applications.isEmpty {
        DisclosureGroup("Advanced OAuth client setup") {
          SettingsSectionCard(footer: "Client secrets are never displayed.") {
            Text("OAuth clients hold provider setup for account authorization. Most people do not need to manage them here.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
            ForEach(Array(applications.enumerated()), id: \.element.id) { index, application in
              if index > 0 { SettingsRowDivider() }
              SettingsRow {
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  Text(application.projectLabel ?? "\(application.providerName) OAuth client").font(NoemaFont.bodyEmphasized)
                  Text("\(application.clientID) · \(application.callbackMode) · \(application.accountCount) accounts")
                    .font(NoemaFont.monoTiny).foregroundStyle(NoemaColor.contentSecondary).textSelection(.enabled)
                  if let redirectURI = oauth?.profiles.first(where: { $0.profileDigest == application.profileDigest })?.credentialSetup?.redirectURI {
                    Text(redirectURI)
                      .font(NoemaFont.monoTiny).foregroundStyle(NoemaColor.contentSecondary).textSelection(.enabled)
                  }
                  HStack(spacing: NoemaSpacing.sm) {
                    SettingsAction(title: "Replace document", symbol: "doc.badge.arrow.up", role: nil, disabled: !settings.canMutate) {
                      replaceApplication = application
                    }
                    SettingsAction(title: "Delete client", symbol: "trash", role: .destructive,
                      disabled: !settings.canMutate || application.grantCount > 0) {
                      Task {
                        if await settings.deleteAdapterOAuthApplication(application) { await settings.loadAdapterOAuthState() }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  private func apiActionTitle(_ integration: SettingsIntegration) -> String {
    guard let action = settings.adapterDefinitions.first(where: { $0.semanticDigest == integration.sourceRevision })?.nextAction else { return "Connect" }
    return switch action.kind {
    case "attach_account": "Connect \(integration.name)"
    case "add_access": "Add access"
    case "add_account":
      settings.adapterOAuthState?.applications
        .first { $0.applicationID == action.applicationID }
        .map { "Add \($0.providerName) account" } ?? "Add account"
    case "import_application": "Set up \(integration.name)"
    default: "Connect"
    }
  }

  private func apiActionDescription(_ integration: SettingsIntegration) -> String {
    guard let action = settings.adapterDefinitions.first(where: { $0.semanticDigest == integration.sourceRevision })?.nextAction else { return integration.sourceSummary }
    return switch action.kind {
    case "attach_account": "Use an account that already has the required access."
    case "add_access": "Approve added access. Current access stays available."
    case "add_account": "Use the existing provider setup. No new document is required."
    case "import_application": "Import the provider's OAuth client document once. Noema will reuse it for compatible APIs."
    default: integration.sourceSummary
    }
  }

  private func apiServiceSymbol(_ definitionID: String) -> String {
    let value = definitionID.lowercased()
    if value.contains("calendar") || value.contains("schedule") { return "calendar" }
    if value.contains("mail") || value.contains("email") { return "envelope" }
    if value.contains("drive") || value.contains("storage") { return "externaldrive" }
    if value.contains("doc") || value.contains("document") { return "doc.text" }
    return "curlybraces.square"
  }

  private func openPendingPolicy() {
    guard let connectionID = pendingPolicyConnectionID else { return }
    pendingPolicyConnectionID = nil
    startPolicyConnectionID = connectionID
    editor = integrations(settings.snapshot?.apis ?? [])
      .flatMap(\.connections)
      .first { $0.id == connectionID }
  }

  @ViewBuilder
  private func capabilityList(_ integrations: [SettingsIntegration]) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if kind == .api {
        let pending = settings.adapterDefinitions.filter { !$0.reviewed && !$0.superseded }
        if !pending.isEmpty {
          SettingsSectionCard("Review before connecting") {
            ForEach(Array(pending.enumerated()), id: \.element.id) { index, definition in
              if index > 0 { SettingsRowDivider() }
              SettingsAdapterDefinitionReview(definition: definition, settings: settings)
            }
          }
        }
      }
      if kind == .mcp {
        SettingsSectionCard {
          SettingsAction(title: "Connect service", symbol: "plus", role: nil, disabled: !settings.canMutate) {
            setupPresented = true
          }
        }
      }
      if integrations.isEmpty {
        SettingsSectionCard {
          if settings.isLoading && settings.snapshot == nil {
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
            SettingsAction(title: "Add", symbol: "plus", role: nil, disabled: !settings.canMutate) {
              addTarget = integration
            }
            if kind == .api {
              SettingsAction(title: "Delete", symbol: "trash", role: .destructive, disabled: !settings.canMutate) {
                deleteTarget = integration
              }
            }
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
                    SettingsAction(title: "Manage", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate) {
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

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}

private struct PrivacySettings: View {
  let settings: SettingsModel

  var body: some View {
    SettingsSectionCard("Risky action reviews") {
      if let snapshot = settings.snapshot {
        let reviewer = snapshot.privacySettings.reviewer
        let options = SettingsModel.modelOptions(from: reviewer.modelOptions)
        SettingsRow {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("Reviewer model").font(NoemaFont.bodyEmphasized)
            if options.isEmpty {
              NoemaInlineState(message: "No reviewer model is available.", symbol: "server.rack", tone: .warning)
            } else {
              SettingsInlineModelControls(
                preference: SettingsModel.preference(from: reviewer.modelPreference),
                options: options,
                useCase: .actionReviewer,
                enabled: settings.canMutate,
                requiresExplicitSelection: true
              ) { option, profile, reasoning, selectionMode, fastMode in
                await settings.saveActionReviewerPreference(
                  providerAccountID: option.providerAccountId,
                  selectionMode: selectionMode.rawValue,
                  modelProfile: profile,
                  reasoningEffort: reasoning,
                  fastMode: fastMode
                )
              }
            }
          }
        }
        if let error = settings.errorMessage, !settings.isMutating {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
      } else {
        SettingsEmpty(settings: settings, message: "Reviewer settings are unavailable.")
      }
    }
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
    .noemaSheet(item: $editor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
  }
}

private struct ExecutionSettings: View {
  let settings: SettingsModel
  @State private var editor = false

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if let policy = settings.snapshot?.taskExecutionPolicy {
        SettingsSectionCard {
          HStack(spacing: NoemaSpacing.sm) {
            Text("Run limits").font(NoemaFont.sectionTitle)
            Spacer(minLength: NoemaSpacing.sm)
            SettingsAction(title: "Edit limits", symbol: nil, role: nil, disabled: !settings.canMutate) { editor = true }
          }
          executionPolicyRow("Provider continuations", value: policy.maxProviderContinuations, description: "Model and tool continuation rounds in one task run.")
          SettingsRowDivider()
          executionPolicyRow("Tool calls", value: policy.maxToolCalls, description: "Total tool calls allowed across the complete run.")
          SettingsRowDivider()
          executionPolicyRow("Active minutes", value: policy.maxActiveMinutes, description: "Time spent executing; queue time does not count.")
          SettingsRowDivider()
          executionPolicyRow("Audit interval", value: policy.progressAuditInterval, description: "Continuation rounds between progress audits.")
        }
      } else {
        SettingsSectionCard {
          SettingsEmpty(settings: settings, message: "Execution policy is unavailable.")
        }
      }
      if let snapshot = settings.snapshot {
        let progressAudit = snapshot.usageSettings.progressAudit
        let options = SettingsModel.modelOptions(from: progressAudit.modelOptions)
        SettingsSectionCard("Progress auditing") {
          SettingsRow {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              Text("Audit model").font(NoemaFont.bodyEmphasized)
              if options.isEmpty {
                NoemaInlineState(message: "No progress-audit model is available.", symbol: "server.rack", tone: .warning)
              } else {
                SettingsInlineModelControls(
                  preference: SettingsModel.preference(from: progressAudit.modelPreference),
                  options: options,
                  useCase: .toolProgressAudit,
                  enabled: settings.canMutate
                ) { option, profile, reasoning, selectionMode, fastMode in
                  await settings.saveToolProgressAuditPreference(
                    providerAccountID: option.providerAccountId,
                    selectionMode: selectionMode.rawValue,
                    modelProfile: profile,
                    reasoningEffort: reasoning,
                    fastMode: fastMode
                  )
                }
              }
            }
          }
        }
      }
    }
    .noemaSheet(isPresented: $editor) {
      if let policy = settings.snapshot?.taskExecutionPolicy { ExecutionPolicyEditor(settings: settings, policy: policy) }
    }
  }

  private func executionPolicyRow(_ label: String, value: Int, description: String) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
      HStack(spacing: NoemaSpacing.sm) {
        Text(label).font(NoemaFont.body)
        Spacer(minLength: NoemaSpacing.sm)
        Text("\(value)").font(NoemaFont.mono).foregroundStyle(NoemaColor.content)
      }
      Text(description).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
    }
  }
}
