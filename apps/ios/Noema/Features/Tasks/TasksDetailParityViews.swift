import Foundation
import SwiftUI

enum TaskDetailTab: String, Hashable, Identifiable {
  case workspace
  case transcript

  var id: String { rawValue }
  var title: String { rawValue.capitalized }
}

struct TasksQueueSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  @State private var isSubmitting = false
  @State private var errorMessage: String?

  var body: some View {
    NoemaNativeSheet(
      title: "Queue this task?",
      dismissDisabled: isSubmitting,
      onDismiss: { dismiss() }
    ) {
      VStack(alignment: .leading, spacing: 0) {
        Text("Task execution will start from the current Inbox request.")
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.top, NoemaSpacing.md)
          .padding(.bottom, 19)
        Text(task.title)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
          .lineLimit(2)
          .padding(.horizontal, NoemaSpacing.lg)
        if let errorMessage {
          NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .warning)
            .padding(.horizontal, NoemaSpacing.lg)
            .padding(.top, NoemaSpacing.md)
        }
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { dismiss() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .disabled(isSubmitting)
          Button {
            Task {
              isSubmitting = true
              errorMessage = nil
              let succeeded = await model.queue(task: task)
              isSubmitting = false
              if succeeded { dismiss() }
              else { errorMessage = model.lastError ?? "The task could not be queued. Try again." }
            }
          } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if isSubmitting { ProgressView().tint(NoemaColor.white).controlSize(.small) }
              Text("Queue task")
            }
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.white)
            .frame(minHeight: 32)
            .padding(.horizontal, NoemaSpacing.md)
          }
          .buttonStyle(.plain)
          .background(NoemaColor.pine500, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .disabled(isSubmitting || !model.isConnected)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, NoemaSpacing.lg)
        .padding(.bottom, NoemaSpacing.sm)
      }
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation([.height(250)], regularHeight: 340)
  }
}

struct TasksCancelSheet: View {
  @Environment(\.dismiss) private var dismiss
  @Bindable var model: TasksModel
  let task: TasksDetailSnapshot
  @State private var reason = ""
  @State private var isSubmitting = false
  @State private var errorMessage: String?
  @FocusState private var reasonFocused: Bool

  var body: some View {
    NoemaNativeSheet(
      title: "Cancel this task?",
      dismissDisabled: isSubmitting,
      onDismiss: { dismiss() }
    ) {
      VStack(alignment: .leading, spacing: 0) {
        Text("Active work is fenced immediately. Historic evidence remains available.")
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.top, NoemaSpacing.md)
          .padding(.bottom, 19)
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          TasksSheetField("Reason (optional)") {
            TextField("", text: $reason, axis: .vertical)
              .lineLimit(3...5)
              .focused($reasonFocused)
              .noemaTaskSheetField(focused: reasonFocused, height: 76)
          }
          if let errorMessage {
            Text(errorMessage)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.danger)
              .fixedSize(horizontal: false, vertical: true)
          }
        }
        .padding(.horizontal, NoemaSpacing.lg)

        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { dismiss() }
            .buttonStyle(.plain)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.content)
            .disabled(isSubmitting)
          Button {
            Task {
              isSubmitting = true
              errorMessage = nil
              let succeeded = await model.cancel(task: task, reason: reason.nilIfBlank)
              isSubmitting = false
              if succeeded {
                dismiss()
              } else {
                errorMessage = model.lastError ?? "Noema could not cancel this task."
              }
            }
          } label: {
            HStack(spacing: NoemaSpacing.xs) {
              if isSubmitting { ProgressView().tint(NoemaColor.white).controlSize(.small) }
              Text("Cancel task")
            }
            .font(NoemaFont.bodyEmphasized)
            .foregroundStyle(NoemaColor.white)
            .frame(minHeight: 32)
            .padding(.horizontal, NoemaSpacing.md)
          }
          .buttonStyle(.plain)
          .background(NoemaColor.danger, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .opacity(model.isConnected ? 1 : 0.42)
          .disabled(isSubmitting || !model.isConnected)
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.top, NoemaSpacing.lg)
        .padding(.bottom, NoemaSpacing.sm)
      }
      .background(NoemaColor.surface)
    }
    .noemaTaskSheetPresentation([.height(330)], regularHeight: 460)
    .interactiveDismissDisabled(isSubmitting)
    .task { reasonFocused = true }
  }
}

