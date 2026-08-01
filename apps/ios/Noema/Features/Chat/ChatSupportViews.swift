import Foundation
import Apollo
import NoemaAPI
import SwiftUI

struct TaskReferenceChip: View {
  let client: ApolloClient?
  let taskID: String
  @Environment(NoemaShellCoordinator.self) private var coordinator
  @State private var title = "Loading task…"
  @State private var status = "UNKNOWN"

  var body: some View {
    Button {
      coordinator.openTask(taskID)
    } label: {
      HStack(spacing: NoemaSpacing.xs) {
        Image(systemName: "checklist")
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(NoemaColor.accent)
        Image(systemName: statusSymbol)
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(statusColor)
        Text(title)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
      }
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.compact)
      .background(NoemaColor.surface, in: Capsule())
      .overlay { Capsule().stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .buttonStyle(.plain)
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
    case "ACTIVE": "clock.arrow.circlepath"
    case "ATTENTION", "HUMAN_GATE": "person.crop.circle.badge.exclamationmark"
    case "TERMINAL_CANCELLED": "xmark.circle"
    default: "circle"
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
        Image(systemName: "arrow.up")
          .font(NoemaFont.bodyEmphasized)
          .frame(width: 32, height: 32)
      }
      .buttonStyle(.glass)
      .disabled(!canSend)
      .accessibilityLabel("Send message")
    }
    .padding(.leading, NoemaSpacing.md)
    .padding(.trailing, NoemaSpacing.xs)
    .padding(.vertical, NoemaSpacing.xs)
    .background(NoemaColor.pine500, in: RoundedRectangle(cornerRadius: NoemaRadius.container, style: .continuous))
    .shadow(color: NoemaColor.pine700.opacity(0.10), radius: 12, y: 5)
  }
}

struct ChatInterventionsView: View {
  @Bindable var model: ChatModel
  @State private var browserURL: URL?
  @State private var setupServerIDs: [String: String] = [:]

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text("Waiting for you")
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.contentSecondary)
      ScrollView(.horizontal, showsIndicators: false) {
        HStack(alignment: .top, spacing: NoemaSpacing.sm) {
          ForEach(model.interventions) { intervention in
            interventionCard(for: intervention)
          }
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
    NoemaCard(padding: NoemaSpacing.sm) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        switch intervention {
        case let .governed(action):
          Label("Approval needed", systemImage: "hand.raised")
            .font(NoemaFont.captionEmphasized)
          Text(action.summary)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(3)
          HStack(spacing: NoemaSpacing.sm) {
            Button("Decline") { Task { await model.resolve(intervention, decision: "DECLINE") } }
              .buttonStyle(.bordered)
            Button("Approve") { Task { await model.resolve(intervention, decision: "APPROVE") } }
              .buttonStyle(.borderedProminent)
          }
        case let .mcpAuth(auth):
          Label("Sign in to (auth.serverName)", systemImage: "person.badge.key")
            .font(NoemaFont.captionEmphasized)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          HStack(spacing: NoemaSpacing.sm) {
            Button("Sign in") { Task { browserURL = await model.startMcpAuthentication(auth) } }
              .buttonStyle(.borderedProminent)
              .disabled(model.isOffline)
            Button("Skip") { Task { await model.skipMcpAuthentication(auth) } }
              .buttonStyle(.bordered)
              .disabled(model.isOffline)
          }
        case let .adapterAuth(auth):
          Label("Sign in to (auth.serviceName)", systemImage: "person.badge.key")
            .font(NoemaFont.captionEmphasized)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          HStack(spacing: NoemaSpacing.sm) {
            Button("Sign in") { Task { browserURL = await model.startAdapterAuthentication(auth) } }
              .buttonStyle(.borderedProminent)
              .disabled(model.isOffline)
            Button("Skip") { Task { await model.skipAdapterAuthentication(auth) } }
              .buttonStyle(.bordered)
              .disabled(model.isOffline)
          }
        case let .setup(setup):
          Label("Set up (setup.displayName)", systemImage: "wrench.and.screwdriver")
            .font(NoemaFont.captionEmphasized)
          Text(setup.status.replacingOccurrences(of: "_", with: " ").capitalized)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          if let serviceURL = setup.serviceURL {
            Link("Open service", destination: serviceURL)
              .font(NoemaFont.captionEmphasized)
          }
          if let serverID = setup.serverID {
            Button("Confirm connected server") {
              Task { await model.resolveMcpSetup(setup, mcpServerID: serverID) }
            }
            .buttonStyle(.borderedProminent)
            .disabled(model.isOffline)
          } else {
            TextField("MCP server id after setup", text: Binding(
              get: { setupServerIDs[setup.itemID] ?? "" },
              set: { setupServerIDs[setup.itemID] = $0 }
            ))
            .textFieldStyle(.roundedBorder)
            Button("Confirm connected server") {
              guard let serverID = setupServerIDs[setup.itemID], !serverID.isEmpty else { return }
              Task { await model.resolveMcpSetup(setup, mcpServerID: serverID) }
            }
            .buttonStyle(.borderedProminent)
            .disabled(model.isOffline || (setupServerIDs[setup.itemID] ?? "").isEmpty)
          }
        case let .attention(title, summary):
          Label(title, systemImage: "questionmark.circle")
            .font(NoemaFont.captionEmphasized)
          Text(summary)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
      }
      .frame(width: 280, alignment: .leading)
    }
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
