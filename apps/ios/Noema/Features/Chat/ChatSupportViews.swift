import Foundation
import Apollo
import MarkdownUI
import NoemaAPI
import SwiftUI

struct TaskReferenceChip: View {
  let client: ApolloClient?
  let taskID: String
  let onOpen: ((String) -> Void)?
  @Environment(NoemaShellCoordinator.self) private var coordinator
  @State private var title = "Loading task…"
  @State private var status = "UNKNOWN"

  init(client: ApolloClient?, taskID: String, onOpen: ((String) -> Void)? = nil) {
    self.client = client
    self.taskID = taskID
    self.onOpen = onOpen
  }

  var body: some View {
    Button {
      if let onOpen {
        onOpen(taskID)
      } else {
        coordinator.openTask(taskID)
      }
    } label: {
      HStack(spacing: NoemaSpacing.xs) {
        Image(systemName: "checklist")
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.content)
        Image(systemName: statusSymbol)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(statusColor)
        Text(title)
          .font(NoemaFont.taskPreview.weight(.semibold))
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
      }
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.compact)
      .background(NoemaColor.surface, in: Capsule())
      .overlay { Capsule().stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .buttonStyle(.plain)
    .contentShape(.interaction, Capsule().inset(by: -NoemaSpacing.sm))
    .accessibilityLabel("Open task \(title)")
    .task(id: taskID) { await loadTitle() }
  }

  private func loadTitle() async {
    guard let client else { return }
    do {
      let response = try await client.fetch(
        query: TasksDetailQuery(taskId: taskID),
        cachePolicy: .networkFirst
      )
      let task = response.data?.task.fragments.tasksCommandTaskFields
      title = task?.title ?? "Task unavailable"
      if task?.completedAt != nil {
        status = "DONE"
      } else if task?.activeGate != nil {
        status = "ATTENTION"
      } else if task?.currentRun != nil {
        status = "ACTIVE"
      } else {
        status = task?.stage.behavior.rawValue ?? "UNKNOWN"
      }
    } catch {
      title = "Task unavailable"
    }
  }

  private var statusSymbol: String {
    switch status {
    case "DONE", "TERMINAL_SUCCESS": "checkmark.circle.fill"
    case "ACTIVE": "arrow.triangle.2.circlepath"
    case "ATTENTION", "HUMAN_GATE": "person"
    case "TERMINAL_CANCELLED": "xmark.circle"
    default: "clock"
    }
  }

  private var statusColor: Color {
    switch status {
    case "DONE", "TERMINAL_SUCCESS": NoemaColor.pine700
    case "ACTIVE": NoemaColor.blue700
    case "ATTENTION", "HUMAN_GATE": NoemaColor.clay600
    case "TERMINAL_CANCELLED": NoemaColor.red700
    default: NoemaColor.contentTertiary
    }
  }
}

struct ChatComposer: View {
  @Bindable var model: ChatModel

  private var preferredWidth: CGFloat {
    let content = model.draft.isEmpty ? placeholder : model.draft
    let longestLine = content.split(whereSeparator: \.isNewline).map(\.count).max() ?? 0
    return min(760, max(200, CGFloat(longestLine) * 7 + 94))
  }

  private var placeholder: String {
    if model.isOffline { return "Write a draft while offline" }
    if let name = model.primaryAgentDisplayName, !name.isEmpty { return "Message " + name }
    return "Send a message"
  }

  private var canSend: Bool {
    !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !model.isSending && !model.isOffline
  }

