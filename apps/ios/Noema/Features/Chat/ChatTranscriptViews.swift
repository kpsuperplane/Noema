import Foundation
import Apollo
import MarkdownUI
import SwiftUI

struct ChatLaneRow<Content: View>: View {
  let lane: ChatLane
  let showAvatar: Bool
  let compactContentInset: CGFloat
  let content: Content
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  init(
    lane: ChatLane,
    showAvatar: Bool,
    compactContentInset: CGFloat = NoemaSpacing.sm,
    @ViewBuilder content: () -> Content
  ) {
    self.lane = lane
    self.showAvatar = showAvatar
    self.compactContentInset = compactContentInset
    self.content = content()
  }

  var body: some View {
    Group {
      if horizontalSizeClass == .compact {
        CompactChatLaneLayout(lane: lane) {
          content
            .padding(lane == .assistant ? .leading : .trailing, compactContentInset)
        }
      } else {
        HStack(alignment: .bottom, spacing: NoemaSpacing.sm) {
          if lane == .assistant {
            ChatAvatarView(lane: lane, visible: showAvatar)
            content
              .frame(maxWidth: .infinity, alignment: .leading)
            Spacer(minLength: NoemaSpacing.xxl)
          } else {
            Spacer(minLength: NoemaSpacing.xxl)
            content
              .frame(maxWidth: 520, alignment: .trailing)
            ChatAvatarView(lane: lane, visible: showAvatar)
          }
        }
      }
    }
    .frame(maxWidth: .infinity, alignment: lane == .human ? .trailing : .leading)
  }
}

private struct CompactChatLaneLayout: Layout {
  let lane: ChatLane

  func sizeThatFits(
    proposal: ProposedViewSize,
    subviews: Subviews,
    cache: inout ()
  ) -> CGSize {
    guard let subview = subviews.first else { return .zero }
    // The compact web transcript caps both message lanes at 80% of the track.
    let widthFraction: CGFloat = 0.8
    let availableWidth = proposal.width ?? subview.sizeThatFits(.unspecified).width / widthFraction
    let width = min(subview.sizeThatFits(.unspecified).width, availableWidth * widthFraction)
    let contentSize = subview.sizeThatFits(ProposedViewSize(width: width, height: proposal.height))
    return CGSize(width: availableWidth, height: contentSize.height)
  }

  func placeSubviews(
    in bounds: CGRect,
    proposal: ProposedViewSize,
    subviews: Subviews,
    cache: inout ()
  ) {
    guard let subview = subviews.first else { return }
    let widthFraction: CGFloat = 0.8
    let width = min(subview.sizeThatFits(.unspecified).width, bounds.width * widthFraction)
    let contentProposal = ProposedViewSize(width: width, height: bounds.height)
    let contentSize = subview.sizeThatFits(contentProposal)
    let x = lane == .human ? bounds.maxX - contentSize.width : bounds.minX
    subview.place(
      at: CGPoint(x: x, y: bounds.minY),
      anchor: .topLeading,
      proposal: ProposedViewSize(contentSize)
    )
  }
}

struct ChatAvatarView: View {
  let lane: ChatLane
  let visible: Bool

  var body: some View {
    Image(systemName: lane == .assistant ? "sparkles" : "person.fill")
      .font(NoemaFont.metadata.weight(.semibold))
      .foregroundStyle(lane == .assistant ? NoemaColor.pine700 : NoemaColor.contentSecondary)
      .frame(width: 28, height: 28)
      .background(lane == .assistant ? NoemaColor.pine100 : NoemaColor.paper200, in: Circle())
      .opacity(visible ? 1 : 0)
      .accessibilityHidden(!visible)
  }
}

struct ChatMessageView: View {
  let client: ApolloClient?
  let message: ChatMessage
  let attachedTaskIDs: [String]
  let group: ChatBubbleGroup
  let showAvatar: Bool
  let submittedChoiceIDs: Set<String>
  let disabled: Bool
  let onChoice: (String, [String]) -> Void
  let onA2UI: (A2UISurfaceModel, String, String, Any?, Any?) -> Void
  let onArtifact: (ArtifactReferenceModel) -> Void
  let onTask: (String) -> Void
  @State private var selectedChoices: Set<String> = []
  @State private var runtimeDebug: RuntimeDebugUsage?

