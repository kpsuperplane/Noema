import ActivityKit
import SwiftUI
import WidgetKit

@main
struct NoemaLiveActivityBundle: WidgetBundle {
  var body: some Widget {
    NoemaTasksLiveActivityWidget()
  }
}

struct NoemaTasksLiveActivityWidget: Widget {
  var body: some WidgetConfiguration {
    ActivityConfiguration(for: NoemaTasksActivityAttributes.self) { context in
      NoemaTasksLockScreenView(state: context.state)
        .widgetURL(taskURL(for: context.state))
        .activityBackgroundTint(nil)
        .activitySystemActionForegroundColor(nil)
    } dynamicIsland: { context in
      DynamicIsland {
        DynamicIslandExpandedRegion(.leading) {
          ActivityIdentity(state: context.state, appearance: .island)
            .frame(width: 106, height: 24, alignment: .leading)
        }
        .contentMargins(.leading, 16)

        DynamicIslandExpandedRegion(.trailing) {
          ActivityHeaderStatus(state: context.state, appearance: .island)
            .frame(width: 106, height: 24, alignment: .trailing)
        }
        .contentMargins(.trailing, 16)

        DynamicIslandExpandedRegion(.bottom) {
          ActivityBody(state: context.state, appearance: .island)
            .padding(.top, 8)
        }
        .contentMargins([.leading, .trailing, .bottom], 16)
      } compactLeading: {
        CompactAgentMark(state: context.state)
          .frame(height: 20)
      } compactTrailing: {
        CompactTrailingStatus(state: context.state)
          .frame(height: 20)
      } minimal: {
        CompactAgentMark(state: context.state, size: 18)
      }
      .keylineTint(PhaseStyle(state: context.state).color(for: .island))
      .contentMargins(.leading, 4, for: .compactLeading)
      .contentMargins(.trailing, 4, for: .compactTrailing)
      .widgetURL(taskURL(for: context.state))
    }
  }

  private func taskURL(for state: NoemaTasksActivityAttributes.ContentState) -> URL? {
    var components = URLComponents()
    components.scheme = "noema"
    components.host = "task"
    components.queryItems = [URLQueryItem(name: "id", value: state.focusTaskId)]
    return components.url
  }
}

private struct NoemaTasksLockScreenView: View {
  let state: NoemaTasksActivityAttributes.ContentState

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack(alignment: .center, spacing: 12) {
        ActivityIdentity(state: state, appearance: .lockScreen)
          .frame(maxWidth: .infinity, alignment: .leading)
        ActivityHeaderStatus(state: state, appearance: .lockScreen)
          .fixedSize()
      }
      ActivityBody(state: state, appearance: .lockScreen)
    }
    .padding(.horizontal, 16)
    .padding(.vertical, 14)
  }
}

private struct ActivityIdentity: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  private var agentName: String {
    state.agentName?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty ?? "Agent"
  }

  private var label: String {
    if appearance == .lockScreen, state.requiresAttention != true, state.activeTaskCount <= 1 {
      return state.focusTitle
    }
    return agentName
  }

  var body: some View {
    HStack(spacing: 7) {
      NoemaAgentMark(size: appearance == .island ? 20 : 22)
        .accessibilityHidden(true)
      Text(label)
        .font(.caption.weight(.semibold))
        .foregroundStyle(appearance.primary)
        .lineLimit(1)
    }
    .accessibilityElement(children: .combine)
    .accessibilityLabel(label)
  }
}

private struct ActivityHeaderStatus: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    Group {
      if state.requiresAttention == true {
        Text("Needs You")
          .foregroundStyle(PhaseStyle(state: state).color(for: appearance))
      } else if state.activeTaskCount <= 1 {
        if state.phase == .completed {
          HStack(spacing: 5) {
            Image(systemName: "checkmark")
              .font(.caption2.weight(.bold))
              .frame(width: 17, height: 17)
              .foregroundStyle(NoemaActivityPalette.doneInk)
              .background(NoemaActivityPalette.done, in: Circle())
            Text("Done")
            Text(Date(timeIntervalSince1970: state.updatedAtEpoch), style: .time)
          }
          .foregroundStyle(NoemaActivityPalette.doneText(for: appearance))
        } else if state.phase == .cancelled {
          Text("Ended")
            .foregroundStyle(appearance.secondary)
        } else if let startedAtEpoch = state.startedAtEpoch {
          Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer)
            .foregroundStyle(appearance.secondary)
        } else {
          Text("Starting")
            .foregroundStyle(appearance.secondary)
        }
      }
    }
    .font(.caption.monospacedDigit().weight(.semibold))
    .lineLimit(1)
  }
}

private struct ActivityBody: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    Group {
      if state.requiresAttention == true {
        AttentionActivityBody(state: state, appearance: appearance)
      } else if state.activeTaskCount > 1 {
        MultipleTasksActivityBody(state: state, appearance: appearance)
      } else {
        SingleTaskActivityBody(state: state, appearance: appearance)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
  }
}

