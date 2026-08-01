import Foundation
import MarkdownUI
import SwiftUI

enum TaskResultTab: String, CaseIterable, Identifiable {
  case result
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
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(
        title: "Queue this task?",
        subtitle: "Work will start from the current Inbox request.",
        onClose: { dismiss() },
        isDisabled: isSubmitting
      )
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
        .background(NoemaColor.pine500, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
        .disabled(isSubmitting || !model.isConnected)
      }
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.top, NoemaSpacing.lg)
      .padding(.bottom, NoemaSpacing.sm)
    }
    .background(NoemaColor.surface)
    .presentationDetents([.height(250)])
    .presentationSizing(.page)
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.container)
    .presentationBackground(NoemaColor.surface)
  }
}

struct TasksSubmissionSection: View {
  let submission: TasksSubmissionSnapshot
  let title: String
  let onArtifact: (TasksArtifactSnapshot) -> Void
  @State private var criteriaExpanded = false

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(alignment: .firstTextBaseline) {
        Text(title).font(NoemaFont.title)
        Spacer(minLength: NoemaSpacing.sm)
      }
      if !submission.summary.isEmpty { Text(submission.summary).font(NoemaFont.bodyEmphasized) }
      Markdown(submission.result).markdownTextStyle { ForegroundColor(NoemaColor.content) }
      if !submission.artifacts.isEmpty {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Artifacts").font(NoemaFont.captionEmphasized).foregroundStyle(NoemaColor.contentSecondary)
          ForEach(submission.artifacts) { artifact in
            ArtifactReferenceView(reference: artifact.reference) { onArtifact(artifact) }
          }
        }
      }
      if !submission.criteria.isEmpty {
        DisclosureGroup("Criteria evidence", isExpanded: $criteriaExpanded) {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            ForEach(submission.criteria) { criterion in
              VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                Text(criterion.description.isEmpty ? "Criterion \(criterion.ordinal + 1)" : criterion.description)
                  .font(NoemaFont.captionEmphasized)
                if let evidence = criterion.evidence, !evidence.isEmpty {
                  Text(evidence).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
                }
              }
            }
          }
          .padding(.top, NoemaSpacing.xs)
        }
        .font(NoemaFont.captionEmphasized)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
  }
}

struct TasksTranscriptSection: View {
  let messages: [TasksMessageSnapshot]
  let runs: [TasksRunSnapshot]
  let runItems: [TasksRunItemSnapshot]
  let hasMore: Bool
  let isLoadingMore: Bool
  let request: String
  let submission: TasksSubmissionSnapshot?
  let loadMore: () -> Void
  let onArtifact: (TasksArtifactSnapshot) -> Void
  @State private var expandedRunItemIDs: Set<String> = []

  private enum Event: Identifiable {
    case request(String)
    case message(TasksMessageSnapshot)
    case run(TasksRunSnapshot)
    case submission(TasksSubmissionSnapshot)

    var id: String {
      switch self {
      case .request: "request"
      case .message(let message): "message:\(message.id)"
      case .run(let run): "run:\(run.id)"
      case .submission(let submission): "submission:\(submission.id)"
      }
    }

    var timestamp: String {
      switch self {
      case .request: ""
      case .message(let message): message.createdAt
      case .run(let run): run.createdAt ?? run.startedAt ?? ""
      case .submission(let submission): submission.createdAt
      }
    }
  }

  private var events: [Event] {
    var events: [Event] = []
    if let request = request.nilIfBlank, !capturedRequestIsAlreadyShown {
      events.append(.request(request))
    }
    events.append(contentsOf: messages.map(Event.message))
    for run in runs {
      events.append(.run(run))
    }
    if let submission { events.append(.submission(submission)) }
    return events.sorted {
      let left = parseTimestamp($0.timestamp) ?? .distantPast
      let right = parseTimestamp($1.timestamp) ?? .distantPast
      return left == right ? $0.id < $1.id : left < right
    }
  }

  private var capturedRequestIsAlreadyShown: Bool {
    guard let request = request.nilIfBlank else { return true }
    return messages.contains { $0.body.trimmingCharacters(in: .whitespacesAndNewlines) == request }
  }

