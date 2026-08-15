import Foundation
import SwiftUI

struct ChatRootView: View {
  let model: NoemaAppModel
  @State private var chat: ChatModel
  @Environment(NoemaShellCoordinator.self) private var coordinator

  init(model: NoemaAppModel, chat: ChatModel) {
    self.model = model
    _chat = State(initialValue: chat)
  }

  var body: some View {
    Group {
      switch chat.phase {
      case .loading:
        ChatLoadingView(model: chat)
      case .onboarding:
        if let onboarding = chat.onboarding {
          OnboardingRootView(model: onboarding) { Task { await chat.onboardingCompleted() } }
        } else {
          ChatFailureView(message: "No onboarding connection is available.", retry: { Task { await chat.retry() } })
        }
      case .ready:
        ChatReadyView(model: chat, notifications: model.notifications)
      case let .failed(message):
        ChatFailureView(message: message, retry: { Task { await chat.retry() } })
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(NoemaColor.surface)
    .task { await chat.start() }
    .onAppear {
      coordinator.clearSecondary(for: .chat)
      syncAgentLabel()
      syncActiveChatState()
    }
    .onChange(of: chat.primaryAgentDisplayName) { _, _ in syncAgentLabel() }
    .onChange(of: chat.phase) { _, _ in syncActiveChatState() }
    .onChange(of: coordinator.activeDestination) { _, _ in syncActiveChatState() }
    .onChange(of: model.recoveryGeneration) { _, _ in
      Task { await chat.recoverConnection() }
    }
    .onDisappear {
      coordinator.primaryNavigationHidden = false
      model.notifications.chatVisibilityChanged(false)
    }
  }

  private func syncAgentLabel() {
    coordinator.primaryAgentLabel = chat.primaryAgentDisplayName?.isEmpty == false
      ? chat.primaryAgentDisplayName!
      : "Chat"
  }

  private func syncShellChrome() {
    coordinator.primaryNavigationHidden = chat.phase == .onboarding
  }

  private func syncActiveChatState() {
    let active = coordinator.activeDestination == .chat
    if active {
      syncShellChrome()
    } else {
      coordinator.primaryNavigationHidden = false
    }
    model.notifications.chatVisibilityChanged(active && chat.phase == .ready)
  }
}

private struct ChatLoadingView: View {
  @Bindable var model: ChatModel

  var body: some View {
    ChatTranscriptLoadingSkeleton()
      .safeAreaInset(edge: .bottom, spacing: 0) {
        ChatComposer(
          model: model,
          isEditable: model.conversationID != nil,
          isSendEnabled: false
        )
          .frame(width: 200, alignment: .trailing)
          .padding(.horizontal, NoemaSpacing.md)
          .padding(.bottom, NoemaSpacing.sm)
          .frame(maxWidth: .infinity, alignment: .trailing)
      }
      .accessibilityElement(children: .contain)
      .accessibilityLabel("Loading chat")
  }
}

struct ChatTranscriptLoadingSkeleton: View {
  var horizontalPadding = NoemaSpacing.md

  var body: some View {
    VStack(spacing: 18) {
      Spacer(minLength: NoemaSpacing.xxl)
      skeletonBubble(lane: .assistant, widths: [204, 130])
      skeletonBubble(lane: .human, widths: [164])
      skeletonBubble(lane: .assistant, widths: [243, 187, 96])
    }
    .frame(maxWidth: 760, maxHeight: .infinity)
    .padding(.horizontal, horizontalPadding)
    .padding(.top, 64)
    .padding(.bottom, 96)
  }

  private func skeletonBubble(lane: ChatLane, widths: [CGFloat]) -> some View {
    ChatLaneRow(lane: lane, showAvatar: true) {
      ChatBubbleView(lane: lane, group: .single) {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          ForEach(Array(widths.enumerated()), id: \.offset) { _, width in
            NoemaSuperellipse.full
              .fill(lane == .human ? NoemaColor.white.opacity(0.28) : NoemaColor.ink900.opacity(0.08))
              .frame(width: width, height: 10)
          }
        }
        .padding(.vertical, NoemaSpacing.xxs)
      }
    }
    .accessibilityHidden(true)
  }
}

struct ChatFailureView: View {
  let message: String
  let retry: () -> Void