private struct SingleTaskActivityBody: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    VStack(alignment: .leading, spacing: 11) {
      if appearance == .island {
        Text(state.focusTitle)
          .font(.subheadline.weight(.semibold))
          .foregroundStyle(appearance.primary)
          .lineLimit(2)
          .multilineTextAlignment(.leading)
          .accessibilityLabel("Task: \(state.focusTitle)")
      }

      TaskProgressRail(state: state, appearance: appearance)
    }
  }
}

private struct MultipleTasksActivityBody: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  private var summaries: [NoemaTasksActivityAttributes.ContentState.TaskSummary] {
    if let summaries = state.taskSummaries, !summaries.isEmpty {
      return Array(summaries.prefix(2))
    }
    return [
      .init(
        taskId: state.focusTaskId,
        title: state.focusTitle,
        phase: state.phase,
        statusLabel: state.statusLabel,
        startedAtEpoch: state.startedAtEpoch
      ),
    ]
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      ForEach(summaries) { summary in
        Divider()
          .overlay(appearance.divider)
        MultipleTaskRow(summary: summary, appearance: appearance)
      }

      let hiddenCount = max(0, state.activeTaskCount - summaries.count)
      if hiddenCount > 0 {
        Text(hiddenCount == 1 ? "+ 1 task running in the background" : "+ \(hiddenCount) tasks running in the background")
          .font(.system(size: 10, weight: .semibold))
          .foregroundStyle(appearance.tertiary)
          .padding(.top, 8)
      }
    }
  }
}

private struct MultipleTaskRow: View {
  @Environment(\.colorScheme) private var colorScheme

  let summary: NoemaTasksActivityAttributes.ContentState.TaskSummary
  let appearance: ActivityAppearance

  var body: some View {
    HStack(spacing: 12) {
      Text(summary.title)
        .font(.system(size: 13, weight: .medium))
        .foregroundStyle(appearance.primary)
        .lineLimit(1)
        .frame(maxWidth: .infinity, alignment: .leading)

      HStack(spacing: 6) {
        Circle()
          .fill(
            TaskSummaryStyle(phase: summary.phase, requiresAttention: summary.requiresAttention == true)
              .color(for: appearance, colorScheme: colorScheme)
          )
          .frame(width: 5, height: 5)
        Text(summary.statusLabel)
          .lineLimit(1)
        if let startedAtEpoch = summary.startedAtEpoch {
          Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer)
            .monospacedDigit()
            .lineLimit(1)
        }
      }
      .font(.system(size: 10, weight: .medium))
      .foregroundStyle(appearance.tertiary)
    }
    .frame(minHeight: 31)
    .accessibilityElement(children: .combine)
  }
}

private struct AttentionActivityBody: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  private var agentName: String {
    state.agentName?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty ?? "The agent"
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text(state.focusTitle)
        .font(appearance == .island ? .headline.weight(.semibold) : .title3.weight(.semibold))
        .foregroundStyle(appearance.primary)
        .lineLimit(2)

      Text("\(agentName) is waiting for your decision on this task.")
        .font(.caption)
        .foregroundStyle(appearance.secondary)
        .lineLimit(2)
        .padding(.top, 6)

      Divider()
        .overlay(appearance.divider)
        .padding(.top, 12)

      HStack(spacing: 12) {
        Text(otherTasksLabel)
          .font(.caption2.weight(.semibold))
          .foregroundStyle(appearance.secondary)
          .lineLimit(1)
        Spacer(minLength: 8)
        Text("Open task ›")
          .font(.caption2.weight(.bold))
          .foregroundStyle(NoemaActivityPalette.attention(for: appearance))
          .lineLimit(1)
      }
      .padding(.top, 12)
    }
  }

  private var otherTasksLabel: String {
    let count = max(0, state.activeTaskCount - 1)
    if count == 0 { return "This task is waiting" }
    if count == 1 { return "1 other task is still running" }
    return "\(count) other tasks are still running"
  }
}

private struct CompactAgentMark: View {
  let state: NoemaTasksActivityAttributes.ContentState
  var size: CGFloat = 19

  var body: some View {
    NoemaAgentMark(size: size)
      .overlay {
        if state.requiresAttention == true {
          Circle()
            .stroke(NoemaActivityPalette.islandClay, lineWidth: 2)
        }
      }
      .accessibilityLabel(state.agentName ?? "Agent")
  }
}

private struct CompactTrailingStatus: View {
  let state: NoemaTasksActivityAttributes.ContentState

