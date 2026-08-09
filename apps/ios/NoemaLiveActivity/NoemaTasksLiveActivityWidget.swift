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
          ActivityIdentity(appearance: .island)
        }
        DynamicIslandExpandedRegion(.trailing) {
          ElapsedTime(state: context.state, appearance: .island)
        }
        DynamicIslandExpandedRegion(.center) {
          ActivityTitle(state: context.state, appearance: .island)
        }
        DynamicIslandExpandedRegion(.bottom) {
          ActivityFooter(state: context.state, appearance: .island)
        }
      } compactLeading: {
        NoemaAgentMark(size: 20)
          .accessibilityLabel("Noema")
      } compactTrailing: {
        CompactTrailingStatus(state: context.state)
      } minimal: {
        PhaseGlyph(state: context.state, appearance: .island)
          .accessibilityLabel(context.state.statusLabel)
      }
      .keylineTint(PhaseStyle(state: context.state).color(for: .island))
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
      HStack(alignment: .center, spacing: 8) {
        ActivityIdentity(appearance: .lockScreen)
        Spacer(minLength: 12)
        ElapsedTime(state: state, appearance: .lockScreen)
      }
      ActivityTitle(state: state, appearance: .lockScreen)
      ActivityFooter(state: state, appearance: .lockScreen)
    }
    .padding(.horizontal, 14)
    .padding(.vertical, 12)
  }
}

private struct ActivityIdentity: View {
  let appearance: ActivityAppearance

  var body: some View {
    HStack(spacing: 7) {
      NoemaAgentMark(size: appearance == .island ? 20 : 22)
        .accessibilityHidden(true)
      Text("Noema")
        .font(.caption.weight(.semibold))
        .foregroundStyle(appearance.primary)
    }
    .accessibilityElement(children: .combine)
    .accessibilityLabel("Noema")
  }
}

private struct ActivityTitle: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    Text(state.focusTitle)
      .font(appearance == .island ? .subheadline.weight(.semibold) : .headline.weight(.semibold))
      .foregroundStyle(appearance.primary)
      .lineLimit(2)
      .multilineTextAlignment(.leading)
      .frame(maxWidth: .infinity, alignment: .leading)
      .accessibilityLabel("Task: \(state.focusTitle)")
  }
}

private struct ActivityFooter: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    HStack(spacing: 7) {
      PhaseDot(state: state, appearance: appearance)
      Text(state.statusLabel)
        .font(.caption.weight(.medium))
        .foregroundStyle(appearance.secondary)
        .lineLimit(1)
      Spacer(minLength: 12)
      Text("\(state.activeTaskCount) active")
        .font(.caption)
        .monospacedDigit()
        .foregroundStyle(appearance.secondary)
        .lineLimit(1)
    }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("\(state.statusLabel), \(state.activeTaskCount) active tasks")
  }
}

private struct ElapsedTime: View {
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    Group {
      if state.phase == .completed {
        Text("Done")
      } else if state.phase == .cancelled {
        Text("Ended")
      } else if let startedAtEpoch = state.startedAtEpoch {
        Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer)
      } else {
        Text("Starting")
      }
    }
    .font(.caption.monospacedDigit().weight(.medium))
    .foregroundStyle(appearance.secondary)
    .lineLimit(1)
  }
}

private struct CompactTrailingStatus: View {
  let state: NoemaTasksActivityAttributes.ContentState

  var body: some View {
    if state.requiresAttention == true {
      Image(systemName: "exclamationmark")
        .font(.caption.weight(.bold))
        .foregroundStyle(NoemaActivityPalette.islandClay)
        .accessibilityLabel("Needs attention")
    } else if state.phase == .completed || state.phase == .cancelled || state.startedAtEpoch == nil {
      PhaseGlyph(state: state, appearance: .island)
        .accessibilityLabel(state.statusLabel)
    } else if let startedAtEpoch = state.startedAtEpoch {
      Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer)
        .font(.caption2.monospacedDigit().weight(.semibold))
        .foregroundStyle(NoemaActivityPalette.islandPrimary)
        .lineLimit(1)
        .accessibilityLabel("Elapsed time")
        .accessibilityValue(Text(Date(timeIntervalSince1970: startedAtEpoch), style: .timer))
    }
  }
}

