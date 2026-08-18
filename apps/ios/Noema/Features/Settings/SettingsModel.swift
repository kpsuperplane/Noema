import Apollo
import ApolloAPI
import Foundation
import NoemaAPI
import Observation

struct PairedClient: Identifiable, Hashable {
  let id: String
  let displayName: String
  let createdAt: String
  let revokedAt: String?
  let isCurrent: Bool

  var isRevoked: Bool { revokedAt != nil }
}

struct SettingsModelProfile: Identifiable, Hashable {
  let id: String
  let label: String
  let disabledReason: String?
  let reasoningEfforts: [String]
  let defaultReasoningEffort: String?
}

struct SettingsModelRecommendation: Hashable {
  let useCase: NoemaAPI.NoemaModelUseCase
  let modelProfile: String
  let reasoningEffort: String?
  let disabledReason: String?
}

struct SettingsModelOption: Identifiable, Hashable {
  var id: String { providerAccountId }
  let providerKind: String
  let providerAccountId: String
  let providerDisplayName: String
  let status: String
  let disabledReason: String?
  let profiles: [SettingsModelProfile]
  let recommendations: [SettingsModelRecommendation]
}

struct SettingsPreference: Hashable {
  let providerKind: String
  let providerAccountId: String
  let modelProfile: String?
  let reasoningEffort: String?
  let selectionMode: String
}

struct SettingsCapabilityTool: Identifiable, Hashable {
  var id: String { toolId }
  let connectionId: String
  let toolId: String
  let name: String
  let description: String?
  let enabled: Bool
  let readOnly: Bool?
  let idempotent: Bool?
  let destructive: Bool?
  let openWorld: Bool?
  let status: String
  let policyRevision: Int
  let sourceRevision: String
  let decisionPreview: String?
}

struct SettingsCapabilityDetail: Identifiable, Hashable {
  var id: String { connectionId }
  let kind: NoemaAPI.CapabilityIntegrationKind
  let definitionId: String
  let connectionId: String
  let name: String
  let connectionLabel: String?
  let sourceRevision: String
  let connectionRevision: String
  let policyRevision: Int
  let status: String
  let healthStatus: String
  let authStatus: String
  let dataSharingPolicy: String?
  let unsafeActionPolicy: String?
  let toolCount: Int
  let availableToolCount: Int
  let pendingToolCount: Int
  let defaultedToolCount: Int
  let disabledToolCount: Int
  let tools: [SettingsCapabilityTool]
}

enum SettingsSection: String, CaseIterable, Identifiable {
  case agents
  case memory
  case web
  case apis
  case mcps
  case privacy
  case usage
  case execution
  case localModels
  case providers
  case notifications
  case clients

  var id: String { rawValue }

  var title: String {
    switch self {
    case .agents: "Agents"
    case .memory: "Memory"
    case .web: "Web"
    case .apis: "APIs"
    case .mcps: "MCPs"
    case .privacy: "Privacy"
    case .usage: "Usage"
    case .execution: "Execution"
    case .localModels: "Local Models"
    case .providers: "Providers"
    case .notifications: "Notifications"
    case .clients: "Clients"
    }
  }

  var icon: NoemaIcon.Name {
    switch self {
    case .agents: .bot
    case .memory: .brain
    case .web: .globe
    case .apis: .cable
    case .mcps: .plugZap
    case .privacy: .shieldCheck
    case .usage, .execution: .gauge
    case .localModels: .cpu
    case .providers: .serverCog
    case .notifications: .bell
    case .clients: .smartphone
    }
  }
}

@MainActor
@Observable
final class SettingsModel {
  private(set) var snapshot: NoemaAPI.SettingsSnapshotQuery.Data?
  private(set) var clients: [PairedClient] = []
  var taskModelPools: [SettingsTaskModelPool] = []
  var acpAgents: [SettingsAcpAgent] = []
  var pairingLink: SettingsPairingLink?
  var pairingErrorMessage: String?
  var isStartingPairing = false
  private(set) var isLoading = false
  var isMutating = false
  var errorMessage: String?
  var clientsErrorMessage: String?
  var taskModelPoolsErrorMessage: String?
  var acpErrorMessage: String?
  var isOffline: Bool { connectionStatus?.isDisconnected ?? (client == nil) }
  var isLoadingClients = false
  var isLoadingTaskModelPools = false
  var isLoadingAcpAgents = false
  var hasLoadedClients = false
  var hasLoadedTaskModelPools = false
  var hasLoadedAcpAgents = false
  var auth: ProviderAuthModel?
  var capabilityDetails: [String: SettingsCapabilityDetail] = [:]
  var adapterDefinitions: [SettingsAdapterDefinition] = []
  var adapterOAuthState: SettingsAdapterOAuthState?
  var client: ApolloClient?
  var authSubscription: Task<Void, Never>?
  private let connectionStatus: NoemaConnectionStatus?