  var body: some View {
    Group {
      if events.isEmpty {
        Text("No transcript entries yet.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      } else {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          if hasMore {
            Button(isLoadingMore ? "Loading earlier activity…" : "Load earlier activity", action: loadMore)
              .font(NoemaFont.captionEmphasized)
              .foregroundStyle(NoemaColor.accent)
              .disabled(isLoadingMore)
              .frame(maxWidth: .infinity, alignment: .center)
          }
          ForEach(events) { event in eventView(event) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, NoemaSpacing.xs)
      }
    }
  }

  @ViewBuilder
  private func eventView(_ event: Event) -> some View {
    switch event {
    case .request(let request):
      messageEntry(body: request, human: true, label: "Captured request")
    case .message(let message):
      messageEntry(body: message.body, human: isHumanAuthor(message.author))
    case .run(let run):
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        runBoundary(run, ending: false)
        ForEach(runItems.filter { $0.runId == run.id }.sorted { $0.sequence < $1.sequence }) { item in
          runItemEntry(item)
        }
        if runItems.allSatisfy({ $0.runId != run.id }), let activity = run.activity.nilIfBlank {
          activityRow(run, activity: activity)
        }
        if run.isTerminal { runBoundary(run, ending: true) }
      }
    case .submission(let submission): submissionEntry(submission)
    }
  }

  @ViewBuilder
  private func runItemEntry(_ item: TasksRunItemSnapshot) -> some View {
    switch item.kind.uppercased() {
    case "MODEL_INPUT", "CONTEXT_CHECKPOINT", "PROGRESS_NOTICE", "TASK_SUBMISSION", "TASK_REVIEW":
      EmptyView()
    case "ASSISTANT_OUTPUT":
      if let content = item.content?.nilIfBlank { messageEntry(body: content, human: false) }
    case "TOOL_CALL":
      toolActivityRow(item, result: matchingToolResult(for: item))
    case "TOOL_RESULT":
      if matchingToolCall(for: item) == nil { toolActivityRow(item, result: nil) }
    default:
      HStack(spacing: NoemaSpacing.compact) {
        Image(systemName: item.status.uppercased() == "FAILED" ? "exclamationmark" : "checkmark")
          .font(NoemaFont.metadata.weight(.semibold))
          .foregroundStyle(item.status.uppercased() == "FAILED" ? NoemaColor.warning : NoemaColor.success)
          .frame(width: 16)
        Text(item.content?.nilIfBlank ?? runItemTitle(item.kind))
          .font(.custom("JetBrains Mono", size: 12, relativeTo: .caption))
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
    let expanded = expandedRunItemIDs.contains(item.id)
    return VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Button {
        if expanded { expandedRunItemIDs.remove(item.id) } else { expandedRunItemIDs.insert(item.id) }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          Image(systemName: item.status.uppercased() == "FAILED" ? "exclamationmark" : "checkmark")
            .font(NoemaFont.metadata.weight(.semibold))
            .foregroundStyle(item.status.uppercased() == "FAILED" ? NoemaColor.warning : NoemaColor.success)
            .frame(width: 16)
          Text(item.content?.nilIfBlank ?? "Tool activity")
            .font(NoemaFont.mono)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(1)
          Spacer(minLength: NoemaSpacing.xs)
          Image(systemName: "chevron.down")
            .font(NoemaFont.metadata.weight(.semibold))
            .foregroundStyle(NoemaColor.contentTertiary)
            .rotationEffect(.degrees(expanded ? 180 : 0))
        }
      }
      .buttonStyle(.plain)
      if expanded {
        Text(toolDetails(item, result: result))
          .font(NoemaFont.monoTiny)
          .foregroundStyle(NoemaColor.contentSecondary)
          .textSelection(.enabled)
          .padding(.leading, NoemaSpacing.xxl)
      }
    }
  }

  private func matchingToolResult(for item: TasksRunItemSnapshot) -> TasksRunItemSnapshot? {
    guard let correlation = item.correlationId else { return nil }
    return runItems.first { $0.kind.uppercased() == "TOOL_RESULT" && $0.correlationId == correlation }
  }

  private func matchingToolCall(for item: TasksRunItemSnapshot) -> TasksRunItemSnapshot? {
    guard let correlation = item.correlationId else { return nil }
    return runItems.first { $0.kind.uppercased() == "TOOL_CALL" && $0.correlationId == correlation }
  }

  private func toolDetails(_ item: TasksRunItemSnapshot, result: TasksRunItemSnapshot?) -> String {
    var details = [item.payloadText]
    if let result {
      if let content = result.content?.nilIfBlank { details.append(content) }
      details.append(result.payloadText)
    }
    return details.filter { $0 != "null" && !$0.isEmpty }.joined(separator: "\n\n")
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
    Markdown(body)
      .markdownTextStyle {
        FontFamily(.custom("Hanken Grotesk"))
        FontSize(14)
        ForegroundColor(human ? NoemaColor.white : NoemaColor.content)
      }
      .markdownBlockStyle(\.paragraph) { configuration in
        configuration.label.markdownMargin(top: 0, bottom: 0)
      }
      .font(NoemaFont.body)
      .lineSpacing(NoemaSpacing.compact)
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.vertical, NoemaSpacing.sm)
      .frame(maxWidth: human ? nil : .infinity, alignment: .leading)
      .background(human ? NoemaColor.pine500 : NoemaColor.paper100, in: RoundedRectangle(cornerRadius: NoemaRadius.page, style: .continuous))
      .clipShape(RoundedRectangle(cornerRadius: NoemaRadius.page, style: .continuous))
  }

  private func runBoundary(_ run: TasksRunSnapshot, ending: Bool) -> some View {
    HStack(spacing: NoemaSpacing.md) {
      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(maxWidth: .infinity, maxHeight: 1)
      runAvatar(run)
      Text(runBoundaryTitle(run, ending: ending))
        .font(.custom("Hanken Grotesk", size: 13, relativeTo: .caption))
        .foregroundStyle(NoemaColor.contentSecondary)
        .lineLimit(1)
        .minimumScaleFactor(0.8)
        .layoutPriority(1)
      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(maxWidth: .infinity, maxHeight: 1)
    }
    .padding(.vertical, NoemaSpacing.xs)
  }

  private func runAvatar(_ run: TasksRunSnapshot) -> some View {
    Text(String(runRoleLabel(run).prefix(1)))
      .font(.custom("Hanken Grotesk", size: 9, relativeTo: .caption2).weight(.semibold))
      .foregroundStyle(NoemaColor.pine700)
      .frame(width: 16, height: 16)
      .background(NoemaColor.pine100, in: Circle())
      .accessibilityLabel("\(runRoleLabel(run)) run")
  }

  private func activityRow(_ run: TasksRunSnapshot, activity: String) -> some View {
    HStack(spacing: NoemaSpacing.compact) {
      Image(systemName: run.isTerminal ? "checkmark" : "ellipsis")
        .font(NoemaFont.metadata.weight(.semibold))
        .foregroundStyle(run.isTerminal ? NoemaColor.success : NoemaColor.contentTertiary)
        .frame(width: 16)
      Text(activity)
        .font(.custom("JetBrains Mono", size: 12, relativeTo: .caption))
        .foregroundStyle(NoemaColor.contentSecondary)
        .lineLimit(1)
      Spacer(minLength: NoemaSpacing.xs)
    }
    .padding(.leading, NoemaSpacing.xxl)
  }

  private func submissionEntry(_ submission: TasksSubmissionSnapshot) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text("Submission result")
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.contentSecondary)
      if let response = submission.result.nilIfBlank ?? submission.summary.nilIfBlank {
        messageBubble(body: response, human: false)
      }
      if !submission.artifacts.isEmpty {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Artifacts")
            .font(NoemaFont.captionEmphasized)
            .foregroundStyle(NoemaColor.contentSecondary)
          ForEach(submission.artifacts) { artifact in
            ArtifactReferenceView(reference: artifact.reference) { onArtifact(artifact) }
          }
        }
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
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

private extension TasksRunSnapshot {
  var isTerminal: Bool {
    switch status.uppercased() {
    case "COMPLETED", "FAILED", "CANCELLED", "INTERRUPTED": true
    default: false
    }
  }
}

extension TasksArtifactSnapshot {
  var reference: ArtifactReferenceModel {
    ArtifactReferenceModel(
      artifactID: id,
      versionID: versionID.nilIfBlank,
      title: title,
      kind: kind,
      storageKind: storageKind,
      externalURL: externalURL.flatMap(URL.init(string:)),
      downloadURL: downloadURL.flatMap(URL.init(string:)),
      mediaType: mediaType
    )
  }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}
