import Apollo
import Foundation
import NoemaAPI
import Observation

enum ChatPhase: Equatable {
  case loading
  case onboarding
  case ready
  case failed(String)
}

enum ChatMessageKind: Equatable {
  case user(String)
  case assistant(String, streaming: Bool)
  case activity(title: String, summary: String?, status: String, metadata: String, activityKind: String)
  case a2ui(A2UISurfaceModel)
  case choicePrompt(prompt: String, mode: String, options: [ChoiceOption])
  case choiceSelection(promptItemID: String, mode: String, options: [ChoiceOption])
  case error(message: String, recoverable: Bool)
  case artifact(ArtifactReferenceModel)
  case task(String)
}

struct ChatTaskGateModel: Equatable {
  let id: String
  let kind: String
  let prompt: String
  let context: String
  let suggestedAnswers: [String]
  let recoveryReason: String?
}

struct ChatTaskAttentionModel: Equatable {
  let taskID: String
  let title: String
  let summary: String
  let revision: Int
  let generation: Int
  let gate: ChatTaskGateModel?
  let validActions: Set<String>
}

struct ChatMessage: Identifiable, Equatable {
  let id: String
  var cursor: String?
  var turnID: String?
  var clientMessageID: String?
  var metadata: String?
  var debugScope: RuntimeDebugScope?
  var kind: ChatMessageKind
  var isOptimistic = false
}

struct ChoiceOption: Identifiable, Equatable {
  let id: String
  let label: String
}

struct ArtifactReferenceModel: Equatable {
  let artifactID: String
  let versionID: String?
  let title: String
  let kind: String
  let storageKind: String
  let externalURL: URL?
  let downloadURL: URL?
  let mediaType: String?
}

struct A2UISurfaceModel: Identifiable, Equatable {
  let id: String
  let interactionID: String?
  let surfaceID: String
  let version: String
  let revision: Int
  let interactionRevision: Int?
  let lifecycle: String
  let catalogJSON: String
  let snapshotJSON: String
  let hasActions: Bool
}

@MainActor
@Observable
final class ChatModel {
  private(set) var phase: ChatPhase = .loading
  private(set) var messages: [ChatMessage] = []
  private(set) var conversationID: String?
  private(set) var providerName = "Noema"
  private(set) var primaryAgentDisplayName: String?
  private(set) var agentStatus = "IDLE"
  private(set) var hasLoadedTranscript = false
  private(set) var hasMoreBefore = false
  private(set) var beforeCursor: String?
  private(set) var isLoadingOlder = false
  private(set) var isSending = false
  var isOffline: Bool { connectionStatus?.isDisconnected ?? (client == nil) }
  private(set) var errorMessage: String?
  private(set) var interventions: [ChatIntervention] = []
  private(set) var interventionErrors: [String: String] = [:]
  private(set) var dismissedAdapterSetupDigests = Set<String>()
  @ObservationIgnored private var draftGeneration = 0
  var draft = "" {
    didSet { draftGeneration &+= 1 }
  }
  var isVoiceInputActive = false

  func dismissAdapterSetup(_ definition: AdapterDefinitionModel) {
    dismissedAdapterSetupDigests.insert(definition.semanticDigest)
    guard let dismissalStorageKey else { return }
    UserDefaults.standard.set(Array(dismissedAdapterSetupDigests).sorted(), forKey: dismissalStorageKey)
  }

  func dismissOauthClientSetup(_ setup: AdapterOauthClientSetupModel) {
    dismissedAdapterSetupDigests.insert("oauth:" + setup.profileDigest)
    guard let dismissalStorageKey else { return }
    UserDefaults.standard.set(Array(dismissedAdapterSetupDigests).sorted(), forKey: dismissalStorageKey)
  }

  func isAdapterSetupDismissed(_ definition: AdapterDefinitionModel) -> Bool {
    definition.reviewed
      && definition.connectionCount == 0
      && dismissedAdapterSetupDigests.contains(definition.semanticDigest)
  }

  func isOauthClientSetupDismissed(_ setup: AdapterOauthClientSetupModel) -> Bool {
    dismissedAdapterSetupDigests.contains("oauth:" + setup.profileDigest)
  }

  let client: ApolloClient?
  let profile: NoemaProfile?
  let connectionStatus: NoemaConnectionStatus?
  let onboarding: OnboardingModel?
  private var subscriptionTask: Task<Void, Never>?
  private var transcriptRefreshTask: Task<Bool, Never>?
  private var knownItemIDs = Set<String>()
  private var knownCursors = Set<String>()
  private var streamingIndex: [String: Int] = [:]
  private var started = false
  private var isRecoveringConnection = false

  private var dismissalStorageKey: String? {
    profile.map { "dev.noema.app.ios.dismissed-adapter-setup.\($0.origin.absoluteString)" }
  }

  private var diagnosticPhase: String {
    switch phase {
    case .loading: "loading"
    case .onboarding: "onboarding"
    case .ready: "ready"
    case .failed: "failed"
    }
  }

