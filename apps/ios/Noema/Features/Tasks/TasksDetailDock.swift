import SwiftUI

struct TasksTaskContextDock: View {
  let run: TasksRunSnapshot?
  let activity: String?
  let criteria: [TasksCriterionSnapshot]
  let canCancel: Bool
  let cancel: () -> Void
  let showInfo: () -> Void
  let showValidation: () -> Void

  var body: some View {
    VStack(spacing: 0) {
      HStack(spacing: NoemaSpacing.sm) {
        Text(String((run?.instanceName ?? "Task").prefix(1)))
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.pine700)
          .frame(width: 30, height: 30)
          .background(NoemaColor.clay50, in: Circle())
        VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
          Text(run.map { "\($0.instanceName) · \($0.kind.capitalized)" } ?? "No agent run yet")
            .font(NoemaFont.captionEmphasized)
          Text(activity?.taskDockText ?? run?.activity.taskDockText ?? "Task context")
            .font(NoemaFont.monoTiny)
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
      if !criteria.isEmpty {
        Rectangle().fill(NoemaColor.separatorSubtle).frame(height: 1)
        Button(action: showValidation) {
          HStack(spacing: NoemaSpacing.xs) {
            Text("Validation").font(NoemaFont.taskPreview.weight(.semibold))
            ForEach(criteria.prefix(4)) { criterion in
              Image(systemName: criterion.verdict.taskCriterionIcon)
                .font(NoemaFont.metadata)
                .foregroundStyle(criterion.verdict.taskCriterionColor)
            }
            Spacer(minLength: 0)
          }
          .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Show validation evidence")
        .foregroundStyle(NoemaColor.contentSecondary)
        .padding(.horizontal, NoemaSpacing.md)
        .frame(height: 34)
      }
    }
    .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaSpacing.xxl))
    .overlay { RoundedRectangle(cornerRadius: NoemaSpacing.xxl).stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    .shadow(color: NoemaColor.ink900.opacity(0.13), radius: 14, y: 5)
  }
}

struct TasksTaskInfoSheet: View {
  @Environment(\.dismiss) private var dismiss
  let detail: TasksDetailSnapshot

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(title: "Task information", subtitle: nil, onClose: { dismiss() })
      VStack(spacing: NoemaSpacing.compact) {
        if let complexity = detail.complexity { metadataRow("Complexity", complexity.lowercased().capitalized) }
        metadataRow("Stage", detail.stage.name)
        metadataRow("Revision", String(detail.revision))
        if let maxReviewRounds = detail.maxReviewRounds { metadataRow("Review limit", "\(maxReviewRounds) rounds") }
        if !detail.createdAt.isEmpty { metadataRow("Created", formattedDate(detail.createdAt)) }
        if let sourceLabel = detail.sourceLabel { metadataRow("From", sourceLabel) }
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.bottom, NoemaSpacing.lg)
    }
    .background(NoemaColor.surface)
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

struct TasksValidationSheet: View {
  @Environment(\.dismiss) private var dismiss
  let criteria: [TasksCriterionSnapshot]

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      TasksSheetHeader(title: "Validation", subtitle: "Evidence for the current task result.", onClose: { dismiss() })
      ScrollView {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.md) {
          if criteria.isEmpty {
            Text("No validation criteria are available yet.")
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
          } else {
            ForEach(criteria) { criterion in
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
                  Image(systemName: criterion.verdict.taskCriterionIcon)
                    .font(NoemaFont.caption)
                    .foregroundStyle(criterion.verdict.taskCriterionColor)
                  Text(criterion.description)
                    .font(NoemaFont.captionEmphasized)
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("\(criterion.verdict.taskCriterionLabel) validation: \(criterion.description)")
                if let expected = criterion.expectedEvidence?.taskDockText {
                  Text("Expected")
                    .font(NoemaFont.metadata.weight(.semibold))
                    .foregroundStyle(NoemaColor.contentTertiary)
                    .padding(.leading, NoemaSpacing.xl)
                  Text(expected)
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                    .padding(.leading, NoemaSpacing.xl)
                }
                if let evidence = criterion.evidence?.taskDockText {
                  Text(evidence)
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                    .padding(.leading, NoemaSpacing.xl)
                }
              }
              .frame(maxWidth: .infinity, alignment: .leading)
            }
          }
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.bottom, NoemaSpacing.lg)
      }
    }
    .background(NoemaColor.surface)
    .noemaTaskSheetPresentation([.medium, .large], regularHeight: 620, compactDragIndicator: .visible)
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
