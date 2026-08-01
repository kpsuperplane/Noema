import SwiftUI
import MarkdownUI

struct ChatRootView: View {
  let model: NoemaAppModel
  @State private var chat: ChatModel

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
    .onChange(of: model.recoveryGeneration) { _, _ in
      Task { await chat.recoverConnection() }
    }
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

  private enum TimelineRow: Identifiable {
    case message(ChatMessage)
    case activities(id: String, messages: [ChatMessage])

    var id: String {
      switch self {
      case let .message(message): message.id
      case let .activities(id, _): "activity-\(id)"
      }
    }
  }

  private var timelineRows: [TimelineRow] {
    var rows: [TimelineRow] = []
    var activityItems: [ChatMessage] = []
    for message in model.messages {
      if case .activity = message.kind {
        activityItems.append(message)
      } else {
        if let first = activityItems.first { rows.append(.activities(id: first.id, messages: activityItems)) }
        activityItems.removeAll(keepingCapacity: true)
        rows.append(.message(message))
      }
    }
    if let first = activityItems.first { rows.append(.activities(id: first.id, messages: activityItems)) }
    return rows
  }

  var body: some View {
    ScrollViewReader { proxy in
      ScrollView {
        LazyVStack(alignment: .leading, spacing: NoemaSpacing.md) {
          if model.hasMoreBefore {
            Button {
              Task { await model.loadOlder() }
            } label: {
              Label(model.isLoadingOlder ? "Loading earlier messages…" : "Load earlier messages", systemImage: "arrow.up")
                .font(NoemaFont.caption)
            }
            .disabled(model.isLoadingOlder)
            .frame(maxWidth: .infinity)
          }
          ForEach(timelineRows) { row in
            switch row {
            case let .message(message):
              ChatMessageView(message: message, onChoice: { promptID, optionIDs in
                Task { await model.choose(promptItemID: promptID, optionIDs: optionIDs) }
              }, onA2UI: { surface, componentID, actionName, dataModel in
                Task { await model.submitA2UI(surface, componentID: componentID, actionName: actionName, dataModel: dataModel) }
              }, onArtifact: { artifact in
                guard let versionID = artifact.versionID else { return }
                selectedArtifact = ArtifactSelection(versionID: versionID, title: artifact.title)
              })
              .id(row.id)
            case let .activities(id, messages):
              ActivityClusterView(items: messages.compactMap(ActivityItem.init))
                .id(id)
            }
          }
          Color.clear.frame(height: 1).id("chat-bottom")
        }
        .padding(.horizontal, NoemaSpacing.lg)
        .padding(.vertical, NoemaSpacing.md)
        .frame(maxWidth: 760)
        .frame(maxWidth: .infinity)
      }
      .scrollDismissesKeyboard(.interactively)
      .simultaneousGesture(DragGesture().onChanged { _ in followBottom = false })
      .overlay(alignment: .bottomTrailing) {
        if !followBottom && !model.messages.isEmpty {
          Button {
            followBottom = true
            withAnimation(NoemaSpring.standard) { proxy.scrollTo("chat-bottom", anchor: .bottom) }
          } label: {
            Label("Latest", systemImage: "arrow.down.to.line")
              .labelStyle(.titleAndIcon)
              .font(NoemaFont.captionEmphasized)
          }
          .buttonStyle(.glass)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.sm)
        }
      }
      .onAppear {
        guard !model.messages.isEmpty else { return }
        proxy.scrollTo("chat-bottom", anchor: .bottom)
      }
      .onChange(of: model.messages.count) { _, _ in
        guard followBottom else { return }
        withAnimation(NoemaSpring.standard) { proxy.scrollTo("chat-bottom", anchor: .bottom) }
      }
    }
    .safeAreaInset(edge: .top, spacing: 0) {
      if !model.interventions.isEmpty {
        ChatInterventionsView(model: model)
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.vertical, NoemaSpacing.sm)
          .background(.thinMaterial)
      }
    }
    .safeAreaInset(edge: .bottom, spacing: 0) {
      VStack(spacing: NoemaSpacing.xxs) {
        if model.isOffline {
          Label("Offline — transcript remains available; sending is paused.", systemImage: "wifi.slash")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
            .frame(maxWidth: 760, alignment: .leading)
        }
        ChatComposer(model: model)
          .padding(.horizontal, NoemaSpacing.md)
          .padding(.vertical, NoemaSpacing.sm)
      }
      .background(.thinMaterial)
    }
    .sheet(item: $selectedArtifact) { selection in
      ArtifactVersionSheet(model: ArtifactModel(client: model.client, profile: model.profile), selection: selection)
    }
    .navigationTitle(model.providerName)
    .navigationBarTitleDisplayMode(.inline)
  }
}