  init(client: ApolloClient?, profile: NoemaProfile?, connectionStatus: NoemaConnectionStatus?) {
    self.client = client
    self.profile = profile
    self.connectionStatus = connectionStatus
    if let profile {
      let key = "dev.noema.app.ios.dismissed-adapter-setup.\(profile.origin.absoluteString)"
      dismissedAdapterSetupDigests = Set(UserDefaults.standard.stringArray(forKey: key) ?? [])
    }
    if let client {
      onboarding = OnboardingModel(client: client)
    } else {
      onboarding = nil
    }
  }

  func start() async {
    guard !started else { return }
    started = true
    NoemaDiagnosticTrace.shared.record(category: "chat", event: "start_requested")
    guard let client else {
      phase = .failed("Pair this device with a Noema server to start chat.")
      return
    }
    let restoredCache = await restoreCachedChat(client: client)
    NoemaDiagnosticTrace.shared.record(
      category: "chat",
      event: "cache_restore_finished",
      fields: ["restored": String(restoredCache)]
    )
    if !restoredCache { phase = .loading }
    do {
      let bootStartedAt = ProcessInfo.processInfo.systemUptime
      let response = try await client.fetchNetworkFirst(query: NoemaAPI.ChatBootQuery())
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "boot_received",
        fields: [
          "source": String(describing: response.source),
          "durationMilliseconds": String(Int((ProcessInfo.processInfo.systemUptime - bootStartedAt) * 1_000))
        ]
      )
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
      guard let boot = response.data else { throw ChatModelError.emptyResponse }
      primaryAgentDisplayName = boot.localStatus.primaryAgentDisplayName?.trimmingCharacters(in: .whitespacesAndNewlines)
      if !boot.onboardingStatus.isUserOnboarded {
        phase = .onboarding
        await onboarding?.start()
        return
      }
      let primary = try await loadPrimaryConversation(client: client)
      conversationID = primary.conversationId
      providerName = primary.provider
      _ = await loadLatest(client: client)
      if case .failed = phase { return }
      phase = .ready
      await refreshInterventions(client: client)
      startSubscription(client: client, conversationID: primary.conversationId)
    } catch {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "start_failed", error: error)
      errorMessage = error.localizedDescription
      if !hasLoadedTranscript, phase != .onboarding {
        phase = .failed(errorMessage ?? "Noema could not load chat.")
      } else {
        if phase != .onboarding { phase = .ready }
      }
    }
  }

  func onboardingCompleted() async {
    started = false
    await start()
  }

  func retry() async {
    started = false
    await start()
  }

  /// Refetches durable transcript state after the shared WebSocket transport resumes.
  func recoverConnection() async {
    NoemaDiagnosticTrace.shared.record(
      category: "chat",
      event: "recovery_requested",
      fields: [
        "hasConversation": String(conversationID != nil),
        "isRecovering": String(isRecoveringConnection),
        "phase": diagnosticPhase
      ]
    )
    guard phase == .ready,
          !isRecoveringConnection,
          let client,
          conversationID != nil else { return }
    agentStatus = "IDLE"
    isRecoveringConnection = true
    defer { isRecoveringConnection = false }
    NoemaDiagnosticTrace.shared.record(category: "chat", event: "recovery_started")
    guard await loadLatest(client: client) else {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "recovery_stopped_offline")
      return
    }
    await refreshInterventions(client: client)
    guard !isOffline else {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "recovery_stopped_offline")
      return
    }
    NoemaDiagnosticTrace.shared.record(category: "chat", event: "recovery_finished")
  }

  func refreshInterventions() async {
    guard let client else { return }
    await refreshInterventions(client: client)
  }

  func loadOlder() async {
    guard let client, let conversationID, hasMoreBefore, !isLoadingOlder, !isOffline else { return }
    isLoadingOlder = true
    defer { isLoadingOlder = false }
    do {
      let input = NoemaAPI.ConversationTranscriptPageInput(
        conversationId: conversationID,
        cursor: beforeCursor.map { .some($0) } ?? .none,
        limit: 80
      )
      let response = try await client.fetch(
        query: NoemaAPI.ConversationTranscriptPageQuery(input: input),
        cachePolicy: .networkOnly
      )
      guard let page = response.data?.conversationTranscriptPage else { return }
      merge(page.items, prepend: true)
      hasMoreBefore = page.pageInfo.hasMoreBefore
      beforeCursor = page.pageInfo.beforeCursor
    } catch {
      errorMessage = error.localizedDescription
    }
  }

  func send() async {
    guard phase == .ready, let client, let conversationID, !isOffline else { return }
    let submittedDraft = draft
    let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty, !isSending else { return }
    draft = ""
    let clearedDraftGeneration = draftGeneration
    let clientMessageID = UUID().uuidString
    messages.append(ChatMessage(
      id: "optimistic-\(clientMessageID)",
      cursor: nil,
      turnID: nil,
      clientMessageID: clientMessageID,
      metadata: nil,
      debugScope: nil,
      kind: .user(text),
      isOptimistic: true
    ))
    isSending = true
    agentStatus = "INPUT_RECEIVED"
    defer { isSending = false }
    NoemaDiagnosticTrace.shared.record(
      category: "chat",
      event: "turn_send_started",
      fields: [
        "clientMessageID": clientMessageID,
        "messageCount": String(messages.count)
      ]
    )
    do {
      let input = NoemaAPI.SendConversationTurnInput(
        conversationId: conversationID,
        input: text,
        clientMessageId: .some(clientMessageID),
        clientTimeZone: .some(TimeZone.current.identifier)
      )
      let response = try await client.perform(mutation: NoemaAPI.SendConversationTurnMutation(input: input))
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "turn_send_accepted",
        fields: [
          "acceptedClientMessageID": response.data?.sendConversationTurn.clientMessageId ?? "none",
          "clientMessageID": clientMessageID
        ]
      )
    } catch {
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "turn_send_failed",
        error: error,
        fields: ["clientMessageID": clientMessageID]
      )
      if draftGeneration == clearedDraftGeneration { draft = submittedDraft }
      recordMutationError(error)
    }
  }

  func choose(promptItemID: String, optionIDs: [String]) async {
    guard let client, let conversationID, !optionIDs.isEmpty, !isOffline, !isSending else { return }
    isSending = true
    agentStatus = "INPUT_RECEIVED"
    defer { isSending = false }
    do {
      let input = NoemaAPI.SendMultipleChoiceSelectionInput(
        conversationId: conversationID,
        promptItemId: promptItemID,
        selectedOptionIds: optionIDs,
        clientMessageId: .some(UUID().uuidString)
      )
      let response = try await client.perform(mutation: NoemaAPI.SendMultipleChoiceSelectionMutation(input: input))
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    } catch {
      recordMutationError(error)
    }
  }

  func answerTask(_ attention: ChatTaskAttentionModel, answer: String, approval: ApprovalDecision? = nil) async {
    guard let client, let gate = attention.gate,
          !answer.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          !isOffline else { return }
    let input = NoemaAPI.AnswerTaskInput(
      taskId: attention.taskID,
      gateId: gate.id,
      expectedRevision: Int32(attention.revision),
      expectedGeneration: Int32(attention.generation),
      answerMarkdown: answer,
      approvalDecision: approval.map(GraphQLEnum.init) ?? .none,
      clientMutationId: UUID().uuidString
    )
    do {
      let response = try await client.perform(mutation: NoemaAPI.TasksAnswerTaskMutation(input: input))
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
      await refreshInterventions(client: client)
    } catch {
      recordMutationError(error)
    }
  }

  func retryTask(_ attention: ChatTaskAttentionModel, note: String? = nil) async {
    guard let client, let gate = attention.gate, !isOffline else { return }
    let input = NoemaAPI.RetryTaskInput(
      taskId: attention.taskID,
      gateId: gate.id,
      expectedRevision: Int32(attention.revision),
      expectedGeneration: Int32(attention.generation),
      retryNote: note.map { .some($0) } ?? .none,
      clientMutationId: UUID().uuidString
    )
    do {
      let response = try await client.perform(mutation: NoemaAPI.TasksRetryTaskMutation(input: input))
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
      await refreshInterventions(client: client)
    } catch {
      recordMutationError(error)
    }
  }

  func submitA2UI(
    _ surface: A2UISurfaceModel,
    componentID: String,
    actionName: String,
    context: Any?,
    dataModel: Any?
  ) async {
    guard let client, let conversationID, let interactionID = surface.interactionID, !isOffline, !isSending else { return }
    isSending = true
    agentStatus = "INPUT_RECEIVED"
    defer { isSending = false }
    do {
      let input = NoemaAPI.ProviderInteractionActionInput(
        conversationId: conversationID,
        interactionId: interactionID,
        expectedRevision: Int32(surface.interactionRevision ?? surface.revision),
        surfaceId: surface.surfaceID,
        sourceComponentId: componentID,
        actionName: actionName,
        context: encodeJSON(context).map { .some($0) } ?? .none,
        dataModel: encodeJSON(dataModel).map { .some($0) } ?? .none,
        clientMessageId: .some(UUID().uuidString)
      )
      let response = try await client.perform(mutation: NoemaAPI.SendA2UIActionMutation(input: input))
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    } catch {
      recordMutationError(error)
    }
  }

  func resolve(_ intervention: ChatIntervention, decision: String) async {
    guard let client, case let .governed(action) = intervention, !isOffline else { return }
    interventionErrors[action.actionID] = nil
    do {
      try await HumanInterventionActions.resolve(action, decision: decision, client: client)
      await refreshInterventions(client: client)
    } catch {
      recordInterventionError(error, id: action.actionID)
    }
  }

  func startMcpAuthentication(_ auth: McpAuthModel) async -> URL? {
    guard !isOffline else { return nil }
    interventionErrors[auth.requestID] = nil
    do {
      guard let client else { return nil }
      guard let url = try await HumanInterventionActions.startMcpAuthentication(auth, client: client, profile: profile) else {
        throw ChatModelError.emptyResponse
      }
      return url
    } catch {
      recordInterventionError(error, id: auth.requestID)
      return nil
    }
  }

  func skipMcpAuthentication(_ auth: McpAuthModel) async {
    guard let client, !isOffline else { return }
    interventionErrors[auth.requestID] = nil
    do {
      try await HumanInterventionActions.skipMcpAuthentication(auth, client: client)
      await refreshInterventions(client: client)
    } catch {
      recordInterventionError(error, id: auth.requestID)
    }
  }

  func startAdapterAuthentication(_ auth: AdapterAuthModel) async -> URL? {
    guard !isOffline else { return nil }
    interventionErrors[auth.requestID] = nil
    do {
      guard let client else { return nil }
      guard let url = try await HumanInterventionActions.startAdapterAuthentication(auth, client: client) else {
        throw ChatModelError.emptyResponse
      }
      return url
    } catch {
      recordInterventionError(error, id: auth.requestID)
      return nil
    }
  }

  func approveAdapterDefinition(_ definition: AdapterDefinitionModel) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.approve(definition, client: client)
    await refreshInterventions(client: client)
  }

  func cancelAdapterDefinition(_ definition: AdapterDefinitionModel) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.cancel(definition, client: client)
    await refreshInterventions(client: client)
  }

  func setupAdapterConnection(_ definition: AdapterDefinitionModel, submission: AdapterCredentialSubmission) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.setup(definition, submission: submission, client: client)
    await refreshInterventions(client: client)
  }

  func importAdapterOauthClient(_ setup: AdapterOauthClientSetupModel, submission: AdapterCredentialSubmission) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.importOauthClient(setup, submission: submission, client: client)
    await refreshInterventions(client: client)
  }

  func startAdapterOauthSetup(_ action: AdapterNextActionModel) async throws -> AdapterOAuthSetupAttempt {
    guard let client, !isOffline else { throw ChatModelError.offline }
    let attempt = try await HumanInterventionActions.startOAuth(action, client: client)
    await refreshInterventions(client: client)
    return attempt
  }

  func attachAdapterGrant(_ action: AdapterNextActionModel) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.attach(action, client: client)
    await refreshInterventions(client: client)
  }

  func completeAdapterOauthSetup(_ attempt: AdapterOAuthSetupAttempt, action: AdapterNextActionModel) async throws -> String {
    guard let client, !isOffline else { throw ChatModelError.offline }
    let status = try await HumanInterventionActions.waitForOAuth(attempt, action: action, client: client)
    await refreshInterventions(client: client)
    return status
  }

  func saveAdapterPolicy(
    _ connection: AdapterConnectionModel,
    dataSharingPolicy: String,
    unsafeActionPolicy: String
  ) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.savePolicy(connection, sharing: dataSharingPolicy, unsafeActions: unsafeActionPolicy, client: client)
    await refreshInterventions(client: client)
  }

  func skipAdapterAuthentication(_ auth: AdapterAuthModel) async {
    guard let client, !isOffline else { return }
    interventionErrors[auth.requestID] = nil
    do {
      try await HumanInterventionActions.skipAdapterAuthentication(auth, client: client)
      await refreshInterventions(client: client)
    } catch {
      recordInterventionError(error, id: auth.requestID)
    }
  }

  func connectMcpPublicly(_ setup: McpSetupModel) async throws -> McpSetupServerModel {
    guard let client, !isOffline else { throw ChatModelError.offline }
    let server = try await HumanInterventionActions.createPublicMcpServer(setup, client: client)
    await refreshInterventions(client: client)
    return server
  }

  func startMcpSetupOAuth(_ setup: McpSetupModel) async throws -> URL {
    guard let client, let profile, !isOffline else { throw ChatModelError.offline }
    return try await HumanInterventionActions.startMcpSetupOAuth(
      setup,
      redirectURI: profile.origin.appending(path: "mcp/oauth/callback").absoluteString,
      client: client
    )
  }

  func saveMcpPolicy(_ server: McpSetupServerModel, sharing: String, unsafeActions: String) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.saveMcpPolicy(server: server, sharing: sharing, unsafeActions: unsafeActions, client: client)
  }

  func resolveMcpSetup(_ setup: McpSetupModel, server: McpSetupServerModel) async throws {
    guard let client, !isOffline else { throw ChatModelError.offline }
    try await HumanInterventionActions.resolveMcpSetup(setup, mcpServerID: server.serverID, client: client)
    await refreshInterventions(client: client)
  }

  func resolveMcpSetup(_ setup: McpSetupModel, mcpServerID: String) async {
    guard let client, !isOffline else { return }
    do {
      try await HumanInterventionActions.resolveMcpSetup(setup, mcpServerID: mcpServerID, client: client)
      await refreshInterventions(client: client)
    } catch {
      recordMutationError(error)
    }
  }

  private func loadPrimaryConversation(
    client: ApolloClient
  ) async throws -> (conversationId: String, provider: String) {
    let startedAt = ProcessInfo.processInfo.systemUptime
    let queryResponse = try await client.fetchNetworkFirst(query: NoemaAPI.PrimaryConversationQuery())
    NoemaDiagnosticTrace.shared.record(
      category: "chat",
      event: "primary_conversation_received",
      fields: [
        "source": String(describing: queryResponse.source),
        "durationMilliseconds": String(Int((ProcessInfo.processInfo.systemUptime - startedAt) * 1_000))
      ]
    )
    if let message = queryResponse.errors?.first?.message { throw ChatModelError.server(message) }
    if let conversation = queryResponse.data?.primaryConversation {
      return (conversation.conversationId, conversation.provider)
    }
    guard queryResponse.source == .server else { throw ChatModelError.emptyResponse }
    let response = try await client.perform(mutation: NoemaAPI.EnsurePrimaryConversationMutation())
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    guard let conversation = response.data?.ensurePrimaryConversation else { throw ChatModelError.emptyResponse }
    _ = try? await client.fetch(
      query: NoemaAPI.PrimaryConversationQuery(),
      cachePolicy: .networkOnly
    )
    return (conversation.conversationId, conversation.provider)
  }

  private func restoreCachedChat(client: ApolloClient) async -> Bool {
    do {
      guard let bootResponse = try await client.fetch(
        query: NoemaAPI.ChatBootQuery(),
        cachePolicy: .cacheOnly
      ), let boot = bootResponse.data else {
        NoemaDiagnosticTrace.shared.record(category: "chat", event: "cache_miss", fields: ["stage": "boot"])
        return false
      }
      primaryAgentDisplayName = boot.localStatus.primaryAgentDisplayName?
        .trimmingCharacters(in: .whitespacesAndNewlines)
      if !boot.onboardingStatus.isUserOnboarded {
        phase = .onboarding
        return true
      }
      guard let primaryResponse = try await client.fetch(
        query: NoemaAPI.PrimaryConversationQuery(),
        cachePolicy: .cacheOnly
      ), let primary = primaryResponse.data?.primaryConversation else {
        NoemaDiagnosticTrace.shared.record(category: "chat", event: "cache_miss", fields: ["stage": "primary"])
        return false
      }
      conversationID = primary.conversationId
      providerName = primary.provider
      let input = NoemaAPI.ConversationTranscriptPageInput(
        conversationId: primary.conversationId,
        cursor: .none,
        limit: 80
      )
      guard let transcriptResponse = try await client.fetch(
        query: NoemaAPI.ConversationTranscriptPageQuery(input: input),
        cachePolicy: .cacheOnly
      ), let page = transcriptResponse.data?.conversationTranscriptPage else {
        NoemaDiagnosticTrace.shared.record(category: "chat", event: "cache_miss", fields: ["stage": "transcript"])
        return false
      }
      applyLatest(page)
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "cache_hit",
        fields: ["stage": "transcript", "messageCount": String(messages.count)]
      )
      phase = .ready
      return true
    } catch {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "cache_read_failed", error: error)
      return false
    }
  }

  private func loadLatest(client: ApolloClient) async -> Bool {
    if let transcriptRefreshTask {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "transcript_refresh_joined")
      return await transcriptRefreshTask.value
    }
    let task = Task { [weak self] in
      guard let self else { return false }
      return await fetchLatest(client: client)
    }
    transcriptRefreshTask = task
    let refreshed = await task.value
    transcriptRefreshTask = nil
    return refreshed
  }

  private func fetchLatest(client: ApolloClient) async -> Bool {
    guard let conversationID else { return false }
    let startedAt = ProcessInfo.processInfo.systemUptime
    NoemaDiagnosticTrace.shared.record(category: "chat", event: "transcript_refresh_started")
    do {
      let input = NoemaAPI.ConversationTranscriptPageInput(conversationId: conversationID, cursor: .none, limit: 80)
      let response = try await client.fetch(
        query: NoemaAPI.ConversationTranscriptPageQuery(input: input),
        cachePolicy: .networkOnly
      )
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "transcript_response",
        fields: [
          "source": String(describing: response.source),
          "hasData": String(response.data != nil),
          "errorCount": String(response.errors?.count ?? 0),
          "durationMilliseconds": String(Int((ProcessInfo.processInfo.systemUptime - startedAt) * 1_000))
        ]
      )
      if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
      guard let page = response.data?.conversationTranscriptPage else { throw ChatModelError.emptyResponse }
      applyLatest(page)
      phase = .ready
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "transcript_refresh_finished")
      return true
    } catch {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "transcript_refresh_failed", error: error)
      errorMessage = error.localizedDescription
      if !hasLoadedTranscript { phase = .failed(error.localizedDescription) }
      return false
    }
  }

  private func applyLatest(_ page: NoemaAPI.ConversationTranscriptPageQuery.Data.ConversationTranscriptPage) {
    let previousMessageCount = messages.count
    let previousStreamingCount = messages.filter {
      if case let .assistant(_, streaming) = $0.kind { return streaming }
      return false
    }.count
    messages.removeAll(keepingCapacity: true)
    knownItemIDs.removeAll(keepingCapacity: true)
    knownCursors.removeAll(keepingCapacity: true)
    streamingIndex.removeAll(keepingCapacity: true)
    merge(page.items, prepend: false)
    hasMoreBefore = page.pageInfo.hasMoreBefore
    beforeCursor = page.pageInfo.beforeCursor
    hasLoadedTranscript = true
    NoemaDiagnosticTrace.shared.record(
      category: "chat",
      event: "transcript_replaced",
      fields: [
        "assistantCount": String(messages.filter { if case .assistant = $0.kind { return true }; return false }.count),
        "messageCount": String(messages.count),
        "pageItemCount": String(page.items.count),
        "previousMessageCount": String(previousMessageCount),
        "previousStreamingCount": String(previousStreamingCount)
      ]
    )
  }

  private func refreshInterventions(client: ApolloClient) async {
    guard let conversationID else { return }
    do {
      let response = try await client.fetch(
        query: NoemaAPI.PendingChatInterventionsQuery(conversationId: .some(conversationID), taskId: .none, projectId: .none, first: 50),
        cachePolicy: .networkOnly
      )
      interventions = response.data?.pendingHumanInterventions.compactMap(ChatIntervention.init) ?? []
      let visibleIDs = Set(interventions.map(\.id))
      interventionErrors = interventionErrors.filter { visibleIDs.contains($0.key) }
    } catch {
      // Interventions are a secondary surface; transcript remains usable.
    }
  }

  private func startSubscription(client: ApolloClient, conversationID: String) {
    subscriptionTask?.cancel()
    NoemaDiagnosticTrace.shared.record(category: "chat", event: "subscription_starting")
    subscriptionTask = Task { [weak self] in
      do {
        let stream = try client.recoveringSubscribe(subscription: NoemaAPI.ConversationEventsSubscription(conversationId: conversationID))
        var connected = false
        for try await response in stream {
          guard let event = response.data?.conversationEvents else {
            if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
            continue
          }
          if !connected {
            connected = true
            NoemaDiagnosticTrace.shared.record(category: "chat", event: "subscription_connected")
          }
          await self?.apply(event)
        }
        guard !Task.isCancelled else { return }
        NoemaDiagnosticTrace.shared.record(category: "chat", event: "subscription_ended")
      } catch {
        guard !Task.isCancelled else { return }
        NoemaDiagnosticTrace.shared.record(category: "chat", event: "subscription_failed", error: error)
      }
    }
  }

  private func apply(_ event: NoemaAPI.ConversationEventsSubscription.Data.ConversationEvents) async {
    if let item = event.asConversationItemEvent {
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "conversation_item_received",
        fields: [
          "clientMessageID": item.clientMessageId ?? "none",
          "itemID": item.itemId,
          "itemKind": diagnosticItemKind(item.item),
          "turnID": item.itemTurnId ?? "none"
        ]
      )
      merge(
        item: item.item,
        itemID: item.itemId,
        cursor: item.cursor,
        turnID: item.itemTurnId,
        clientMessageID: item.clientMessageId,
        metadata: item.metadata.encodedString
      )
    } else if let delta = event.asAssistantTextDeltaEvent {
      apply(delta: delta)
    } else if let status = event.asAgentStatusEvent {
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "agent_status_received",
        fields: ["status": status.status.rawValue]
      )
      agentStatus = status.status.rawValue
    } else if let completed = event.asTurnCompletedEvent {
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "turn_completed_received",
        fields: [
          "clientMessageID": completed.clientMessageId ?? "none",
          "messageCount": String(messages.count)
        ]
      )
      agentStatus = "IDLE"
      for index in messages.indices {
        if case let .assistant(text, streaming) = messages[index].kind, streaming {
          messages[index].kind = .assistant(text, streaming: false)
        }
      }
      rebuildIndexes()
    } else if event.asSubscriptionReadyEvent != nil {
      NoemaDiagnosticTrace.shared.record(category: "chat", event: "subscription_ready_received")
      if let client { _ = await loadLatest(client: client) }
    } else if event.asHumanInterventionsChangedEvent != nil {
      if let client { await refreshInterventions(client: client) }
    }
  }

  private func apply(delta: NoemaAPI.ConversationEventsSubscription.Data.ConversationEvents.AsAssistantTextDeltaEvent) {
    let key = "\(delta.deltaTurnId):\(delta.streamId):\(delta.responseIndex)"
    if streamingIndex[key] == nil {
      NoemaDiagnosticTrace.shared.record(
        category: "chat",
        event: "assistant_stream_started",
        fields: [
          "deltaCharacters": String(delta.delta.count),
          "responseIndex": String(delta.responseIndex),
          "streamID": delta.streamId,
          "turnID": delta.deltaTurnId
        ]
      )
    }
    if messages.contains(where: { message in
      guard message.turnID == delta.deltaTurnId,
            case let .assistant(_, streaming) = message.kind,
            !streaming,
            let metadata = message.metadata else { return false }
      return streamKey(metadata: metadata, turnID: message.turnID) == key
    }) {
      return
    }
    if let index = streamingIndex[key] {
      guard index < messages.count else { streamingIndex.removeValue(forKey: key); return }
      if case let .assistant(text, _) = messages[index].kind {
        messages[index].kind = .assistant(text + delta.delta, streaming: true)
      }
      return
    }
    let message = ChatMessage(
      id: "stream-\(key)",
      cursor: nil,
      turnID: delta.deltaTurnId,
      clientMessageID: nil,
      metadata: nil,
      debugScope: runtimeDebugScope(turnID: delta.deltaTurnId, metadata: nil),
      kind: .assistant(delta.delta, streaming: true),
      isOptimistic: false
    )
    streamingIndex[key] = messages.endIndex
    messages.append(message)
  }

  private func merge(_ items: [NoemaAPI.ConversationTranscriptPageQuery.Data.ConversationTranscriptPage.Item], prepend: Bool) {
    let converted = items.map {
      convert(
        $0,
        itemID: $0.itemId,
        cursor: $0.cursor,
        turnID: $0.turnId,
        clientMessageID: nil,
        metadata: $0.metadata.encodedString
      )
    }
    if prepend { messages.insert(contentsOf: converted.filter { !knownItemIDs.contains($0.id) }, at: 0) }
    else { converted.forEach { merge($0) } }
    rebuildIndexes()
  }

  private func merge(
    item: NoemaAPI.ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item,
    itemID: String,
    cursor: String?,
    turnID: String?,
    clientMessageID: String?,
    metadata: String
  ) {
    let converted = convert(
      item,
      itemID: itemID,
      cursor: cursor,
      turnID: turnID,
      clientMessageID: clientMessageID,
      metadata: metadata
    )
    if item.asAssistantText != nil,
       let key = streamKey(metadata: metadata, turnID: turnID),
       let index = streamingIndex[key],
       messages.indices.contains(index) {
      messages[index] = converted
    } else {
      merge(converted)
    }
    if let clientMessageID, let index = messages.firstIndex(where: { $0.clientMessageID == clientMessageID && $0.isOptimistic }) {
      messages.remove(at: index)
    }
    rebuildIndexes()
  }

  private func merge(_ message: ChatMessage) {
    if let index = messages.firstIndex(where: { $0.id == message.id }) {
      messages[index] = message
    } else if let cursor = message.cursor, knownCursors.contains(cursor) {
      return
    } else {
      messages.append(message)
    }
  }

  private func rebuildIndexes() {
    knownItemIDs = Set(messages.map(\.id))
    knownCursors = Set(messages.compactMap(\.cursor))
    streamingIndex = Dictionary(uniqueKeysWithValues: messages.enumerated().compactMap { index, message in
      guard message.id.hasPrefix("stream-"),
            case let .assistant(_, streaming) = message.kind,
            streaming else { return nil }
      return (String(message.id.dropFirst("stream-".count)), index)
    })
  }

  private func streamKey(metadata: String, turnID: String?) -> String? {
    guard let turnID,
          let data = metadata.data(using: .utf8),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let streamID = object["stream_id"] as? String,
          let responseIndex = object["response_index"] as? NSNumber else { return nil }
    return "\(turnID):\(streamID):\(responseIndex.intValue)"
  }

  private func convert(
    _ item: NoemaAPI.ConversationTranscriptPageQuery.Data.ConversationTranscriptPage.Item,
    itemID: String,
    cursor: String?,
    turnID: String?,
    clientMessageID: String?,
    metadata: String?
  ) -> ChatMessage {
    let kind = convert(item.item)
    return ChatMessage(
      id: item.item.asActivity?.id ?? itemID,
      cursor: cursor,
      turnID: turnID,
      clientMessageID: clientMessageID,
      metadata: metadata,
      debugScope: runtimeDebugScope(turnID: turnID, metadata: metadata)
        ?? runtimeDebugScope(turnID: nil, metadata: activityMetadata(kind)),
      kind: kind
    )
  }

  private func convert(
    _ item: NoemaAPI.ConversationTranscriptPageQuery.Data.ConversationTranscriptPage.Item.Item
  ) -> ChatMessageKind {
    if let value = item.asUserText { return .user(value.text) }
    if let value = item.asAssistantText { return .assistant(value.text, streaming: false) }
    if let value = item.asActivity {
      return .activity(title: value.title, summary: value.summary, status: value.status.rawValue, metadata: value.metadata.encodedString, activityKind: value.activityKind)
    }
    if let value = item.asA2UISurface {
      return .a2ui(A2UISurfaceModel(id: value.id, interactionID: value.interactionId, surfaceID: value.surfaceId, version: value.version, revision: value.revision, interactionRevision: value.interactionRevision, lifecycle: value.lifecycle, catalogJSON: value.catalog.encodedString, snapshotJSON: value.snapshot.encodedString, hasActions: value.hasActions))
    }
    if let value = item.asMultipleChoicePrompt {
      return .choicePrompt(prompt: value.prompt, mode: value.selectionMode.rawValue, options: value.options.map { ChoiceOption(id: $0.id, label: $0.label) })
    }
    if let value = item.asMultipleChoiceSelection {
      return .choiceSelection(promptItemID: value.promptItemId, mode: value.selectionMode.rawValue, options: value.selectedOptions.map { ChoiceOption(id: $0.id, label: $0.label) })
    }
    if let value = item.asErrorNotice { return .error(message: value.message, recoverable: value.recoverable) }
    if let value = item.asArtifactReference {
      return .artifact(ArtifactReferenceModel(
        artifactID: value.artifactId,
        versionID: ArtifactLinkResolver.detailVersionID(storageKind: value.storageKind, versionID: value.artifactVersionId),
        title: value.title,
        kind: value.artifactKind,
        storageKind: value.storageKind,
        externalURL: ArtifactLinkResolver.externalURL(value.externalUrl),
        downloadURL: ArtifactLinkResolver.downloadURL(value.downloadUrl, origin: profile?.origin),
        mediaType: value.mediaType
      ))
    }
    if let value = item.asTaskReference { return .task(value.taskId) }
    return .error(message: "Noema returned an unsupported transcript item.", recoverable: false)
  }

  private func convert(
    _ item: NoemaAPI.ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item,
    itemID: String,
    cursor: String?,
    turnID: String?,
    clientMessageID: String?,
    metadata: String?
  ) -> ChatMessage {
    let kind = convert(item)
    return ChatMessage(
      id: item.asActivity?.id ?? itemID,
      cursor: cursor,
      turnID: turnID,
      clientMessageID: clientMessageID,
      metadata: metadata,
      debugScope: runtimeDebugScope(turnID: turnID, metadata: metadata)
        ?? runtimeDebugScope(turnID: nil, metadata: activityMetadata(kind)),
      kind: kind
    )
  }

  private func activityMetadata(_ kind: ChatMessageKind) -> String? {
    guard case let .activity(_, _, _, metadata, _) = kind else { return nil }
    return metadata
  }

  private func diagnosticItemKind(
    _ item: NoemaAPI.ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
  ) -> String {
    if item.asUserText != nil { return "user_text" }
    if item.asAssistantText != nil { return "assistant_text" }
    if item.asActivity != nil { return "activity" }
    if item.asA2UISurface != nil { return "a2ui" }
    if item.asMultipleChoicePrompt != nil { return "choice_prompt" }
    if item.asMultipleChoiceSelection != nil { return "choice_selection" }
    if item.asErrorNotice != nil { return "error" }
    if item.asArtifactReference != nil { return "artifact" }
    if item.asTaskReference != nil { return "task" }
    return "unsupported"
  }

  private func convert(
    _ item: NoemaAPI.ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
  ) -> ChatMessageKind {
    if let value = item.asUserText { return .user(value.text) }
    if let value = item.asAssistantText { return .assistant(value.text, streaming: false) }
    if let value = item.asActivity {
      return .activity(title: value.title, summary: value.summary, status: value.status.rawValue, metadata: value.metadata.encodedString, activityKind: value.activityKind)
    }
    if let value = item.asA2UISurface {
      return .a2ui(A2UISurfaceModel(id: value.id, interactionID: value.interactionId, surfaceID: value.surfaceId, version: value.version, revision: value.revision, interactionRevision: value.interactionRevision, lifecycle: value.lifecycle, catalogJSON: value.catalog.encodedString, snapshotJSON: value.snapshot.encodedString, hasActions: value.hasActions))
    }
    if let value = item.asMultipleChoicePrompt {
      return .choicePrompt(prompt: value.prompt, mode: value.selectionMode.rawValue, options: value.options.map { ChoiceOption(id: $0.id, label: $0.label) })
    }
    if let value = item.asMultipleChoiceSelection {
      return .choiceSelection(promptItemID: value.promptItemId, mode: value.selectionMode.rawValue, options: value.selectedOptions.map { ChoiceOption(id: $0.id, label: $0.label) })
    }
    if let value = item.asErrorNotice { return .error(message: value.message, recoverable: value.recoverable) }
    if let value = item.asArtifactReference {
      return .artifact(ArtifactReferenceModel(
        artifactID: value.artifactId,
        versionID: ArtifactLinkResolver.detailVersionID(storageKind: value.storageKind, versionID: value.artifactVersionId),
        title: value.title,
        kind: value.artifactKind,
        storageKind: value.storageKind,
        externalURL: ArtifactLinkResolver.externalURL(value.externalUrl),
        downloadURL: ArtifactLinkResolver.downloadURL(value.downloadUrl, origin: profile?.origin),
        mediaType: value.mediaType
      ))
    }
    if let value = item.asTaskReference { return .task(value.taskId) }
    return .error(message: "Noema returned an unsupported transcript item.", recoverable: false)
  }

  private func appendError(_ message: String, recoverable: Bool) {
    messages.append(ChatMessage(
      id: "error-\(UUID().uuidString)",
      cursor: nil,
      turnID: nil,
      clientMessageID: nil,
      metadata: nil,
      debugScope: nil,
      kind: .error(message: message, recoverable: recoverable)
    ))
  }

  private func recordMutationError(_ error: Error) {
    appendError(error.localizedDescription, recoverable: true)
  }

  private func recordInterventionError(_ error: Error, id: String) {
    interventionErrors[id] = error.localizedDescription
  }
}
