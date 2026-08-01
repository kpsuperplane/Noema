import Foundation
import MarkdownUI
import SwiftUI

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
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.element)
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
        Text(submission.createdAt).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
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
  let request: String
  @State private var expanded = false

  private enum Event: Identifiable {
    case message(TasksMessageSnapshot)
    case run(TasksRunSnapshot)

    var id: String {
      switch self {
      case .message(let message): "message:\(message.id)"
      case .run(let run): "run:\(run.id)"
      }
    }

    var timestamp: String {
      switch self {
      case .message(let message): message.createdAt
      case .run(let run): run.createdAt ?? run.startedAt ?? ""
      }
    }
  }

  private var events: [Event] {
    (messages.map(Event.message) + runs.map(Event.run)).sorted { $0.timestamp < $1.timestamp }
  }

  private var capturedRequestIsAlreadyShown: Bool {
    guard let request = request.nilIfBlank else { return true }
    return messages.contains { $0.body.trimmingCharacters(in: .whitespacesAndNewlines) == request }
  }

  var body: some View {
    DisclosureGroup("Transcript", isExpanded: $expanded) {
      if events.isEmpty && request.nilIfBlank == nil {
        Text("No transcript entries yet.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
          .padding(.top, NoemaSpacing.xs)
      } else {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          if let request = request.nilIfBlank, !capturedRequestIsAlreadyShown {
            transcriptEntry(author: "Captured request", timestamp: nil, body: request)
          }
          ForEach(events) { event in eventView(event) }
        }
        .padding(.top, NoemaSpacing.xs)
      }
    }
    .font(NoemaFont.title)
  }

  private func transcriptEntry(author: String, timestamp: String?, body: String) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      HStack {
        Text(author).font(NoemaFont.captionEmphasized)
        Spacer(minLength: NoemaSpacing.sm)
        if let timestamp, !timestamp.isEmpty {
          Text(timestamp).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
        }
      }
      Markdown(body).markdownTextStyle { ForegroundColor(NoemaColor.content) }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
  }

  @ViewBuilder
  private func eventView(_ event: Event) -> some View {
    switch event {
    case .message(let message):
      transcriptEntry(author: message.author, timestamp: message.createdAt, body: message.body)
    case .run(let run):
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        HStack {
          Text("Run · \(run.kind.capitalized)").font(NoemaFont.captionEmphasized)
          Spacer(minLength: NoemaSpacing.sm)
          Text(run.createdAt ?? run.startedAt ?? "").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
        }
        Text(run.activity.nilIfBlank ?? run.status.capitalized)
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
        if let error = run.error?.nilIfBlank {
          Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.warning)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
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
