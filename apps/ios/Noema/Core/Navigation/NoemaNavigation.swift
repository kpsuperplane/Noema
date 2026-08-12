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
  var activeSurfaceAtTop = true
  private var activeDestination: NoemaDestination = .chat

  func activate(_ destination: NoemaDestination) {
    guard activeDestination != destination else { return }
    activeDestination = destination
    secondary = nil
  }

  func show(_ navigation: NoemaSecondaryNavigation, for destination: NoemaDestination) {
    guard activeDestination == destination else { return }
    secondary = navigation
  }

  func clearSecondary(for destination: NoemaDestination) {
    guard activeDestination == destination else { return }
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
  var agentAvatarActivity: NoemaAvatarActivity = .idle
  @State private var agentAvatarHovered = false
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    HStack(spacing: NoemaSpacing.xs) {
      Spacer(minLength: 0)
      ForEach(NoemaDestination.allCases) { destination in
        Button {
          selection = destination
        } label: {
          railLabel(for: destination)
            .frame(minWidth: 28, minHeight: 36)
            .padding(.horizontal, selection == destination ? NoemaSpacing.sm : NoemaSpacing.compact)
            .contentShape(NoemaSuperellipse.full)
            .background {
              if selection == destination {
                NoemaSuperellipse.full
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
    .animation(NoemaMotion.animation(NoemaSpring.micro, reduceMotion: reduceMotion), value: selection)
  }

  @ViewBuilder
  private func railLabel(for destination: NoemaDestination) -> some View {
    let label = displayLabel(for: destination)
    let isCompactInactive = breakpoint == .compact && selection != destination
    if destination == .chat {
      HStack(spacing: NoemaSpacing.sm) {
        NoemaIdentityAvatar(
          actorID: "agent:local",
          actorType: .agent,
          activity: agentAvatarActivity,
          animated: agentAvatarHovered || agentAvatarActivity != .idle
        )
          .frame(width: 24, height: 24)
          .accessibilityHidden(true)
        if !isCompactInactive {
          Text(label)
            .lineLimit(1)
            .transition(.opacity.combined(with: .offset(x: -4)))
        }
      }
      .onHover { agentAvatarHovered = $0 }
    } else {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: destination.symbol)
          .frame(width: 20, height: 20)
          .accessibilityHidden(true)
        if !isCompactInactive {
          Text(label)
            .lineLimit(1)
            .transition(.opacity.combined(with: .offset(x: -4)))
        }
      }
    }
  }

  private func displayLabel(for destination: NoemaDestination) -> String {
    destination == .chat ? agentLabel : destination.title
  }
}

struct NoemaShellView: View {
  var model: NoemaAppModel
  @State private var chat: ChatModel
  @State private var tasks: TasksModel?
  @State private var memory: MemoryModel
  @State private var settings: SettingsModel
  @State private var selection: NoemaDestination = .chat
  @State private var navigationOpen = false
  @State private var navigationDragOffset: CGFloat = 0
  @State private var navigationGestureStarted = false
  @State private var navigationDragMayOpen = false
  @State private var measuredMobileRevealHeight: CGFloat = 0
  @State private var coordinator = NoemaShellCoordinator()
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  init(model: NoemaAppModel) {
    self.model = model
    _chat = State(initialValue: ChatModel(client: model.graphQLClient?.client, profile: model.profile))
    _tasks = State(initialValue: model.graphQLClient.map {
      TasksModel(client: $0.client, profile: model.profile)
    })
    _memory = State(initialValue: MemoryModel())
    _settings = State(initialValue: SettingsModel())
  }

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
      let reveal = min(
        measuredMobileRevealHeight > 0 ? measuredMobileRevealHeight : estimatedMobileRevealHeight,
        max(0, proxy.size.height - deckTop - 48)
      )

      ZStack(alignment: .topLeading) {
        NoemaColor.pine50.ignoresSafeArea()

        if coordinator.secondary != nil {
          NoemaSidebar(
            navigation: coordinator.secondary,
            compact: compact,
            close: { setNavigationOpen(false) },
            onContentHeightChange: { measuredMobileRevealHeight = $0 }
          )
          .frame(width: sidebarWidth)
          .padding(.top, deckTop)
          .padding(.bottom, compact ? proxy.safeAreaInsets.bottom : 8)
          .opacity(compact && !navigationOpen ? 0 : 1)
          .allowsHitTesting(!compact || navigationOpen)
          .zIndex(25)
        }

        if compact && navigationOpen {
          Color.clear
            .contentShape(Rectangle())
            .onTapGesture { setNavigationOpen(false) }
            .padding(.top, deckTop)
            .zIndex(20)
        }

        contentDeck(
          compact: compact,
          safeBottom: proxy.safeAreaInsets.bottom,
          width: proxy.size.width,
          reveal: reveal
        )
          .frame(
            width: proxy.size.width - deckLeft - deckRight,
            height: proxy.size.height + safeTop + proxy.safeAreaInsets.bottom - deckTop - deckBottom
          )
          .offset(
            x: deckLeft,
            y: deckTop + (compact && navigationOpen ? reveal : 0) + (compact ? navigationDragOffset : 0)
          )
          .shadow(color: NoemaColor.pine500.opacity(compact ? 0.08 : 0.16), radius: compact ? 8 : 24)
          .zIndex(30)

        if !coordinator.primaryNavigationHidden {
          NoemaTopRail(
            selection: $selection,
            breakpoint: breakpoint,
            agentLabel: coordinator.primaryAgentLabel,
            agentAvatarActivity: shellAgentAvatarActivity
          )
            .padding(.top, safeTop)
            .zIndex(40)
        }
      }
      .ignoresSafeArea()
      .onAppear {
        installFallbackNavigation(for: selection)
        routePendingTask()
      }
      .onChange(of: selection) { _, destination in
        navigationOpen = false
        navigationDragOffset = 0
        coordinator.activeSurfaceAtTop = true
        installFallbackNavigation(for: destination)
      }
      .onChange(of: coordinator.requestedDestination) { _, destination in
        guard let destination else { return }
        selection = destination
        navigationOpen = false
        coordinator.requestedDestination = nil
      }
      .onChange(of: model.notificationTapGeneration) { _, _ in
        selection = .chat
        navigationOpen = false
        coordinator.activate(.chat)
      }
      .onChange(of: model.pendingTaskID) { _, _ in routePendingTask() }
    }
    .environment(coordinator)
  }

  private var shellAgentAvatarActivity: NoemaAvatarActivity {
    let runtimeActivity = noemaAvatarActivity(agentStatus: chat.agentStatus)
    if runtimeActivity == .idle, !chat.draft.isEmpty { return .listening }
    return runtimeActivity
  }

  @ViewBuilder
  private func contentDeck(compact: Bool, safeBottom: CGFloat, width: CGFloat, reveal: CGFloat) -> some View {
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
          ChatRootView(model: model, chat: chat)
        case .tasks:
          TasksRootView(model: model, tasksModel: tasks)
        case .memory:
          MemoryRootView(model: model, memory: memory)
        case .settings:
          SettingsRootView(model: model, settings: settings)
        }
      }
      .id(selection)
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .padding(.bottom, compact ? safeBottom : 0)
      .background(NoemaColor.surface)
      .transition(.opacity)
    }
    .background(NoemaColor.surface)
    .clipShape(
      NoemaSuperellipse(
        topLeftRadius: NoemaRadius.page,
        topRightRadius: NoemaRadius.page,
        bottomRightRadius: compact ? 0 : NoemaRadius.page,
        bottomLeftRadius: compact ? 0 : NoemaRadius.page,
        treatment: .page
      )
    )
    .overlay {
      if !compact {
        NoemaSuperellipse(cornerRadius: NoemaRadius.page, treatment: .page)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
    }
    .contentShape(Rectangle())
    .simultaneousGesture(
      DragGesture(minimumDistance: 8)
        .onChanged { value in
          guard compact else { return }
          if hasSecondary, !navigationGestureStarted {
            navigationGestureStarted = true
            navigationDragMayOpen = coordinator.activeSurfaceAtTop
          }
          let vertical = abs(value.translation.height) >= abs(value.translation.width) * 1.2
          guard hasSecondary, vertical else { return }
          if !navigationOpen, navigationDragMayOpen, value.translation.height > 0 {
            navigationDragOffset = min(max(0, reveal - 1), max(0, value.translation.height - 8))
          } else if navigationOpen, value.translation.height < 0 {
            navigationDragOffset = max(-max(0, reveal - 1), min(0, value.translation.height + 8))
          }
        }
        .onEnded { value in
          defer {
            withAnimation(NoemaMotion.animation(NoemaSpring.surface, reduceMotion: reduceMotion)) {
              navigationDragOffset = 0
            }
            navigationGestureStarted = false
            navigationDragMayOpen = false
          }
          guard compact else { return }
          let horizontal = abs(value.translation.width) >= abs(value.translation.height) * 1.2
          if !navigationOpen, horizontal {
            let threshold = min(84, width * 0.22)
            let distance = max(abs(value.translation.width), abs(value.predictedEndTranslation.width))
            guard distance >= threshold else { return }
            let destinations = NoemaDestination.allCases
            guard let current = destinations.firstIndex(of: selection) else { return }
            let next = current + (value.translation.width > 0 ? -1 : 1)
            guard destinations.indices.contains(next) else { return }
            selection = destinations[next]
          } else if hasSecondary, navigationOpen || navigationDragMayOpen {
            let distance = navigationOpen ? -navigationDragOffset : navigationDragOffset
            let threshold = min(84, reveal * 0.35)
            let predicted = navigationOpen
              ? -value.predictedEndTranslation.height
              : value.predictedEndTranslation.height
            if distance >= threshold || predicted >= threshold {
              setNavigationOpen(!navigationOpen)
            }
          }
        }
    )
    .animation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion), value: selection)
  }

  private var estimatedMobileRevealHeight: CGFloat {
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

  private func routePendingTask() {
    guard let taskID = model.pendingTaskID else { return }
    selection = .tasks
    navigationOpen = false
    coordinator.openTask(taskID)
    model.clearPendingTaskID()
  }

  private func installFallbackNavigation(for destination: NoemaDestination) {
    coordinator.activate(destination)
    switch destination {
    case .chat:
      coordinator.clearSecondary(for: .chat)
    case .tasks:
      coordinator.show(NoemaSecondaryNavigation(
        title: "Personal",
        symbol: "briefcase",
        entries: [
          .item(id: "personal", label: "Personal", symbol: "briefcase", selected: true) {}
        ]
      ), for: .tasks)
    case .memory:
      coordinator.show(NoemaSecondaryNavigation(
        title: "Memory",
        symbol: "brain",
        entries: [
          .item(id: "memory-root", label: "Memory", symbol: "brain", selected: true) {}
        ]
      ), for: .memory)
    case .settings:
      coordinator.show(NoemaSecondaryNavigation(
        title: "Agents",
        symbol: "person.2",
        entries: Self.settingsEntries
      ), for: .settings)
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
      .item(id: "notifications", label: "Notifications", symbol: "bell") {},
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
        .background(NoemaColor.pine100.opacity(0.48), in: NoemaSuperellipse.full)
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
  let onContentHeightChange: (CGFloat) -> Void

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
                .background(entry.isSelected ? NoemaColor.pine100.opacity(0.72) : Color.clear, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
                .contentShape(Rectangle())
              }
              .buttonStyle(.plain)
              .padding(.horizontal, 10)
              .accessibilityAddTraits(entry.isSelected ? .isSelected : [])
            }
          }
        }
      }
      .padding(.vertical, NoemaSpacing.sm)
      .onGeometryChange(for: CGFloat.self) { geometry in
        geometry.size.height
      } action: { height in
        if compact { onContentHeightChange(height) }
      }
      .scrollIndicators(.hidden)
    }
    .background(NoemaColor.pine50)
  }
}

private struct NoemaSurfaceTopTrackingModifier: ViewModifier {
  @Environment(NoemaShellCoordinator.self) private var coordinator

  func body(content: Content) -> some View {
    content.onScrollGeometryChange(for: Bool.self) { geometry in
      geometry.contentOffset.y <= 1
    } action: { _, isAtTop in
      coordinator.activeSurfaceAtTop = isAtTop
    }
  }
}

extension View {
  func tracksNoemaSurfaceTop() -> some View {
    modifier(NoemaSurfaceTopTrackingModifier())
  }
}
