import AVFoundation
import Foundation
import Apollo
import MarkdownUI
import NoemaAPI
import SwiftUI
import UIKit

private let chatVoiceCoordinateSpace = "chat-voice-composer"

struct TaskReferenceChip: View {
  let client: ApolloClient?
  let taskID: String
  let onOpen: ((String) -> Void)?
  @Environment(NoemaShellCoordinator.self) private var coordinator
  @State private var title = "Loading task…"
  @State private var status = "UNKNOWN"
  @State private var progress = "Loading"

  init(client: ApolloClient?, taskID: String, onOpen: ((String) -> Void)? = nil) {
    self.client = client
    self.taskID = taskID
    self.onOpen = onOpen
  }

  var body: some View {
    Button {
      if let onOpen {
        onOpen(taskID)
      } else {
        coordinator.openTask(taskID)
      }
    } label: {
      HStack(spacing: NoemaSpacing.xs) {
        Image(systemName: "checklist")
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.content)
        Image(systemName: statusSymbol)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(statusColor)
        Text(title)
          .font(NoemaFont.taskPreview.weight(.semibold))
          .foregroundStyle(NoemaColor.content)
          .lineLimit(1)
      }
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.compact)
      .background(NoemaColor.surface, in: NoemaSuperellipse.full)
      .overlay { NoemaSuperellipse.full.stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .buttonStyle(.plain)
    .contentShape(.interaction, NoemaSuperellipse.full.inset(by: -NoemaSpacing.sm))
    .accessibilityLabel("Open task: \(title), \(progress)")
    .task(id: taskID) { await observeTask() }
  }

  private func observeTask() async {
    await loadTask()
    guard let client else { return }
    do {
      let stream = try client.subscribe(
        subscription: TasksTaskEventsSubscription(taskId: taskID, after: .none)
      )
      for try await response in stream {
        guard !Task.isCancelled, response.data?.taskEvents.taskId == taskID else { continue }
        await loadTask()
      }
    } catch {
      // The shared transport reconnect path refetches chat; keep the last readable task projection.
    }
  }

  private func loadTask() async {
    guard let client else { return }
    do {
      let response = try await client.fetchNetworkFirst(query: TasksDetailCoreQuery(taskId: taskID))
      let task = response.data?.task.fragments.tasksCommandTaskFields
      title = task?.title ?? "Task unavailable"
      if task == nil {
        status = "UNAVAILABLE"
        progress = "Unavailable"
      } else if task?.completedAt != nil {
        status = "DONE"
        progress = "Completed"
      } else if task?.activeGate != nil {
        status = "ATTENTION"
        progress = task?.activeGate?.prompt ?? "Waiting for you"
      } else if task?.currentRun != nil {
        status = "ACTIVE"
        progress = task?.currentRun?.activityLabel ?? "Running"
      } else {
        status = task?.stage.behavior.rawValue ?? "UNKNOWN"
        progress = task?.stage.name ?? "Unavailable"
      }
    } catch {
      title = "Task unavailable"
      status = "UNAVAILABLE"
      progress = "Unavailable"
    }
  }

  private var statusSymbol: String {
    switch status {
    case "DONE", "TERMINAL_SUCCESS": "checkmark.circle.fill"
    case "ACTIVE": "arrow.triangle.2.circlepath"
    case "ATTENTION", "HUMAN_GATE": "person"
    case "TERMINAL_CANCELLED": "xmark.circle"
    case "UNAVAILABLE": "exclamationmark.circle.fill"
    default: "clock"
    }
  }

  private var statusColor: Color {
    switch status {
    case "DONE", "TERMINAL_SUCCESS": NoemaColor.pine700
    case "ACTIVE": NoemaColor.blue700
    case "ATTENTION", "HUMAN_GATE": NoemaColor.clay600
    case "TERMINAL_CANCELLED", "UNAVAILABLE": NoemaColor.red700
    default: NoemaColor.contentTertiary
    }
  }
}

struct ChatComposer: View {
  @Bindable var model: ChatModel
  var isEditable = true
  var isSendEnabled = true
  var placeholderOverride: String?
  var autoFocus = false
  var restingBottomOffset: CGFloat = 0
  @State private var voiceInput = ChatVoiceInput()
  @State private var cancelFrame = CGRect.zero
  @State private var microphoneLocation = CGPoint.zero
  @State private var microphonePressed = false
  @State private var holdTask: Task<Void, Never>?
  @State private var voicePressFeedback = UIImpactFeedbackGenerator(style: .light)
  @State private var cancelFeedback = UIImpactFeedbackGenerator(style: .rigid)
  @FocusState private var inputFocused: Bool
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  private let composerMaxWidth: CGFloat = 760
  private let voiceControlSize: CGFloat = 50

