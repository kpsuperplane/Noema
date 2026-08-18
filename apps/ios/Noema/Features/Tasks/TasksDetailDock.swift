import SwiftUI

struct TasksTaskContextDock: View {
  let run: TasksRunSnapshot?
  let activity: String?
  let canCancel: Bool
  let cancel: () -> Void
  let showInfo: () -> Void

  var body: some View {
    VStack(spacing: 0) {
      HStack(spacing: NoemaSpacing.sm) {
        let avatarMotion = noemaTaskRunAvatarMotion(status: run?.status ?? "")
        NoemaIdentityAvatar(
          actorID: "subagent:\(run?.instanceName ?? "Task")",
          actorType: .agent,
          activity: avatarMotion.activity,
          animated: avatarMotion.animated
        )
          .frame(width: 30, height: 30)
          .accessibilityLabel("\(run?.instanceName ?? "Task") run")
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(run.map { "\($0.instanceName) · \($0.kind.capitalized)" } ?? "No agent run yet")
            .font(NoemaFont.compactEmphasized)
          Text(activity?.taskDockText ?? run?.activity.taskDockText ?? "Task context")
            .font(NoemaFont.monoCompact)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(1)
        }
        Spacer(minLength: NoemaSpacing.sm)
        if canCancel {
          Button(action: cancel) {
            Image(systemName: "nosign")
              .foregroundStyle(NoemaColor.danger)
              .frame(width: 32, height: 32)
          }
          .buttonStyle(.plain)
          .accessibilityLabel("Cancel task")
        }
        Button(action: showInfo) {
          Image(systemName: "info.circle")
            .foregroundStyle(NoemaColor.contentSecondary)
            .frame(width: 28, height: 28)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Show task information")
      }
      .padding(.horizontal, NoemaSpacing.md)
      .frame(height: 45)
    }
    .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaSpacing.xxl, treatment: .container))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaSpacing.xxl, treatment: .container)
        .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
    .shadow(color: NoemaColor.ink900.opacity(0.13), radius: 14, y: 5)
  }
}

struct TasksTaskInfoSheet: View {
  @Environment(\.dismiss) private var dismiss
  let detail: TasksDetailSnapshot

  var body: some View {
    NoemaNativeSheet(title: "Task information", onDismiss: { dismiss() }) {
      VStack(spacing: NoemaSpacing.compact) {
        metadataRow("Stage", detail.stage.name)
        metadataRow("Revision", String(detail.revision))
        if !detail.createdAt.isEmpty { metadataRow("Created", formattedDate(detail.createdAt)) }
        if let sourceLabel = detail.sourceLabel { metadataRow("From", sourceLabel) }
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.bottom, NoemaSpacing.lg)
    }
    .noemaTaskSheetPresentation([.height(270)], regularHeight: 380, compactDragIndicator: .visible)
  }

  private func metadataRow(_ label: String, _ value: String) -> some View {
    HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.md) {
      Text(label)
        .font(NoemaFont.taskPreview)
        .foregroundStyle(NoemaColor.contentTertiary)
        .frame(width: 88, alignment: .leading)
      Text(value)
        .font(NoemaFont.taskPreview)
        .foregroundStyle(NoemaColor.contentSecondary)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  private func formattedDate(_ value: String) -> String {
    let fractional = ISO8601DateFormatter()
    fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let plain = ISO8601DateFormatter()
    plain.formatOptions = [.withInternetDateTime]
    guard let date = fractional.date(from: value) ?? plain.date(from: value) else { return value }
    return date.formatted(date: .abbreviated, time: .shortened)
  }
}


extension String {
  var taskDockText: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }

  var taskCriterionIcon: String {
    switch uppercased() {
    case "PASS": "checkmark"
    case "FAIL": "xmark"
    case "UNCERTAIN": "exclamationmark.circle"
    default: "clock"
    }
  }

  var taskCriterionColor: Color {
    switch uppercased() {
    case "PASS": NoemaColor.success
    case "FAIL": NoemaColor.danger
    case "UNCERTAIN": NoemaColor.warning
    default: NoemaColor.contentTertiary
    }
  }

  var taskCriterionLabel: String {
    switch uppercased() {
    case "PASS": "Passed"
    case "FAIL": "Failed"
    case "UNCERTAIN": "Uncertain"
    default: "Pending"
    }
  }
}
