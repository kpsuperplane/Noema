import NoemaAPI
import SwiftUI

struct SettingsSheetActions: View {
  let primaryTitle: String
  let isSaving: Bool
  let primaryDisabled: Bool
  let onCancel: () -> Void
  let onPrimary: () -> Void

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Spacer(minLength: 0)
      Button("Cancel") { onCancel() }
        .buttonStyle(.plain)
        .font(NoemaFont.body)
        .disabled(isSaving)
      Button { onPrimary() } label: {
        HStack(spacing: NoemaSpacing.xs) {
          if isSaving { ProgressView().controlSize(.small) }
          Text(primaryTitle)
        }
        .frame(minHeight: 32)
        .padding(.horizontal, NoemaSpacing.md)
      }
      .buttonStyle(.plain)
      .font(NoemaFont.bodyEmphasized)
      .foregroundStyle(NoemaColor.white)
      .background(NoemaColor.clay600, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .opacity(primaryDisabled ? 0.42 : 1)
      .disabled(primaryDisabled)
    }
  }
}

struct WebBindingEditor: View {
  let binding: SettingsWebBinding
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var accountID: String
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Bool

  private var isDirty: Bool { accountID != binding.activeProviderAccountID }

  init(binding: SettingsWebBinding, settings: SettingsModel) {
    self.binding = binding
    self.settings = settings
    _accountID = State(initialValue: binding.activeProviderAccountID)
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Web provider",
      subtitle: "Choose the provider used by \(binding.toolName).",
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Provider") {
          Picker("Provider", selection: $accountID) {
            ForEach(binding.options) { option in
              Text("\(option.displayName) · \(option.providerKind)").tag(option.providerAccountID)
            }
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
          .settingsSheetControl(focused: focusedField)
          .focused($focusedField)
        }
        if binding.options.isEmpty {
          NoemaInlineState(message: "No provider binding is available.", symbol: "server.rack", tone: .warning)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(
          primaryTitle: "Save",
          isSaving: isSaving,
          primaryDisabled: isSaving || accountID.isEmpty || !settings.canMutate,
          onCancel: requestDismissal,
          onPrimary: save
        )
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .sheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard provider changes?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
    .task { focusedField = binding.options.isEmpty ? false : true }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    guard !accountID.isEmpty, settings.canMutate else { return }
    isSaving = true
    Task {
      let ok = await settings.saveWebToolProviderBinding(toolName: binding.toolName, capabilityID: binding.capabilityID, providerAccountID: accountID)
      if ok { dismiss() } else { isSaving = false }
    }
  }
}

struct ExecutionPolicyEditor: View {
  let settings: SettingsModel
  let policy: NoemaAPI.SettingsSnapshotQuery.Data.TaskExecutionPolicy
  @Environment(\.dismiss) private var dismiss
  @State private var values: [String]
  @State private var isSaving = false
  @State private var discardPresented = false
  @FocusState private var focusedField: Bool

  private var isDirty: Bool {
    values != [policy.maxProviderContinuations, policy.maxToolCalls, policy.maxActiveMinutes, policy.progressAuditInterval].map(String.init)
  }

  private var invalid: Bool {
    guard values.count == 4,
          let continuations = Int(values[0]),
          let toolCalls = Int(values[1]),
          let activeMinutes = Int(values[2]),
          let auditInterval = Int(values[3]) else { return true }
    return [continuations, toolCalls, activeMinutes, auditInterval].contains { $0 < 1 }
      || auditInterval > continuations
  }

