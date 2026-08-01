import Observation
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
    case .chat: "bubble.left"
    case .tasks: "checklist"
    case .memory: "brain"
    case .settings: "gearshape"
    }
  }
}

struct NoemaSidebarEntry: Identifiable {
  enum Kind { case group, item }

  let id: String
  let kind: Kind
  let label: String
  var symbol: String?
  var depth = 0
  var isSelected = false
  var isPinned = false
  var action: (@MainActor () -> Void)?

  static func group(_ label: String) -> Self {
    Self(id: "group-\(label)", kind: .group, label: label)
  }

  static func item(
    id: String,
    label: String,
    symbol: String,
    depth: Int = 0,
    selected: Bool = false,
    pinned: Bool = false,
    action: @escaping @MainActor () -> Void
  ) -> Self {
    Self(
      id: id,
      kind: .item,
      label: label,
      symbol: symbol,
      depth: depth,
      isSelected: selected,
      isPinned: pinned,
      action: action
    )
  }
}

struct NoemaSecondaryNavigation {
  let title: String
  let symbol: String
  let entries: [NoemaSidebarEntry]
}

@MainActor
@Observable
final class NoemaShellCoordinator {
  var primaryAgentLabel = "Chat"
  var primaryNavigationHidden = false
  var secondary: NoemaSecondaryNavigation?
  var requestedDestination: NoemaDestination?
  var requestedTaskID: String?

  func show(_ navigation: NoemaSecondaryNavigation) {
    secondary = navigation
  }

  func clearSecondary() {
    secondary = nil
  }

  func openTask(_ taskID: String) {
    requestedTaskID = taskID
    requestedDestination = .tasks
  }
}

struct NoemaTopRail: View {
  @Binding var selection: NoemaDestination
  let breakpoint: NoemaBreakpoint
  let agentLabel: String
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    HStack(spacing: NoemaSpacing.xs) {
      Spacer(minLength: 0)
      ForEach(NoemaDestination.allCases) { destination in
        Button {
          withAnimation(NoemaMotion.animation(NoemaSpring.micro, reduceMotion: reduceMotion)) {
            selection = destination
          }
        } label: {
          railLabel(for: destination)
            .frame(minWidth: 28, minHeight: 36)
            .padding(.horizontal, selection == destination ? NoemaSpacing.sm : NoemaSpacing.compact)
            .contentShape(Capsule())
            .background {
              if selection == destination {
                Capsule()
                  .fill(NoemaColor.white)
                  .shadow(color: NoemaColor.pine600.opacity(0.10), radius: 4, y: 3)
              }
            }
            .frame(minWidth: 44, minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(NoemaColor.pine700)
        .accessibilityLabel(displayLabel(for: destination))
        .accessibilityAddTraits(selection == destination ? .isSelected : [])
      }
      Spacer(minLength: 0)
    }
    .font(NoemaFont.navigation)
    .padding(.horizontal, NoemaSpacing.lg)
    .frame(height: 52)
  }

  @ViewBuilder
  private func railLabel(for destination: NoemaDestination) -> some View {
    let label = displayLabel(for: destination)
    if destination == .chat {
      HStack(spacing: NoemaSpacing.sm) {
        NoemaAgentNavigationAvatar()
        if breakpoint != .compact || selection == destination {
          Text(label).lineLimit(1)
        }
      }
    } else if breakpoint != .compact || selection == destination {
      Label(label, systemImage: destination.symbol)
        .labelStyle(.titleAndIcon)
        .lineLimit(1)
    } else {
      Image(systemName: destination.symbol)
        .frame(width: 20, height: 20)
        .accessibilityHidden(true)
    }
  }

  private func displayLabel(for destination: NoemaDestination) -> String {
    destination == .chat ? agentLabel : destination.title
  }
}

private struct NoemaAgentNavigationAvatar: View {
  var body: some View {
    ZStack {
      Circle().fill(NoemaColor.agentAvatarFill)
      HStack(spacing: 5) {
        Circle().fill(NoemaColor.agentAvatarInk).frame(width: 2.5, height: 2.5)
        Circle().fill(NoemaColor.agentAvatarInk).frame(width: 2.5, height: 2.5)
      }
      .offset(y: -2.5)
      Capsule()
        .fill(NoemaColor.agentAvatarInk)
        .frame(width: 6, height: 2.5)
        .offset(y: 4)
    }
    .frame(width: 24, height: 24)
    .accessibilityHidden(true)
  }
}

struct NoemaShellView: View {
  var model: NoemaAppModel
  @State private var selection: NoemaDestination = .chat
  @State private var navigationOpen = false
  @State private var coordinator = NoemaShellCoordinator()
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    GeometryReader { proxy in
      let safeTop = proxy.safeAreaInsets.top
      let breakpoint = NoemaBreakpoint.resolve(width: proxy.size.width)
      let compact = breakpoint == .compact
      let sidebarWidth: CGFloat = compact ? proxy.size.width : 216
      let deckTop = safeTop + (coordinator.primaryNavigationHidden ? 0 : 52)
      let deckLeft: CGFloat = compact || coordinator.secondary == nil ? 0 : sidebarWidth
      let deckRight: CGFloat = compact ? 0 : 8
      let deckBottom: CGFloat = compact ? 0 : 8
      let reveal = min(mobileRevealHeight, max(0, proxy.size.height - deckTop - 48))

      ZStack(alignment: .topLeading) {
        NoemaColor.pine50.ignoresSafeArea()

        if coordinator.secondary != nil {
          NoemaSidebar(
            navigation: coordinator.secondary,
            compact: compact,
            close: { setNavigationOpen(false) }
          )
          .frame(width: sidebarWidth)
          .padding(.top, deckTop)
          .padding(.bottom, compact ? proxy.safeAreaInsets.bottom : 8)
          .opacity(compact && !navigationOpen ? 0 : 1)
          .allowsHitTesting(!compact || navigationOpen)
          .zIndex(10)
        }

        if compact && navigationOpen {
          Color.clear
            .contentShape(Rectangle())
            .onTapGesture { setNavigationOpen(false) }
            .padding(.top, deckTop)
            .zIndex(20)
        }

        contentDeck(compact: compact, safeBottom: proxy.safeAreaInsets.bottom)
          .frame(
            width: proxy.size.width - deckLeft - deckRight,
            height: proxy.size.height + safeTop + proxy.safeAreaInsets.bottom - deckTop - deckBottom
          )
          .offset(x: deckLeft, y: deckTop + (compact && navigationOpen ? reveal : 0))
          .shadow(color: NoemaColor.pine500.opacity(compact ? 0.08 : 0.16), radius: compact ? 8 : 24)
          .zIndex(30)

        if !coordinator.primaryNavigationHidden {
          NoemaTopRail(selection: $selection, breakpoint: breakpoint, agentLabel: coordinator.primaryAgentLabel)
            .padding(.top, safeTop)
            .zIndex(40)
        }
      }
      .ignoresSafeArea()
      .onAppear { installFallbackNavigation(for: selection) }
      .onChange(of: selection) { _, destination in
        navigationOpen = false
        installFallbackNavigation(for: destination)
      }
      .onChange(of: coordinator.requestedDestination) { _, destination in
        guard let destination else { return }
        selection = destination
        navigationOpen = false
        coordinator.requestedDestination = nil
      }
    }
    .environment(coordinator)
  }