  var body: some View {
    HStack(alignment: .bottom, spacing: NoemaSpacing.sm) {
      TextField(
        "",
        text: $model.draft,
        prompt: Text(placeholder).foregroundStyle(NoemaColor.white.opacity(0.72)),
        axis: .vertical
      )
      .font(NoemaFont.body)
      .foregroundStyle(NoemaColor.white)
      .tint(NoemaColor.white)
      .lineLimit(1...5)
      .textFieldStyle(.plain)
      .onSubmit { Task { await model.send() } }

      Button {
        Task { await model.send() }
      } label: {
        Image(systemName: "paperplane")
          .font(NoemaFont.bodyEmphasized)
          .frame(width: 44, height: 44)
      }
      .buttonStyle(.plain)
      .glassEffect(.regular.interactive(), in: Circle())
      .disabled(!canSend)
      .accessibilityLabel("Send message")
    }
    .padding(.leading, NoemaSpacing.lg)
    .padding(.trailing, NoemaSpacing.xs)
    .padding(.vertical, 3)
    .background(NoemaColor.pine500, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
    .shadow(color: NoemaColor.pine700.opacity(0.10), radius: 12, y: 5)
    .frame(width: preferredWidth)
  }
}

struct ChatInterventionsView: View {
  @Bindable var model: ChatModel
  @State private var browserURL: URL?
  @State private var taskResponses: [String: String] = [:]

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text("Waiting for you")
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.contentSecondary)
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        ForEach(model.interventions) { intervention in
          interventionCard(for: intervention)
        }
      }
    }
    .frame(maxWidth: 760, alignment: .leading)
    .sheet(isPresented: Binding(
      get: { browserURL != nil },
      set: { if !$0 { browserURL = nil } }
    ), onDismiss: {
      Task { await model.recoverConnection() }
    }) {
      if let browserURL { SafariView(url: browserURL) }
    }
  }

  @ViewBuilder
  private func interventionCard(for intervention: ChatIntervention) -> some View {
    NoemaCard(padding: NoemaSpacing.md) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        switch intervention {
        case let .governed(action):
          Label(action.readOnly == true ? "Read only · Human review" : "Can make changes · Human review", systemImage: "hand.raised")
            .font(NoemaFont.captionEmphasized)
          Text(action.summary)
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.content)
          Text(action.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          if let failureCode = action.failureCode {
            Text(failureCode)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.danger)
          }
          DisclosureGroup("Review exact arguments") {
            Text(action.arguments)
              .font(NoemaFont.monoTiny)
              .foregroundStyle(NoemaColor.contentSecondary)
              .textSelection(.enabled)
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          GovernedInterventionActions(disabled: model.isOffline) {
            await model.resolve(intervention, decision: $0)
          }
        case let .mcpAuth(auth):
          Label("Sign-in required", systemImage: "person.badge.key")
            .font(NoemaFont.captionEmphasized)
          Text("Sign in to \(auth.serverName)")
            .font(NoemaFont.bodyEmphasized)
          Text(auth.failureCode == nil
            ? (auth.taskID == nil ? "Your request is paused until you sign in." : "This task is paused until you sign in.")
            : "The previous sign-in did not finish. Try again to continue.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          AuthenticationInterventionActions(
            primaryTitle: "Continue in browser",
            disabled: model.isOffline,
            onStart: { browserURL = await model.startMcpAuthentication(auth) },
            onSkip: { await model.skipMcpAuthentication(auth) }
          )
        case let .adapterAuth(auth):
          Label("Sign-in required", systemImage: "person.badge.key")
            .font(NoemaFont.captionEmphasized)
          Text("Sign in to \(auth.serviceName)")
            .font(NoemaFont.bodyEmphasized)
          Text(auth.failureCode == nil
            ? (auth.taskID == nil ? "Your request is paused until you sign in." : "This task is paused until you sign in.")
            : "The previous sign-in did not finish. Try again to continue.")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          AuthenticationInterventionActions(
            primaryTitle: "Continue in browser",
            disabled: model.isOffline,
            onStart: { browserURL = await model.startAdapterAuthentication(auth) },
            onSkip: { await model.skipAdapterAuthentication(auth) }
          )
        case let .setup(setup):
          McpSetupInterventionCard(
            setup: setup,
            isOffline: model.isOffline,
            onOpenBrowser: { browserURL = $0 },
            onRefresh: { await model.refreshInterventions() },
            onConnectPublicly: { try await model.connectMcpPublicly(setup) },
            onStartOAuth: { try await model.startMcpSetupOAuth(setup) },
            onSavePolicy: { try await model.saveMcpPolicy($0, sharing: $1, unsafeActions: $2) },
            onResolve: { try await model.resolveMcpSetup(setup, server: $0) }
          )
        case let .attention(attention):
          taskAttentionContent(attention)
        case let .adapterDefinition(definition):
          AdapterDefinitionInterventionCard(
            definition: definition,
            isOffline: model.isOffline,
            onOpenBrowser: { browserURL = $0 },
            onRefresh: { await model.refreshInterventions() },
            onApprove: { try await model.approveAdapterDefinition(definition) },
            onImportClientJSON: { try await model.importAdapterOauthClientJSON(definition, data: $0) },
            onStartOAuth: { try await model.startAdapterOauthSetup($0) },
            onSavePolicy: { try await model.saveAdapterPolicy($0, dataSharingPolicy: $1, unsafeActionPolicy: $2) }
          )
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  @ViewBuilder
  private func taskAttentionContent(_ attention: ChatTaskAttentionModel) -> some View {
    Label(attention.gate?.kind == "APPROVAL" ? "Approval needed" : attention.gate?.kind == "RECOVERY" ? "Recovery needed" : "Clarification needed", systemImage: "hand.raised")
      .font(NoemaFont.captionEmphasized)
    Text(attention.title)
      .font(NoemaFont.bodyEmphasized)
    Text(attention.summary)
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
    if let gate = attention.gate {
      if !gate.context.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        Markdown(gate.context)
          .markdownTextStyle { ForegroundColor(NoemaColor.contentSecondary) }
      }
      switch gate.kind {
      case "APPROVAL":
        TextField("Optional note", text: taskResponseBinding(attention), axis: .vertical)
          .textFieldStyle(.roundedBorder)
        HStack(spacing: NoemaSpacing.sm) {
          Button("Decline") { Task { await model.answerTask(attention, answer: taskResponses[taskResponseKey(attention)]?.nilIfBlank ?? "Declined", approval: .declined) } }
            .buttonStyle(.bordered)
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
          Button("Approve") { Task { await model.answerTask(attention, answer: taskResponses[taskResponseKey(attention)]?.nilIfBlank ?? "Approved", approval: .approved) } }
            .buttonStyle(.borderedProminent)
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
        }
      case "RECOVERY":
        if let reason = gate.recoveryReason?.nilIfBlank {
          Text(reason.capitalized)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
        }
        TextField("Optional retry note", text: taskResponseBinding(attention), axis: .vertical)
          .textFieldStyle(.roundedBorder)
        Button("Retry", systemImage: "arrow.clockwise") {
          Task { await model.retryTask(attention, note: taskResponses[taskResponseKey(attention)]?.nilIfBlank) }
        }
        .buttonStyle(.borderedProminent)
        .disabled(model.isOffline || !hasTaskAction(attention, "RETRY"))
      default:
        ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
          Button(suggestion) { Task { await model.answerTask(attention, answer: suggestion) } }
            .buttonStyle(.bordered)
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
        }
        HStack(spacing: NoemaSpacing.sm) {
          TextField("Or type another answer", text: taskResponseBinding(attention), axis: .vertical)
            .textFieldStyle(.roundedBorder)
          Button("Answer") {
            guard let answer = taskResponses[taskResponseKey(attention)]?.nilIfBlank else { return }
            Task { await model.answerTask(attention, answer: answer) }
          }
          .buttonStyle(.borderedProminent)
          .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER") || taskResponses[taskResponseKey(attention)]?.nilIfBlank == nil)
        }
      }
    }
  }

  private func taskResponseKey(_ attention: ChatTaskAttentionModel) -> String {
    attention.taskID + ":" + (attention.gate?.id ?? "attention")
  }

  private func taskResponseBinding(_ attention: ChatTaskAttentionModel) -> Binding<String> {
    let key = taskResponseKey(attention)
    return Binding(get: { taskResponses[key] ?? "" }, set: { taskResponses[key] = $0 })
  }

  private func hasTaskAction(_ attention: ChatTaskAttentionModel, _ action: String) -> Bool {
    attention.validActions.contains(action) || attention.validActions.contains(action.lowercased())
  }
}