  init(settings: SettingsModel, policy: NoemaAPI.SettingsSnapshotQuery.Data.TaskExecutionPolicy) {
    self.settings = settings
    self.policy = policy
    _values = State(initialValue: [policy.maxProviderContinuations, policy.maxToolCalls, policy.maxActiveMinutes, policy.progressAuditInterval].map(String.init))
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Edit execution limits",
      detent: .height(500),
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Provider continuations") {
          TextField("", text: $values[0]).keyboardType(.numberPad).settingsSheetControl(focused: focusedField).focused($focusedField)
        }
        Text("Model/tool continuation rounds").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        SettingsSheetField("Tool calls") {
          TextField("", text: $values[1]).keyboardType(.numberPad).settingsSheetControl()
        }
        Text("Calls across the complete run").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        SettingsSheetField("Active minutes") {
          TextField("", text: $values[2]).keyboardType(.numberPad).settingsSheetControl()
        }
        Text("Queue time does not count").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        SettingsSheetField("Audit interval") {
          TextField("", text: $values[3]).keyboardType(.numberPad).settingsSheetControl()
        }
        Text("Continuations between audits").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        if values.count == 4, let continuations = Int(values[0]), let auditInterval = Int(values[3]), auditInterval > continuations {
          Text("The progress audit interval cannot exceed the continuation limit.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(
          primaryTitle: "Save",
          isSaving: isSaving,
          primaryDisabled: isSaving || invalid || !settings.canMutate,
          onCancel: requestDismissal,
          onPrimary: save
        )
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .sheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard execution changes?",
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
    guard values.count == 4,
          let first = Int(values[0]), let second = Int(values[1]),
          let third = Int(values[2]), let fourth = Int(values[3]) else { return }
    isSaving = true
    Task {
      let ok = await settings.updateExecutionPolicy(maxProviderContinuations: first, maxToolCalls: second, maxActiveMinutes: third, progressAuditInterval: fourth)
      if ok { dismiss() } else { isSaving = false }
    }
  }
}

struct CapabilityConnectionEditor: View {
  let connection: SettingsIntegrationConnection
  let settings: SettingsModel
  let appModel: NoemaAppModel
  let startPolicyEditing: Bool
  @Environment(\.dismiss) private var dismiss
  @State private var label: String
  @State private var sharing: String
  @State private var unsafeActions: String
  @State private var connectionRevision: String
  @State private var policyRevision: Int
  @State private var expectedConnectionLabel: String?
  @State private var labelDraft = ""
  @State private var sharingDraft = ""
  @State private var unsafeActionsDraft = ""
  @State private var detail: SettingsCapabilityDetail?
  @State private var editingTool: SettingsCapabilityTool?
  @State private var isSaving = false
  @State private var renamePresented = false
  @State private var policyPresented = false
  @State private var renameDiscardPresented = false
  @State private var policyDiscardPresented = false
  @State private var deletePresented = false
  @State private var browserURL: URL?
  @State private var browserAttemptID: String?
  @State private var authorizationTask: Task<Void, Never>?
  @State private var reauthPresented = false
  @FocusState private var focusedField: Bool

  private var displayName: String {
    let trimmed = label.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? connection.name : trimmed
  }

  private var currentStatus: String { detail?.status ?? connection.authStatus }
  private var currentHealth: String { detail?.healthStatus ?? connection.authStatus }
  private var currentAuth: String { detail?.authStatus ?? connection.authStatus }
  private var invalidPolicy: Bool { sharingDraft == "review_every_call" && unsafeActionsDraft == "never_ask" }
  private var toolCount: Int { detail?.toolCount ?? connection.toolCount }
  private var availableToolCount: Int { detail?.availableToolCount ?? connection.availableToolCount }
  private var adapterAction: SettingsAdapterNextAction? {
    let action = settings.adapterDefinitions.first { $0.semanticDigest == connection.sourceRevision }?.nextAction
    return action?.connectionID == connection.id ? action : nil
  }
  private var adapterDescriptor: SettingsAdapterConnection? {
    settings.adapterDefinitions.first { $0.semanticDigest == connection.sourceRevision }?
      .connections.first { $0.id == connection.id }
  }

  init(
    connection: SettingsIntegrationConnection,
    settings: SettingsModel,
    appModel: NoemaAppModel,
    startPolicyEditing: Bool = false
  ) {
    self.connection = connection
    self.settings = settings
    self.appModel = appModel
    self.startPolicyEditing = startPolicyEditing
    _label = State(initialValue: connection.connectionLabel ?? "")
    _sharing = State(initialValue: connection.dataSharingPolicy ?? "")
    _unsafeActions = State(initialValue: connection.unsafeActionPolicy ?? "")
    _connectionRevision = State(initialValue: connection.connectionRevision)
    _policyRevision = State(initialValue: connection.policyRevision)
    _expectedConnectionLabel = State(initialValue: connection.connectionLabel)
  }