  init(connectionStatus: NoemaConnectionStatus?) {
    self.connectionStatus = connectionStatus
  }

  var canMutate: Bool { client != nil && !isMutating && !isOffline }

  func load(client: ApolloClient?) async {
    guard let client else {
      self.client = nil
      errorMessage = "Pair this device with Noema to view settings."
      return
    }
    self.client = client
    isLoading = true
    errorMessage = nil
    do {
      let stream = try client.fetch(query: NoemaAPI.SettingsSnapshotQuery(), cachePolicy: .cacheAndNetwork)
      var received = false
      for try await response in stream {
        if let data = response.data {
          snapshot = data
          received = true
        }
        if let message = response.errors?.first?.message, !received {
          errorMessage = message
        }
      }
      if !received, snapshot == nil { throw SettingsError.unavailable }
    } catch {
      if snapshot == nil { errorMessage = "Settings could not be loaded." }
    }
    isLoading = false
    await loadAdapterDefinitions(client: client)
    await loadAdapterOAuthState(client: client)
    await loadTaskModelPools(client: client)
    await loadAcpAgents(client: client)
    await loadClients(client: client)
  }

  func loadClients(client: ApolloClient? = nil) async {
    guard let client = client ?? self.client else { return }
    isLoadingClients = true
    clientsErrorMessage = nil
    defer { isLoadingClients = false }
    do {
      let stream = try client.fetch(query: NoemaAPI.ClientsQuery(), cachePolicy: .cacheAndNetwork)
      var received = false
      for try await response in stream {
        if let data = response.data {
          clients = data.clients.map(Self.client(from:))
          received = true
          hasLoadedClients = true
        }
        if let message = response.errors?.first?.message, !received {
          clientsErrorMessage = message
        }
      }
    } catch {
      if clients.isEmpty { clientsErrorMessage = "Paired clients could not be loaded." }
    }
  }

  func loadCapabilityDetail(kind: NoemaAPI.CapabilityIntegrationKind, connectionID: String) async {
    guard let client else { return }
    do {
      let query = NoemaAPI.CapabilityConnectionQuery(
        ref: NoemaAPI.CapabilityConnectionRefInput(kind: GraphQLEnum(kind), connectionId: connectionID)
      )
      let stream = try client.fetch(query: query, cachePolicy: .cacheAndNetwork)
      for try await response in stream {
        if let data = response.data, let detail = Self.capabilityDetail(from: data) {
          capabilityDetails[detail.id] = detail
        }
        if let message = response.errors?.first?.message { errorMessage = message }
      }
    } catch {
      errorMessage = "Connection details could not be loaded."
    }
  }

  func startProviderAuth(providerKind: String, providerAccountID: String?, method: String) async -> Bool {
    guard canMutate, let client else { return false }
    isMutating = true
    defer { isMutating = false }
    do {
      let authMethod = NoemaAPI.ProviderAuthMethod(rawValue: method) ?? .oauthPkce
      let input = NoemaAPI.StartProviderAuthAttemptInput(
        providerKind: providerKind,
        providerAccountId: providerAccountID.map { .some($0) } ?? .none,
        method: GraphQLEnum(authMethod)
      )
      let response = try await client.perform(mutation: NoemaAPI.StartProviderAuthAttemptMutation(input: input))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      guard let attempt = response.data?.startProviderAuthAttempt else { throw SettingsError.unavailable }
      auth = ProviderAuthModel(
        attemptID: attempt.attemptId,
        providerAccountID: attempt.providerAccountId,
        providerKind: attempt.providerKind,
        status: attempt.status.rawValue,
        method: attempt.method.rawValue,
        verificationURL: URL(string: attempt.verificationUrl ?? ""),
        userCode: attempt.userCode,
        instructions: attempt.instructions,
        errorMessage: attempt.errorMessage
      )
      startAuthSubscription(attemptID: attempt.attemptId)
      return true
    } catch {
      errorMessage = error.localizedDescription
      return false
    }
  }