  var body: some View {
    messageContent
      .contextMenu {
        if let usage = runtimeDebugUsage(for: message) {
          Button("Debug", systemImage: "chart.xyaxis.line") { runtimeDebug = usage }
        }
      }
      .sheet(item: $runtimeDebug) { usage in
        RuntimeDebugSheet(usage: usage)
      }
  }

  @ViewBuilder
  private var messageContent: some View {
    switch message.kind {
    case let .user(text):
      ChatLaneRow(lane: .human, showAvatar: showAvatar) {
        ChatBubbleView(lane: .human, group: group) {
          ChatMarkdownText(text: text, color: NoemaColor.white)
        }
      }
    case let .assistant(text, streaming):
      let minimumContentWidth: CGFloat? = !attachedTaskIDs.isEmpty && text.count > 30 ? 244 : nil
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        ChatBubbleView(lane: .assistant, group: group) {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            ChatMarkdownText(text: text, color: NoemaColor.content)
            ForEach(attachedTaskIDs, id: \.self) { taskID in
              TaskReferenceChip(client: client, taskID: taskID, onOpen: onTask)
            }
            if streaming {
              TypingDotsView()
                .padding(.top, NoemaSpacing.xs)
            }
          }
          .padding(.vertical, attachedTaskIDs.isEmpty ? 10 : 5)
          .frame(minWidth: minimumContentWidth, alignment: .leading)
        }
      }
    case let .activity(title, summary, status, metadata, activityKind):
      if isSystemNotice(message) {
        SystemNoticeView(message: message)
      } else {
        ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
          ActivityRowView(message: ChatMessage(
            id: message.id,
            cursor: message.cursor,
            turnID: message.turnID,
            clientMessageID: message.clientMessageID,
            kind: .activity(title: title, summary: summary, status: status, metadata: metadata, activityKind: activityKind),
            isOptimistic: message.isOptimistic
          ))
        }
      }
    case let .a2ui(surface):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        A2UISurfaceView(surface: surface, disabled: disabled) { componentID, actionName, context, dataModel in
          onA2UI(surface, componentID, actionName, context, dataModel)
        }
      }
    case let .choicePrompt(prompt, mode, options):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        ChoicePromptView(
          prompt: prompt,
          mode: mode,
          options: options,
          selection: $selectedChoices,
          submittedSelection: submittedChoiceIDs,
          disabled: disabled,
          group: group,
          submit: { ids in onChoice(message.id, ids) }
        )
      }
    case let .choiceSelection(_, _, options):
      ChatLaneRow(lane: .human, showAvatar: showAvatar) {
        ChatBubbleView(
          text: options.map(\.label).joined(separator: ", "),
          lane: .human,
          group: .single
        )
      }
    case let .error(message, recoverable):
      SystemNoticeView(
        text: message,
        symbol: recoverable ? "exclamationmark.triangle" : "xmark.octagon",
        tone: .error
      )
    case let .artifact(reference):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        ArtifactReferenceView(reference: reference, onOpen: { onArtifact(reference) })
      }
    case let .task(taskID):
      ChatLaneRow(lane: .assistant, showAvatar: showAvatar) {
        TaskReferenceChip(client: client, taskID: taskID, onOpen: onTask)
      }
      }
  }
}

private struct ChatMarkdownText: View {
  let text: String
  let color: Color

  var body: some View {
    Markdown(text)
      .frame(alignment: .leading)
      .markdownTextStyle {
        FontFamily(.custom("Hanken Grotesk"))
        FontSize(14)
        ForegroundColor(color)
      }
      .markdownTextStyle(\.link) {
        FontWeight(.semibold)
        ForegroundColor(color)
      }
      .markdownBlockStyle(\.paragraph) { configuration in
        configuration.label
          .markdownMargin(top: 0, bottom: 0)
      }
  }
}

struct ChatBubbleView<Content: View>: View {
  let lane: ChatLane
  let group: ChatBubbleGroup
  let text: String?
  let content: Content?