  var body: some View {
    SettingsBottomSheet(
      title: displayName,
      detent: .large,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSectionCard {
          HStack(alignment: .top, spacing: NoemaSpacing.sm) {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              Text("\(currentHealth) · \(currentAuth)")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
            }
            Spacer(minLength: NoemaSpacing.sm)
            SettingsAction(title: "Rename", symbol: nil, role: nil, disabled: isSaving || !settings.canMutate) {
              labelDraft = label
              renamePresented = true
            }
            NoemaStatusToken(text: currentStatus.replacingOccurrences(of: "_", with: " ").capitalized, tone: currentStatus == "active" ? .success : .neutral)
          }
          if connection.kind == .api,
             let action = adapterAction,
             ["add_access", "reconnect_account"].contains(action.kind) {
            SettingsAction(title: action.kind == "add_access" ? "Add access" : "Reconnect account", symbol: "person.badge.key", role: nil, disabled: isSaving || !settings.canMutate) {
              Task {
                if let attempt = await settings.startAdapterOAuth(action) {
                  browserAttemptID = attempt.attemptID
                  browserURL = attempt.authorizationURL
                  authorizationTask?.cancel()
                  authorizationTask = Task {
                    guard let result = await settings.waitForAdapterOAuth(attemptID: attempt.attemptID) else { return }
                    await finishAdapterOAuth(status: result.status)
                  }
                }
              }
            }
          }
          if connection.kind == .api, let descriptor = adapterDescriptor {
            SettingsAction(
              title: descriptor.status == "suspended" ? "Resume connection" : "Suspend connection",
              symbol: descriptor.status == "suspended" ? "play" : "pause",
              role: nil,
              disabled: isSaving || !settings.canMutate
            ) {
              Task {
                if await settings.setAdapterConnectionActive(descriptor, active: descriptor.status == "suspended") {
                  await settings.load(client: settings.client)
                }
              }
            }
          }
          if connection.kind == .mcp,
             let server = settings.snapshot?.mcpServers.first(where: { $0.mcpServerId == connection.id }),
             server.healthStatus == "unavailable" || !["none", "authenticated"].contains(server.authStatus) {
            SettingsAction(title: "Reconnect", symbol: "arrow.clockwise", role: nil, disabled: isSaving || !settings.canMutate) {
              reauthPresented = true
            }
          }
        }

        SettingsSectionCard("Connection policy") {
          HStack(alignment: .top, spacing: NoemaSpacing.sm) {
            VStack(alignment: .leading, spacing: NoemaSpacing.md) {
              policySummary("Data sharing", sharing == "review_every_call" ? "Review sharing every time" : "Share when needed")
              policySummary("Risky actions", unsafeActionSummary)
            }
            Spacer(minLength: NoemaSpacing.sm)
            SettingsAction(title: "Edit policy", symbol: nil, role: nil, disabled: isSaving || !settings.canMutate) {
              sharingDraft = sharing
              unsafeActionsDraft = unsafeActions
              policyPresented = true
            }
          }
        }

        SettingsSectionCard("Tools") {
          HStack(spacing: NoemaSpacing.sm) {
            Text("\(availableToolCount)/\(toolCount) available")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
            Spacer(minLength: NoemaSpacing.sm)
          }
          if let detail {
            VStack(alignment: .leading, spacing: 0) {
              ForEach(Array(detail.tools.enumerated()), id: \.element.toolId) { index, tool in
                if index > 0 { SettingsRowDivider() }
                Button { editingTool = tool } label: {
                  HStack(spacing: NoemaSpacing.sm) {
                    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                      Text(tool.name).font(NoemaFont.bodyEmphasized)
                      Text(tool.status.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(NoemaFont.caption)
                        .foregroundStyle(NoemaColor.contentSecondary)
                    }
                    Spacer(minLength: NoemaSpacing.sm)
                    NoemaStatusToken(text: tool.enabled ? "Enabled" : "Disabled", tone: tool.enabled ? .success : .neutral)
                    Image(systemName: "chevron.right").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
                  }
                  .frame(maxWidth: .infinity, alignment: .leading)
                  .padding(.vertical, NoemaSpacing.sm)
                }
                .buttonStyle(.plain)
              }
            }
          } else {
            NoemaInlineState(message: "Loading tools…", symbol: "arrow.triangle.2.circlepath")
          }
        }

        DisclosureGroup("Source details") {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("Definition \(connection.sourceRevision)")
            Text("Connection \(connection.connectionRevision) · policy v\(connection.policyRevision)")
            if let detail {
              Text("\(detail.pendingToolCount) pending · \(detail.defaultedToolCount) defaulted · \(detail.disabledToolCount) disabled tools")
            }
            if let definition = settings.adapterDefinitions.first(where: { $0.semanticDigest == connection.sourceRevision }) {
              Text(definition.manifestJSON).font(NoemaFont.monoTiny).textSelection(.enabled)
            }
          }
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        }

