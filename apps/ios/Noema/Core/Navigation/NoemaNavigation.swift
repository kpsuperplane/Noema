import SwiftUI

enum NoemaBreakpoint {
  case compact
  case regular
  case wide

  static let compactMaximum: CGFloat = 760
  static let regularMinimum: CGFloat = 980

  static func resolve(width: CGFloat) -> Self {
    if width < compactMaximum { return .compact }
    if width < regularMinimum { return .regular }
    return .wide
  }
}

enum NoemaDestination: String, CaseIterable, Identifiable {
  case chat
  case tasks
  case memory
  case settings

  var id: String { rawValue }

  var title: String {
    switch self {
    case .chat: "Chat"
    case .tasks: "Tasks"
    case .memory: "Memory"
    case .settings: "Settings"
    }
  }

  var symbol: String {
    switch self {
    case .chat: "bubble.left.and.bubble.right"
    case .tasks: "checklist"
    case .memory: "books.vertical"
    case .settings: "gearshape"
    }
  }
}

struct NoemaTopRail: View {
  @Binding var selection: NoemaDestination
  let width: CGFloat

  var body: some View {
    let breakpoint = NoemaBreakpoint.resolve(width: width)
    HStack(spacing: NoemaSpacing.xs) {
      Image(systemName: "circle.hexagongrid.fill")
        .font(.system(size: 18, weight: .semibold))
        .foregroundStyle(NoemaColor.accent)
        .accessibilityHidden(true)

      HStack(spacing: NoemaSpacing.xs) {
        ForEach(NoemaDestination.allCases) { destination in
          Button {
            withAnimation(NoemaSpring.micro) {
              selection = destination
            }
          } label: {
            railLabel(for: destination, breakpoint: breakpoint)
          }
          .buttonStyle(.glass)
          .padding(.horizontal, breakpoint == .compact ? NoemaSpacing.compact : NoemaSpacing.sm)
          .padding(.vertical, NoemaSpacing.compact)
          .contentShape(Capsule())
          .foregroundStyle(selection == destination ? NoemaColor.accent : NoemaColor.content)
          .accessibilityLabel(destination.title)
          .accessibilityAddTraits(selection == destination ? .isSelected : [])
        }
      }

      Spacer(minLength: NoemaSpacing.sm)
    }
    .font(NoemaFont.captionEmphasized)
    .foregroundStyle(NoemaColor.content)
    .padding(.horizontal, NoemaSpacing.lg)
    .padding(.vertical, NoemaSpacing.sm)
    .frame(minHeight: 44)
    .background(NoemaColor.surface)
    .overlay(alignment: .bottom) {
      Rectangle()
        .fill(NoemaColor.separator.opacity(0.35))
        .frame(height: 0.5)
    }
  }

  @ViewBuilder
  private func railLabel(for destination: NoemaDestination, breakpoint: NoemaBreakpoint) -> some View {
    if breakpoint != .compact || selection == destination {
      Label(destination.title, systemImage: destination.symbol)
        .labelStyle(.titleAndIcon)
    } else {
      Image(systemName: destination.symbol)
        .frame(width: 24, height: 24)
        .accessibilityHidden(true)
    }
  }
}

struct NoemaShellView: View {
  var model: NoemaAppModel
  @State private var selection: NoemaDestination = .chat
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    GeometryReader { proxy in
      VStack(spacing: 0) {
        NoemaTopRail(selection: $selection, width: proxy.size.width)
        Group {
          switch selection {
          case .chat:
            ChatRootView(model: model)
          case .tasks:
            TasksRootView(model: model)
          case .memory:
            MemoryRootView(model: model)
          case .settings:
            SettingsRootView(model: model)
          }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(NoemaColor.surface)
        .transition(.opacity)
      }
      .animation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion), value: selection)
    }
    .ignoresSafeArea(edges: .bottom)
  }
}