  init(text: String, lane: ChatLane, group: ChatBubbleGroup) where Content == EmptyView {
    self.lane = lane
    self.group = group
    self.text = text
    self.content = nil
  }

  init(lane: ChatLane, group: ChatBubbleGroup, @ViewBuilder content: () -> Content) {
    self.lane = lane
    self.group = group
    self.text = nil
    self.content = content()
  }

  var body: some View {
    ViewThatFits(in: .horizontal) {
      bubbleContent
        .fixedSize(horizontal: true, vertical: false)
      bubbleContent
    }
    .font(NoemaFont.body)
    .foregroundStyle(lane == .human ? NoemaColor.white : NoemaColor.content)
    .lineSpacing(NoemaSpacing.compact)
    .padding(.horizontal, NoemaSpacing.lg)
    .padding(.vertical, NoemaSpacing.sm)
    .frame(minHeight: 40, alignment: .center)
    .background(lane == .human ? NoemaColor.pine500 : NoemaColor.paper100, in: bubbleShape)
    .clipShape(bubbleShape)
  }

  @ViewBuilder
  private var bubbleContent: some View {
    if let text {
      Text(text)
        .multilineTextAlignment(lane == .human ? .trailing : .leading)
    } else if let content {
      content
    }
  }

  private var bubbleShape: UnevenRoundedRectangle {
    let outer: CGFloat = 28
    let inner = NoemaRadius.inner
    let radii: (CGFloat, CGFloat, CGFloat, CGFloat)
    switch group {
    case .single: radii = (outer, outer, outer, outer)
    case .first: radii = (outer, outer, inner, inner)
    case .middle: radii = (inner, inner, inner, inner)
    case .last: radii = (inner, inner, outer, outer)
    }
    return UnevenRoundedRectangle(
      topLeadingRadius: radii.0,
      bottomLeadingRadius: radii.2,
      bottomTrailingRadius: radii.3,
      topTrailingRadius: radii.1,
      style: .continuous
    )
  }
}

struct TypingDotsView: View {
  var body: some View {
    HStack(spacing: NoemaSpacing.xs) {
      ForEach(0..<3, id: \.self) { index in
        Circle()
          .fill(NoemaColor.contentTertiary)
          .frame(width: 5, height: 5)
          .opacity(index == 1 ? 0.72 : 0.48)
      }
    }
    .accessibilityLabel("Noema is typing")
  }
}

struct ToolMarkerView: View {
  let messages: [ChatMessage]
  @State private var expanded = false
  @State private var runtimeDebug: RuntimeDebugUsage?

  var body: some View {
    VStack(alignment: .leading, spacing: expanded ? NoemaSpacing.xs : 0) {
      Button {
        guard expandable else { return }
        expanded.toggle()
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          ToolStatusIcon(status: markerStatus)
          ToolTypeIcon(name: markerName)
          Text(messages.count > 1 && expanded ? String(messages.count) + " tool calls" : markerName)
            .font(NoemaFont.mono)
            .foregroundStyle(NoemaColor.contentSecondary)
            .lineLimit(2)
            .multilineTextAlignment(.leading)
          Spacer(minLength: NoemaSpacing.xs)
          if expandable {
            Image(systemName: expanded ? "chevron.up" : "chevron.down")
              .font(NoemaFont.metadata.weight(.semibold))
              .foregroundStyle(NoemaColor.contentTertiary)
          }
        }
        .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .disabled(!expandable)

      if expanded {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          ForEach(messages) { message in
            ToolMarkerDetailView(message: message)
          }
        }
        .padding(.leading, NoemaSpacing.xl)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .contextMenu {
      if let usage = runtimeDebugUsage(from: messages) {
        Button("Debug", systemImage: "chart.xyaxis.line") { runtimeDebug = usage }
      }
    }
    .sheet(item: $runtimeDebug) { usage in
      RuntimeDebugSheet(usage: usage)
    }
  }

  private var markerName: String {
    for message in messages {
      let metadata = metadataObject(for: message)
      if let name = nestedString(metadata, path: ["display", "name"]) ??
          nestedString(metadata, path: ["action", "name"]) ??
          stringValue(metadata["name"]) ??
          stringValue(metadata["tool_name"]) {
        return humanizeToolName(name)
      }
      if let query = stringValue(metadata["query"]) {
        return "Search " + query
      }
      if let url = stringValue(metadata["url"]) {
        return url.replacingOccurrences(of: "https://", with: "")
      }
    }
    return messages.first.flatMap { activityValues($0)?.title } ?? "Tool activity"
  }

  private var markerStatus: ToolMarkerStatus {
    if messages.contains(where: { activityValues($0)?.status.uppercased() == "FAILED" }) { return .error }
    if messages.contains(where: { activityValues($0)?.activityKind.normalizedActivityKind == "TOOL_RESULT" }) { return .complete }
    if messages.contains(where: { activityValues($0)?.status.uppercased() == "STARTED" }) { return .running }
    return .pending
  }

  private var expandable: Bool { messages.contains { toolDetail(for: $0) != nil } }
}

struct ToolMarkerDetailView: View {
  let message: ChatMessage