  private var preferredWidth: CGFloat {
    if voiceInput.showsCancel { return composerMaxWidth }
    let content = model.draft.isEmpty ? placeholder : model.draft
    let longestLine = content.split(whereSeparator: \.isNewline).map(\.count).max() ?? 0
    return min(composerMaxWidth, max(200, CGFloat(longestLine) * 7 + 94))
  }

  private var placeholder: String {
    if let placeholderOverride { return placeholderOverride }
    if model.isOffline { return "Write a draft while offline" }
    if let name = model.primaryAgentDisplayName, !name.isEmpty { return "Message " + name }
    return "Send a message"
  }

  private var canSend: Bool {
    isSendEnabled && hasDraft && !model.isSending && !model.isOffline
  }

  private var hasDraft: Bool {
    !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  private var composerActionSends: Bool {
    hasDraft && !voiceInput.isEngaged
  }

  private var composerActionSymbol: String {
    if composerActionSends || voiceInput.isRecording { return "paperplane" }
    return "mic.fill"
  }

  private var composerActionEnabled: Bool {
    composerActionSends ? canSend : canUseVoice
  }

  private var canUseVoice: Bool {
    voiceInput.showsCancel || (isEditable && isSendEnabled && !model.isSending && !model.isOffline)
  }

  private var composerTransitionAnimation: Animation? {
    guard !reduceMotion else { return nil }
    return voiceInput.showsCancel
      ? NoemaSpring.surface
      : NoemaSpring.surface.speed(0.8)
  }

  private var composerActionIconTransition: AnyTransition {
    guard !reduceMotion else { return .identity }
    return .asymmetric(
      insertion: .scale(scale: 0.25)
        .combined(with: .opacity)
        .animation(NoemaSpring.micro.delay(0.07)),
      removal: .scale(scale: 0.25)
        .combined(with: .opacity)
        .animation(.easeOut(duration: 0.07))
    )
  }

  private var composerText: Binding<String> {
    let displayedText = voiceInput.isEngaged ? voiceInput.previewText : model.draft
    return Binding {
      displayedText
    } set: { text in
      if !voiceInput.isEngaged { model.draft = text }
    }
  }

  var body: some View {
    ZStack(alignment: .bottomTrailing) {
      composerRow
      cancelOverlay
      composerActionOverlay
    }
    .fixedSize(horizontal: false, vertical: true)
    .coordinateSpace(name: chatVoiceCoordinateSpace)
    .frame(maxWidth: composerMaxWidth)
    .offset(y: inputFocused ? 0 : restingBottomOffset)
    .padding(.bottom, inputFocused ? NoemaSpacing.sm : 0)
    .task(id: autoFocus) {
      guard autoFocus, isEditable, !voiceInput.isEngaged else { return }
      await Task.yield()
      inputFocused = true
    }
    .onChange(of: autoFocus) { _, active in
      if !active { inputFocused = false }
    }
    .onChange(of: voiceInput.isEngaged) { _, engaged in
      model.isVoiceInputActive = engaged
    }
    .onChange(of: voiceInput.completionGeneration) { _, _ in consumeVoiceCompletion() }
    .onChange(of: scenePhase) { _, phase in
      if phase != .active { preserveAndStopVoiceInput() }
    }
    .onReceive(NotificationCenter.default.publisher(for: AVAudioSession.interruptionNotification)) { notification in
      let value = notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
      if value == AVAudioSession.InterruptionType.began.rawValue {
        preserveAndStopVoiceInput(message: "Voice input stopped because another audio session began.")
      }
    }
    .onReceive(NotificationCenter.default.publisher(for: AVAudioSession.routeChangeNotification)) { notification in
      let value = notification.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt
      if value == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue {
        preserveAndStopVoiceInput(message: "Voice input stopped because the microphone route changed.")
      }
    }
    .onDisappear {
      preserveAndStopVoiceInput()
      model.isVoiceInputActive = false
    }
    .alert(item: Binding(
      get: { voiceInput.alert },
      set: { if $0 == nil { voiceInput.clearAlert() } }
    )) { alert in
      if alert.offersSettings {
        return Alert(
          title: Text("Voice input unavailable"),
          message: Text(alert.message),
          primaryButton: .default(Text("Open Settings"), action: openSettings),
          secondaryButton: .cancel()
        )
      }
      return Alert(title: Text("Voice input unavailable"), message: Text(alert.message))
    }
  }

  private var composerRow: some View {
    HStack(alignment: .center, spacing: 0) {
      Color.clear.frame(
        width: voiceInput.showsCancel ? voiceControlSize + NoemaSpacing.sm : 0
      )
      composerSurface
        .layoutPriority(1)
    }
    .geometryGroup()
    .frame(idealWidth: preferredWidth, maxWidth: preferredWidth)
    .animation(composerTransitionAnimation, value: voiceInput.showsCancel)
    .frame(maxWidth: .infinity, alignment: .trailing)
  }

  private var cancelOverlay: some View {
    cancelButton
      .frame(maxWidth: .infinity, alignment: .leading)
      .offset(x: voiceInput.showsCancel ? 0 : -composerMaxWidth)
      .allowsHitTesting(voiceInput.showsCancel)
      .accessibilityHidden(!voiceInput.showsCancel)
      .zIndex(1)
      .animation(composerTransitionAnimation, value: voiceInput.showsCancel)
  }

  private var composerActionOverlay: some View {
    composerActionButton
      .padding(.trailing, NoemaSpacing.xs)
      .padding(.vertical, NoemaSpacing.compact - 1)
      .frame(maxWidth: .infinity, alignment: .trailing)
      .zIndex(2)
  }

  private var composerSurface: some View {
    TextField(
      "",
      text: composerText,
      prompt: Text(voiceInput.isEngaged ? voiceInput.previewPlaceholder : placeholder)
        .foregroundStyle(NoemaColor.white.opacity(0.72)),
      axis: .vertical
    )
    .font(NoemaFont.composer)
    .foregroundStyle(NoemaColor.white)
    .tint(NoemaColor.white)
    .lineLimit(1...5)
    .fixedSize(horizontal: false, vertical: true)
    .textFieldStyle(.plain)
    .focused($inputFocused)
    .disabled(!isEditable)
    .onSubmit {
      guard canSend, !voiceInput.isEngaged else { return }
      Task { await model.send() }
    }
    .padding(.leading, NoemaSpacing.lg)
    .padding(.trailing, NoemaSpacing.xs + 40 + NoemaSpacing.sm)
    .padding(.vertical, NoemaSpacing.lg - 1)
    .frame(maxWidth: .infinity, alignment: .leading)
    .frame(minHeight: voiceControlSize)
    .background {
      NoemaSuperellipse(cornerRadius: 26)
        .fill(NoemaColor.pine500)
        .shadow(color: NoemaColor.white, radius: NoemaSpacing.md)
    }
  }

  private var composerActionButton: some View {
    ZStack {
      Button {
        guard composerActionSends else { return }
        Task { await model.send() }
      } label: {
        composerActionLabel(symbol: composerActionSymbol, enabled: composerActionEnabled)
      }
      .buttonStyle(.plain)
      .disabled(composerActionSends && !canSend)
      .accessibilityHidden(!composerActionSends)
      .accessibilityLabel("Send message")

      if !composerActionSends { microphoneHitTarget }
    }
  }

  private var microphoneHitTarget: some View {
    Color.clear
      .frame(width: 40, height: 40)
      .contentShape(.interaction, NoemaSuperellipse(cornerRadius: 26))
      .gesture(microphoneGesture)
      .disabled(!canUseVoice)
      .accessibilityElement()
      .accessibilityAddTraits(.isButton)
      .accessibilityLabel(voiceInput.isEngaged ? "Stop and send voice input" : "Start voice input")
      .accessibilityHint("Double-tap to toggle voice input. Touch and hold to speak until release.")
      .accessibilityAction { voiceInput.accessibilityActivate(originalDraft: model.draft) }
  }

  private var cancelButton: some View {
    Button {
      cancelFeedback.impactOccurred(intensity: 1)
      voiceInput.cancel()
    } label: {
      Image(systemName: "xmark")
        .font(NoemaFont.bodyEmphasized)
        .foregroundStyle(NoemaColor.white)
        .frame(width: voiceControlSize, height: voiceControlSize)
        .background(NoemaColor.red700.opacity(voiceInput.cancelTargeted ? 1 : 0.82), in: NoemaSuperellipse(cornerRadius: 26))
        .overlay {
          NoemaSuperellipse(cornerRadius: 26)
            .stroke(NoemaColor.white.opacity(voiceInput.cancelTargeted ? 0.9 : 0), lineWidth: 2)
        }
        .scaleEffect(voiceInput.cancelTargeted ? 1.06 : 1)
        .animation(NoemaMotion.animation(NoemaSpring.micro, reduceMotion: reduceMotion), value: voiceInput.cancelTargeted)
    }
    .buttonStyle(.plain)
    .accessibilityLabel("Cancel voice input")
    .onAppear { cancelFeedback.prepare() }
    .onGeometryChange(for: CGRect.self) { proxy in
      proxy.frame(in: .named(chatVoiceCoordinateSpace))
    } action: { frame in
      cancelFrame = frame
    }
  }

  private func composerActionLabel(symbol: String, enabled: Bool) -> some View {
    ZStack {
      Image(systemName: symbol)
        .id(symbol)
        .transition(composerActionIconTransition)
        .symbolEffect(.pulse, options: .repeating, isActive: voiceInput.isRecording && !reduceMotion)
    }
    .font(NoemaFont.bodyEmphasized)
    .foregroundStyle(NoemaColor.pine500.opacity(enabled ? 1 : 0.7))
    .animation(reduceMotion ? nil : NoemaSpring.micro, value: symbol)
    .frame(width: 40, height: 40)
    .background(NoemaColor.white, in: NoemaSuperellipse(cornerRadius: 26))
    .scaleEffect(microphonePressed ? 0.94 : 1)
    .animation(NoemaMotion.animation(NoemaSpring.micro, reduceMotion: reduceMotion), value: microphonePressed)
  }

  private var microphoneGesture: some Gesture {
    DragGesture(minimumDistance: 0, coordinateSpace: .named(chatVoiceCoordinateSpace))
      .onChanged { value in
        guard canUseVoice else { return }
        if !microphonePressed {
          microphonePressed = true
          voicePressFeedback.impactOccurred(intensity: 0.8)
          voiceInput.pressBegan(originalDraft: model.draft)
          holdTask?.cancel()
          holdTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, microphonePressed else { return }
            if voiceInput.holdThresholdReached() {
              cancelFeedback.prepare()
              updateCancelTarget(cancelFrame.contains(microphoneLocation))
              UIImpactFeedbackGenerator(style: .light).impactOccurred()
            }
          }
        }
        microphoneLocation = value.location
        updateCancelTarget(cancelFrame.contains(value.location))
      }
      .onEnded { value in
        holdTask?.cancel()
        holdTask = nil
        guard microphonePressed else { return }
        let overCancel = cancelFrame.contains(value.location)
        microphonePressed = false
        microphoneLocation = .zero
        voiceInput.pressEnded(overCancel: overCancel)
      }
  }