  @ViewBuilder
  private func contentDeck(compact: Bool, safeBottom: CGFloat) -> some View {
    let hasSecondary = coordinator.secondary != nil
    VStack(spacing: 0) {
      if compact, let navigation = coordinator.secondary {
        NoemaMobileTitleNavigation(navigation: navigation, isOpen: navigationOpen) {
          setNavigationOpen(!navigationOpen)
        }
        .frame(height: 52)
        .zIndex(1)
      }

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
      .padding(.bottom, compact ? safeBottom : 0)
      .background(NoemaColor.surface)
      .transition(.opacity)
    }
    .background(NoemaColor.surface)
    .clipShape(
      UnevenRoundedRectangle(
        topLeadingRadius: NoemaRadius.page,
        bottomLeadingRadius: compact ? 0 : NoemaRadius.page,
        bottomTrailingRadius: compact ? 0 : NoemaRadius.page,
        topTrailingRadius: NoemaRadius.page
      )
    )
    .overlay {
      if !compact {
        RoundedRectangle(cornerRadius: NoemaRadius.page)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
    .contentShape(Rectangle())
    .simultaneousGesture(
      DragGesture(minimumDistance: 24).onEnded { value in
        guard compact, hasSecondary else { return }
        if navigationOpen, value.translation.height < -44 {
          setNavigationOpen(false)
        }
      }
    )
    .animation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion), value: selection)
  }

  private var mobileRevealHeight: CGFloat {
    guard let secondary = coordinator.secondary else { return 0 }
    let groups = secondary.entries.filter { $0.kind == .group }.count
    let items = secondary.entries.count - groups
    return min(520, 16 + CGFloat(groups * 33 + items * 36))
  }

  private func setNavigationOpen(_ open: Bool) {
    withAnimation(NoemaMotion.animation(NoemaSpring.surface, reduceMotion: reduceMotion)) {
      navigationOpen = open
    }
  }