struct ActivityValues {
  let title: String
  let summary: String?
  let status: String
  let metadata: String
  let activityKind: String
}

func activityValues(_ message: ChatMessage?) -> ActivityValues? {
  guard let message,
        case let .activity(title, summary, status, metadata, activityKind) = message.kind else { return nil }
  return ActivityValues(title: title, summary: summary, status: status, metadata: metadata, activityKind: activityKind)
}

func metadataObject(for message: ChatMessage) -> [String: Any] {
  guard let values = activityValues(message),
        let data = values.metadata.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
  return object
}

func nestedString(_ object: [String: Any], path: [String]) -> String? {
  var current: Any = object
  for key in path {
    guard let values = current as? [String: Any], let next = values[key] else { return nil }
    current = next
  }
  return stringValue(current)
}

func stringValue(_ value: Any?) -> String? {
  guard let value else { return nil }
  if let string = value as? String, !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return string }
  return nil
}

func activityDetail(for message: ChatMessage) -> String? {
  let metadata = metadataObject(for: message)
  if let detail = stringValue(metadata["detail"]) { return detail }
  if let result = stringValue(metadata["result"]) { return result }
  if let output = stringValue(metadata["output"]) { return output }
  if let action = metadata["action"] as? [String: Any], let arguments = action["arguments"] {
    return prettyJSON(arguments)
  }
  return nil
}