  func cancelProviderAuth() async -> Bool {
    guard let auth, let client else { return false }
    do {
      let response = try await client.perform(
        mutation: NoemaAPI.CancelProviderAuthAttemptMutation(
          input: NoemaAPI.CancelProviderAuthAttemptInput(attemptId: auth.attemptID)
        )
      )
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      authSubscription?.cancel()
      self.auth = nil
      return true
    } catch {
      errorMessage = error.localizedDescription
      return false
    }
  }

  func revoke(_ pairedClient: PairedClient) async throws -> Bool {
    guard canMutate, let client else { throw SettingsError.mutationsUnavailable }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.RevokeClientMutation(clientId: pairedClient.id))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      guard response.data != nil else { throw SettingsError.unavailable }
      clients = clients.map { item in
        guard item.id == pairedClient.id else { return item }
        return PairedClient(
          id: item.id,
          displayName: item.displayName,
          createdAt: item.createdAt,
          revokedAt: response.data?.revokeClient.revokedAt ?? item.revokedAt,
          isCurrent: item.isCurrent
        )
      }
      return pairedClient.isCurrent
    } catch {
      throw error
    }
  }

  @discardableResult
  func installLocalModel(modelID: String, file: String?) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsInstallLocalModelMutation(
          input: NoemaAPI.InstallLocalModelInput(
            modelId: modelID,
            file: file.map(GraphQLNullable.some) ?? .none
          )
        )
      )
    }
  }

  @discardableResult
  func saveAgentModelPreference(
    agentID: String,
    providerAccountID: String,
    selectionMode: String,
    modelProfile: String?,
    reasoningEffort: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveAgentModelPreferenceMutation(
          input: NoemaAPI.SaveAgentModelPreferenceInput(
            agentId: agentID,
            providerAccountId: providerAccountID,
            selectionMode: GraphQLEnum(Self.selectionMode(from: selectionMode)),
            modelProfile: Self.optional(modelProfile),
            reasoningEffort: Self.reasoning(from: reasoningEffort),
            fastMode: false
          )
        )
      )
    }
  }

  @discardableResult
  func saveMemoryModelPreference(
    providerAccountID: String,
    selectionMode: String,
    modelProfile: String?,
    reasoningEffort: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveMemoryModelPreferenceMutation(
          input: NoemaAPI.GraphqlSaveMemoryModelPreferenceInput(
            providerAccountId: providerAccountID,
            selectionMode: GraphQLEnum(Self.selectionMode(from: selectionMode)),
            modelProfile: Self.optional(modelProfile),
            reasoningEffort: Self.reasoning(from: reasoningEffort),
            fastMode: false
          )
        )
      )
    }
  }

  @discardableResult
  func saveWebFetchSummarizerPreference(
    providerAccountID: String,
    selectionMode: String,
    modelProfile: String?,
    reasoningEffort: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveWebFetchSummarizerPreferenceMutation(
          input: NoemaAPI.SaveWebFetchSummarizerPreferenceInput(
            providerAccountId: providerAccountID,
            selectionMode: GraphQLEnum(Self.selectionMode(from: selectionMode)),
            modelProfile: Self.optional(modelProfile),
            reasoningEffort: Self.reasoning(from: reasoningEffort),
            fastMode: false
          )
        )
      )
    }
  }

  @discardableResult
  func saveActionReviewerPreference(
    providerAccountID: String,
    selectionMode: String,
    modelProfile: String?,
    reasoningEffort: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveActionReviewerPreferenceMutation(
          input: NoemaAPI.SaveActionReviewerPreferenceInput(
            providerAccountId: providerAccountID,
            selectionMode: GraphQLEnum(Self.selectionMode(from: selectionMode)),
            modelProfile: Self.optional(modelProfile),
            reasoningEffort: Self.reasoning(from: reasoningEffort),
            fastMode: false
          )
        )
      )
    }
  }

  @discardableResult
  func saveToolProgressAuditPreference(
    providerAccountID: String,
    selectionMode: String,
    modelProfile: String?,
    reasoningEffort: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsSaveToolProgressAuditPreferenceMutation(
          input: NoemaAPI.SaveToolProgressAuditPreferenceInput(
            providerAccountId: providerAccountID,
            selectionMode: GraphQLEnum(Self.selectionMode(from: selectionMode)),
            modelProfile: Self.optional(modelProfile),
            reasoningEffort: Self.reasoning(from: reasoningEffort),
            fastMode: false
          )
        )
      )
    }
  }

  @discardableResult
  func saveDefaultModelPreference(
    providerKind: String,
    providerAccountID: String,
    selectionMode: String,
    modelProfile: String?,
    reasoningEffort: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsSaveDefaultModelPreferenceMutation(
          input: NoemaAPI.SaveDefaultModelPreferenceInput(
            providerKind: providerKind,
            providerAccountId: providerAccountID,
            selectionMode: GraphQLEnum(Self.selectionMode(from: selectionMode)),
            modelProfile: Self.optional(modelProfile),
            reasoningEffort: Self.reasoning(from: reasoningEffort),
            fastMode: false
          )
        )
      )
    }
  }

  @discardableResult
  func updateExecutionPolicy(
    maxProviderContinuations: Int,
    maxToolCalls: Int,
    maxActiveMinutes: Int,
    progressAuditInterval: Int
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.UpdateTaskExecutionPolicyMutation(
          input: NoemaAPI.TaskExecutionPolicyInput(
            maxProviderContinuations: Int32(maxProviderContinuations),
            maxToolCalls: Int32(maxToolCalls),
            maxActiveMinutes: Int32(maxActiveMinutes),
            progressAuditInterval: Int32(progressAuditInterval)
          )
        )
      )
    }
  }

  func activateLocalModel(installationID: String) async {
    guard canMutate, let client else { return }
    _ = await performMutation { try await client.perform(mutation: NoemaAPI.SettingsActivateLocalModelMutation(installationId: installationID)) }
  }

  func removeLocalModel(installationID: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation { try await client.perform(mutation: NoemaAPI.SettingsRemoveLocalModelMutation(installationId: installationID)) }
  }

  func cancelLocalModelInstall(installationID: String) async {
    guard canMutate, let client else { return }
    _ = await performMutation { try await client.perform(mutation: NoemaAPI.SettingsCancelLocalModelInstallMutation(installationId: installationID)) }
  }

  func retryLocalModelRuntime() async {
    guard canMutate, let client else { return }
    _ = await performMutation { try await client.perform(mutation: NoemaAPI.SettingsRetryLocalModelRuntimeMutation()) }
  }

  func importLocalModel(
    name: String,
    sourceKind: String,
    localPath: String?,
    repo: String?,
    revision: String?,
    file: String?,
    sha256: String?,
    license: String?
  ) async {
    guard canMutate, let client else { return }
    _ = await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsImportLocalModelMutation(
          input: NoemaAPI.ImportLocalModelInput(
            name: name,
            sourceKind: GraphQLEnum(NoemaAPI.LocalModelSourceKind(rawValue: sourceKind) ?? .localFile),
            localPath: Self.optional(localPath),
            repo: Self.optional(repo),
            revision: Self.optional(revision),
            file: Self.optional(file),
            sha256: Self.optional(sha256),
            license: Self.optional(license)
          )
        )
      )
    }
  }

  @discardableResult
  func saveWebToolProviderBinding(toolName: String, capabilityID: String, providerAccountID: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveWebToolProviderBindingMutation(
          input: NoemaAPI.SaveWebToolProviderBindingInput(
            toolName: toolName,
            capabilityId: capabilityID,
            providerAccountId: providerAccountID
          )
        )
      )
    }
  }

  @discardableResult
  func saveCapabilityConnectionPolicy(
    kind: NoemaAPI.CapabilityIntegrationKind,
    connectionID: String,
    expectedConnectionRevision: String,
    expectedPolicyRevision: Int,
    dataSharingPolicy: String,
    unsafeActionPolicy: String
  ) async -> Bool {
    guard canMutate, let client else { return false }
    guard !(dataSharingPolicy == "review_every_call" && unsafeActionPolicy == "never_ask") else {
      errorMessage = "Reviewing every call requires an approval step."
      return false
    }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveCapabilityConnectionPolicyMutation(
          input: NoemaAPI.SaveCapabilityConnectionPolicyInput(
            kind: GraphQLEnum(kind),
            connectionId: connectionID,
            expectedConnectionRevision: expectedConnectionRevision,
            expectedPolicyRevision: Int32(expectedPolicyRevision),
            dataSharingPolicy: dataSharingPolicy,
            unsafeActionPolicy: unsafeActionPolicy
          )
        )
      )
    }
  }

  @discardableResult
  func saveCapabilityConnectionLabel(
    kind: NoemaAPI.CapabilityIntegrationKind,
    connectionID: String,
    expectedConnectionRevision: String,
    expectedConnectionLabel: String?,
    connectionLabel: String?
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveCapabilityConnectionLabelMutation(
          input: NoemaAPI.SaveCapabilityConnectionLabelInput(
            kind: GraphQLEnum(kind),
            connectionId: connectionID,
            expectedConnectionRevision: expectedConnectionRevision,
            expectedConnectionLabel: Self.optional(expectedConnectionLabel),
            connectionLabel: Self.optional(connectionLabel)
          )
        )
      )
    }
  }

  @discardableResult
  func saveCapabilityToolOverride(
    kind: NoemaAPI.CapabilityIntegrationKind,
    connectionID: String,
    expectedConnectionRevision: String,
    toolID: String,
    sourceRevision: String,
    expectedPolicyRevision: Int,
    readOnly: Bool,
    idempotent: Bool,
    destructive: Bool,
    openWorld: Bool
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SaveCapabilityToolOverrideMutation(
          input: NoemaAPI.SaveCapabilityToolOverrideInput(
            kind: GraphQLEnum(kind),
            connectionId: connectionID,
            expectedConnectionRevision: expectedConnectionRevision,
            toolId: toolID,
            sourceRevision: sourceRevision,
            expectedPolicyRevision: Int32(expectedPolicyRevision),
            readOnly: readOnly,
            idempotent: idempotent,
            destructive: destructive,
            openWorld: openWorld
          )
        )
      )
    }
  }

  @discardableResult
  func resetCapabilityToolPolicy(
    kind: NoemaAPI.CapabilityIntegrationKind,
    connectionID: String,
    expectedConnectionRevision: String,
    toolID: String,
    sourceRevision: String,
    expectedPolicyRevision: Int
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.ResetCapabilityToolPolicyMutation(
          input: NoemaAPI.ResetCapabilityToolPolicyInput(
            kind: GraphQLEnum(kind),
            connectionId: connectionID,
            expectedConnectionRevision: expectedConnectionRevision,
            toolId: toolID,
            sourceRevision: sourceRevision,
            expectedPolicyRevision: Int32(expectedPolicyRevision)
          )
        )
      )
    }
  }

  @discardableResult
  func setCapabilityToolEnabled(
    kind: NoemaAPI.CapabilityIntegrationKind,
    connectionID: String,
    expectedConnectionRevision: String,
    toolID: String,
    sourceRevision: String,
    expectedPolicyRevision: Int,
    enabled: Bool
  ) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SetCapabilityToolEnabledMutation(
          input: NoemaAPI.SetCapabilityToolEnabledInput(
            kind: GraphQLEnum(kind),
            connectionId: connectionID,
            expectedConnectionRevision: expectedConnectionRevision,
            toolId: toolID,
            sourceRevision: sourceRevision,
            expectedPolicyRevision: Int32(expectedPolicyRevision),
            enabled: enabled
          )
        )
      )
    }
  }

  func createProviderAccount(
    providerKind: String,
    displayName: String?,
    secret: String,
    authMethod: String
  ) async {
    guard canMutate, let client else { return }
    _ = await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsCreateProviderAccountMutation(
          input: NoemaAPI.CreateProviderAccountInput(
            providerKind: providerKind,
            displayName: Self.optional(displayName),
            secret: secret,
            authMethod: GraphQLEnum(NoemaAPI.ProviderAuthMethod(rawValue: authMethod) ?? .secretInput)
          )
        )
      )
    }
  }

  @discardableResult
  func saveProviderSecret(providerAccountID: String, secret: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsSaveProviderSecretMutation(
          input: NoemaAPI.ProviderSecretInput(providerAccountId: providerAccountID, secret: secret)
        )
      )
    }
  }

  func clearProviderSecret(providerAccountID: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsClearProviderSecretMutation(
          input: NoemaAPI.ClearProviderSecretInput(providerAccountId: providerAccountID)
        )
      )
    }
  }

  func deleteProviderAccount(providerAccountID: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(
        mutation: NoemaAPI.SettingsDeleteProviderAccountMutation(
          input: NoemaAPI.DeleteProviderAccountInput(providerAccountId: providerAccountID)
        )
      )
    }
  }

  func performMutation<Mutation: GraphQLMutation>(
    _ operation: () async throws -> GraphQLResponse<Mutation>
  ) async -> Bool {
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await operation()
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      await load(client: client)
      return true
    } catch let error as SettingsError {
      errorMessage = error.localizedDescription
      return false
    } catch {
      errorMessage = error.localizedDescription
      return false
    }
  }

}