struct TasksDetailTabBar: View {
  @Binding var selection: TaskDetailTab
  let tabs: [TaskDetailTab]
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    HStack(spacing: NoemaSpacing.xxs) {
      ForEach(tabs) { tab in
        Button {
          withAnimation(NoemaMotion.animation(NoemaSpring.surface, reduceMotion: reduceMotion)) {
            selection = tab
          }
        } label: {
          Text(tab.title)
            .font(selection == tab ? NoemaFont.captionEmphasized : NoemaFont.caption)
            .foregroundStyle(selection == tab ? NoemaColor.content : NoemaColor.contentSecondary)
            .padding(.horizontal, NoemaSpacing.md)
            .frame(height: 28)
            .overlay(alignment: .bottom) {
              NoemaSuperellipse.full
                .fill(selection == tab ? NoemaColor.accent : Color.clear)
                .frame(height: 2)
                .padding(.horizontal, NoemaSpacing.md)
                .offset(y: 1)
            }
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selection == tab ? .isSelected : [])
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, NoemaSpacing.xs)
    .overlay(alignment: .bottom) {
      Rectangle().fill(NoemaColor.separator).frame(height: 1)
    }
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Task detail view")
  }
}

struct TasksDocumentView: View {
  var citations: [ProviderCitation] = []
  var profile: NoemaProfile?
  let document: String
  let fileName: String

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if let response = document.nilIfBlank {
        ProviderCitationMarkdown(
          text: response,
          citations: citations,
          role: .assistantMessage,
          profile: profile
        )
          .textSelection(.enabled)
      } else {
        Text("\(fileName) has no text content.")
          .font(NoemaFont.taskTitle)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
    }
    .frame(maxWidth: 760, alignment: .leading)
    .padding(.horizontal, NoemaSpacing.xxl)
    .padding(.top, NoemaSpacing.lg)
    .padding(.bottom, NoemaSpacing.xxl)
    .frame(maxWidth: .infinity, alignment: .leading)
  }
}

struct TasksTranscriptSection: View {
  let profile: NoemaProfile?
  let messages: [TasksMessageSnapshot]
  let runs: [TasksRunSnapshot]
  let runItems: [TasksRunItemSnapshot]
  let hasMore: Bool
  let isLoadingMore: Bool
  let request: String
  let bottomSpacing: CGFloat
  let loadMore: () -> Void

  private enum Event: Identifiable {
    case request(String)
    case message(TasksMessageSnapshot)
    case run(TasksRunSnapshot)

    var id: String {
      switch self {
      case .request: "request"
      case .message(let message): "message:\(message.id)"
      case .run(let run): "run:\(run.id)"
      }
    }

    var timestamp: String {
      switch self {
      case .request: ""
      case .message(let message): message.createdAt
      case .run(let run): run.createdAt ?? run.startedAt ?? ""
      }
    }
  }

  private enum Row: Identifiable {
    case request(String)
    case message(TasksMessageSnapshot)
    case runBoundary(TasksRunSnapshot, ending: Bool)
    case runItem(TasksRunItemSnapshot, result: TasksRunItemSnapshot?)
    case activity(TasksRunSnapshot, String)

    var id: String {
      switch self {
      case .request: "request"
      case .message(let message): "message:\(message.id)"
      case .runBoundary(let run, let ending): "run:\(run.id):\(ending ? "end" : "start")"
      case .runItem(let item, _): "run-item:\(item.id)"
      case .activity(let run, _): "run:\(run.id):activity"
      }
    }
  }

  private struct OrderedEvent {
    let event: Event
    let timestamp: Date
  }

  private var rows: [Row] {
    var events: [Event] = []
    if let request = request.nilIfBlank, !capturedRequestIsAlreadyShown {
      events.append(.request(request))
    }
    events.append(contentsOf: messages.map(Event.message))
    for run in runs {
      events.append(.run(run))
    }
    let fractional = ISO8601DateFormatter()
    fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let plain = ISO8601DateFormatter()
    plain.formatOptions = [.withInternetDateTime]
    var orderedEvents: [OrderedEvent] = []
    orderedEvents.reserveCapacity(events.count)
    for event in events {
      let timestamp = fractional.date(from: event.timestamp)
        ?? plain.date(from: event.timestamp)
        ?? .distantPast
      orderedEvents.append(OrderedEvent(event: event, timestamp: timestamp))
    }
    orderedEvents.sort {
      $0.timestamp == $1.timestamp ? $0.event.id < $1.event.id : $0.timestamp < $1.timestamp
    }
    var itemsByRunID: [String: [TasksRunItemSnapshot]] = [:]
    for item in runItems { itemsByRunID[item.runId, default: []].append(item) }
    return orderedEvents.flatMap { orderedEvent -> [Row] in
      switch orderedEvent.event {
      case .request(let request):
        [.request(request)]
      case .message(let message):
        [.message(message)]
      case .run(let run):
        runRows(run, items: itemsByRunID[run.id] ?? [])
      }
    }
  }

