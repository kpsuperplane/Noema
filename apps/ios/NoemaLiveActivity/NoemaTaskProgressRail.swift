import SwiftUI

struct TaskProgressRail: View {
  @Environment(\.colorScheme) private var colorScheme

  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  private var step: Int {
    switch state.phase {
    case .inProgress, .planning, .working: 1
    case .reviewing: 2
    case .completed, .cancelled: 3
    }
  }

  var body: some View {
    railContent
      .background(alignment: .top) {
        HStack(spacing: 0) {
          ForEach(1...3, id: \.self) { segment in
            Rectangle()
              .fill(
                segment <= step
                  ? PhaseStyle(state: state).color(for: appearance, colorScheme: colorScheme)
                  : NoemaActivityPalette.track(for: appearance, colorScheme: colorScheme)
              )
          }
        }
          .frame(height: 4)
          .clipShape(NoemaSuperellipse.full)
          .padding(.horizontal, 20)
          .offset(y: 18)
      }
    .frame(height: 52)
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(accessibilityLabel)
  }

  @ViewBuilder
  private var railContent: some View {
    switch step {
    case 1:
      HStack(alignment: .top, spacing: 7) {
        RailStop(label: "Started", isPast: true, state: state, appearance: appearance)
          .frame(width: 43)
        CurrentRailStation(state: state, appearance: appearance)
          .frame(maxWidth: .infinity)
        RailStop(label: "Review", isPast: false, state: state, appearance: appearance)
          .frame(width: 49)
        RailStop(label: "Done", isPast: false, state: state, appearance: appearance)
          .frame(width: 42)
      }
    case 2:
      HStack(alignment: .top, spacing: 7) {
        RailStop(label: "Started", isPast: true, state: state, appearance: appearance)
          .frame(width: 43)
        RailStop(label: "Working", isPast: true, state: state, appearance: appearance)
          .frame(width: 49)
        CurrentRailStation(state: state, appearance: appearance)
          .frame(maxWidth: .infinity)
        RailStop(label: "Done", isPast: false, state: state, appearance: appearance)
          .frame(width: 42)
      }
    default:
      HStack(alignment: .top, spacing: 7) {
        RailStop(label: "Started", isPast: true, state: state, appearance: appearance)
          .frame(width: 43)
        RailStop(label: "Working", isPast: true, state: state, appearance: appearance)
          .frame(width: 49)
        RailStop(label: "Review", isPast: true, state: state, appearance: appearance)
          .frame(width: 49)
        CurrentRailStation(state: state, appearance: appearance)
          .frame(maxWidth: .infinity)
      }
    }
  }

  private var accessibilityLabel: String {
    if state.phase == .completed {
      return "Task complete. \(completedDetail)."
    }
    let nextSteps = step == 1 ? "Review and Done" : "Done"
    let progress = "Task progress: Started, \(stationTitle) now, then \(nextSteps)."
    guard let activeDetail else { return progress }
    return "\(progress) \(activeDetail)."
  }

  private var stationTitle: String {
    switch state.phase {
    case .planning: "Planning"
    case .reviewing: "Review"
    case .completed: "Done"
    case .cancelled: "Ended"
    case .inProgress, .working: "Working"
    }
  }

  private var activeDetail: String? {
    state.updateLabel
  }

  private var completedDetail: String {
    guard let count = state.completedOutputCount, count > 0 else { return "Task complete" }
    return count == 1 ? "1 output" : "\(count) outputs"
  }
}

private struct RailStop: View {
  @Environment(\.colorScheme) private var colorScheme

  let label: String
  let isPast: Bool
  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    VStack(spacing: 4) {
      Circle()
        .fill(
          isPast
            ? PhaseStyle(state: state).color(for: appearance, colorScheme: colorScheme)
            : appearance.surface(for: colorScheme)
        )
        .overlay {
          Circle()
            .stroke(
              isPast
                ? PhaseStyle(state: state).color(for: appearance, colorScheme: colorScheme)
                : NoemaActivityPalette.marker(for: appearance, colorScheme: colorScheme),
              lineWidth: 2
            )
        }
        .background(appearance.surface(for: colorScheme), in: Circle())
        .padding(3)
        .background(appearance.surface(for: colorScheme), in: Circle())
        .frame(width: 23, height: 23)
      Text(label)
        .font(.system(size: 9.5, weight: .medium))
        .foregroundStyle(appearance.tertiary)
        .lineLimit(1)
    }
    .frame(maxWidth: .infinity)
    .padding(.top, 8)
  }
}