  private func updateCancelTarget(_ targeted: Bool) {
    let wasTargeted = voiceInput.cancelTargeted
    voiceInput.setCancelTargeted(targeted)
    if !wasTargeted, voiceInput.cancelTargeted {
      cancelFeedback.impactOccurred(intensity: 1)
    } else if wasTargeted, !voiceInput.cancelTargeted {
      cancelFeedback.prepare()
    }
  }

  private func consumeVoiceCompletion() {
    guard let completion = voiceInput.takeCompletion() else { return }
    model.isVoiceInputActive = false
    switch completion {
    case let .send(text):
      model.draft = text
      Task { await model.send() }
    case let .keepDraft(text, _), let .restoreDraft(text):
      model.draft = text
    }
  }

  private func preserveAndStopVoiceInput(message: String? = nil) {
    guard voiceInput.showsCancel else { return }
    model.draft = voiceInput.currentDraft
    voiceInput.forceStop(message: message)
  }

  private func openSettings() {
    guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
    UIApplication.shared.open(url)
  }
}

struct ChatInterventionsView: View {
  @Bindable var model: ChatModel
  @State private var browserURL: URL?
  @State private var taskResponses: [String: String] = [:]
  @State private var selectedInterventionID: String?

  var body: some View {
    let visibleInterventions = model.interventions.filter(isVisible)
    let selectedIndex = max(
      0,
      visibleInterventions.firstIndex { $0.id == selectedInterventionID } ?? 0
    )
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      if visibleInterventions.count > 1 {
        HStack(spacing: NoemaSpacing.sm) {
          Text("\(selectedIndex + 1) of \(visibleInterventions.count) waiting")
            .font(NoemaFont.metadata)
            .foregroundStyle(NoemaColor.contentTertiary)
          Spacer(minLength: NoemaSpacing.sm)
          Button("Previous request", systemImage: "chevron.left") {
            selectedInterventionID = visibleInterventions[selectedIndex - 1].id
          }
          .labelStyle(.iconOnly)
          .buttonStyle(.borderless)
          .disabled(selectedIndex == 0)
          Button("Next request", systemImage: "chevron.right") {
            selectedInterventionID = visibleInterventions[selectedIndex + 1].id
          }
          .labelStyle(.iconOnly)
          .buttonStyle(.borderless)
          .disabled(selectedIndex == visibleInterventions.count - 1)
        }
      }
      if !visibleInterventions.isEmpty {
        interventionCard(for: visibleInterventions[selectedIndex])
      }
    }
    .frame(maxWidth: 760, alignment: .leading)
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
  private func interventionCard(for intervention: ChatIntervention) -> some View {
    NoemaCard(padding: NoemaSpacing.md, cornerRadius: 18) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        switch intervention {
        case let .governed(action):
          if !action.isToolEnablement {
            HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
              Text(reviewLabel(action)).interventionEyebrow()
              Spacer(minLength: NoemaSpacing.sm)
              Text(action.taskID == nil ? "Primary conversation" : "Background task")
                .font(NoemaFont.metadata)
                .foregroundStyle(NoemaColor.contentTertiary)
            }
          }
          ActionRequestReviewContent(action: action)
          if let failureCode = action.failureCode {
            Text(failureCode)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.danger)
          }
          interventionError(action.actionID)
          GovernedInterventionActions(
            disabled: model.isOffline,
            approveTitle: action.isToolEnablement ? "Enable tool" : "Approve once"
          ) {
            await model.resolve(intervention, decision: $0)
          }
        case let .mcpAuth(auth):
          Label("Sign-in required", systemImage: "person.badge.key")
            .interventionEyebrow()
          Text("Sign in to \(auth.serverName)")
            .font(NoemaFont.taskTitle)
          Text(authenticationDescription(state: auth.state, failureCode: auth.failureCode, taskID: auth.taskID))
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          interventionError(auth.requestID)
          AuthenticationInterventionActions(
            primaryTitle: authenticationTitle(state: auth.state),
            disabled: model.isOffline,
            onStart: { browserURL = await model.startMcpAuthentication(auth) },
            onSkip: { await model.skipMcpAuthentication(auth) }
          )
        case let .adapterAuth(auth):
          Label("Sign-in required", systemImage: "person.badge.key")
            .interventionEyebrow()
          Text("Sign in to \(auth.serviceName)")
            .font(NoemaFont.taskTitle)
          Text(authenticationDescription(state: auth.state, failureCode: auth.failureCode, taskID: auth.taskID))
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(auth.capabilityName)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          interventionError(auth.requestID)
          AuthenticationInterventionActions(
            primaryTitle: authenticationTitle(state: auth.state),
            disabled: model.isOffline,
            onStart: { browserURL = await model.startAdapterAuthentication(auth) },
            onSkip: { await model.skipAdapterAuthentication(auth) }
          )
        case let .setup(setup):
          McpSetupInterventionCard(
            setup: setup,
            isOffline: model.isOffline,
            onOpenBrowser: { browserURL = $0 },
            onRefresh: { await model.refreshInterventions() },
            onConnectPublicly: { try await model.connectMcpPublicly(setup) },
            onStartOAuth: { try await model.startMcpSetupOAuth(setup) },
            onSavePolicy: { try await model.saveMcpPolicy($0, sharing: $1, unsafeActions: $2) },
            onResolve: { try await model.resolveMcpSetup(setup, server: $0) }
          )
        case let .attention(attention):
          taskAttentionContent(attention)
        case let .oauthClientSetup(setup):
          AdapterOauthClientSetupInterventionCard(
            setup: setup,
            isOffline: model.isOffline,
            onOpenBrowser: { browserURL = $0 },
            onDismiss: { model.dismissOauthClientSetup(setup) },
            onImport: { try await model.importAdapterOauthClient(setup, submission: $0) }
          )
        case let .adapterDefinition(definition):
          AdapterDefinitionInterventionCard(
            definition: definition,
            isOffline: model.isOffline,
            onOpenBrowser: { browserURL = $0 },
            onRefresh: { await model.refreshInterventions() },
            onDismiss: { model.dismissAdapterSetup(definition) },
            onApprove: { try await model.approveAdapterDefinition(definition) },
            onCancel: { try await model.cancelAdapterDefinition(definition) },
            onSetup: { try await model.setupAdapterConnection(definition, submission: $0) },
            onStartOAuth: { try await model.startAdapterOauthSetup($0) },
            onWaitForOAuth: { try await model.completeAdapterOauthSetup($0, action: $1) },
            onAttach: { try await model.attachAdapterGrant($0) },
            onSavePolicy: { try await model.saveAdapterPolicy($0, dataSharingPolicy: $1, unsafeActionPolicy: $2) }
          )
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .shadow(color: NoemaColor.content.opacity(0.08), radius: 4, y: 3)
  }

  private func isVisible(_ intervention: ChatIntervention) -> Bool {
    switch intervention {
    case let .adapterDefinition(definition): return !model.isAdapterSetupDismissed(definition)
    case let .oauthClientSetup(setup): return !model.isOauthClientSetupDismissed(setup)
    default: return true
    }
  }

  @ViewBuilder
  private func interventionError(_ id: String) -> some View {
    if let message = model.interventionErrors[id] {
      Text(message)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.danger)
        .fixedSize(horizontal: false, vertical: true)
        .accessibilityLabel("Action failed: \(message)")
    }
  }

  private func authenticationTitle(state: String) -> String {
    state.uppercased() == "AUTHORIZING" ? "Open sign-in again" : "Continue in browser"
  }

  private func authenticationDescription(state: String, failureCode: String?, taskID: String?) -> String {
    if failureCode != nil { return "The previous sign-in did not finish. Try again to continue." }
    if state.uppercased() == "AUTHORIZING" { return "A sign-in was already opened. You can continue it or start again." }
    return taskID == nil ? "Your request is paused until you sign in." : "This task is paused until you sign in."
  }

  private func reviewLabel(_ action: GovernedActionModel) -> String {
    let behavior = action.readOnly == true ? "Read only" : "Can make changes"
    return "\(behavior) · \(action.reviewRoute.uppercased() == "LLM_REVIEW" ? "LLM review" : "Human review")"
  }

  @ViewBuilder
  private func taskAttentionContent(_ attention: ChatTaskAttentionModel) -> some View {
    Label(attention.gate?.kind == "APPROVAL" ? "Approval needed" : attention.gate?.kind == "RECOVERY" ? "Recovery needed" : "Clarification needed", systemImage: "hand.raised")
      .interventionEyebrow()
    if let gate = attention.gate {
      Text(gate.prompt.nilIfBlank ?? attention.summary)
        .font(NoemaFont.taskTitle)
      if !gate.context.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        Markdown(gate.context)
          .markdownTextStyle { ForegroundColor(NoemaColor.contentSecondary) }
      }
      switch gate.kind {
      case "APPROVAL":
        TextField("Optional note", text: taskResponseBinding(attention), axis: .vertical)
          .noemaTextField()
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Decline") { Task { await model.answerTask(attention, answer: taskResponses[taskResponseKey(attention)]?.nilIfBlank ?? "Declined", approval: .declined) } }
            .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
          Button("Approve") { Task { await model.answerTask(attention, answer: taskResponses[taskResponseKey(attention)]?.nilIfBlank ?? "Approved", approval: .approved) } }
            .buttonStyle(NoemaActionButtonStyle(variant: .primary))
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
        }
      case "RECOVERY":
        if let reason = gate.recoveryReason?.nilIfBlank {
          Text(reason.capitalized)
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.warning)
        }
        HStack(spacing: NoemaSpacing.xs) {
          TextField(recoveryPlaceholder(attention), text: taskResponseBinding(attention), axis: .vertical)
            .noemaTextField()
          Button("Respond", systemImage: "arrow.up") {
            let response = taskResponses[taskResponseKey(attention)]?.nilIfBlank
            if let response, hasTaskAction(attention, "ANSWER") {
              Task { await model.answerTask(attention, answer: response) }
            } else {
              Task { await model.retryTask(attention, note: response) }
            }
          }
          .labelStyle(.iconOnly)
          .buttonStyle(NoemaTaskResponseSubmitStyle())
          .accessibilityLabel(recoveryActionLabel(attention))
        }
        .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER") && !hasTaskAction(attention, "RETRY") || hasTaskAction(attention, "ANSWER") && !hasTaskAction(attention, "RETRY") && taskResponses[taskResponseKey(attention)]?.nilIfBlank == nil)
      default:
        ForEach(gate.suggestedAnswers, id: \.self) { suggestion in
          Button(suggestion) { Task { await model.answerTask(attention, answer: suggestion) } }
            .buttonStyle(NoemaTaskAnswerButtonStyle())
            .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER"))
        }
        HStack(spacing: NoemaSpacing.sm) {
          TextField("Or type another answer", text: taskResponseBinding(attention), axis: .vertical)
            .noemaTaskResponseField()
          Button {
            guard let answer = taskResponses[taskResponseKey(attention)]?.nilIfBlank else { return }
            Task { await model.answerTask(attention, answer: answer) }
          } label: {
            Image(systemName: "arrow.right")
              .font(.system(size: 14, weight: .semibold))
          }
          .buttonStyle(NoemaTaskResponseSubmitStyle())
          .accessibilityLabel("Answer")
          .disabled(model.isOffline || !hasTaskAction(attention, "ANSWER") || taskResponses[taskResponseKey(attention)]?.nilIfBlank == nil)
        }
        .padding(NoemaSpacing.xs)
        .background(NoemaColor.pine500, in: NoemaSuperellipse.full)
        .shadow(color: NoemaColor.pine700.opacity(0.10), radius: 6, y: 3)
      }
    } else {
      Text(attention.title)
        .font(NoemaFont.taskTitle)
      Text(attention.summary)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
    }
  }

  private func taskResponseKey(_ attention: ChatTaskAttentionModel) -> String {
    attention.taskID + ":" + (attention.gate?.id ?? "attention")
  }

  private func taskResponseBinding(_ attention: ChatTaskAttentionModel) -> Binding<String> {
    let key = taskResponseKey(attention)
    return Binding(get: { taskResponses[key] ?? "" }, set: { taskResponses[key] = $0 })
  }

  private func hasTaskAction(_ attention: ChatTaskAttentionModel, _ action: String) -> Bool {
    attention.validActions.contains(action) || attention.validActions.contains(action.lowercased())
  }

  private func recoveryPlaceholder(_ attention: ChatTaskAttentionModel) -> String {
    let canAnswer = hasTaskAction(attention, "ANSWER")
    let canRetry = hasTaskAction(attention, "RETRY")
    if canAnswer && canRetry { return "Answer, or leave blank to retry" }
    if canRetry { return "Optional retry guidance" }
    return "Type your answer"
  }

  private func recoveryActionLabel(_ attention: ChatTaskAttentionModel) -> String {
    taskResponses[taskResponseKey(attention)]?.nilIfBlank != nil && hasTaskAction(attention, "ANSWER") ? "Answer" : "Retry"
  }
}