  var body: some View {
    Group {
      if state.requiresAttention == true {
        Text("!")
          .foregroundStyle(NoemaActivityPalette.islandClay)
          .accessibilityLabel("Needs you")
      } else if state.activeTaskCount > 1 {
        Text("\(state.activeTaskCount)")
          .foregroundStyle(NoemaActivityPalette.islandPrimary)
          .accessibilityLabel("\(state.activeTaskCount) active tasks")
      } else if state.phase == .completed {
        Image(systemName: "checkmark")
          .foregroundStyle(NoemaActivityPalette.done)
          .accessibilityLabel("Task complete")
      } else if state.phase == .cancelled {
        Image(systemName: "xmark")
          .foregroundStyle(NoemaActivityPalette.islandClay)
          .accessibilityLabel("Task ended")
      } else if let startedAtEpoch = state.startedAtEpoch {
        Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer)
          .foregroundStyle(NoemaActivityPalette.islandPrimary)
          .accessibilityLabel("Elapsed time")
          .accessibilityValue(Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer))
      } else {
        Image(systemName: "ellipsis")
          .foregroundStyle(NoemaActivityPalette.islandSecondary)
          .accessibilityLabel("Starting")
      }
    }
    .font(.caption2.monospacedDigit().weight(.bold))
    .lineLimit(1)
    .minimumScaleFactor(0.75)
  }
}

private struct TaskSummaryStyle {
  let phase: NoemaTasksActivityAttributes.ContentState.Phase
  let requiresAttention: Bool

  func color(for appearance: ActivityAppearance, colorScheme: ColorScheme) -> Color {
    if requiresAttention || phase == .cancelled {
      return appearance == .island
        ? NoemaActivityPalette.islandClay
        : NoemaActivityPalette.lockClay(for: colorScheme)
    }
    if phase == .reviewing {
      return appearance == .island
        ? NoemaActivityPalette.islandBlue
        : NoemaActivityPalette.lockBlue(for: colorScheme)
    }
    return appearance == .island
      ? NoemaActivityPalette.islandPine
      : NoemaActivityPalette.lockPine(for: colorScheme)
  }
}

private extension String {
  var nilIfEmpty: String? {
    isEmpty ? nil : self
  }
}

#if DEBUG
private let previewAttributes = NoemaTasksActivityAttributes(
  activityId: "live_activity:preview",
  clientId: "client:preview",
  serverOrigin: "https://noema.example"
)

private let workingPreviewState = NoemaTasksActivityAttributes.ContentState(
  focusTaskId: "task:preview",
  focusTitle: "Refine the iOS Live Activity",
  projectName: "Personal",
  agentName: "Atlas",
  phase: .working,
  statusLabel: "Running",
  activeTaskCount: 1,
  startedAtEpoch: Date.now.addingTimeInterval(-754).timeIntervalSince1970,
  updatedAtEpoch: Date.now.timeIntervalSince1970,
  updateLabel: "Testing UI",
  updateAtEpoch: Date.now.addingTimeInterval(-120).timeIntervalSince1970
)

private let multiplePreviewState = NoemaTasksActivityAttributes.ContentState(
  focusTaskId: "task:preview",
  focusTitle: "Refine the iOS Live Activity",
  projectName: "Personal",
  agentName: "Atlas",
  phase: .working,
  statusLabel: "Running",
  activeTaskCount: 3,
  startedAtEpoch: Date.now.addingTimeInterval(-754).timeIntervalSince1970,
  updatedAtEpoch: Date.now.timeIntervalSince1970,
  taskSummaries: [
    .init(
      taskId: "task:preview",
      title: "Refine the iOS Live Activity",
      phase: .working,
      statusLabel: "Working",
      startedAtEpoch: Date.now.addingTimeInterval(-754).timeIntervalSince1970
    ),
    .init(
      taskId: "task:review",
      title: "Review the memory architecture",
      phase: .reviewing,
      statusLabel: "Reviewing",
      startedAtEpoch: Date.now.addingTimeInterval(-258).timeIntervalSince1970
    ),
  ]
)

private let attentionPreviewState = NoemaTasksActivityAttributes.ContentState(
  focusTaskId: "task:preview",
  focusTitle: "Choose the Live Activity direction",
  projectName: "Personal",
  agentName: "Atlas",
  phase: .reviewing,
  statusLabel: "Needs You",
  activeTaskCount: 3,
  startedAtEpoch: nil,
  updatedAtEpoch: Date.now.timeIntervalSince1970,
  requiresAttention: true
)

private let completedPreviewState = NoemaTasksActivityAttributes.ContentState(
  focusTaskId: "task:preview",
  focusTitle: "Refine the iOS Live Activity",
  projectName: "Personal",
  agentName: "Atlas",
  phase: .completed,
  statusLabel: "Completed",
  activeTaskCount: 0,
  startedAtEpoch: nil,
  updatedAtEpoch: Date.now.addingTimeInterval(-180).timeIntervalSince1970,
  completedOutputCount: 6
)

#Preview("One Task · Lock Screen", as: .content, using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  workingPreviewState
}

#Preview("One Task · Expanded", as: .dynamicIsland(.expanded), using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  workingPreviewState
}

#Preview("Multiple Tasks", as: .content, using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  multiplePreviewState
}

#Preview("Needs You", as: .content, using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  attentionPreviewState
}

#Preview("Completed", as: .content, using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  completedPreviewState
}
#endif