  var body: some View {
    if let detail = toolDetail(for: message) {
      Text(detail)
        .font(NoemaFont.monoTiny)
        .foregroundStyle(NoemaColor.contentSecondary)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
  }
}

enum ToolMarkerStatus {
  case pending
  case running
  case complete
  case error
}

struct ToolStatusIcon: View {
  let status: ToolMarkerStatus

  var body: some View {
    Image(systemName: symbol)
      .font(NoemaFont.metadata.weight(.semibold))
      .foregroundStyle(color)
      .frame(width: 16, height: 16)
      .accessibilityHidden(true)
  }

  private var symbol: String {
    switch status {
    case .pending: "clock"
    case .running: "arrow.triangle.2.circlepath"
    case .complete: "checkmark"
    case .error: "xmark"
    }
  }

  private var color: Color {
    switch status {
    case .pending: NoemaColor.contentTertiary
    case .running: NoemaColor.success
    case .complete: NoemaColor.success
    case .error: NoemaColor.danger
    }
  }
}

struct ToolTypeIcon: View {
  let name: String

  var body: some View {
    Group {
      if name == "web.search" {
        Image(systemName: "magnifyingglass")
      } else if name == "web.fetch" {
        Image(systemName: "globe")
      } else {
        Image(systemName: "wrench.and.screwdriver")
      }
    }
    .font(NoemaFont.metadata)
    .foregroundStyle(NoemaColor.contentTertiary)
    .frame(width: 16, height: 16)
    .accessibilityHidden(true)
  }
}

struct ActivityRowView: View {
  let message: ChatMessage
  @State private var expanded = false

  var body: some View {
    NoemaCard(padding: NoemaSpacing.md) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Button {
          guard detail != nil else { return }
          expanded.toggle()
        } label: {
          HStack(alignment: .top, spacing: NoemaSpacing.sm) {
            Image(systemName: "sparkles")
              .foregroundStyle(NoemaColor.accent)
              .frame(width: 18)
            VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
              Text(title)
                .font(NoemaFont.captionEmphasized)
                .foregroundStyle(NoemaColor.content)
                .multilineTextAlignment(.leading)
              if let summary {
                Text(summary)
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                  .multilineTextAlignment(.leading)
              }
            }
            Spacer(minLength: NoemaSpacing.sm)
            HStack(spacing: NoemaSpacing.xs) {
              NoemaStatusToken(text: statusLabel(status), tone: statusTone(status))
              if detail != nil {
                Image(systemName: expanded ? "chevron.up" : "chevron.down")
                  .font(NoemaFont.metadata.weight(.semibold))
                  .foregroundStyle(NoemaColor.contentTertiary)
              }
            }
          }
          .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(detail == nil)

        if expanded, let detail {
          NoemaDivider()
          Text(detail)
            .font(NoemaFont.monoTiny)
            .foregroundStyle(NoemaColor.contentSecondary)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
      }
    }
  }

  private var values: ActivityValues? { activityValues(message) }
  private var title: String { values?.title ?? "Activity" }
  private var summary: String? { values?.summary }
  private var status: String { values?.status ?? "UNKNOWN" }
  private var detail: String? { activityDetail(for: message) }
}

struct SystemNoticeView: View {
  let message: ChatMessage?
  let textValue: String?
  let symbol: String
  let tone: NoemaStatusToken.Tone?