struct ActivityValues {
  let title: String
  let summary: String?
  let status: String
  let metadata: String
  let activityKind: String
}

func activityValues(_ message: ChatMessage?) -> ActivityValues? {
  guard let message,
        case let .activity(title, summary, status, metadata, activityKind) = message.kind else { return nil }
  return ActivityValues(title: title, summary: summary, status: status, metadata: metadata, activityKind: activityKind)
}

func metadataObject(for message: ChatMessage) -> [String: Any] {
  guard let values = activityValues(message),
        let data = values.metadata.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
  return object
}

func nestedString(_ object: [String: Any], path: [String]) -> String? {
  var current: Any = object
  for key in path {
    guard let values = current as? [String: Any], let next = values[key] else { return nil }
    current = next
  }
  return stringValue(current)
}

func stringValue(_ value: Any?) -> String? {
  guard let value else { return nil }
  if let string = value as? String, !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return string }
  return nil
}

func activityDetail(for message: ChatMessage) -> String? {
  stringValue(metadataObject(for: message)["detail"])
}

func toolDetail(for message: ChatMessage) -> String? {
  let metadata = metadataObject(for: message)
  if let detail = stringValue(metadata["detail"]) { return detail }
  if let result = stringValue(metadata["result"]) { return result }
  if let output = stringValue(metadata["output"]) { return output }
  if let action = metadata["action"] as? [String: Any], let arguments = action["arguments"] {
    return prettyJSON(arguments)
  }
  return nil
}

