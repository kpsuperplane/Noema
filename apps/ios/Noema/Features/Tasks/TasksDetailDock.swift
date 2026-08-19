import SwiftUI

struct TasksTaskContextDock: View {
  let run: TasksRunSnapshot?
  let embeddedInSystemAccessory: Bool
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
          Text(run?.instanceName ?? "No agent run yet")
            .font(NoemaFont.compactEmphasized)
          if let run {
            Text(run.kind.capitalized)
              .font(NoemaFont.monoCompact)
              .foregroundStyle(NoemaColor.contentSecondary)
              .lineLimit(1)
          }
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
    .background {
      if !embeddedInSystemAccessory {
        NoemaSuperellipse(cornerRadius: NoemaSpacing.xxl, treatment: .container)
          .fill(NoemaColor.surface)
      }
    }
    .overlay {
      if !embeddedInSystemAccessory {
        NoemaSuperellipse(cornerRadius: NoemaSpacing.xxl, treatment: .container)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
    .shadow(
      color: embeddedInSystemAccessory ? .clear : NoemaColor.ink900.opacity(0.13),
      radius: 14,
      y: 5
    )
  }
}

struct TasksTaskStatusSurface: View {
  let run: TasksRunSnapshot?
  let activity: String?
  let embeddedInGlassAccessory: Bool
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  private var statusText: String {
    activity?.taskDockText ?? run?.activity.taskDockText ?? fallbackStatus
  }

  var body: some View {
    HStack(spacing: NoemaSpacing.compact) {
      statusIcon
        .font(NoemaFont.monoCompact)
        .frame(width: NoemaSpacing.md, height: NoemaSpacing.md)
      ZStack(alignment: .leading) {
        Text(statusText)
          .id(statusText)
          .transition(statusTransition)
      }
      .font(NoemaFont.monoCompact)
      .foregroundStyle(NoemaColor.contentSecondary)
      .lineLimit(1)
      .clipped()
    }
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.top, embeddedInGlassAccessory ? NoemaSpacing.sm : NoemaSpacing.xs)
    .padding(.bottom, embeddedInGlassAccessory ? NoemaSpacing.sm : NoemaSpacing.xxl + NoemaSpacing.xxs)
    .frame(maxWidth: embeddedInGlassAccessory ? nil : .infinity, alignment: .leading)
    .background {
      if !embeddedInGlassAccessory {
        attachedShape.fill(NoemaColor.surface)
      }
    }
    .overlay {
      if !embeddedInGlassAccessory {
        attachedShape.stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
    .shadow(
      color: embeddedInGlassAccessory ? .clear : NoemaColor.ink900.opacity(0.07),
      radius: NoemaSpacing.sm,
      y: NoemaSpacing.xs
    )
    .animation(NoemaMotion.animation(NoemaSpring.micro, reduceMotion: reduceMotion), value: statusText)
    .accessibilityElement(children: .combine)
    .accessibilityLabel("Task status: \(statusText)")
  }

  @ViewBuilder
  private var statusIcon: some View {
    switch (run?.status ?? "").uppercased() {
    case "QUEUED", "LEASED", "STARTED", "RUNNING":
      ProgressView()
        .controlSize(.mini)
        .tint(NoemaColor.success)
    case "FAILED", "INTERRUPTED":
      Image(systemName: "exclamationmark")
        .foregroundStyle(NoemaColor.warning)
    case "CANCELLED":
      Image(systemName: "xmark")
        .foregroundStyle(NoemaColor.contentTertiary)
    case "COMPLETED":
      Image(systemName: "checkmark")
        .foregroundStyle(NoemaColor.success)
    default:
      Image(systemName: "ellipsis")
        .foregroundStyle(NoemaColor.contentTertiary)
    }
  }

  private var fallbackStatus: String {
    switch (run?.status ?? "").uppercased() {
    case "QUEUED": "Queued"
    case "LEASED", "STARTED": "Starting"
    case "RUNNING": "Running"
    case "COMPLETED": "Completed"
    case "FAILED": "Failed"
    case "CANCELLED": "Cancelled"
    case "INTERRUPTED": "Interrupted"
    case "WAITING_FOR_APPROVAL": "Waiting for approval"
    default: "No output yet"
    }
  }

  private var statusTransition: AnyTransition {
    guard !reduceMotion else { return .identity }
    return .asymmetric(
      insertion: .move(edge: .bottom).combined(with: .opacity),
      removal: .move(edge: .top).combined(with: .opacity)
    )
  }

  private var attachedShape: NoemaSuperellipse {
    NoemaSuperellipse(
      topLeftRadius: NoemaSpacing.xxl,
      topRightRadius: NoemaSpacing.xxl,
      bottomRightRadius: 0,
      bottomLeftRadius: 0,
      treatment: .page
    )
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