private struct PhaseDot: View {
  @Environment(\.colorScheme) private var colorScheme

  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    Circle()
      .fill(PhaseStyle(state: state).color(for: appearance, colorScheme: colorScheme))
      .frame(width: 7, height: 7)
      .accessibilityHidden(true)
  }
}

private struct PhaseGlyph: View {
  @Environment(\.colorScheme) private var colorScheme

  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    Image(systemName: PhaseStyle(state: state).symbol)
      .font(.caption2.weight(.bold))
      .foregroundStyle(PhaseStyle(state: state).color(for: appearance, colorScheme: colorScheme))
  }
}

private struct PhaseStyle {
  let state: NoemaTasksActivityAttributes.ContentState

  var symbol: String {
    if state.requiresAttention == true { return "exclamationmark" }
    switch state.phase {
    case .inProgress: return "sparkles"
    case .planning: return "list.bullet.clipboard"
    case .working: return "gearshape.2"
    case .reviewing: return "checkmark.bubble"
    case .completed: return "checkmark"
    case .cancelled: return "xmark"
    }
  }

  func color(for appearance: ActivityAppearance, colorScheme: ColorScheme = .light) -> Color {
    if state.requiresAttention == true {
      return appearance == .island
        ? NoemaActivityPalette.islandClay
        : NoemaActivityPalette.lockClay(for: colorScheme)
    }
    if state.phase == .cancelled {
      return appearance == .island
        ? NoemaActivityPalette.islandClay
        : NoemaActivityPalette.lockClay(for: colorScheme)
    }
    if state.phase == .reviewing {
      return appearance == .island
        ? NoemaActivityPalette.islandBlue
        : NoemaActivityPalette.lockBlue(for: colorScheme)
    }
    return appearance == .island
      ? NoemaActivityPalette.islandPine
      : NoemaActivityPalette.lockPine(for: colorScheme)
  }
}

private enum ActivityAppearance {
  case lockScreen
  case island

  var primary: Color {
    self == .island ? NoemaActivityPalette.islandPrimary : .primary
  }

  var secondary: Color {
    self == .island ? NoemaActivityPalette.islandSecondary : .secondary
  }
}

private enum NoemaActivityPalette {
  static let islandPrimary = Color.white
  static let islandSecondary = Color(red: 0.722, green: 0.710, blue: 0.678)
  static let islandPine = Color(red: 0.388, green: 0.867, blue: 0.667)
  static let islandClay = Color(red: 1.000, green: 0.608, blue: 0.486)
  static let islandBlue = Color(red: 0.471, green: 0.784, blue: 0.933)

  static func lockPine(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandPine : Color(red: 0.090, green: 0.376, blue: 0.275)
  }

  static func lockClay(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandClay : Color(red: 0.604, green: 0.243, blue: 0.133)
  }

  static func lockBlue(for colorScheme: ColorScheme) -> Color {
    colorScheme == .dark ? islandBlue : Color(red: 0.114, green: 0.290, blue: 0.376)
  }
}

#if DEBUG
private let previewAttributes = NoemaTasksActivityAttributes(
  activityId: "live_activity:preview",
  clientId: "client:preview",
  serverOrigin: "https://noema.example"
)

private let previewState = NoemaTasksActivityAttributes.ContentState(
  focusTaskId: "task:preview",
  focusTitle: "Research the history of the Paradise area",
  projectName: "Personal",
  phase: .working,
  statusLabel: "Working",
  activeTaskCount: 2,
  startedAtEpoch: Date.now.addingTimeInterval(-754).timeIntervalSince1970,
  updatedAtEpoch: Date.now.timeIntervalSince1970
)

#Preview("Lock Screen", as: .content, using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  previewState
}

#Preview("Dynamic Island", as: .dynamicIsland(.expanded), using: previewAttributes) {
  NoemaTasksLiveActivityWidget()
} contentStates: {
  previewState
}
#endif