func prettyJSON(_ value: Any) -> String? {
  guard JSONSerialization.isValidJSONObject(value),
        let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]),
        let string = String(data: data, encoding: .utf8) else {
    return stringValue(value)
  }
  return string
}

func isToolActivity(_ message: ChatMessage) -> Bool {
  guard let kind = activityValues(message)?.activityKind.normalizedActivityKind else { return false }
  return kind == "TOOL_CALL" || kind == "TOOL_RESULT"
}

func sameToolGroup(_ messages: [ChatMessage], _ next: ChatMessage) -> Bool {
  guard let first = messages.first, let previous = messages.last else { return false }
  if isToolCallResultPair(previous, next) { return true }
  guard let turnID = first.turnID, next.turnID == turnID else { return false }
  return toolAgentKey(first) == toolAgentKey(next)
}

func isToolCallResultPair(_ call: ChatMessage, _ result: ChatMessage) -> Bool {
  guard activityValues(call)?.activityKind.normalizedActivityKind == "TOOL_CALL",
        activityValues(result)?.activityKind.normalizedActivityKind == "TOOL_RESULT",
        sameToolTurn(call, result)
  else { return false }
  let callID = toolCorrelationID(call)
  let resultID = toolCorrelationID(result)
  return callID == nil || callID == resultID
}

