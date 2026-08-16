import Observation
import SwiftUI
import UIKit

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

  var icon: NoemaIcon.Name {
    switch self {
    case .chat: .messageCircle
    case .tasks: .listTodo
    case .memory: .brain
    case .settings: .settings
    }
  }
}

struct NoemaSidebarEntry: Identifiable {
  enum Kind { case group, item }

  let id: String
  let kind: Kind
  let label: String
  var icon: NoemaIcon.Name?
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
    icon: NoemaIcon.Name,
    depth: Int = 0,
    selected: Bool = false,
    pinned: Bool = false,
    action: @escaping @MainActor () -> Void
  ) -> Self {
    Self(
      id: id,
      kind: .item,
      label: label,
      icon: icon,
      depth: depth,
      isSelected: selected,
      isPinned: pinned,
      action: action
    )
  }
}

struct NoemaSecondaryNavigation {
  let title: String
  let icon: NoemaIcon.Name
  let entries: [NoemaSidebarEntry]
}

@MainActor
@Observable
final class NoemaShellCoordinator {
  var primaryAgentLabel = "Chat"
  var primaryNavigationHidden = false
  var requestedDestination: NoemaDestination?
  var requestedTaskID: String?
  var activeSurfaceAtTop = true
  var chatFocusDeferred = false
  private(set) var activeDestination: NoemaDestination = .chat
  private var secondaryNavigation: [NoemaDestination: NoemaSecondaryNavigation] = [:]

  var secondary: NoemaSecondaryNavigation? {
    secondaryNavigation[activeDestination]
  }

  func activate(_ destination: NoemaDestination) {
    guard activeDestination != destination else { return }
    activeDestination = destination
  }

  func show(_ navigation: NoemaSecondaryNavigation, for destination: NoemaDestination) {
    secondaryNavigation[destination] = navigation
  }

  func clearSecondary(for destination: NoemaDestination) {
    secondaryNavigation[destination] = nil
  }

