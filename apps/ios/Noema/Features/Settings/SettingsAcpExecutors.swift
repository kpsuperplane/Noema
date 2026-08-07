import Apollo
import ApolloAPI
import NoemaAPI
import SwiftUI

struct SettingsAcpAuthMethod: Identifiable, Hashable {
  let id: String
  let name: String?
}

struct SettingsAcpAgent: Identifiable, Hashable {
  var id: String { agentID }

  let agentID: String
  let displayName: String
  let command: String
  let arguments: [String]
  let enabled: Bool
  let authStatus: String
  let healthStatus: String
  let implementationName: String?
  let implementationVersion: String?
  let connectionRevision: Int
  let lastError: String?
  let authMethods: [SettingsAcpAuthMethod]

  var implementationDescription: String {
    let implementation = [implementationName, implementationVersion]
      .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }
      .joined(separator: " ")
    return implementation.isEmpty ? command : implementation
  }

  var statusDescription: String {
    let status = "\(statusLabel(healthStatus)) · auth \(statusLabel(authStatus))"
    return lastError.map { "\(status) · \($0)" } ?? status
  }

  private func statusLabel(_ value: String) -> String {
    value.replacingOccurrences(of: "_", with: " ").lowercased()
  }
}

enum SettingsAcpEditorTarget: Identifiable {
  case add
  case edit(SettingsAcpAgent)

  var id: String {
    switch self {
    case .add: "new-acp-agent"
    case .edit(let agent): "edit-\(agent.id)"
    }
  }

  var agent: SettingsAcpAgent? {
    if case .edit(let agent) = self { return agent }
    return nil
  }
}

extension SettingsModel {
  func loadAcpAgents(client: ApolloClient? = nil) async {
    guard let client = client ?? self.client else { return }
    isLoadingAcpAgents = true
    acpErrorMessage = nil
    defer { isLoadingAcpAgents = false }
    do {
      let stream = try client.fetch(query: NoemaAPI.SettingsAcpAgentsQuery(), cachePolicy: .cacheAndNetwork)
      var received = false
      for try await response in stream {
        if let values = response.data?.acpAgents {
          acpAgents = values.map(Self.acpAgent(from:))
          received = true
        }
        if let message = response.errors?.first?.message, !received {
          acpErrorMessage = message
        }
      }
      if !received && acpAgents.isEmpty { acpErrorMessage = "ACP executors could not be loaded." }
    } catch {
      if acpAgents.isEmpty { acpErrorMessage = "ACP executors could not be loaded." }
    }
  }

