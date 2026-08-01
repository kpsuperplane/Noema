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
  @State private var expandedGovernedActionIDs: Set<String> = []

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
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
          HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
            Text(reviewLabel(action)).interventionEyebrow()
            Spacer(minLength: NoemaSpacing.sm)
            Text(action.taskID == nil ? "Primary conversation" : "Background task")
              .font(NoemaFont.metadata)
              .foregroundStyle(NoemaColor.contentTertiary)
          }
          Text(action.summary)
            .font(NoemaFont.taskTitle)
            .foregroundStyle(NoemaColor.content)
          Text(action.capabilityName)
            .font(NoemaFont.monoTiny)
            .foregroundStyle(NoemaColor.contentSecondary)
          if let failureCode = action.failureCode {
            Text(failureCode)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.danger)
          }
          governedArguments(action)
          GovernedInterventionActions(disabled: model.isOffline) {
            await model.resolve(intervention, decision: $0)
          }
        case let .mcpAuth(auth):
          Label("Sign-in required", systemImage: "person.badge.key")
            .interventionEyebrow()
          Text("Sign in to \(auth.serverName)")
            .font(NoemaFont.taskTitle)
          Text(authenticationDescription(state: auth.state, failureCode: auth.failureCode, taskID: auth.taskID))
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          AuthenticationInterventionActions(
            primaryTitle: authenticationTitle(state: auth.state),
            disabled: model.isOffline,
            skipDisabled: auth.state.uppercased() == "AUTHORIZING",
            onStart: { browserURL = await model.startMcpAuthentication(auth) },
            onSkip: { await model.skipMcpAuthentication(auth) }
          )
        case let .adapterAuth(auth):
          Label("Sign-in required", systemImage: "person.badge.key")
            .interventionEyebrow()
          Text("Sign in to \(auth.serviceName)")
            .font(NoemaFont.taskTitle)
          Text(authenticationDescription(state: auth.state, failureCode: auth.failureCode, taskID: auth.taskID))
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          AuthenticationInterventionActions(
            primaryTitle: authenticationTitle(state: auth.state),
            disabled: model.isOffline,
            skipDisabled: auth.state.uppercased() == "AUTHORIZING",
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
    .shadow(color: NoemaColor.content.opacity(0.08), radius: 4, y: 3)
  }

  private func authenticationTitle(state: String) -> String {
    state.uppercased() == "AUTHORIZING" ? "Open sign-in again" : "Continue in browser"
  }

  private func authenticationDescription(state: String, failureCode: String?, taskID: String?) -> String {
    if failureCode != nil { return "The previous sign-in did not finish. Try again to continue." }
    if state.uppercased() == "AUTHORIZING" { return "A sign-in was already opened. You can continue it or start again." }
    return taskID == nil ? "Your request is paused until you sign in." : "This task is paused until you sign in."
  }

  private func reviewLabel(_ action: GovernedActionModel) -> String {
    let behavior = action.readOnly == true ? "Read only" : "Can make changes"
    return "\(behavior) · \(action.reviewRoute.uppercased() == "LLM_REVIEW" ? "LLM review" : "Human review")"
  }

  private func governedArguments(_ action: GovernedActionModel) -> some View {
    let isExpanded = expandedGovernedActionIDs.contains(action.actionID)
    return VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Button {
        if isExpanded {
          expandedGovernedActionIDs.remove(action.actionID)
        } else {
          expandedGovernedActionIDs.insert(action.actionID)
        }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          Image(systemName: "chevron.right")
            .font(.system(size: 9, weight: .semibold))
            .rotationEffect(.degrees(isExpanded ? 90 : 0))
          Text("Review exact arguments")
            .font(NoemaFont.caption)
        }
        .foregroundStyle(NoemaColor.contentSecondary)
        .frame(minHeight: 20)
      }
      .buttonStyle(.plain)
      .accessibilityValue(isExpanded ? "Expanded" : "Collapsed")

      if isExpanded {
        Text(action.arguments)
          .font(NoemaFont.monoTiny)
          .foregroundStyle(NoemaColor.content)
          .textSelection(.enabled)
          .padding(NoemaSpacing.sm)
          .frame(maxWidth: .infinity, alignment: .leading)
          .background(NoemaColor.paper100, in: RoundedRectangle(cornerRadius: NoemaRadius.element, style: .continuous))
      }
    }
  }

  @ViewBuilder
  private func taskAttentionContent(_ attention: ChatTaskAttentionModel) -> some View {
    Label(attention.gate?.kind == "APPROVAL" ? "Approval needed" : attention.gate?.kind == "RECOVERY" ? "Recovery needed" : "Clarification needed", systemImage: "hand.raised")
      .interventionEyebrow()
    if let gate = attention.gate {
      Text(gate.prompt.nilIfBlank ?? attention.summary)
        .font(NoemaFont.taskTitle)
      if !gate.context.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        Markdown(gate.context)
          .markdownTextStyle { ForegroundColor(NoemaColor.contentSecondary) }
      }
      switch gate.kind {
      case "APPROVAL":
        TextField("Optional note", text: taskResponseBinding(attention), axis: .vertical)
          .noemaTextField()
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Decline") { Task { await model.answerTask(attention, answer: taskResponses[taskResponseKey(attention)]?.nilIfBlank ?? "Declined", approval: .declined) } }
            .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
          Button("Approve") { Task { await model.answerTask(attention, answer: taskResponses[taskResponseKey(attention)]?.nilIfBlank ?? "Approved", approval: .approved) } }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
        }
      case "RECOVERY":
        if let reason = gate.recoveryReason?.nilIfBlank {
          Text(reason.capitalized)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
        }
        HStack(spacing: NoemaSpacing.xs) {
          TextField(recoveryPlaceholder(attention), text: taskResponseBinding(attention), axis: .vertical)
            .noemaTextField()
          Button("Respond", systemImage: "arrow.up") {
            let response = taskResponses[taskResponseKey(attention)]?.nilIfBlank
            if let response, hasTaskAction(attention, "ANSWER") {
              Task { await model.answerTask(attention, answer: response) }
            } else {
              Task { await model.retryTask(attention, note: response) }
            }
          }
          .labelStyle(.iconOnly)
          .buttonStyle(NoemaTaskResponseSubmitStyle())
          .accessibilityLabel(recoveryActionLabel(attention))
        }
        .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER") && !hasTaskAction(attention, "RETRY") || hasTaskAction(attention, "ANSWER") && !hasTaskAction(attention, "RETRY") && taskResponses[taskResponseKey(attention)]?.nilIfBlank == nil)
      default:
        ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
          Button(suggestion) { Task { await model.answerTask(attention, answer: suggestion) } }
            .buttonStyle(NoemaTaskAnswerButtonStyle())
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
        }
        HStack(spacing: NoemaSpacing.sm) {
          TextField("Or type another answer", text: taskResponseBinding(attention), axis: .vertical)
            .noemaTaskResponseField()
          Button {
            guard let answer = taskResponses[taskResponseKey(attention)]?.nilIfBlank else { return }
            Task { await model.answerTask(attention, answer: answer) }
          } label: {
            Image(systemName: "arrow.right")
              .font(.system(size: 14, weight: .semibold))
          }
          .buttonStyle(NoemaTaskResponseSubmitStyle())
          .accessibilityLabel("Answer")
          .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER") || taskResponses[taskResponseKey(attention)]?.nilIfBlank == nil)
        }
        .padding(NoemaSpacing.xs)
        .background(NoemaColor.pine500, in: Capsule())
        .shadow(color: NoemaColor.pine700.opacity(0.10), radius: 6, y: 3)
      }
    } else {
      Text(attention.title)
        .font(NoemaFont.taskTitle)
      Text(attention.summary)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
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

  private func recoveryPlaceholder(_ attention: ChatTaskAttentionModel) -> String {
    let canAnswer = hasTaskAction(attention, "ANSWER")
    let canRetry = hasTaskAction(attention, "RETRY")
    if canAnswer && canRetry { return "Answer, or leave blank to retry" }
    if canRetry { return "Optional retry guidance" }
    return "Type your answer"
  }

  private func recoveryActionLabel(_ attention: ChatTaskAttentionModel) -> String {
    taskResponses[taskResponseKey(attention)]?.nilIfBlank != nil && hasTaskAction(attention, "ANSWER") ? "Answer" : "Retry"
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

private struct NoemaTaskAnswerButtonStyle: ButtonStyle {
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(NoemaFont.bodyEmphasized)
      .foregroundStyle(NoemaColor.paper50)
      .multilineTextAlignment(.leading)
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.vertical, NoemaSpacing.compact)
      .frame(minHeight: 44, alignment: .leading)
      .background(NoemaColor.pine500.opacity(configuration.isPressed ? 0.8 : 1), in: Capsule())
      .opacity(isEnabled ? 1 : 0.5)
  }
}

private struct NoemaTaskResponseFieldModifier: ViewModifier {
  func body(content: Content) -> some View {
    content
      .font(.custom("Hanken Grotesk", size: 16, relativeTo: .body))
      .foregroundStyle(NoemaColor.paper50)
      .tint(NoemaColor.paper50)
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.xs)
      .frame(minHeight: 36)
  }
}

private extension View {
  func noemaTaskResponseField() -> some View {
    modifier(NoemaTaskResponseFieldModifier())
  }

  func interventionEyebrow() -> some View {
    font(NoemaFont.taskPreview.weight(.semibold))
      .textCase(.uppercase)
      .tracking(0.5)
      .foregroundStyle(NoemaColor.contentSecondary)
  }
}

private struct NoemaTaskResponseSubmitStyle: ButtonStyle {
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .foregroundStyle(NoemaColor.content)
      .frame(width: 36, height: 36)
      .background(NoemaColor.paper50.opacity(configuration.isPressed ? 0.78 : 1), in: Circle())
      .opacity(isEnabled ? 1 : 0.46)
  }
}