        SettingsSectionCard("Connection") {
          Text(connection.kind == .api
            ? "Remove this API binding and tool settings. The account and OAuth client stay connected."
            : "Remove this connection, its credentials, and its tool settings.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Button(role: .destructive) { deletePresented = true } label: {
            Label("Delete connection", systemImage: "trash")
              .font(NoemaFont.bodyEmphasized)
              .foregroundStyle(NoemaColor.white)
              .padding(.horizontal, NoemaSpacing.md)
              .frame(minHeight: 32)
          }
          .buttonStyle(.plain)
          .background(NoemaColor.danger, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .disabled(isSaving || !settings.canMutate)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
      }
    }
    .interactiveDismissDisabled(isSaving)
    .onDisappear { authorizationTask?.cancel() }
    .task {
      await settings.loadCapabilityDetail(kind: connection.kind, connectionID: connection.id)
      detail = settings.capabilityDetails[connection.id]
      if startPolicyEditing {
        sharingDraft = sharing
        unsafeActionsDraft = unsafeActions
        policyPresented = true
      }
    }
    .sheet(item: $editingTool, onDismiss: { Task { await refreshConnectionState() } }) { tool in
      CapabilityToolEditor(connection: currentConnection, tool: tool, settings: settings)
    }
    .sheet(isPresented: $renamePresented) { renameSheet }
    .sheet(isPresented: $policyPresented) { policySheet }
    .sheet(isPresented: Binding(get: { browserURL != nil }, set: { if !$0 { browserURL = nil } }), onDismiss: {
      Task {
        if let browserAttemptID,
           let result = await settings.adapterOAuthAttempt(attemptID: browserAttemptID),
           result.status != "authorizing" {
          await finishAdapterOAuth(status: result.status)
        }
      }
    }) {
      if let browserURL { SafariView(url: browserURL) }
    }
    .sheet(isPresented: $reauthPresented) {
      let server = settings.snapshot?.mcpServers.first { $0.mcpServerId == connection.id }
      MCPReauthenticationSheet(
        serverID: connection.id,
        usesBrowserOAuth: server?.browserOauthReauthenticationSupported == true,
        isHTTP: server?.transportKind == "streamable_http",
        settings: settings,
        appModel: appModel
      )
    }
    .sheet(isPresented: $deletePresented) {
      SettingsMutationConfirmationSheet(
        title: "Delete \(displayName)?",
        message: connection.kind == .api
          ? "Removes this API binding and \(toolCount) tool settings. The account and OAuth client stay connected. Past activity is kept."
          : "Removes the connection, sign-in details, \(toolCount) \(toolCount == 1 ? "tool" : "tools"), and tool settings. You can't undo this. Past activity is kept.",
        confirmTitle: "Delete"
      ) {
        let deleted = connection.kind == .api
          ? await settings.deleteAdapterConnection(connection)
          : await settings.deleteMCPServer(connection.id)
        if deleted { dismiss() }
        return deleted
      }
    }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    dismiss()
  }

  @MainActor
  private func finishAdapterOAuth(status: String) async {
    guard status == "completed" else {
      settings.errorMessage = switch status {
      case "denied": "Access was not approved. Current account access did not change."
      case "expired": "Account authorization expired. Current account access did not change."
      case "superseded": "A newer account authorization replaced this attempt."
      default: "Account authorization failed. Current account access did not change."
      }
      return
    }
    await settings.load(client: settings.client)
  }

  private var unsafeActionSummary: String {
    switch unsafeActions {
    case "always_ask": "You approve risky calls"
    case "never_ask": "Risky calls run automatically"
    default: "Noema reviews risky calls first"
    }
  }