func sameToolTurn(_ left: ChatMessage, _ right: ChatMessage) -> Bool {
  guard let leftTurnID = left.turnID, let rightTurnID = right.turnID else { return true }
  return leftTurnID == rightTurnID
}

func toolAgentKey(_ message: ChatMessage) -> String? {
  let metadata = metadataObject(for: message)
  for key in ["agent_id", "agentId", "instance_name", "instanceName"] {
    if let value = stringValue(metadata[key]) { return value }
  }
  return nil
}

func toolCorrelationID(_ message: ChatMessage) -> String? {
  let metadata = metadataObject(for: message)
  guard let action = metadata["action"] as? [String: Any] else { return nil }
  return stringValue(action["id"]) ?? stringValue(action["call_id"])
}

func isSystemNotice(_ message: ChatMessage) -> Bool {
  guard let values = activityValues(message) else { return false }
  let kind = values.activityKind.normalizedActivityKind
  return kind == "TASK_RUN_START" || kind == "TASK_RUN_END" || kind == "AUTHENTICATION_REQUEST" || activityDetail(for: message) == nil
}

func activityTone(_ message: ChatMessage?) -> NoemaStatusToken.Tone {
  guard let values = activityValues(message) else { return .neutral }
  let metadata = metadataObject(for: message!)
  let presentationTone = nestedString(metadata, path: ["presentation", "tone"])?.lowercased()
  switch presentationTone {
  case "neutral": return .neutral
  case "success": return .success
  case "warning": return .warning
  case "error": return .error
  default: break
  }
  switch values.status.uppercased() {
  case "FAILED", "ERROR": return .error
  case "COMPLETED", "SUCCEEDED": return .success
  default: return .neutral
  }
}