func toolDetail(for message: ChatMessage) -> String? {
  activityDetail(for: message)
}

func prettyJSON(_ value: Any) -> String? {
  guard JSONSerialization.isValidJSONObject(value),
        let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]),
        let string = String(data: data, encoding: .utf8) else {
    return stringValue(value)
  }
  return string
}

func isToolActivity(_ message: ChatMessage) -> Bool {
  guard let kind = activityValues(message)?.activityKind.normalizedActivityKind else { return false }
  return kind == "TOOL_CALL" || kind == "TOOL_RESULT"
}

func sameToolTurn(_ left: ChatMessage?, _ right: ChatMessage) -> Bool {
  guard let leftTurnID = left?.turnID, let rightTurnID = right.turnID else { return true }
  return leftTurnID == rightTurnID
}

func isSystemNotice(_ message: ChatMessage) -> Bool {
  guard let values = activityValues(message) else { return false }
  let kind = values.activityKind.normalizedActivityKind
  return kind == "TASK_RUN_START" || kind == "TASK_RUN_END" || kind == "AUTHENTICATION_REQUEST" || activityDetail(for: message) == nil
}

func activityTone(_ message: ChatMessage?) -> NoemaStatusToken.Tone {
  guard let values = activityValues(message) else { return .neutral }
  let metadata = metadataObject(for: message!)
  let presentationTone = nestedString(metadata, path: ["presentation", "tone"])?.lowercased()
  switch presentationTone {
  case "neutral": return .neutral
  case "success": return .success
  case "warning": return .warning
  case "error": return .error
  default: break
  }
  switch values.status.uppercased() {
  case "FAILED", "ERROR": return .error
  case "COMPLETED", "SUCCEEDED": return .success
  case "WAITING", "PAUSED": return .warning
  default: return .neutral
  }
}

func statusTone(_ status: String) -> NoemaStatusToken.Tone {
  switch status.uppercased() {
  case "FAILED", "ERROR": .error
  case "COMPLETED", "SUCCEEDED": .success
  case "WAITING", "PAUSED": .warning
  default: .neutral
  }
}

func statusLabel(_ status: String) -> String {
  status
    .replacingOccurrences(of: "_", with: " ")
    .lowercased()
    .capitalized
}

func humanizeToolName(_ name: String) -> String {
  name.replacingOccurrences(of: "_", with: " ")
}

extension String {
  var normalizedActivityKind: String { uppercased().replacingOccurrences(of: "-", with: "_") }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