  var body: some View {
    NoemaDeckState(
      title: "Chat unavailable",
      message: message,
      symbol: "bubble.left.and.exclamationmark.bubble.right",
      tone: .warning,
      actionTitle: "Try again",
      action: retry
    )
  }
}

private struct ChatScrollGeometry: Equatable {
  let isAtBottom: Bool
  let containerHeight: CGFloat
  let contentHeight: CGFloat
}

struct ChatReadyView: View {
  @Bindable var model: ChatModel
  let notifications: NoemaNotificationService
  @Environment(NoemaShellCoordinator.self) private var shellCoordinator
  @State private var followBottom = true
  @State private var scrollToBottomRequest = 0
  @State private var selectedArtifact: ArtifactSelection?
  @State private var selectedTaskID: String?
  @State private var notificationPromptDismissed = false
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  fileprivate enum TimelineRow: Identifiable {
    case message(ChatMessage, taskIDs: [String])
    case task(id: String, taskID: String)
    case toolMarkers(id: String, messages: [ChatMessage])
    case activity(ChatMessage)
    case systemNotice(ChatMessage)
    case typing

    var id: String {
      switch self {
      case let .message(message, _): message.id
      case let .task(id, _): id
      case let .toolMarkers(id, _): "tool-" + id
      case let .activity(message), let .systemNotice(message): message.id
      case .typing: "chat-typing"
      }
    }
  }

  private var timelineRows: [TimelineRow] {
    let taskProjection = taskReferenceProjection
    let visibleMessages = latestA2UISurfaces(model.messages).filter { message in
      guard case let .activity(_, _, _, _, activityKind) = message.kind else { return true }
      return activityKind.replacingOccurrences(of: "_", with: "").lowercased() != "hostedwebsearch"
    }
    var rows: [TimelineRow] = []
    var index = 0
    while index < visibleMessages.count {
      let message = visibleMessages[index]
      if case let .task(taskID) = message.kind {
        if taskProjection.consumedTaskIDs.contains(message.id) || taskProjection.hiddenTaskIDs.contains(message.id) {
          index += 1
          continue
        }
        rows.append(.task(id: message.id, taskID: taskID))
        index += 1
        continue
      }
      if isToolActivity(message) {
        var markers = [message]
        index += 1
        while index < visibleMessages.count,
              isToolActivity(visibleMessages[index]),
              sameToolGroup(markers, visibleMessages[index]) {
          markers.append(visibleMessages[index])
          index += 1
        }
        rows.append(.toolMarkers(id: markers[0].id, messages: markers))
        continue
      }
      if case .assistant = message.kind {
        rows.append(.message(message, taskIDs: taskProjection.attachedTaskIDsByAssistantID[message.id] ?? []))
        index += 1
        continue
      } else if case .activity = message.kind {
        rows.append(isSystemNotice(message) ? .systemNotice(message) : .activity(message))
      } else {
        rows.append(.message(message, taskIDs: []))
      }
      index += 1
    }
    if shouldShowTyping(in: visibleMessages) { rows.append(.typing) }
    return rows
  }

  private func shouldShowTyping(in messages: [ChatMessage]) -> Bool {
    guard let lastUserIndex = messages.lastIndex(where: { if case .user = $0.kind { return true }; return false }) else {
      return false
    }
    var hasCompletedAssistant = false
    for message in messages.suffix(from: messages.index(after: lastUserIndex)) {
      if case let .assistant(_, streaming) = message.kind {
        if streaming { return true }
        hasCompletedAssistant = true
      }
    }
    guard !hasCompletedAssistant else { return false }
    let normalized = model.agentStatus.replacingOccurrences(of: "_", with: "").lowercased()
    let active = ["inputreceived", "thinking", "toolrunning", "waitingforpreviousturncompletion", "interrupting"].contains(normalized)
    return model.isSending || active
  }

  private func latestA2UISurfaces(_ messages: [ChatMessage]) -> [ChatMessage] {
    var latestIndex: [String: Int] = [:]
    for (index, message) in messages.enumerated() {
      if case let .a2ui(surface) = message.kind { latestIndex[surface.surfaceID] = index }
    }
    return messages.enumerated().compactMap { index, message in
      guard case let .a2ui(surface) = message.kind else { return message }
      return latestIndex[surface.surfaceID] == index ? message : nil
    }
  }