private struct ChatComposer: View {
  @Bindable var model: ChatModel

  var body: some View {
    HStack(alignment: .bottom, spacing: NoemaSpacing.sm) {
      TextField("Message Noema", text: $model.draft, axis: .vertical)
        .textFieldStyle(.roundedBorder)
        .lineLimit(1...5)
        .onSubmit { Task { await model.send() } }
      Button {
        Task { await model.send() }
      } label: {
        Image(systemName: "arrow.up.circle.fill")
          .font(.title2)
      }
      .buttonStyle(.glass)
      .disabled(model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isSending || model.isOffline)
      .accessibilityLabel("Send message")
    }
    .frame(maxWidth: 760)
    .frame(maxWidth: .infinity)
    .padding(NoemaSpacing.xs)
    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: NoemaSpacing.md))
  }
}

private struct ChatMessageView: View {
  let message: ChatMessage
  let onChoice: (String, [String]) -> Void
  let onA2UI: (A2UISurfaceModel, String, String, Any?) -> Void
  let onArtifact: (ArtifactReferenceModel) -> Void
  @State private var selectedChoices: Set<String> = []

  var body: some View {
    switch message.kind {
    case let .user(text):
      HStack { Spacer(minLength: 36); Text(text).padding(NoemaSpacing.md).background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.md)) }
    case let .assistant(text, streaming):
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Markdown(text)
        if streaming { ProgressView().controlSize(.mini) }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    case let .activity(title, summary, status, _):
      ActivityClusterView(items: [ActivityItem(title: title, summary: summary, status: status)])
    case let .a2ui(surface):
      A2UISurfaceView(surface: surface, onSubmit: { componentID, actionName, dataModel in
        onA2UI(surface, componentID, actionName, dataModel)
      })
    case let .choicePrompt(prompt, mode, options):
      ChoicePromptView(prompt: prompt, mode: mode, options: options, selection: $selectedChoices) {
        onChoice(message.id, Array(selectedChoices))
      }
    case let .choiceSelection(_, _, options):
      Label(options.map(\.label).joined(separator: ", "), systemImage: "checkmark.circle")
        .foregroundStyle(NoemaColor.contentSecondary)
    case let .error(message, recoverable):
      Label(message, systemImage: recoverable ? "exclamationmark.triangle" : "xmark.octagon")
        .foregroundStyle(recoverable ? NoemaColor.warning : NoemaColor.danger)
    case let .artifact(reference):
      ArtifactReferenceView(reference: reference, onOpen: { onArtifact(reference) })
    case let .task(taskID):
      Label("Task \(taskID)", systemImage: "checklist")
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.accent)
    }
  }
}

private struct ActivityItem: Identifiable {
  let id: String
  let title: String
  let summary: String?
  let status: String

  init(id: String = UUID().uuidString, title: String, summary: String?, status: String) {
    self.id = id
    self.title = title
    self.summary = summary
    self.status = status
  }

  init?(message: ChatMessage) {
    guard case let .activity(title, summary, status, _) = message.kind else { return nil }
    id = message.id
    self.title = title
    self.summary = summary
    self.status = status
  }
}

private struct ActivityClusterView: View {
  let items: [ActivityItem]
  @State private var expanded = false

  var body: some View {
    DisclosureGroup(isExpanded: $expanded) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        ForEach(items) { item in
          VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
            Text(item.title).font(NoemaFont.captionEmphasized)
            if let summary = item.summary { Text(summary).font(NoemaFont.caption) }
          }
        }
      }
    } label: {
      Label(items.count == 1 ? items[0].title : "\(items.count) activities", systemImage: "sparkles")
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.contentSecondary)
    }
  }
}