func statusTone(_ status: String) -> NoemaStatusToken.Tone {
  switch status.uppercased() {
  case "FAILED", "ERROR": .error
  case "COMPLETED", "SUCCEEDED": .success
  default: .neutral
  }
}

func statusLabel(_ status: String) -> String {
  switch status.uppercased() {
  case "STARTED": "Running"
  case "FAILED": "Failed"
  default: "Done"
  }
}

func humanizeToolName(_ name: String) -> String {
  name.replacingOccurrences(of: "_", with: " ")
}

extension String {
  var normalizedActivityKind: String { uppercased().replacingOccurrences(of: "-", with: "_") }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}

private struct NoemaTaskAnswerButtonStyle: ButtonStyle {
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(NoemaFont.bodyEmphasized)
      .foregroundStyle(NoemaColor.paper50)
      .multilineTextAlignment(.leading)
      .padding(.horizontal, NoemaSpacing.md)
      .padding(.vertical, NoemaSpacing.compact)
      .frame(minHeight: 44, alignment: .leading)
      .background(NoemaColor.pine500.opacity(configuration.isPressed ? 0.8 : 1), in: NoemaSuperellipse.full)
      .opacity(isEnabled ? 1 : 0.5)
  }
}

private struct NoemaTaskResponseFieldModifier: ViewModifier {
  func body(content: Content) -> some View {
    content
      .font(.custom("Hanken Grotesk", size: 16, relativeTo: .body))
      .foregroundStyle(NoemaColor.paper50)
      .tint(NoemaColor.paper50)
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.xs)
      .frame(minHeight: 36)
  }
}

private extension View {
  func noemaTaskResponseField() -> some View {
    modifier(NoemaTaskResponseFieldModifier())
  }
}

extension View {
  func interventionEyebrow() -> some View {
    font(NoemaFont.taskPreview.weight(.semibold))
      .textCase(.uppercase)
      .tracking(0.5)
      .foregroundStyle(NoemaColor.contentSecondary)
  }
}

private struct NoemaTaskResponseSubmitStyle: ButtonStyle {
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .foregroundStyle(NoemaColor.content)
      .frame(width: 36, height: 36)
      .background(NoemaColor.paper50.opacity(configuration.isPressed ? 0.78 : 1), in: Circle())
      .opacity(isEnabled ? 1 : 0.46)
  }
}
