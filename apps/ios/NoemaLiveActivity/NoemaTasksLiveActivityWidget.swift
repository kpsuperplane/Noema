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
        .activityBackgroundTint(NoemaActivityPalette.paper)
        .activitySystemActionForegroundColor(NoemaActivityPalette.pine)
    } dynamicIsland: { context in
      DynamicIsland {
        DynamicIslandExpandedRegion(.leading) {
          NoemaActivityMark()
        }
        DynamicIslandExpandedRegion(.center) {
          VStack(alignment: .leading, spacing: 2) {
            Text("Noema Tasks")
              .font(.caption.weight(.semibold))
              .foregroundStyle(NoemaActivityPalette.ink)
            Text(context.state.focusTitle)
              .font(.caption2)
              .lineLimit(1)
          }
        }
        DynamicIslandExpandedRegion(.trailing) {
          ActivityCount(state: context.state, expanded: true)
        }
        DynamicIslandExpandedRegion(.bottom) {
          HStack(spacing: 8) {
            PhaseDot(phase: context.state.phase)
            Text(context.state.statusLabel)
              .font(.caption)
              .foregroundStyle(NoemaActivityPalette.inkSecondary)
            Spacer()
            Text(context.state.projectName ?? "Personal")
              .font(.caption2)
              .foregroundStyle(NoemaActivityPalette.inkSecondary)
              .lineLimit(1)
          }
        }
      } compactLeading: {
        NoemaActivityMark()
      } compactTrailing: {
        ActivityCount(state: context.state, expanded: false)
      } minimal: {
        NoemaActivityMark()
      }
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

private struct ActivityCount: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let expanded: Bool

  var body: some View {
    if state.phase == .completed || state.phase == .cancelled {
      Image(systemName: state.phase == .completed ? "checkmark" : "xmark")
        .font(expanded ? .title3.weight(.semibold) : .caption2.weight(.semibold))
        .foregroundStyle(state.phase == .completed ? NoemaActivityPalette.pine : NoemaActivityPalette.clay)
        .accessibilityLabel(state.statusLabel)
    } else {
      Text("\(state.activeTaskCount)")
        .font(expanded ? .title3.weight(.semibold) : .caption2.weight(.semibold))
        .foregroundStyle(NoemaActivityPalette.pine)
        .accessibilityLabel("\(state.activeTaskCount) active tasks")
    }
  }
}

private struct NoemaTasksLockScreenView: View {
  let state: NoemaTasksActivityAttributes.ContentState

  var body: some View {
    HStack(spacing: 12) {
      NoemaActivityMark()
      VStack(alignment: .leading, spacing: 3) {
        HStack(spacing: 6) {
          Text("Noema Tasks")
            .font(.caption.weight(.semibold))
          PhaseDot(phase: state.phase)
          Text(state.statusLabel)
            .font(.caption2)
            .foregroundStyle(NoemaActivityPalette.inkSecondary)
        }
        Text(state.focusTitle)
          .font(.headline.weight(.semibold))
          .foregroundStyle(NoemaActivityPalette.ink)
          .lineLimit(2)
        HStack(spacing: 6) {
          Text(state.projectName ?? "Personal")
          if state.activeTaskCount > 1 {
            Text("·")
            Text("\(state.activeTaskCount) active")
          }
        }
        .font(.caption2)
        .foregroundStyle(NoemaActivityPalette.inkSecondary)
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, 16)
    .padding(.vertical, 12)
  }
}

private struct NoemaActivityMark: View {
  var body: some View {
    NoemaAgentMark(size: 28)
      .accessibilityLabel("Noema Tasks")
  }
}

private struct PhaseDot: View {
  let phase: NoemaTasksActivityAttributes.ContentState.Phase

  var body: some View {
    Circle()
      .fill(color)
      .frame(width: 7, height: 7)
      .accessibilityHidden(true)
  }

  private var color: Color {
    switch phase {
    case .completed: NoemaActivityPalette.pine
    case .cancelled: NoemaActivityPalette.clay
    case .reviewing: NoemaActivityPalette.blue
    case .planning, .working, .inProgress: NoemaActivityPalette.pine.opacity(0.72)
    }
  }
}

private enum NoemaActivityPalette {
  static let paper = Color(red: 0.988, green: 0.980, blue: 0.961)
  static let white = Color.white
  static let ink = Color(red: 0.09, green: 0.086, blue: 0.059)
  static let inkSecondary = Color(red: 0.369, green: 0.353, blue: 0.294)
  static let pine = Color(red: 0.122, green: 0.478, blue: 0.341)
  static let clay = Color(red: 0.737, green: 0.306, blue: 0.169)
  static let blue = Color(red: 0.114, green: 0.290, blue: 0.376)
}