  private func installFallbackNavigation(for destination: NoemaDestination) {
    switch destination {
    case .chat:
      coordinator.clearSecondary()
    case .tasks:
      coordinator.show(NoemaSecondaryNavigation(
        title: "Personal",
        symbol: "briefcase",
        entries: [
          .item(id: "personal", label: "Personal", symbol: "briefcase", selected: true) {}
        ]
      ))
    case .memory:
      coordinator.show(NoemaSecondaryNavigation(
        title: "Memory",
        symbol: "brain",
        entries: [
          .item(id: "memory-root", label: "Memory", symbol: "brain", selected: true) {}
        ]
      ))
    case .settings:
      coordinator.show(NoemaSecondaryNavigation(
        title: "Agents",
        symbol: "person.2",
        entries: Self.settingsEntries
      ))
    }
  }

  private static var settingsEntries: [NoemaSidebarEntry] {
    [
      .item(id: "agents", label: "Agents", symbol: "person.2", selected: true) {},
      .item(id: "memory", label: "Memory", symbol: "brain") {},
      .group("Tools"),
      .item(id: "web", label: "Web", symbol: "globe") {},
      .item(id: "apis", label: "APIs", symbol: "cable.connector") {},
      .item(id: "mcps", label: "MCPs", symbol: "bolt.horizontal.circle") {},
      .group("Safety"),
      .item(id: "privacy", label: "Privacy", symbol: "hand.raised") {},
      .item(id: "execution", label: "Execution", symbol: "gauge.with.dots.needle.67percent") {},
      .group("System"),
      .item(id: "models", label: "Local Models", symbol: "cpu") {},
      .item(id: "providers", label: "Providers", symbol: "server.rack") {},
      .item(id: "clients", label: "Clients", symbol: "iphone") {}
    ]
  }
}

private struct NoemaMobileTitleNavigation: View {
  let navigation: NoemaSecondaryNavigation
  let isOpen: Bool
  let toggle: () -> Void

  var body: some View {
    ZStack {
      LinearGradient(
        colors: [NoemaColor.surface, NoemaColor.surface, NoemaColor.surface.opacity(0)],
        startPoint: .top,
        endPoint: .bottom
      )
      Button(action: toggle) {
        HStack(spacing: NoemaSpacing.sm) {
          Image(systemName: navigation.symbol)
          Text(navigation.title).lineLimit(1)
          Image(systemName: "chevron.down")
            .font(.system(size: 12, weight: .semibold))
            .rotationEffect(.degrees(isOpen ? 180 : 0))
        }
        .font(NoemaFont.title)
        .foregroundStyle(NoemaColor.pine700)
        .padding(.horizontal, NoemaSpacing.md)
        .frame(minHeight: 36)
        .background(NoemaColor.pine100.opacity(0.48), in: Capsule())
        .shadow(color: NoemaColor.pine700.opacity(0.06), radius: 3, y: 1)
      }
      .buttonStyle(.plain)
      .padding(.horizontal, NoemaSpacing.lg)
      .frame(minHeight: 44)
      .accessibilityLabel("\(isOpen ? "Close" : "Open") \(navigation.title) navigation")
      .accessibilityValue(isOpen ? "Expanded" : "Collapsed")
    }
  }
}

private struct NoemaSidebar: View {
  let navigation: NoemaSecondaryNavigation?
  let compact: Bool
  let close: () -> Void

  var body: some View {
    ScrollView {
      LazyVStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        if let navigation {
          ForEach(navigation.entries) { entry in
            switch entry.kind {
            case .group:
              Text(entry.label)
                .font(Font.custom("Hanken Grotesk", size: 11, relativeTo: .caption).weight(.semibold))
                .foregroundStyle(NoemaColor.contentTertiary)
                .textCase(.uppercase)
                .padding(.horizontal, NoemaSpacing.md)
                .padding(.vertical, NoemaSpacing.md)
            case .item:
              Button {
                entry.action?()
                if compact { close() }
              } label: {
                HStack(spacing: 10) {
                  if let symbol = entry.symbol {
                    Image(systemName: symbol)
                      .frame(width: 18)
                      .accessibilityHidden(true)
                  }
                  Text(entry.label).lineLimit(1)
                  Spacer(minLength: 0)
                }
                .font(NoemaFont.body)
                .foregroundStyle(entry.isSelected ? NoemaColor.pine700 : NoemaColor.contentSecondary)
                .padding(.leading, 10 + CGFloat(entry.depth) * NoemaSpacing.md)
                .padding(.trailing, 10)
                .frame(minHeight: 34)
                .background(entry.isSelected ? NoemaColor.pine100.opacity(0.72) : Color.clear, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
                .contentShape(Rectangle())
              }
              .buttonStyle(.plain)
              .padding(.horizontal, 10)
              .accessibilityAddTraits(entry.isSelected ? .isSelected : [])
            }
          }
        }
      }
      .padding(.top, NoemaSpacing.sm)
      .scrollIndicators(.hidden)
    }
    .background(NoemaColor.pine50)
  }
}