private struct CurrentRailStation: View {
  @Environment(\.colorScheme) private var colorScheme

  let state: NoemaTasksActivityAttributes.ContentState
  let appearance: ActivityAppearance

  var body: some View {
    HStack(spacing: 5) {
      VStack(alignment: .leading, spacing: 2) {
        Text(title)
          .font(.system(size: 10.5, weight: .bold))
          .foregroundStyle(appearance.primary)
          .lineLimit(1)
        if let detail {
          HStack(spacing: 3) {
            Text(detail)
              .lineLimit(1)
            if showsUpdateAge, let updateAtEpoch = state.updateAtEpoch {
              Text("·")
              UpdateAge(epoch: updateAtEpoch)
            }
          }
          .font(.system(size: 9.25, weight: .medium))
          .foregroundStyle(appearance.secondary)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(.leading, 19)
    .padding(.trailing, 8)
    .frame(height: 40)
    .background(stationSurface, in: NoemaSuperellipse(cornerRadius: 12, treatment: .container))
    .overlay {
      NoemaSuperellipse(cornerRadius: 12, treatment: .container)
        .stroke(appearance.stationBorder(for: colorScheme), lineWidth: 1)
    }
    .overlay(alignment: .leading) {
      stationMarker
        .offset(x: -11)
    }
    .padding(.leading, 11)
    .shadow(color: appearance.stationShadow(for: colorScheme), radius: 8, y: 3)
  }

  private var stationSurface: Color {
    if state.phase == .completed {
      return NoemaActivityPalette.doneStation(for: appearance, colorScheme: colorScheme)
    }
    return NoemaActivityPalette.station(for: appearance, colorScheme: colorScheme)
  }

  @ViewBuilder
  private var stationMarker: some View {
    if state.phase == .completed {
      Image(systemName: "checkmark")
        .font(.caption.weight(.bold))
        .foregroundStyle(NoemaActivityPalette.doneInk)
        .frame(width: 23, height: 23)
        .background(NoemaActivityPalette.done, in: Circle())
        .padding(2)
        .background(appearance.surface(for: colorScheme), in: Circle())
        .overlay {
          Circle()
            .stroke(NoemaActivityPalette.doneRing, lineWidth: 2)
        }
    } else {
      NoemaAgentMark(size: 23)
        .padding(2)
        .background(appearance.surface(for: colorScheme), in: Circle())
        .overlay {
          Circle()
            .stroke(
              PhaseStyle(state: state).color(for: appearance, colorScheme: colorScheme),
              lineWidth: 2
            )
        }
    }
  }

  private var title: String {
    switch state.phase {
    case .planning: "Planning"
    case .reviewing: "Review"
    case .completed: "Done"
    case .cancelled: "Ended"
    case .inProgress, .working: "Working"
    }
  }

  private var detail: String? {
    if state.phase == .completed, let count = state.completedOutputCount, count > 0 {
      return count == 1 ? "1 output" : "\(count) outputs"
    }
    if state.phase == .completed { return "Task complete" }
    if state.phase == .cancelled { return "Task ended" }
    return state.updateLabel
  }

  private var showsUpdateAge: Bool {
    state.phase != .completed && state.phase != .cancelled && state.updateLabel != nil
  }
}

private struct UpdateAge: View {
  let epoch: Double

  var body: some View {
    TimelineView(.periodic(from: .now, by: 60)) { context in
      Text(age(at: context.date))
        .monospacedDigit()
    }
  }

  private func age(at date: Date) -> String {
    let seconds = max(0, date.timeIntervalSince1970 - epoch)
    if seconds < 60 { return "now" }
    if seconds < 3_600 { return "\(Int(seconds / 60))m" }
    return "\(Int(seconds / 3_600))h"
  }
}