  private var capturedRequestIsAlreadyShown: Bool {
    guard let request = request.nilIfBlank else { return true }
    return messages.contains { $0.body.trimmingCharacters(in: .whitespacesAndNewlines) == request }
  }

  var body: some View {
    let rows = rows
    LazyVStack(alignment: .leading, spacing: NoemaSpacing.md) {
      if rows.isEmpty {
        Text("No transcript entries yet.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      } else {
        if hasMore {
          Button(isLoadingMore ? "Loading earlier activity…" : "Load earlier activity", action: loadMore)
            .font(NoemaFont.captionEmphasized)
            .foregroundStyle(NoemaColor.accent)
            .disabled(isLoadingMore)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        ForEach(rows) { row in rowView(row) }
      }
      Color.clear
        .frame(height: bottomSpacing)
        .padding(.top, NoemaSpacing.xs)
        .id("task-transcript-bottom")
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.vertical, NoemaSpacing.xs)
  }

  @ViewBuilder
  private func rowView(_ row: Row) -> some View {
    switch row {
    case .request(let request):
      messageEntry(body: request, human: true, label: "Captured request")
    case .message(let message):
      messageEntry(body: message.body, human: isHumanAuthor(message.author))
    case .runBoundary(let run, let ending):
      runBoundary(run, ending: ending)
    case .runItem(let item, let result):
      runItemEntry(item, result: result)
    case .activity(let run, let activity):
      activityRow(run, activity: activity)
    }
  }

  @ViewBuilder
  private func runItemEntry(_ item: TasksRunItemSnapshot, result: TasksRunItemSnapshot?) -> some View {
    switch item.kind.uppercased() {
    case "ASSISTANT_OUTPUT":
      if let content = item.content?.nilIfBlank { messageEntry(body: content, human: false) }
    case "TOOL_CALL":
      toolActivityRow(item, result: result)
    case "TOOL_RESULT":
      toolActivityRow(item, result: nil)
    default:
      HStack(spacing: NoemaSpacing.compact) {
        runItemStatusIcon(item.status)
        Text(item.content?.nilIfBlank ?? runItemTitle(item.kind))
          .font(NoemaFont.mono)
          .foregroundStyle(NoemaColor.contentSecondary)
          .lineLimit(2)
        Spacer(minLength: NoemaSpacing.xs)
        Image(systemName: "chevron.down")
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(NoemaColor.contentTertiary)
      }
    }
  }

  private func toolActivityRow(_ item: TasksRunItemSnapshot, result: TasksRunItemSnapshot?) -> some View {
    ToolMarkerView(client: nil, profile: profile, messages: taskToolMessages(item, result: result))
  }

  @ViewBuilder
  private func runItemStatusIcon(_ status: String) -> some View {
    let normalized = status.uppercased()
    if ["STARTED", "RUNNING", "QUEUED", "ACTIVE"].contains(normalized) {
      ProgressView()
        .controlSize(.mini)
        .tint(NoemaColor.success)
        .frame(width: 16)
        .accessibilityLabel("Running")
    } else {
      Image(systemName: normalized == "FAILED" ? "exclamationmark" : "checkmark")
        .font(NoemaFont.metadata.weight(.semibold))
        .foregroundStyle(normalized == "FAILED" ? NoemaColor.warning : NoemaColor.success)
        .frame(width: 16)
        .accessibilityLabel(normalized == "FAILED" ? "Failed" : "Completed")
    }
  }


  private func runRows(_ run: TasksRunSnapshot, items: [TasksRunItemSnapshot]) -> [Row] {
    var rows: [Row] = [.runBoundary(run, ending: false)]
    var resultsByCorrelation: [String: TasksRunItemSnapshot] = [:]
    for item in items where item.kind.uppercased() == "TOOL_RESULT" {
      if let correlation = item.correlationId, resultsByCorrelation[correlation] == nil {
        resultsByCorrelation[correlation] = item
      }
    }
    let pairedResultIDs = Set(items.compactMap { item -> String? in
      guard item.kind.uppercased() == "TOOL_CALL", let correlation = item.correlationId else { return nil }
      return resultsByCorrelation[correlation]?.id
    })
    for item in items {
      switch item.kind.uppercased() {
      case "MODEL_INPUT", "CONTEXT_CHECKPOINT", "PROGRESS_NOTICE", "TASK_SUBMISSION", "TASK_REVIEW":
        continue
      case "ASSISTANT_OUTPUT" where item.content?.nilIfBlank == nil:
        continue
      case "TOOL_CALL":
        let result = item.correlationId.flatMap { resultsByCorrelation[$0] }
        rows.append(.runItem(item, result: result))
      case "TOOL_RESULT" where pairedResultIDs.contains(item.id):
        continue
      default:
        rows.append(.runItem(item, result: nil))
      }
    }
    if items.isEmpty, let activity = run.activity.nilIfBlank {
      rows.append(.activity(run, activity))
    }
    if run.isTerminal { rows.append(.runBoundary(run, ending: true)) }
    return rows
  }

  private func runItemTitle(_ kind: String) -> String {
    kind.lowercased().split(separator: "_").map { $0.capitalized }.joined(separator: " ")
  }

  private func messageEntry(body: String, human: Bool, label: String? = nil) -> some View {
    HStack(alignment: .bottom, spacing: 0) {
      if human { Spacer(minLength: 0) }
      VStack(alignment: human ? .trailing : .leading, spacing: NoemaSpacing.xxs) {
        if let label {
          Text(label)
            .font(NoemaFont.captionEmphasized)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        messageBubble(body: body, human: human)
      }
      .frame(maxWidth: human ? 296 : .infinity, alignment: human ? .trailing : .leading)
    }
    .frame(maxWidth: .infinity, alignment: human ? .trailing : .leading)
  }

  private func messageBubble(body: String, human: Bool) -> some View {
    NoemaMarkdown(body, role: human ? .humanMessage : .assistantMessage)
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.vertical, NoemaSpacing.sm)
      .frame(maxWidth: human ? nil : .infinity, alignment: .leading)
      .background(
        human ? NoemaColor.pine500 : NoemaColor.paper100,
        in: NoemaSuperellipse(cornerRadius: NoemaRadius.page, treatment: .chat)
      )
      .clipShape(NoemaSuperellipse(cornerRadius: NoemaRadius.page, treatment: .chat))
  }

  private func runBoundary(_ run: TasksRunSnapshot, ending: Bool) -> some View {
    HStack(spacing: NoemaSpacing.md) {
      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(maxWidth: .infinity, maxHeight: 1)
      runAvatar(run)
      Text(runBoundaryTitle(run, ending: ending))
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
        .lineLimit(1)
        .layoutPriority(1)
      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(maxWidth: .infinity, maxHeight: 1)
    }
    .padding(.vertical, NoemaSpacing.xs)
  }

  private func runAvatar(_ run: TasksRunSnapshot) -> some View {
    let avatarMotion = noemaTaskRunAvatarMotion(status: run.status)
    return NoemaIdentityAvatar(
      actorID: "subagent:\(run.instanceName)",
      actorType: .agent,
      activity: avatarMotion.activity,
      animated: avatarMotion.animated
    )
      .frame(width: 16, height: 16)
      .accessibilityLabel("\(runRoleLabel(run)) run")
  }

  private func activityRow(_ run: TasksRunSnapshot, activity: String) -> some View {
    HStack(spacing: NoemaSpacing.compact) {
      Image(systemName: run.isTerminal ? "checkmark" : "ellipsis")
        .font(NoemaFont.metadata.weight(.semibold))
        .foregroundStyle(run.isTerminal ? NoemaColor.success : NoemaColor.contentTertiary)
        .frame(width: 16)
      Text(activity)
        .font(NoemaFont.mono)
        .foregroundStyle(NoemaColor.contentSecondary)
        .lineLimit(1)
      Spacer(minLength: NoemaSpacing.xs)
    }
    .padding(.leading, NoemaSpacing.xxl)
  }

  private func isHumanAuthor(_ author: String) -> Bool {
    let components = author.split(separator: ":", omittingEmptySubsequences: true)
    return components.contains { $0.caseInsensitiveCompare("human") == .orderedSame }
  }

  private func runBoundaryTitle(_ run: TasksRunSnapshot, ending: Bool) -> String {
    let role = runRoleLabel(run)
    guard ending else { return "\(run.instanceName) · \(role) · Running" }
    let outcome: String
    switch run.status.uppercased() {
    case "COMPLETED": outcome = "Completed"
    case "FAILED": outcome = "Failed"
    case "CANCELLED": outcome = "Cancelled"
    case "INTERRUPTED": outcome = "Interrupted"
    default: outcome = "Finished"
    }
    let duration = runDurationLabel(run).map { " · \($0)" } ?? ""
    return "\(run.instanceName) · \(role) · \(outcome)\(duration)"
  }

  private func runRoleLabel(_ run: TasksRunSnapshot) -> String {
    switch run.kind.uppercased() {
    case "PLANNER": "Planner"
    case "EXECUTOR": "Executor"
    case "REVIEWER": "Reviewer"
    default: run.kind.capitalized
    }
  }

  private func runDurationLabel(_ run: TasksRunSnapshot) -> String? {
    guard let started = parseTimestamp(run.startedAt ?? run.createdAt),
          let ended = parseTimestamp(run.endedAt ?? run.createdAt) else { return nil }
    let seconds = max(0, Int(ended.timeIntervalSince(started).rounded()))
    if seconds < 60 { return "\(seconds)s" }
    let minutes = seconds / 60
    if minutes < 60 { return "\(minutes)m" }
    let hours = minutes / 60
    let remaining = minutes % 60
    return remaining > 0 ? "\(hours)h \(remaining)m" : "\(hours)h"
  }

  private func parseTimestamp(_ value: String?) -> Date? {
    guard let value else { return nil }
    let fractional = ISO8601DateFormatter()
    fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return fractional.date(from: value) ?? ISO8601DateFormatter().date(from: value)
  }
}

func taskRunItemActivityText(in items: [TasksRunItemSnapshot], runId: String) -> String? {
  let runItems = items.filter { $0.runId == runId }
  guard let latest = runItems.max(by: { $0.sequence < $1.sequence }) else { return nil }
  switch latest.kind.uppercased() {
  case "TOOL_CALL":
    return toolMarkerName(in: taskToolMessages(latest, result: nil))
  case "TOOL_RESULT":
    let call = latest.correlationId.flatMap { correlationID in
      runItems.last { $0.kind.uppercased() == "TOOL_CALL" && $0.correlationId == correlationID }
    }
    return toolMarkerName(in: call.map { taskToolMessages($0, result: latest) } ?? [taskToolMessage(latest)])
  default:
    return latest.content
  }
}

private func taskToolMessages(_ item: TasksRunItemSnapshot, result: TasksRunItemSnapshot?) -> [ChatMessage] {
  var messages = [taskToolMessage(item)]
  if let result { messages.append(taskToolMessage(result)) }
  return messages
}

private func taskToolMessage(_ item: TasksRunItemSnapshot) -> ChatMessage {
  let isResult = item.kind.uppercased() == "TOOL_RESULT"
  let payload = taskToolPayload(item.payloadText)
  let name = item.content?.nilIfBlank ?? "Tool activity"
  var action: [String: Any] = ["name": name]
  if let correlationID = item.correlationId?.nilIfBlank {
    action["correlation_id"] = correlationID
  }
  if isResult {
    if let correlationID = item.correlationId?.nilIfBlank {
      action["call_id"] = correlationID
    }
    action["payload"] = payload["payload"] ?? payload
    action["success"] = item.status.uppercased() != "FAILED"
  } else {
    if let correlationID = item.correlationId?.nilIfBlank {
      action["id"] = correlationID
    }
    action["arguments"] = payload["arguments"] ?? payload
  }
  var display: [String: Any] = ["name": name]
  if let serverDisplay = payload["display"] as? [String: Any] {
    display.merge(serverDisplay) { _, server in server }
  }
  let metadata: [String: Any] = [
    "action": action,
    "display": display
  ]
  return ChatMessage(
    id: item.id,
    kind: .activity(
      title: name,
      summary: name,
      status: taskToolStatus(item.status),
      metadata: prettyJSON(metadata) ?? "{}",
      activityKind: isResult ? "TOOL_RESULT" : "TOOL_CALL"
    )
  )
}

private func taskToolPayload(_ text: String) -> [String: Any] {
  guard text != "null",
        let data = text.data(using: .utf8),
        let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
  else { return [:] }
  return payload
}

private func taskToolStatus(_ status: String) -> String {
  switch status.uppercased() {
  case "FAILED": "FAILED"
  case "STARTED", "RUNNING", "QUEUED", "ACTIVE": "STARTED"
  default: "COMPLETED"
  }
}

private extension TasksRunSnapshot {
  var isTerminal: Bool {
    switch status.uppercased() {
    case "COMPLETED", "FAILED", "CANCELLED", "INTERRUPTED": true
    default: false
    }
  }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