  init(message: ChatMessage) {
    self.message = message
    textValue = nil
    symbol = "sparkles"
    tone = nil
  }

  init(text: String, symbol: String = "info.circle", tone: NoemaStatusToken.Tone = .neutral) {
    message = nil
    textValue = text
    self.symbol = symbol
    self.tone = tone
  }

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(minWidth: NoemaSpacing.lg, maxWidth: .infinity, maxHeight: 1)
      HStack(spacing: NoemaSpacing.xs) {
        Image(systemName: symbol)
          .font(NoemaFont.metadata.weight(.semibold))
        Text(displayText)
          .multilineTextAlignment(.center)
          .fixedSize(horizontal: false, vertical: true)
      }
      .font(NoemaFont.caption)
      .foregroundStyle(foreground)
      Rectangle()
        .fill(NoemaColor.separatorSubtle)
        .frame(minWidth: NoemaSpacing.lg, maxWidth: .infinity, maxHeight: 1)
    }
    .frame(maxWidth: .infinity)
    .padding(.vertical, NoemaSpacing.xs)
  }

  private var displayText: String {
    if let textValue { return textValue }
    let values = activityValues(message)
    return values?.summary ?? values?.title ?? "System update"
  }

  private var foreground: Color {
    switch tone ?? activityTone(message) {
    case .neutral: NoemaColor.contentSecondary
    case .success: NoemaColor.success
    case .warning: NoemaColor.warning
    case .error: NoemaColor.danger
    }
  }
}

struct ChoicePromptView: View {
  let prompt: String
  let mode: String
  let options: [ChoiceOption]
  @Binding var selection: Set<String>
  let submittedSelection: Set<String>
  let disabled: Bool
  let group: ChatBubbleGroup
  let submit: ([String]) -> Void
  @State private var submittedLocally = false

  private var allowsMultiple: Bool {
    mode.normalizedActivityKind == "PICK_MANY"
  }

  private var isDisabled: Bool {
    disabled || submittedLocally || !submittedSelection.isEmpty
  }

  var body: some View {
    ChatBubbleView(lane: .assistant, group: group) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        Text(prompt)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
        VStack(spacing: NoemaSpacing.xs) {
          ForEach(options) { option in
            let selected = (submittedSelection.isEmpty ? selection : submittedSelection).contains(option.id)
            Button {
              guard !isDisabled else { return }
              if allowsMultiple {
                if selection.contains(option.id) {
                  selection.remove(option.id)
                } else {
                  selection.insert(option.id)
                }
              } else {
                selection = [option.id]
                submittedLocally = true
                submit([option.id])
              }
            } label: {
              HStack(spacing: NoemaSpacing.sm) {
                Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                  .font(NoemaFont.bodyEmphasized)
                  .foregroundStyle(selected ? NoemaColor.accent : NoemaColor.contentTertiary)
                Text(option.label)
                  .font(NoemaFont.body)
                  .foregroundStyle(NoemaColor.content)
                  .multilineTextAlignment(.leading)
                Spacer(minLength: 0)
              }
              .padding(.horizontal, NoemaSpacing.sm)
              .padding(.vertical, NoemaSpacing.compact)
              .frame(maxWidth: .infinity, minHeight: 36, alignment: .leading)
              .background(
                selected ? NoemaColor.pine50 : NoemaColor.surface,
                in: RoundedRectangle(cornerRadius: NoemaRadius.element)
              )
              .overlay {
                RoundedRectangle(cornerRadius: NoemaRadius.element)
                  .stroke(selected ? NoemaColor.accent.opacity(0.45) : NoemaColor.separatorSubtle, lineWidth: 1)
              }
            }
            .buttonStyle(.plain)
            .disabled(isDisabled)
          }
        }
        if allowsMultiple {
          HStack {
            Spacer(minLength: 0)
            Button("Done") {
              let ids = options.filter { selection.contains($0.id) }.map(\.id)
              guard !ids.isEmpty else { return }
              submittedLocally = true
              submit(ids)
            }
            .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
            .disabled(isDisabled || selection.isEmpty)
          }
        }
      }
    }
  }
}