  func navigation(for destination: NoemaDestination) -> NoemaSecondaryNavigation? {
    secondaryNavigation[destination]
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
  let selectionPosition: CGFloat
  var agentAvatarActivity: NoemaAvatarActivity = .idle
  @State private var agentAvatarHovered = false
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    HStack(spacing: NoemaSpacing.xs) {
      Spacer(minLength: 0)
      ForEach(NoemaDestination.allCases) { destination in
        let expansion = expansion(for: destination)
        Button {
          selection = destination
        } label: {
          railLabel(for: destination, expansion: expansion)
            .frame(minWidth: 28, minHeight: 36)
            .padding(.horizontal, horizontalPadding(for: expansion))
            .contentShape(NoemaSuperellipse.full)
            .background {
              NoemaSuperellipse.full
                .fill(NoemaColor.white.opacity(expansion))
                .shadow(color: NoemaColor.pine600.opacity(0.10 * expansion), radius: 4, y: 3)
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
  private func railLabel(for destination: NoemaDestination, expansion: CGFloat) -> some View {
    let label = displayLabel(for: destination)
    if destination == .chat {
      HStack(spacing: 0) {
        NoemaIdentityAvatar(
          actorID: "agent:local",
          actorType: .agent,
          activity: agentAvatarActivity,
          animated: agentAvatarHovered || agentAvatarActivity != .idle
        )
          .frame(width: 24, height: 24)
          .accessibilityHidden(true)
        railText(label, expansion: expansion)
      }
      .onHover { agentAvatarHovered = $0 }
    } else {
      HStack(spacing: 0) {
        NoemaIcon(destination.icon, size: 20)
          .frame(width: 20, height: 20)
        railText(label, expansion: expansion)
      }
    }
  }

  @ViewBuilder
  private func railText(_ label: String, expansion: CGFloat) -> some View {
    if breakpoint == .compact {
      NoemaExpandableRailText(label: label, expansion: expansion)
    } else {
      Text(label)
        .lineLimit(1)
        .padding(.leading, NoemaSpacing.sm)
        .padding(.trailing, NoemaSpacing.xs)
    }
  }

  private func displayLabel(for destination: NoemaDestination) -> String {
    destination == .chat ? agentLabel : destination.title
  }

  private func expansion(for destination: NoemaDestination) -> CGFloat {
    guard breakpoint == .compact,
          let index = NoemaDestination.allCases.firstIndex(of: destination)
    else { return selection == destination ? 1 : 0 }
    return max(0, 1 - abs(selectionPosition - CGFloat(index)))
  }

  private func horizontalPadding(for expansion: CGFloat) -> CGFloat {
    NoemaSpacing.compact + (NoemaSpacing.sm - NoemaSpacing.compact) * expansion
  }
}

private struct NoemaConnectionBanner: View {
  let message: String
  let symbol: String

  var body: some View {
    HStack(spacing: NoemaSpacing.compact) {
      Image(systemName: symbol)
        .accessibilityHidden(true)
      Text(message)
        .font(NoemaFont.captionEmphasized)
    }
    .foregroundStyle(NoemaColor.danger)
    .padding(.horizontal, NoemaSpacing.md)
    .padding(.vertical, NoemaSpacing.compact)
    .frame(maxWidth: .infinity)
    .frame(minHeight: NoemaSpacing.xxl + NoemaSpacing.sm)
    .accessibilityElement(children: .combine)
  }
}

private struct NoemaExpandableRailText: View {
  let label: String
  let expansion: CGFloat
  @State private var labelWidth: CGFloat = 0

  var body: some View {
    Text(label)
      .lineLimit(1)
      .fixedSize()
      .onGeometryChange(for: CGFloat.self) { geometry in
        geometry.size.width
      } action: { width in
        labelWidth = width
      }
      .padding(.trailing, NoemaSpacing.xs)
      .frame(width: (labelWidth + NoemaSpacing.xs) * expansion, alignment: .leading)
      .clipped()
      .padding(.leading, NoemaSpacing.sm * expansion)
      .opacity(expansion)
  }
}

private enum NoemaShellDragAxis {
  case horizontal
  case vertical
}

private final class NoemaHorizontalPanGestureRecognizer: UIPanGestureRecognizer, UIGestureRecognizerDelegate {
  override init(target: Any?, action: Selector?) {
    super.init(target: target, action: action)
    cancelsTouchesInView = true
    delegate = self
  }

  func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
    let velocity = velocity(in: view)
    return abs(velocity.x) >= abs(velocity.y) * 1.2
  }
}

private struct NoemaHorizontalSwipeGesture: UIGestureRecognizerRepresentable {
  let enabled: Bool
  let changed: (CGFloat) -> Void
  let ended: (_ translation: CGFloat, _ projectedTranslation: CGFloat) -> Void
  let cancelled: () -> Void

  func makeUIGestureRecognizer(context: Context) -> NoemaHorizontalPanGestureRecognizer {
    NoemaHorizontalPanGestureRecognizer()
  }

  func updateUIGestureRecognizer(_ recognizer: NoemaHorizontalPanGestureRecognizer, context: Context) {
    recognizer.isEnabled = enabled
  }

  func handleUIGestureRecognizerAction(
    _ recognizer: NoemaHorizontalPanGestureRecognizer,
    context: Context
  ) {
    let translation = recognizer.translation(in: recognizer.view).x
    switch recognizer.state {
    case .began, .changed:
      changed(translation)
    case .ended:
      ended(translation, translation + recognizer.velocity(in: recognizer.view).x * 0.2)
    case .cancelled, .failed:
      cancelled()
    default:
      break
    }
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
  @State private var navigationDragMayOpen = false
  @State private var pageDragOffset: CGFloat = 0
  @State private var pageDragTarget: NoemaDestination?
  @State private var pageSwipeInFlight = false
  @State private var pageSwipeGeneration = 0
  @State private var shellDragAxis: NoemaShellDragAxis?
  @State private var measuredMobileRevealHeight: CGFloat = 0
  @State private var measuredConnectionBannerHeight: CGFloat = 0
  @State private var coordinator = NoemaShellCoordinator()
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  init(model: NoemaAppModel) {
    self.model = model
    _chat = State(initialValue: ChatModel(
      client: model.graphQLClient?.client,
      profile: model.profile,
      connectionStatus: model.graphQLClient?.connectionStatus
    ))
    _tasks = State(initialValue: model.graphQLClient.map {
      TasksModel(client: $0.client, profile: model.profile, connectionStatus: $0.connectionStatus)
    })
    _memory = State(initialValue: MemoryModel(connectionStatus: model.graphQLClient?.connectionStatus))
    _settings = State(initialValue: SettingsModel(connectionStatus: model.graphQLClient?.connectionStatus))
  }

  var body: some View {
    GeometryReader { proxy in
      let safeTop = proxy.safeAreaInsets.top
      let breakpoint = NoemaBreakpoint.resolve(width: proxy.size.width)
      let compact = breakpoint == .compact
      let sidebarWidth: CGFloat = compact ? proxy.size.width : 216
      let connectionBannerHeight = model.connectionMessage != nil
        ? max(measuredConnectionBannerHeight, NoemaSpacing.xxl + NoemaSpacing.sm)
        : 0
      let shellTop = safeTop + connectionBannerHeight
      let deckTop = shellTop + (coordinator.primaryNavigationHidden ? 0 : 52)
      let deckLeft: CGFloat = compact || coordinator.secondary == nil ? 0 : sidebarWidth
      let deckRight: CGFloat = compact ? 0 : 8
      let deckBottom: CGFloat = compact ? 0 : 8
      let deckWidth = proxy.size.width - deckLeft - deckRight
      let reveal = min(
        measuredMobileRevealHeight > 0 ? measuredMobileRevealHeight : estimatedMobileRevealHeight,
        max(0, proxy.size.height - deckTop - 48)
      )

      ZStack(alignment: .topLeading) {
        NoemaColor.pine50.ignoresSafeArea()

        if let connectionMessage = model.connectionMessage {
          NoemaConnectionBanner(message: connectionMessage, symbol: model.connectionSymbol)
            .onGeometryChange(for: CGFloat.self) { geometry in
              geometry.size.height
            } action: { height in
              measuredConnectionBannerHeight = height
            }
            .offset(y: safeTop)
            .transition(.move(edge: .top).combined(with: .opacity))
            .zIndex(50)
        }

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
          width: deckWidth,
          reveal: reveal
        )
          .frame(
            width: deckWidth,
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
            selection: Binding(
              get: { selection },
              set: { selectDestination($0) }
            ),
            breakpoint: breakpoint,
            agentLabel: coordinator.primaryAgentLabel,
            selectionPosition: pageSelectionPosition(width: proxy.size.width),
            agentAvatarActivity: shellAgentAvatarActivity
          )
            .padding(.top, shellTop)
            .zIndex(40)
        }
      }
      .ignoresSafeArea()
      .animation(
        NoemaMotion.animation(NoemaSpring.surface, reduceMotion: reduceMotion),
        value: model.isDisconnected
      )
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
        selectDestination(destination)
        navigationOpen = false
        coordinator.requestedDestination = nil
      }
      .onChange(of: model.notificationTapGeneration) { _, _ in
        selectDestination(.chat)
        navigationOpen = false
        coordinator.activate(.chat)
      }
      .onChange(of: model.pendingTaskID) { _, _ in routePendingTask() }
    }
    .environment(coordinator)
  }

  private var shellAgentAvatarActivity: NoemaAvatarActivity {
    let runtimeActivity = noemaAvatarActivity(agentStatus: chat.agentStatus)
    if runtimeActivity == .idle, chat.isVoiceInputActive || !chat.draft.isEmpty { return .listening }
    return runtimeActivity
  }

  @ViewBuilder
  private func contentDeck(compact: Bool, safeBottom: CGFloat, width: CGFloat, reveal: CGFloat) -> some View {
    let hasSecondary = coordinator.secondary != nil
    ZStack {
      ForEach(visibleDestinations) { destination in
        destinationPage(destination, compact: compact, safeBottom: safeBottom)
          .frame(width: width)
          .offset(x: pageOffset(for: destination, width: width))
          .allowsHitTesting(destination == selection && !pageSwipeInFlight)
          .accessibilityHidden(destination != selection)
          .transition(.opacity)
      }
    }
    .scrollDisabled(pageSwipeInFlight)
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
    .gesture(
      NoemaHorizontalSwipeGesture(
        enabled: compact && !navigationOpen,
        changed: { translation in
          if !pageSwipeInFlight {
            pageSwipeInFlight = true
            coordinator.chatFocusDeferred = true
            pageSwipeGeneration += 1
          }
          updatePageSwipe(translation: translation, width: width)
        },
        ended: { translation, projectedTranslation in
          finishPageSwipe(
            translation: translation,
            projectedTranslation: projectedTranslation,
            width: width
          )
        },
        cancelled: {
          settlePageSwipe()
        }
      )
    )
    .simultaneousGesture(
      DragGesture(minimumDistance: 8)
        .onChanged { value in
          guard compact else { return }
          if shellDragAxis == nil {
            let horizontal = abs(value.translation.width) >= abs(value.translation.height) * 1.2
            let vertical = abs(value.translation.height) >= abs(value.translation.width) * 1.2
            if horizontal {
              shellDragAxis = .horizontal
            } else if vertical {
              shellDragAxis = .vertical
              navigationDragMayOpen = hasSecondary && coordinator.activeSurfaceAtTop
            } else {
              return
            }
          }

          switch shellDragAxis {
          case .vertical where hasSecondary:
            if !navigationOpen, navigationDragMayOpen, value.translation.height > 0 {
              navigationDragOffset = min(max(0, reveal - 1), max(0, value.translation.height - 8))
            } else if navigationOpen, value.translation.height < 0 {
              navigationDragOffset = max(-max(0, reveal - 1), min(0, value.translation.height + 8))
            }
          default:
            break
          }
        }
        .onEnded { value in
          defer {
            withAnimation(NoemaMotion.animation(NoemaSpring.surface, reduceMotion: reduceMotion)) {
              navigationDragOffset = 0
            }
            navigationDragMayOpen = false
            shellDragAxis = nil
          }
          guard compact else { return }
          if shellDragAxis == .vertical, hasSecondary, navigationOpen || navigationDragMayOpen {
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
  }

  @ViewBuilder
  private func destinationPage(_ destination: NoemaDestination, compact: Bool, safeBottom: CGFloat) -> some View {
    VStack(spacing: 0) {
      if compact, let navigation = coordinator.navigation(for: destination) ?? fallbackNavigation(for: destination) {
        NoemaMobileTitleNavigation(
          navigation: navigation,
          isOpen: destination == selection && navigationOpen
        ) {
          if destination == selection { setNavigationOpen(!navigationOpen) }
        }
        .frame(height: 52)
        .zIndex(1)
      }

      primarySurface(destination)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.bottom, compact ? safeBottom : 0)
        .background(NoemaColor.surface)
    }
    .background(NoemaColor.surface)
  }

  @ViewBuilder
  private func primarySurface(_ destination: NoemaDestination) -> some View {
    switch destination {
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

  private var visibleDestinations: [NoemaDestination] {
    NoemaDestination.allCases.filter { $0 == selection || $0 == pageDragTarget }
  }

  private func pageOffset(for destination: NoemaDestination, width: CGFloat) -> CGFloat {
    let destinations = NoemaDestination.allCases
    guard let current = destinations.firstIndex(of: selection),
          let index = destinations.firstIndex(of: destination)
    else { return 0 }
    return CGFloat(index - current) * width + pageDragOffset
  }

  private func pageSelectionPosition(width: CGFloat) -> CGFloat {
    guard width > 0,
          let current = NoemaDestination.allCases.firstIndex(of: selection)
    else { return 0 }
    let last = CGFloat(NoemaDestination.allCases.count - 1)
    return min(last, max(0, CGFloat(current) - pageDragOffset / width))
  }

  private func updatePageSwipe(translation: CGFloat, width: CGFloat) {
    guard let target = adjacentDestination(translation: translation) else {
      pageDragOffset = 0
      pageDragTarget = nil
      return
    }
    pageDragTarget = target
    pageDragOffset = min(width, max(-width, translation))
  }

  private func finishPageSwipe(
    translation: CGFloat,
    projectedTranslation: CGFloat,
    width: CGFloat
  ) {
    let threshold = min(84, width * 0.22)
    let distance = max(abs(translation), abs(projectedTranslation))
    guard distance >= threshold,
          let target = adjacentDestination(translation: translation),
          let currentIndex = NoemaDestination.allCases.firstIndex(of: selection),
          let targetIndex = NoemaDestination.allCases.firstIndex(of: target)
    else {
      settlePageSwipe()
      return
    }

    let outgoing = selection
    var transaction = Transaction(animation: nil)
    transaction.disablesAnimations = true
    withTransaction(transaction) {
      selection = target
      pageDragTarget = outgoing
      pageDragOffset += CGFloat(targetIndex - currentIndex) * width
    }
    settlePageSwipe()
  }

  private func settlePageSwipe() {
    let generation = pageSwipeGeneration
    Task { @MainActor in
      if !reduceMotion { try? await Task.sleep(for: .milliseconds(235)) }
      guard pageSwipeGeneration == generation else { return }
      coordinator.chatFocusDeferred = false
    }
    withAnimation(
      NoemaMotion.animation(NoemaSpring.surface, reduceMotion: reduceMotion),
      completionCriteria: .logicallyComplete
    ) {
      pageDragOffset = 0
    } completion: {
      guard pageSwipeGeneration == generation else { return }
      coordinator.chatFocusDeferred = false
      Task { @MainActor in
        await Task.yield()
        guard pageSwipeGeneration == generation else { return }
        pageDragTarget = nil
        pageSwipeInFlight = false
      }
    }
  }

  private func adjacentDestination(translation: CGFloat) -> NoemaDestination? {
    guard translation != 0,
          let current = NoemaDestination.allCases.firstIndex(of: selection)
    else { return nil }
    let next = current + (translation > 0 ? -1 : 1)
    guard NoemaDestination.allCases.indices.contains(next) else { return nil }
    return NoemaDestination.allCases[next]
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

  private func selectDestination(_ destination: NoemaDestination) {
    guard destination != selection else { return }
    pageSwipeGeneration += 1
    withAnimation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion)) {
      pageDragOffset = 0
      pageDragTarget = nil
      pageSwipeInFlight = false
      coordinator.chatFocusDeferred = false
      selection = destination
    }
  }

  private func routePendingTask() {
    guard let taskID = model.pendingTaskID else { return }
    selectDestination(.tasks)
    navigationOpen = false
    coordinator.openTask(taskID)
    model.clearPendingTaskID()
  }

  private func installFallbackNavigation(for destination: NoemaDestination) {
    coordinator.activate(destination)
    if destination == .chat {
      coordinator.clearSecondary(for: .chat)
      return
    }
    guard coordinator.navigation(for: destination) == nil,
          let navigation = fallbackNavigation(for: destination)
    else { return }
    coordinator.show(navigation, for: destination)
  }

  private func fallbackNavigation(for destination: NoemaDestination) -> NoemaSecondaryNavigation? {
    switch destination {
    case .chat:
      nil
    case .tasks:
      NoemaSecondaryNavigation(
        title: "Personal",
        icon: .briefcaseBusiness,
        entries: [
          .item(id: "personal", label: "Personal", icon: .briefcaseBusiness, selected: true) {}
        ]
      )
    case .memory:
      NoemaSecondaryNavigation(
        title: "Memory",
        icon: .brain,
        entries: [
          .item(id: "memory-root", label: "Memory", icon: .brain, selected: true) {}
        ]
      )
    case .settings:
      NoemaSecondaryNavigation(
        title: "Agents",
        icon: .bot,
        entries: Self.settingsEntries
      )
    }
  }

  private static var settingsEntries: [NoemaSidebarEntry] {
    [
      .item(id: "agents", label: "Agents", icon: .bot, selected: true) {},
      .item(id: "memory", label: "Memory", icon: .brain) {},
      .group("Tools"),
      .item(id: "web", label: "Web", icon: .globe) {},
      .item(id: "apis", label: "APIs", icon: .cable) {},
      .item(id: "mcps", label: "MCPs", icon: .plugZap) {},
      .group("Safety"),
      .item(id: "privacy", label: "Privacy", icon: .shieldCheck) {},
      .item(id: "execution", label: "Execution", icon: .gauge) {},
      .group("System"),
      .item(id: "models", label: "Local Models", icon: .cpu) {},
      .item(id: "providers", label: "Providers", icon: .serverCog) {},
      .item(id: "notifications", label: "Notifications", icon: .bell) {},
      .item(id: "clients", label: "Clients", icon: .smartphone) {}
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
          NoemaIcon(navigation.icon)
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
                .font(NoemaFont.captionEmphasized)
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
                  if let icon = entry.icon {
                    NoemaIcon(icon)
                      .frame(width: 18)
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
  let destination: NoemaDestination
  @Environment(NoemaShellCoordinator.self) private var coordinator

  func body(content: Content) -> some View {
    content.onScrollGeometryChange(for: Bool.self) { geometry in
      geometry.contentOffset.y <= 1
    } action: { _, isAtTop in
      guard coordinator.activeDestination == destination else { return }
      coordinator.activeSurfaceAtTop = isAtTop
    }
  }
}

extension View {
  func tracksNoemaSurfaceTop(for destination: NoemaDestination) -> some View {
    modifier(NoemaSurfaceTopTrackingModifier(destination: destination))
  }
}