  @discardableResult
  func createAcpAgent(displayName: String, command: String, arguments: [String]) async -> Bool {
    guard canMutate, let client else { return false }
    return await performAcpMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsCreateAcpAgentMutation(
          input: NoemaAPI.CreateAcpAgentInput(displayName: displayName, command: command, arguments: arguments)
        )
      )
    }
  }

  @discardableResult
  func updateAcpAgent(_ agent: SettingsAcpAgent, displayName: String, command: String, arguments: [String], enabled: Bool) async -> Bool {
    guard canMutate, let client else { return false }
    return await performAcpMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsUpdateAcpAgentMutation(
          input: NoemaAPI.UpdateAcpAgentInput(
            agentId: agent.agentID,
            expectedRevision: Int32(agent.connectionRevision),
            displayName: displayName,
            command: command,
            arguments: arguments,
            enabled: enabled
          )
        )
      )
    }
  }

  @discardableResult
  func testAcpAgent(_ agent: SettingsAcpAgent) async -> Bool {
    guard canMutate, let client else { return false }
    return await performAcpMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsTestAcpAgentMutation(
          input: NoemaAPI.TestAcpAgentInput(agentId: agent.agentID, expectedRevision: Int32(agent.connectionRevision))
        )
      )
    }
  }

  @discardableResult
  func authenticateAcpAgent(_ agent: SettingsAcpAgent, methodID: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performAcpMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsAuthenticateAcpAgentMutation(
          input: NoemaAPI.AuthenticateAcpAgentInput(
            agentId: agent.agentID,
            expectedRevision: Int32(agent.connectionRevision),
            methodId: methodID
          )
        )
      )
    }
  }

  private func performAcpMutation<Mutation: GraphQLMutation>(
    _ operation: () async throws -> GraphQLResponse<Mutation>
  ) async -> Bool {
    let success = await performMutation(operation)
    acpErrorMessage = success ? nil : errorMessage
    return success
  }

  static func acpAgent(from value: NoemaAPI.SettingsAcpAgentsQuery.Data.AcpAgent) -> SettingsAcpAgent {
    SettingsAcpAgent(
      agentID: value.agentId,
      displayName: value.displayName,
      command: value.command,
      arguments: value.arguments,
      enabled: value.enabled,
      authStatus: value.authStatus.rawValue,
      healthStatus: value.healthStatus.rawValue,
      implementationName: value.implementationName,
      implementationVersion: value.implementationVersion,
      connectionRevision: value.connectionRevision,
      lastError: value.lastError,
      authMethods: acpAuthMethods(from: value.capabilities)
    )
  }

  private static func acpAuthMethods(from capabilities: NoemaAPI.JSON) -> [SettingsAcpAuthMethod] {
    guard case .object(let values) = capabilities,
          case .some(.array(let methods)) = values["authMethods"] else { return [] }
    return methods.compactMap { method in
      guard case .object(let fields) = method,
            case .some(.string(let id)) = fields["id"], !id.isEmpty else { return nil }
      let name: String?
      if case .some(.string(let value)) = fields["name"], !value.isEmpty { name = value } else { name = nil }
      return SettingsAcpAuthMethod(id: id, name: name)
    }
  }
}

struct AcpWorkExecutorsSettings: View {
  let settings: SettingsModel
  @State private var editor: SettingsAcpEditorTarget?

  var body: some View {
    SettingsSectionCard {
      HStack(alignment: .center, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text("ACP Work executors")
            .font(NoemaFont.sectionTitle)
          Text("Trusted local commands Noema can launch for Executor runs.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        Spacer(minLength: 0)
        SettingsAction(title: "Add ACP agent", symbol: "plus", role: nil, disabled: !settings.canMutate) {
          editor = .add
        }
      }
      if settings.isLoadingAcpAgents && settings.acpAgents.isEmpty {
        NoemaInlineState(message: "Loading ACP executors…", symbol: "arrow.triangle.2.circlepath")
      } else if let error = settings.acpErrorMessage, settings.acpAgents.isEmpty {
        NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
        SettingsAction(title: "Retry", symbol: "arrow.clockwise", role: nil, disabled: settings.client == nil) {
          Task { await settings.loadAcpAgents() }
        }
      } else if settings.acpAgents.isEmpty {
        NoemaInlineState(message: "No ACP executors are configured.", symbol: "terminal")
      } else {
        ForEach(Array(settings.acpAgents.enumerated()), id: \.element.id) { index, agent in
          if index > 0 { SettingsRowDivider() }
          acpRow(agent)
        }
        if let error = settings.acpErrorMessage {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
      }
    }
    .sheet(item: $editor) { target in
      AcpAgentEditorSheet(target: target, settings: settings)
    }
  }

  @ViewBuilder
  private func acpRow(_ agent: SettingsAcpAgent) -> some View {
    SettingsRow {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
          Text(agent.displayName).font(NoemaFont.bodyEmphasized)
          NoemaStatusToken(text: "ACP", tone: .neutral)
          if !agent.enabled { NoemaStatusToken(text: "Disabled", tone: .warning) }
          Spacer(minLength: NoemaSpacing.sm)
        }
        Text("\(agent.implementationDescription) · \(agent.statusDescription)")
          .font(NoemaFont.caption)
          .foregroundStyle(agent.lastError == nil ? NoemaColor.contentSecondary : NoemaColor.danger)
          .fixedSize(horizontal: false, vertical: true)
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          if agent.authStatus == "REQUIRED" {
            ForEach(agent.authMethods) { method in
              SettingsAction(
                title: "Authenticate with \(method.name ?? method.id)",
                symbol: "person.badge.key",
                role: nil,
                disabled: !settings.canMutate
              ) {
                Task { _ = await settings.authenticateAcpAgent(agent, methodID: method.id) }
              }
            }
          }
          SettingsAction(title: "Test", symbol: "bolt.horizontal", role: nil, disabled: !settings.canMutate) {
            Task { _ = await settings.testAcpAgent(agent) }
          }
          SettingsAction(title: "Edit", symbol: "pencil", role: nil, disabled: !settings.canMutate) {
            editor = .edit(agent)
          }
        }
      }
    }
  }
}