  private var currentConnection: SettingsIntegrationConnection {
    SettingsIntegrationConnection(
      id: connection.id,
      kind: connection.kind,
      definitionId: connection.definitionId,
      name: connection.name,
      connectionLabel: expectedConnectionLabel,
      sourceRevision: connection.sourceRevision,
      connectionRevision: connectionRevision,
      policyRevision: policyRevision,
      authStatus: connection.authStatus,
      dataSharingPolicy: sharing,
      unsafeActionPolicy: unsafeActions,
      toolCount: connection.toolCount,
      availableToolCount: connection.availableToolCount
    )
  }

  @ViewBuilder
  private func policySummary(_ label: String, _ value: String) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
      Text(label).font(NoemaFont.body)
      Text(value).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
    }
  }

  @ViewBuilder
  private var renameSheet: some View {
    SettingsBottomSheet(title: "Rename connection", subtitle: "Leave blank to use the generated connection name.", detent: .height(246), onClose: requestRenameDismissal) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Connection label") {
          TextField("Connection label", text: $labelDraft)
            .textInputAutocapitalization(.sentences)
            .settingsSheetControl(focused: focusedField)
            .focused($focusedField)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(primaryTitle: "Save label", isSaving: isSaving, primaryDisabled: isSaving || !settings.canMutate, onCancel: requestRenameDismissal, onPrimary: saveLabel)
      }
    }
    .interactiveDismissDisabled(isSaving || labelDraft != label)
    .sheet(isPresented: $renameDiscardPresented) {
      SettingsConfirmationSheet(title: "Discard label changes?", message: "Any unsaved changes will be lost.", confirmTitle: "Discard changes", cancelTitle: "Keep editing") {
        renameDiscardPresented = false
        renamePresented = false
      }
    }
    .task { focusedField = true }
  }

  @ViewBuilder
  private var policySheet: some View {
    SettingsBottomSheet(title: "Edit connection policy", subtitle: "Choose how Noema shares context and approves risky actions.", detent: .height(360), onClose: requestPolicyDismissal) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Data sharing policy") {
          Picker("Data sharing policy", selection: $sharingDraft) {
            Text("Choose a sharing policy").tag("")
            Text("Share when needed").tag("allow_automatically")
            Text("Review every time").tag("review_every_call")
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
          .settingsSheetControl()
        }
        SettingsSheetField("Unsafe action policy") {
          Picker("Unsafe action policy", selection: $unsafeActionsDraft) {
            Text("Choose an approval policy").tag("")
            Text("Always me").tag("always_ask")
            Text("Noema first").tag("reviewer_may_approve")
            Text("Run automatically").tag("never_ask")
          }
          .pickerStyle(.menu)
          .tint(NoemaColor.content)
          .settingsSheetControl()
        }
        if invalidPolicy {
          Text("Reviewing every call requires an approval step.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
        }
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(primaryTitle: "Save policy", isSaving: isSaving, primaryDisabled: isSaving || sharingDraft.isEmpty || unsafeActionsDraft.isEmpty || invalidPolicy || !settings.canMutate, onCancel: requestPolicyDismissal, onPrimary: savePolicy)
      }
    }
    .interactiveDismissDisabled(isSaving || sharingDraft != sharing || unsafeActionsDraft != unsafeActions)
    .sheet(isPresented: $policyDiscardPresented) {
      SettingsConfirmationSheet(title: "Discard policy changes?", message: "Any unsaved changes will be lost.", confirmTitle: "Discard changes", cancelTitle: "Keep editing") {
        policyDiscardPresented = false
        policyPresented = false
      }
    }
  }

  private func requestRenameDismissal() {
    guard !isSaving else { return }
    if labelDraft != label { renameDiscardPresented = true } else { renamePresented = false }
  }

  private func requestPolicyDismissal() {
    guard !isSaving else { return }
    if sharingDraft != sharing || unsafeActionsDraft != unsafeActions { policyDiscardPresented = true } else { policyPresented = false }
  }

  private func saveLabel() {
    isSaving = true
    Task {
      let nextLabel = labelDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : labelDraft
      let ok = await settings.saveCapabilityConnectionLabel(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connectionRevision, expectedConnectionLabel: expectedConnectionLabel, connectionLabel: nextLabel)
      isSaving = false
      if ok {
        label = labelDraft
        expectedConnectionLabel = nextLabel
        await refreshConnectionState()
        renamePresented = false
      }
    }
  }

  private func savePolicy() {
    guard !invalidPolicy else { return }
    isSaving = true
    Task {
      let ok = await settings.saveCapabilityConnectionPolicy(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connectionRevision, expectedPolicyRevision: policyRevision, dataSharingPolicy: sharingDraft, unsafeActionPolicy: unsafeActionsDraft)
      isSaving = false
      if ok {
        sharing = sharingDraft
        unsafeActions = unsafeActionsDraft
        await refreshConnectionState()
        policyPresented = false
      }
    }
  }

  private func refreshConnectionState() async {
    await settings.loadCapabilityDetail(kind: connection.kind, connectionID: connection.id)
    detail = settings.capabilityDetails[connection.id]
    if connection.kind == .api,
       let refreshed = settings.snapshot?.apis.flatMap(\.connections).first(where: { $0.connectionId == connection.id }) {
      connectionRevision = refreshed.connectionRevision
      policyRevision = refreshed.policyRevision
      expectedConnectionLabel = refreshed.connectionLabel
      return
    }
    if connection.kind == .mcp,
       let refreshed = settings.snapshot?.mcps.flatMap(\.connections).first(where: { $0.connectionId == connection.id }) {
      connectionRevision = refreshed.connectionRevision
      policyRevision = refreshed.policyRevision
      expectedConnectionLabel = refreshed.connectionLabel
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
  @State private var discardPresented = false

  private var isDirty: Bool {
    readOnly != (tool.readOnly ?? false) || idempotent != (tool.idempotent ?? false)
      || destructive != (tool.destructive ?? true) || openWorld != (tool.openWorld ?? true)
  }

  init(connection: SettingsIntegrationConnection, tool: SettingsCapabilityTool, settings: SettingsModel) {
    self.connection = connection
    self.tool = tool
    self.settings = settings
    _readOnly = State(initialValue: tool.readOnly ?? false)
    _idempotent = State(initialValue: tool.idempotent ?? false)
    _destructive = State(initialValue: tool.destructive ?? true)
    _openWorld = State(initialValue: tool.openWorld ?? true)
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Tool policy",
      subtitle: tool.name,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        if let description = tool.description?.trimmingCharacters(in: .whitespacesAndNewlines),
           !description.isEmpty
        {
          Text(description)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        Toggle("Enabled", isOn: Binding(
          get: { tool.enabled },
          set: { value in
            isSaving = true
            Task {
              let ok = await settings.setCapabilityToolEnabled(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision, enabled: value)
              if ok { dismiss() } else { isSaving = false }
            }
          }
        ))
        SettingsSheetField("Policy hints") {
          VStack(spacing: NoemaSpacing.sm) {
            Toggle("Read-only", isOn: $readOnly)
            Toggle("Idempotent", isOn: $idempotent)
            Toggle("Destructive", isOn: $destructive)
            Toggle("Open-world", isOn: $openWorld)
          }
        }
        Text("Source revision \(tool.sourceRevision) · \(tool.status.capitalized)")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentTertiary)
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
        SettingsSheetActions(
          primaryTitle: "Save tool override",
          isSaving: isSaving,
          primaryDisabled: isSaving || !settings.canMutate,
          onCancel: requestDismissal,
          onPrimary: save
        )
        Button("Reset to source policy", role: .destructive) {
          isSaving = true
          Task {
            let ok = await settings.resetCapabilityToolPolicy(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision)
            if ok { dismiss() } else { isSaving = false }
          }
        }
        .buttonStyle(.plain)
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.danger)
        .disabled(isSaving || !settings.canMutate)
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .sheet(isPresented: $discardPresented) {
      SettingsConfirmationSheet(
        title: "Discard tool changes?",
        message: "Any unsaved changes will be lost.",
        confirmTitle: "Discard changes",
        cancelTitle: "Keep editing"
      ) {
        dismiss()
      }
    }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    isSaving = true
    Task {
      let ok = await settings.saveCapabilityToolOverride(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision, readOnly: readOnly, idempotent: idempotent, destructive: destructive, openWorld: openWorld)
      if ok { dismiss() } else { isSaving = false }
    }
  }
}