private struct ChoicePromptView: View {
  let prompt: String
  let mode: String
  let options: [ChoiceOption]
  @Binding var selection: Set<String>
  let submit: () -> Void

  var allowsMultiple: Bool { mode == "PICK_MANY" || mode == "pickMany" }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text(prompt).font(NoemaFont.bodyEmphasized)
      ForEach(options) { option in
        Button {
          if allowsMultiple {
            if selection.contains(option.id) { selection.remove(option.id) } else { selection.insert(option.id) }
          } else {
            selection = [option.id]
          }
        } label: {
          Label(option.label, systemImage: selection.contains(option.id) ? "checkmark.circle.fill" : "circle")
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(.borderless)
      }
      Button(allowsMultiple ? "Submit choices" : "Choose") { submit() }
        .buttonStyle(.borderedProminent)
        .disabled(selection.isEmpty)
    }
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.md))
  }
}

private struct ChatInterventionsView: View {
  @Bindable var model: ChatModel
  @State private var browserURL: URL?
  @State private var setupServerIDs: [String: String] = [:]

  var body: some View {
    ScrollView(.horizontal, showsIndicators: false) {
      HStack(spacing: NoemaSpacing.sm) {
        ForEach(model.interventions) { intervention in
          switch intervention {
          case let .governed(action):
            interventionCard {
              Label(action.summary, systemImage: "hand.raised")
              HStack {
                Button("Decline") { Task { await model.resolve(intervention, decision: "DECLINE") } }
                Button("Approve") { Task { await model.resolve(intervention, decision: "APPROVE") } }.buttonStyle(.borderedProminent)
              }
            }
          case let .mcpAuth(auth):
            interventionCard {
              Label("Sign in to \(auth.serverName)", systemImage: "person.badge.key")
              HStack {
                Button("Sign in") {
                  Task { browserURL = await model.startMcpAuthentication(auth) }
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.isOffline)
                Button("Skip") { Task { await model.skipMcpAuthentication(auth) } }
                  .disabled(model.isOffline)
              }
            }
          case let .adapterAuth(auth):
            interventionCard {
              Label("Sign in to \(auth.serviceName)", systemImage: "person.badge.key")
              HStack {
                Button("Sign in") {
                  Task { browserURL = await model.startAdapterAuthentication(auth) }
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.isOffline)
                Button("Skip") { Task { await model.skipAdapterAuthentication(auth) } }
                  .disabled(model.isOffline)
              }
            }
          case let .setup(setup):
            interventionCard {
              Label("Set up \(setup.displayName)", systemImage: "wrench.and.screwdriver")
              if let serviceURL = setup.serviceURL {
                Link("Open service", destination: serviceURL)
              }
              Text(setup.status.replacingOccurrences(of: "_", with: " ").capitalized)
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              if let serverID = setup.serverID {
                Button("Confirm connected server") {
                  Task { await model.resolveMcpSetup(setup, mcpServerID: serverID) }
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.isOffline)
              } else {
                TextField("MCP server id after setup", text: Binding(
                  get: { setupServerIDs[setup.itemID] ?? "" },
                  set: { setupServerIDs[setup.itemID] = $0 }
                ))
                .textFieldStyle(.roundedBorder)
                Button("Confirm connected server") {
                  guard let serverID = setupServerIDs[setup.itemID], !serverID.isEmpty else { return }
                  Task { await model.resolveMcpSetup(setup, mcpServerID: serverID) }
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.isOffline || (setupServerIDs[setup.itemID] ?? "").isEmpty)
              }
            }
          case let .attention(title, summary):
            interventionCard { Label("\(title): \(summary)", systemImage: "questionmark.circle") }
          }
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .sheet(isPresented: Binding(
      get: { browserURL != nil },
      set: { if !$0 { browserURL = nil } }
    ), onDismiss: {
      Task { await model.recoverConnection() }
    }) {
      if let browserURL { SafariView(url: browserURL) }
    }
  }

  @ViewBuilder
  private func interventionCard<Content: View>(@ViewBuilder content: () -> Content) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      content()
    }
    .padding(NoemaSpacing.sm)
    .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaSpacing.sm))
  }
}
