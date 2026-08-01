import NoemaAPI
import SwiftUI

extension View {
  func settingsSheetControl(focused: Bool = false) -> some View {
    font(NoemaFont.body)
      .foregroundStyle(NoemaColor.content)
      .frame(maxWidth: .infinity, minHeight: 40, alignment: .leading)
      .padding(.horizontal, NoemaSpacing.md)
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .overlay {
        RoundedRectangle(cornerRadius: NoemaRadius.element)
          .stroke(focused ? NoemaColor.pine500 : NoemaColor.separator, lineWidth: focused ? 2 : 1)
      }
  }
}

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
      .background(NoemaColor.clay600, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
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
    .confirmationDialog("Discard provider changes?", isPresented: $discardPresented, titleVisibility: .visible) {
      Button("Discard changes", role: .destructive) { dismiss() }
      Button("Keep editing", role: .cancel) { }
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
    values.count != 4 || values.contains { Int($0) == nil || Int($0)! < 1 }
  }

  init(settings: SettingsModel, policy: NoemaAPI.SettingsSnapshotQuery.Data.TaskExecutionPolicy) {
    self.settings = settings
    self.policy = policy
    _values = State(initialValue: [policy.maxProviderContinuations, policy.maxToolCalls, policy.maxActiveMinutes, policy.progressAuditInterval].map(String.init))
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Edit execution limits",
      subtitle: "These ceilings apply across Work task execution and review.",
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Provider continuations") {
          TextField("", text: $values[0]).keyboardType(.numberPad).settingsSheetControl(focused: focusedField).focused($focusedField)
        }
        SettingsSheetField("Tool calls") {
          TextField("", text: $values[1]).keyboardType(.numberPad).settingsSheetControl()
        }
        SettingsSheetField("Active minutes") {
          TextField("", text: $values[2]).keyboardType(.numberPad).settingsSheetControl()
        }
        SettingsSheetField("Progress-audit interval") {
          TextField("", text: $values[3]).keyboardType(.numberPad).settingsSheetControl()
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
    .confirmationDialog("Discard execution changes?", isPresented: $discardPresented, titleVisibility: .visible) {
      Button("Discard changes", role: .destructive) { dismiss() }
      Button("Keep editing", role: .cancel) { }
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
  @Environment(\.dismiss) private var dismiss
  @State private var label: String
  @State private var sharing: String
  @State private var unsafeActions: String
  @State private var detail: SettingsCapabilityDetail?
  @State private var editingTool: SettingsCapabilityTool?
  @State private var isSaving = false
  @State private var discardPresented = false
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
    SettingsBottomSheet(
      title: connection.connectionLabel ?? connection.name,
      subtitle: "Review the connection policy and enabled tools.",
      detent: .large,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Label") {
          TextField("", text: $label)
            .textInputAutocapitalization(.sentences)
            .settingsSheetControl(focused: focusedField)
            .focused($focusedField)
        }
        Text("Revision \(connection.connectionRevision) · policy v\(connection.policyRevision)")
          .font(NoemaFont.mono)
          .foregroundStyle(NoemaColor.contentTertiary)
        SettingsSheetActions(
          primaryTitle: "Save label",
          isSaving: isSaving,
          primaryDisabled: isSaving || !settings.canMutate,
          onCancel: requestDismissal,
          onPrimary: saveLabel
        )
        SettingsSheetField("Data sharing policy") {
          TextField("", text: $sharing).settingsSheetControl()
        }
        SettingsSheetField("Unsafe action policy") {
          TextField("", text: $unsafeActions).settingsSheetControl()
        }
        SettingsSheetActions(
          primaryTitle: "Save policy",
          isSaving: isSaving,
          primaryDisabled: isSaving || sharing.isEmpty || unsafeActions.isEmpty || !settings.canMutate,
          onCancel: requestDismissal,
          onPrimary: savePolicy
        )
        SettingsSheetField("Tools") {
          if let detail {
            VStack(alignment: .leading, spacing: 0) {
              ForEach(Array(detail.tools.enumerated()), id: \.element.toolId) { index, tool in
                if index > 0 { SettingsRowDivider() }
                Button { editingTool = tool } label: {
                  HStack(spacing: NoemaSpacing.sm) {
                    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                      Text(tool.name).font(NoemaFont.bodyEmphasized)
                      Text(tool.status).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
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
        if let error = settings.errorMessage, !isSaving {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
        }
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .confirmationDialog("Discard connection changes?", isPresented: $discardPresented, titleVisibility: .visible) {
      Button("Discard changes", role: .destructive) { dismiss() }
      Button("Keep editing", role: .cancel) { }
    }
    .task {
      await settings.loadCapabilityDetail(kind: connection.kind, connectionID: connection.id)
      detail = settings.capabilityDetails[connection.id]
    }
    .task { focusedField = true }
    .sheet(item: $editingTool) { tool in
      CapabilityToolEditor(connection: connection, tool: tool, settings: settings)
    }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
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
  @State private var discardPresented = false

  private var isDirty: Bool {
    readOnly != (tool.readOnly ?? false) || idempotent != (tool.idempotent ?? false)
      || destructive != (tool.destructive ?? false) || openWorld != (tool.openWorld ?? false)
  }

  init(connection: SettingsIntegrationConnection, tool: SettingsCapabilityTool, settings: SettingsModel) {
    self.connection = connection
    self.tool = tool
    self.settings = settings
    _readOnly = State(initialValue: tool.readOnly ?? false)
    _idempotent = State(initialValue: tool.idempotent ?? false)
    _destructive = State(initialValue: tool.destructive ?? false)
    _openWorld = State(initialValue: tool.openWorld ?? false)
  }

  var body: some View {
    SettingsBottomSheet(
      title: "Tool policy",
      subtitle: tool.name,
      onClose: requestDismissal
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        Toggle("Enabled", isOn: Binding(
          get: { tool.enabled },
          set: { value in
            isSaving = true
            Task {
              _ = await settings.setCapabilityToolEnabled(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision, enabled: value)
              dismiss()
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
            _ = await settings.resetCapabilityToolPolicy(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision)
            dismiss()
          }
        }
        .buttonStyle(.plain)
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.danger)
        .disabled(isSaving || !settings.canMutate)
      }
    }
    .interactiveDismissDisabled(isSaving || isDirty)
    .confirmationDialog("Discard tool changes?", isPresented: $discardPresented, titleVisibility: .visible) {
      Button("Discard changes", role: .destructive) { dismiss() }
      Button("Keep editing", role: .cancel) { }
    }
  }

  private func requestDismissal() {
    guard !isSaving else { return }
    if isDirty { discardPresented = true } else { dismiss() }
  }

  private func save() {
    isSaving = true
    Task {
      _ = await settings.saveCapabilityToolOverride(kind: connection.kind, connectionID: connection.id, expectedConnectionRevision: connection.connectionRevision, toolID: tool.toolId, sourceRevision: tool.sourceRevision, expectedPolicyRevision: tool.policyRevision, readOnly: readOnly, idempotent: idempotent, destructive: destructive, openWorld: openWorld)
      dismiss()
    }
  }
}