  var body: some View {
    GeometryReader { geometry in
      if geometry.size.width < NoemaBreakpoint.regularMinimum - 216 - 8 {
        transcriptSurface
          .noemaSheet(isPresented: taskDetailPresented) {
            taskDetail
          }
      } else {
        transcriptSurface
          .inspector(isPresented: taskDetailPresented) {
            taskDetail
              .inspectorColumnWidth(min: 320, ideal: 380, max: 640)
          }
      }
    }
    .noemaSheet(item: $selectedArtifact) { selection in
      ArtifactVersionSheet(model: ArtifactModel(client: model.client, profile: model.profile), selection: selection)
    }
  }

  private var transcriptSurface: some View {
    ScrollViewReader { proxy in
      ScrollView {
        NoemaPageTrack(maxWidth: 760, horizontalPadding: NoemaSpacing.md) {
          LazyVStack(alignment: .leading, spacing: 0) {
            if model.messages.isEmpty {
              if model.isOffline {
                SystemNoticeView(
                  text: "Reconnect to load this conversation.",
                  symbol: "wifi.slash",
                  tone: .warning
                )
                .containerRelativeFrame(.vertical, alignment: .center)
              } else if !model.hasLoadedTranscript {
                ChatTranscriptLoadingSkeleton(horizontalPadding: 0)
                  .containerRelativeFrame(.vertical, alignment: .bottom)
              }
            }
            if model.hasMoreBefore {
              Button {
                Task { await model.loadOlder() }
              } label: {
                if model.isLoadingOlder {
                  ProgressView()
                    .controlSize(.small)
                } else {
                  Text("Load earlier messages")
                    .underline()
                }
              }
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
              .frame(maxWidth: .infinity)
              .padding(.bottom, NoemaSpacing.md)
              .disabled(model.isLoadingOlder)
            }

            let rows = timelineRows
            let avatarAnchor = latestAssistantAvatarAnchor(in: rows)
            let avatarMotion = conversationAvatarMotion
            ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
              let previous = index > 0 ? rows[index - 1] : nil
              let next = index + 1 < rows.count ? rows[index + 1] : nil
              let lane = row.lane
              let ownsActiveAvatar = index == avatarAnchor
              let showAvatar = previous?.lane != lane || ownsActiveAvatar
              let group = bubbleGroup(row: row, previous: previous, next: next)
              timelineContent(
                row,
                lane: lane,
                showAvatar: showAvatar,
                group: group,
                avatarActivity: ownsActiveAvatar ? avatarMotion.activity : .idle,
                avatarAnimated: ownsActiveAvatar && avatarMotion.animated
              )
                .padding(.top, index == 0 ? 0 : rowSpacing(row: row, previous: previous))
                .id(row.id)
            }

            Color.clear
              .frame(height: NoemaSpacing.md)
              .id("chat-bottom")
          }
          .padding(.top, NoemaSpacing.xxl + NoemaSpacing.xl + NoemaSpacing.compact)
        }
      }
      .defaultScrollAnchor(.bottom)
      .scrollDismissesKeyboard(.interactively)
      .onScrollGeometryChange(for: ChatScrollGeometry.self) { geometry in
        let bottomDistance = geometry.contentSize.height
          - (geometry.contentOffset.y + geometry.containerSize.height)
        return ChatScrollGeometry(
          isAtBottom: bottomDistance <= NoemaSpacing.sm,
          containerHeight: geometry.containerSize.height,
          contentHeight: geometry.contentSize.height
        )
      } action: { oldGeometry, newGeometry in
        if newGeometry.isAtBottom {
          followBottom = true
        } else if followBottom,
                  oldGeometry.containerHeight != newGeometry.containerHeight
                    || oldGeometry.contentHeight != newGeometry.contentHeight {
          proxy.scrollTo("chat-bottom", anchor: .bottom)
        } else {
          followBottom = false
        }
      }
      .overlay(alignment: .top) {
        LinearGradient(
          colors: [NoemaColor.surface, NoemaColor.surface.opacity(0)],
          startPoint: .top,
          endPoint: .bottom
        )
        .frame(height: 56)
        .allowsHitTesting(false)
      }
      .task {
        guard !model.messages.isEmpty else { return }
        await Task.yield()
        proxy.scrollTo("chat-bottom", anchor: .bottom)
      }
      .onChange(of: chatTranscriptFollowKey) { _, _ in
        let sentMessage = model.messages.last.map { message in
          if case .user = message.kind { return message.isOptimistic }
          return false
        } ?? false
        guard followBottom || sentMessage else { return }
        followBottom = true
        withAnimation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion)) {
          proxy.scrollTo("chat-bottom", anchor: .bottom)
        }
      }
      .onChange(of: scrollToBottomRequest) { _, _ in
        withAnimation(NoemaMotion.animation(NoemaSpring.standard, reduceMotion: reduceMotion)) {
          proxy.scrollTo("chat-bottom", anchor: .bottom)
        }
      }
    }
    .safeAreaInset(edge: .bottom, spacing: 0) {
      VStack(spacing: NoemaSpacing.xs) {
        if model.isOffline {
          NoemaInlineState(
            message: "Offline — transcript remains available; sending is paused.",
            symbol: "wifi.slash",
            tone: .warning
          )
          .frame(maxWidth: 760)
        }
        if !model.interventions.isEmpty {
          ChatInterventionsView(model: model)
        }
        if !notificationPromptDismissed,
           notifications.chatPromptVisible,
           !notificationPromptBlockedByActiveTurn,
           model.interventions.isEmpty {
          ClientNotificationPrompt(notifications: notifications) {
            notifications.dismissPrompt()
            notificationPromptDismissed = true
          }
        }
        ChatComposer(
          model: model,
          autoFocus: shellCoordinator.activeDestination == .chat
            && !shellCoordinator.chatFocusDeferred
        )
          .frame(maxWidth: horizontalSizeClass == .compact ? .infinity : 760, alignment: .trailing)
      }
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.bottom, horizontalSizeClass == .compact ? 0 : NoemaSpacing.sm)
      .frame(maxWidth: .infinity)
      .overlay(alignment: .top) {
        if !followBottom && !model.messages.isEmpty {
          Button {
            followBottom = true
            scrollToBottomRequest += 1
          } label: {
            Image(systemName: "arrow.down")
              .font(.system(size: 16, weight: .regular))
              .foregroundStyle(NoemaColor.content)
              .frame(width: 32, height: 32)
              .background(NoemaColor.surface, in: Circle())
              .overlay {
                Circle()
                  .stroke(NoemaColor.separator, lineWidth: 1)
              }
          }
          .buttonStyle(.plain)
          .accessibilityLabel("Scroll to end")
          .offset(y: -(32 + NoemaSpacing.sm + 2))
        }
      }
    }
  }

  private var notificationPromptBlockedByActiveTurn: Bool {
    let normalized = model.agentStatus.replacingOccurrences(of: "_", with: "").lowercased()
    return model.isSending
      || ["inputreceived", "thinking", "toolrunning", "waitingforpreviousturncompletion", "interrupting"]
        .contains(normalized)
  }

  private var chatTranscriptFollowKey: String {
    guard let last = model.messages.last else { return "empty" }
    let streamedText: String
    if case let .assistant(text, _) = last.kind { streamedText = text } else { streamedText = "" }
    return "\(model.messages.count):\(last.id):\(streamedText)"
  }

  @ViewBuilder
  private var taskDetail: some View {
    if let selectedTaskID {
      ChatTaskDetailSheet(client: model.client, profile: model.profile, taskID: selectedTaskID)
    }
  }

  private var taskDetailPresented: Binding<Bool> {
    Binding(
      get: { selectedTaskID != nil },
      set: { if !$0 { selectedTaskID = nil } }
    )
  }

  @ViewBuilder
  private func timelineContent(
    _ row: TimelineRow,
    lane: ChatLane,
    showAvatar: Bool,
    group: ChatBubbleGroup,
    avatarActivity: NoemaAvatarActivity,
    avatarAnimated: Bool
  ) -> some View {
    switch row {
    case let .message(message, taskIDs):
      ChatMessageView(
        client: model.client,
        message: message,
        attachedTaskIDs: taskIDs,
        group: group,
        showAvatar: showAvatar,
        avatarActivity: avatarActivity,
        avatarAnimated: avatarAnimated,
        submittedChoiceIDs: submittedChoices(for: message),
        disabled: model.isSending || model.isOffline,
        onChoice: { promptID, optionIDs in
          Task { await model.choose(promptItemID: promptID, optionIDs: optionIDs) }
        },
        onA2UI: { surface, componentID, actionName, context, dataModel in
          Task {
            await model.submitA2UI(
              surface,
              componentID: componentID,
              actionName: actionName,
              context: context,
              dataModel: dataModel
            )
          }
        },
        onArtifact: { artifact in
          guard let versionID = artifact.versionID else { return }
          selectedArtifact = ArtifactSelection(versionID: versionID, title: artifact.title)
        },
        onTask: { selectedTaskID = $0 }
      )
    case let .task(_, taskID):
      ChatLaneRow(
        lane: .assistant,
        showAvatar: showAvatar,
        avatarActivity: avatarActivity,
        avatarAnimated: avatarAnimated,
        compactContentInset: 0
      ) {
        TaskReferenceChip(client: model.client, taskID: taskID, onOpen: { selectedTaskID = $0 })
      }
    case let .toolMarkers(_, messages):
      ChatLaneRow(
        lane: .assistant,
        showAvatar: showAvatar,
        avatarActivity: avatarActivity,
        avatarAnimated: avatarAnimated,
        compactContentInset: 0
      ) {
        ToolMarkerView(client: model.client, messages: messages)
      }
    case let .activity(message):
      ChatLaneRow(
        lane: .assistant,
        showAvatar: showAvatar,
        avatarActivity: avatarActivity,
        avatarAnimated: avatarAnimated
      ) {
        ActivityRowView(message: message)
      }
    case let .systemNotice(message):
      SystemNoticeView(message: message)
        .padding(.horizontal, NoemaSpacing.xs)
    case .typing:
      ChatLaneRow(
        lane: .assistant,
        showAvatar: true,
        avatarActivity: avatarActivity,
        avatarAnimated: avatarAnimated
      ) {
        ChatBubbleView(lane: .assistant, group: .single) {
          TypingDotsView()
        }
      }
    }
  }

  private func latestAssistantAvatarAnchor(in rows: [TimelineRow]) -> Int? {
    guard let humanIndex = rows.lastIndex(where: { $0.lane == .human }) else { return nil }
    return rows.indices.first { index in
      guard index > humanIndex else { return false }
      switch rows[index] {
      case .systemNotice: return false
      default: return rows[index].lane == .assistant
      }
    }
  }

  private var conversationAvatarMotion: (activity: NoemaAvatarActivity, animated: Bool) {
    let hasLiveStream = model.messages.contains { message in
      if case let .assistant(_, streaming) = message.kind { return streaming }
      return false
    }
    if hasLiveStream { return (.idle, true) }
    let activity = noemaAvatarActivity(agentStatus: model.agentStatus)
    return (activity, activity != .idle)
  }

  private func bubbleGroup(row: TimelineRow, previous: TimelineRow?, next: TimelineRow?) -> ChatBubbleGroup {
    guard case let .message(message, _) = row, message.kind.isBubble else { return .single }
    let samePrevious = previous?.isBubbleLane == row.lane && sameBubbleTurn(previous, row)
    let sameNext = next?.isBubbleLane == row.lane && sameBubbleTurn(row, next)
    switch (samePrevious, sameNext) {
    case (false, false): return .single
    case (false, true): return .first
    case (true, true): return .middle
    case (true, false): return .last
    }
  }

  private func sameBubbleTurn(_ left: TimelineRow?, _ right: TimelineRow?) -> Bool {
    guard case let .message(leftMessage, _) = left,
          case let .message(rightMessage, _) = right,
          let leftTurnID = leftMessage.turnID,
          let rightTurnID = rightMessage.turnID else { return false }
    return leftTurnID == rightTurnID
  }

  private func rowSpacing(row: TimelineRow, previous: TimelineRow?) -> CGFloat {
    guard let previous, previous.lane == row.lane else { return NoemaSpacing.md }
    if row.isTaskReference || previous.isTaskReference { return NoemaSpacing.md }
    if row.isToolMarker || previous.isToolMarker { return NoemaSpacing.xs }
    if row.isBubble && previous.isBubble {
      return sameBubbleTurn(previous, row) ? NoemaSpacing.xs : NoemaSpacing.md
    }
    return NoemaSpacing.sm
  }

  private struct TaskReferenceProjection {
    var attachedTaskIDsByAssistantID: [String: [String]] = [:]
    var consumedTaskIDs = Set<String>()
    var hiddenTaskIDs = Set<String>()
  }

  private var taskReferenceProjection: TaskReferenceProjection {
    var projection = TaskReferenceProjection()
    var referencesByNotificationID: [String: (messageID: String, taskID: String)] = [:]
    var assistantByNotificationID: [String: String] = [:]

    for message in model.messages {
      guard let notificationID = transcriptMetadataString(message, key: "notification_id") else { continue }
      switch message.kind {
      case let .task(taskID):
        if transcriptMetadataString(message, key: "notification_kind") != "task_created" {
          referencesByNotificationID[notificationID] = (message.id, taskID)
          projection.hiddenTaskIDs.insert(message.id)
        }
      case .assistant:
        if transcriptMetadataString(message, key: "source") == "work_notification" {
          assistantByNotificationID[notificationID] = message.id
        }
      default:
        break
      }
    }

    for message in model.messages {
      guard case .assistant = message.kind,
            let notificationID = transcriptMetadataString(message, key: "notification_id"),
            assistantByNotificationID[notificationID] == message.id,
            let reference = referencesByNotificationID[notificationID] else { continue }
      projection.attachedTaskIDsByAssistantID[message.id, default: []].append(reference.taskID)
      projection.consumedTaskIDs.insert(reference.messageID)
    }

    var previousVisibleMessage: ChatMessage?
    for message in model.messages {
      if projection.hiddenTaskIDs.contains(message.id) || projection.consumedTaskIDs.contains(message.id) {
        continue
      }
      guard case let .task(taskID) = message.kind,
            transcriptMetadataString(message, key: "notification_kind") == "task_created",
            let turnID = message.turnID,
            let previous = previousVisibleMessage,
            case .assistant = previous.kind,
            previous.turnID == turnID else {
        previousVisibleMessage = message
        continue
      }
      projection.attachedTaskIDsByAssistantID[previous.id, default: []].append(taskID)
      projection.consumedTaskIDs.insert(message.id)
    }

    return projection
  }

  private func transcriptMetadataString(_ message: ChatMessage, key: String) -> String? {
    guard let metadata = message.metadata,
          let data = metadata.data(using: .utf8),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let value = object[key] as? String,
          !value.isEmpty else { return nil }
    return value
  }

  private func submittedChoices(for message: ChatMessage) -> Set<String> {
    guard case .choicePrompt = message.kind else { return [] }
    let promptID = message.id
    for candidate in model.messages {
      if case let .choiceSelection(selectionPromptID, _, options) = candidate.kind,
         selectionPromptID == promptID {
        return Set(options.map(\.id))
      }
    }
    return []
  }
}

enum ChatLane: Equatable {
  case human
  case assistant
}

enum ChatBubbleGroup {
  case single
  case first
  case middle
  case last
}

private extension ChatMessageKind {
  var isBubble: Bool {
    switch self {
    case .user, .assistant, .choicePrompt, .choiceSelection:
      return true
    default:
      return false
    }
  }

  var lane: ChatLane {
    switch self {
    case .user, .choiceSelection:
      return .human
    default:
      return .assistant
    }
  }
}

private extension ChatReadyView.TimelineRow {
  var lane: ChatLane {
    switch self {
    case let .message(message, _), let .activity(message), let .systemNotice(message): return message.kind.lane
    case .task, .toolMarkers, .typing: return .assistant
    }
  }

  var isBubble: Bool {
    if case let .message(message, _) = self { return message.kind.isBubble }
    return false
  }

  var isToolMarker: Bool {
    if case .toolMarkers = self { return true }
    return false
  }

  var isTaskReference: Bool {
    if case .task = self { return true }
    return false
  }

  var isBubbleLane: ChatLane? {
    guard isBubble else { return nil }
    return lane
  }
}