struct AcpAgentEditorSheet: View {
  let target: SettingsAcpEditorTarget
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var displayName: String
  @State private var command: String
  @State private var argumentsText: String
  @State private var enabled: Bool
  @State private var isSaving = false
  @State private var errorMessage: String?
  @State private var discardPresented = false
  @FocusState private var focusedField: Field?

  private enum Field: Hashable { case name }

  private var existing: SettingsAcpAgent? { target.agent }
  private var isDirty: Bool {
    guard let existing else {
      return !displayName.isEmpty || !command.isEmpty || !argumentsText.isEmpty
    }
    return displayName != existing.displayName
      || command != existing.command
      || argumentsText != existing.arguments.joined(separator: "\n")
      || enabled != existing.enabled
  }
  private var canSave: Bool {
    !isSaving
      && settings.canMutate
      && !displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && !command.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  init(target: SettingsAcpEditorTarget, settings: SettingsModel) {
    self.target = target
    self.settings = settings
    let agent = target.agent
    _displayName = State(initialValue: agent?.displayName ?? "")
    _command = State(initialValue: agent?.command ?? "")
    _argumentsText = State(initialValue: agent?.arguments.joined(separator: "\n") ?? "")
    _enabled = State(initialValue: agent?.enabled ?? true)
  }

  var body: some View {
    SettingsBottomSheet(
      title: existing == nil ? "Add ACP agent" : "Edit ACP agent",
      subtitle: "The executable is launched directly with this exact argument array—never through a shell.",
      detent: .large,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Name") {
          TextField("Agent name", text: $displayName)
            .textInputAutocapitalization(.words)
            .settingsSheetControl(focused: focusedField == .name)
            .focused($focusedField, equals: .name)
        }
        SettingsSheetField("Executable") {
          TextField("/absolute/path/to/agent", text: $command)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .settingsSheetControl()
        }
        SettingsSheetField("Arguments (one per line)") {
          TextEditor(text: $argumentsText)
            .frame(minHeight: 100)
            .font(NoemaFont.body)
            .settingsSheetControl()
        }
        if existing != nil {
          Toggle("Enabled for new tasks", isOn: $enabled)
            .tint(NoemaColor.clay600)
        }
        if let errorMessage {
          Text(errorMessage).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(
          primaryTitle: existing == nil ? "Add agent" : "Save",
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
        title: "Discard ACP changes?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) { dismiss() }
    }
    .task { focusedField = .name }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    guard canSave else { return }
    let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
    let executable = command.trimmingCharacters(in: .whitespacesAndNewlines)
    let arguments = argumentsText
      .split(whereSeparator: \.isNewline)
      .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }
    isSaving = true
    Task {
      let success: Bool
      if let existing {
        success = await settings.updateAcpAgent(existing, displayName: name, command: executable, arguments: arguments, enabled: enabled)
      } else {
        success = await settings.createAcpAgent(displayName: name, command: executable, arguments: arguments)
      }
      if success {
        dismiss()
      } else {
        errorMessage = settings.acpErrorMessage ?? settings.errorMessage ?? "Noema could not save the ACP executor."
        isSaving = false
      }
    }
  }
}
