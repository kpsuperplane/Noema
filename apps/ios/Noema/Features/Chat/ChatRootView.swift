import Foundation
import MarkdownUI
import SwiftUI

struct ChatRootView: View {
  let model: NoemaAppModel
  @State private var chat: ChatModel
  @Environment(NoemaShellCoordinator.self) private var coordinator

  init(model: NoemaAppModel) {
    self.model = model
    _chat = State(initialValue: ChatModel(client: model.graphQLClient?.client, profile: model.profile))
  }

  var body: some View {
    Group {
      switch chat.phase {
      case .loading:
        ProgressView("Loading chat…")
      case .onboarding:
        if let onboarding = chat.onboarding {
          OnboardingRootView(model: onboarding) { Task { await chat.onboardingCompleted() } }
        } else {
          ChatFailureView(message: "No onboarding connection is available.", retry: { Task { await chat.retry() } })
        }
      case .ready:
        ChatReadyView(model: chat)
      case let .failed(message):
        ChatFailureView(message: message, retry: { Task { await chat.retry() } })
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(NoemaColor.surface)
    .task { await chat.start() }
    .onAppear {
      coordinator.clearSecondary()
      syncAgentLabel()
    }
    .onChange(of: chat.primaryAgentDisplayName) { _, _ in syncAgentLabel() }
    .onChange(of: model.recoveryGeneration) { _, _ in
      Task { await chat.recoverConnection() }
    }
  }

  private func syncAgentLabel() {
    coordinator.primaryAgentLabel = chat.primaryAgentDisplayName?.isEmpty == false
      ? chat.primaryAgentDisplayName!
      : "Chat"
  }
}

struct ChatFailureView: View {
  let message: String
  let retry: () -> Void

  var body: some View {
    ContentUnavailableView {
      Label("Chat unavailable", systemImage: "bubble.left.and.exclamationmark.bubble.right")
    } description: {
      Text(message)
    } actions: {
      Button("Try again", action: retry)
        .buttonStyle(.borderedProminent)
    }
  }
}

struct ChatReadyView: View {
  @Bindable var model: ChatModel
  @State private var followBottom = true
  @State private var selectedArtifact: ArtifactSelection?
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  fileprivate enum TimelineRow: Identifiable {
    case message(ChatMessage, taskIDs: [String])
    case task(id: String, taskID: String)
    case toolMarkers(id: String, messages: [ChatMessage])
    case activity(ChatMessage)
    case systemNotice(ChatMessage)

    var id: String {
      switch self {
      case let .message(message, _): message.id
      case let .task(id, _): id
      case let .toolMarkers(id, _): "tool-" + id
      case let .activity(message), let .systemNotice(message): message.id
      }
    }
  }

  private var timelineRows: [TimelineRow] {
    let taskProjection = taskReferenceProjection
    let visibleMessages = latestA2UISurfaces(model.messages)
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
              sameToolTurn(markers.last, visibleMessages[index]) {
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
    return rows
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
    ScrollViewReader { proxy in
      ScrollView {
        NoemaPageTrack(maxWidth: 760, horizontalPadding: 22) {
          LazyVStack(alignment: .leading, spacing: 0) {
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
            ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
              let previous = index > 0 ? rows[index - 1] : nil
              let next = index + 1 < rows.count ? rows[index + 1] : nil
              let lane = row.lane
              let showAvatar = previous?.lane != lane
              let group = bubbleGroup(row: row, previous: previous, next: next)
              timelineContent(row, lane: lane, showAvatar: showAvatar, group: group)
                .padding(.top, index == 0 ? 0 : rowSpacing(row: row, previous: previous))
                .id(row.id)
            }

            Color.clear
              .frame(height: horizontalSizeClass == .compact ? 21 : 1)
              .id("chat-bottom")
          }
          .padding(.top, NoemaSpacing.xxl)
          .padding(.bottom, 96)
        }
      }
      .defaultScrollAnchor(.bottom)
      .scrollDismissesKeyboard(.interactively)
      .simultaneousGesture(DragGesture().onChanged { _ in followBottom = false })
      .overlay(alignment: .top) {
        LinearGradient(
          colors: [NoemaColor.surface, NoemaColor.surface.opacity(0)],
          startPoint: .top,
          endPoint: .bottom
        )
        .frame(height: 56)
        .allowsHitTesting(false)
      }
      .overlay(alignment: .bottom) {
        LinearGradient(
          colors: [NoemaColor.surface.opacity(0), NoemaColor.surface],
          startPoint: .top,
          endPoint: .bottom
        )
        .frame(height: 72)
        .allowsHitTesting(false)
      }
      .overlay(alignment: .bottomTrailing) {
        if !followBottom && !model.messages.isEmpty {
          Button {
            followBottom = true
            withAnimation(NoemaSpring.standard) { proxy.scrollTo("chat-bottom", anchor: .bottom) }
          } label: {
            Image(systemName: "arrow.down.to.line")
              .font(NoemaFont.captionEmphasized)
              .frame(width: 32, height: 32)
          }
          .buttonStyle(.glass)
          .accessibilityLabel("Latest")
          .padding(.trailing, NoemaSpacing.xl)
          .padding(.bottom, NoemaSpacing.sm)
        }
      }
      .task {
        guard !model.messages.isEmpty else { return }
        await Task.yield()
        proxy.scrollTo("chat-bottom", anchor: .bottom)
      }
      .onChange(of: model.messages.count) { _, _ in
        guard followBottom else { return }
        withAnimation(NoemaSpring.standard) { proxy.scrollTo("chat-bottom", anchor: .bottom) }
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
        ChatComposer(model: model)
          .frame(maxWidth: horizontalSizeClass == .compact ? .infinity : 760, alignment: .trailing)
          .offset(y: horizontalSizeClass == .compact ? 12 : 0)
      }
      .padding(.horizontal, NoemaSpacing.xl)
      .padding(.bottom, horizontalSizeClass == .compact ? 0 : NoemaSpacing.sm)
      .frame(maxWidth: .infinity)
      .background(NoemaColor.surface)
      .overlay(alignment: .top) {
        LinearGradient(
          colors: [NoemaColor.surface.opacity(0), NoemaColor.surface],
          startPoint: .top,
          endPoint: .bottom
        )
        .frame(height: 44)
        .offset(y: -44)
        .allowsHitTesting(false)
      }
    }
    .sheet(item: $selectedArtifact) { selection in
      ArtifactVersionSheet(model: ArtifactModel(client: model.client, profile: model.profile), selection: selection)
    }
  }

  @ViewBuilder
  private func timelineContent(
    _ row: TimelineRow,
    lane: ChatLane,
    showAvatar: Bool,
    group: ChatBubbleGroup
  ) -> some View {
    switch row {
    case let .message(message, taskIDs):
      ChatMessageView(
        client: model.client,
        message: message,
        attachedTaskIDs: taskIDs,
        group: group,
        showAvatar: showAvatar,
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
        }
      )
    case let .task(_, taskID):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar, compactContentInset: 0) {
        TaskReferenceChip(client: model.client, taskID: taskID)
      }
    case let .toolMarkers(_, messages):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        ToolMarkerView(messages: messages)
      }
    case let .activity(message):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        ActivityRowView(message: message)
      }
    case let .systemNotice(message):
      SystemNoticeView(message: message)
        .padding(.horizontal, NoemaSpacing.xs)
    }
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
    case .task, .toolMarkers: return .assistant
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
