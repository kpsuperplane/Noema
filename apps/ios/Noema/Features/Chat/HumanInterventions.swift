import Apollo
import Foundation
import NoemaAPI

/// Chat and Work share one projection authority for the server union while
/// retaining independent refresh cadence and presentation.
typealias HumanIntervention = ChatIntervention

extension ChatIntervention {
  var taskID: String? {
    switch self {
    case let .governed(value): value.taskID
    case let .mcpAuth(value): value.taskID
    case let .adapterAuth(value): value.taskID
    case let .attention(value): value.taskID
    case .setup, .adapterDefinition: nil
    }
  }
}

/// Shared mutation construction for foreground Chat and background Work.
/// The server remains the authority for revisions, policy, and approval state.
enum HumanInterventionActions {
  static func createPublicMcpServer(_ setup: McpSetupModel, client: ApolloClient) async throws -> McpSetupServerModel {
    let input = mcpCreateInput(setup, authPreference: .useAnonymous)
    let response = try await client.perform(mutation: NoemaAPI.SettingsCreateMcpServerMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    guard let result = response.data?.createMcpServer,
          result.setupStatus == "ready_for_policy",
          let server = result.server else { throw ChatModelError.server("Noema could not finish public MCP discovery.") }
    return McpSetupServerModel(serverID: server.mcpServerId, connectionRevision: server.connectionRevision, policyRevision: server.policyRevision, toolCount: server.toolCount)
  }

  static func startMcpSetupOAuth(_ setup: McpSetupModel, redirectURI: String, client: ApolloClient) async throws -> URL {
    let input = NoemaAPI.StartMcpServerOAuthSetupInput(server: mcpCreateInput(setup), redirectUri: redirectURI)
    let response = try await client.perform(mutation: NoemaAPI.SettingsStartMcpServerOauthSetupMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    guard let value = response.data?.startMcpServerOauthSetup.authorizationUrl,
          let url = URL(string: value) else { throw ChatModelError.emptyResponse }
    return url
  }

  static func saveMcpPolicy(
    server: McpSetupServerModel,
    sharing: String,
    unsafeActions: String,
    client: ApolloClient
  ) async throws {
    let policy = NoemaAPI.SaveCapabilityConnectionPolicyInput(
      kind: GraphQLEnum(.mcp),
      connectionId: server.serverID,
      expectedConnectionRevision: server.connectionRevision,
      expectedPolicyRevision: Int32(server.policyRevision),
      dataSharingPolicy: sharing,
      unsafeActionPolicy: unsafeActions
    )
    let saved = try await client.perform(mutation: NoemaAPI.ChatSaveCapabilityConnectionPolicyMutation(input: policy))
    if let message = saved.errors?.first?.message { throw ChatModelError.server(message) }
  }

  private static func mcpCreateInput(
    _ setup: McpSetupModel,
    authPreference: NoemaAPI.McpSetupAuthPreference? = nil
  ) -> NoemaAPI.CreateMcpServerInput {
    NoemaAPI.CreateMcpServerInput(
      displayName: setup.displayName,
      transportKind: "streamable_http",
      stdio: .none,
      http: .some(NoemaAPI.McpHttpConfigInput(url: setup.endpointURL?.absoluteString ?? "", headers: .none, secretHeaders: .none, oauthClientCredentials: .none)),
      authPreference: authPreference.map { .some(GraphQLEnum($0)) } ?? .none
    )
  }

  static func approve(_ definition: AdapterDefinitionModel, client: ApolloClient) async throws {
    let input = NoemaAPI.ApproveAdapterDefinitionInput(semanticDigest: definition.semanticDigest)
    let response = try await client.perform(mutation: NoemaAPI.ApproveAdapterDefinitionMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func cancel(_ definition: AdapterDefinitionModel, client: ApolloClient) async throws {
    let input = NoemaAPI.CancelAdapterDefinitionInput(semanticDigest: definition.semanticDigest)
    let response = try await client.perform(mutation: NoemaAPI.CancelAdapterDefinitionMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func setup(_ definition: AdapterDefinitionModel, submission: AdapterCredentialSubmission, client: ApolloClient) async throws {
    if let document = submission.document,
       document.isEmpty || document.count > 128 * 1024 {
      throw AdapterCredentialError.invalidDocument
    }
    let input = NoemaAPI.SetupAdapterConnectionInput(
      semanticDigest: definition.semanticDigest,
      fieldValues: submission.fieldValues.map {
        NoemaAPI.AdapterCredentialFieldValueInput(fieldId: $0.fieldID, value: $0.value)
      },
      documentBase64: submission.document.map { .some($0.base64EncodedString()) } ?? .none
    )
    let response = try await client.perform(mutation: NoemaAPI.SetupAdapterConnectionMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func startOAuth(_ connection: AdapterConnectionModel, client: ApolloClient) async throws -> AdapterOAuthSetupAttempt {
    let input = NoemaAPI.StartAdapterOauthSetupInput(
      connectionId: connection.connectionID,
      expectedConnectionRevision: Int32(connection.connectionRevision),
      expectedCredentialRevision: Int32(connection.credentialRevision),
      expectedGrantRevision: Int32(connection.grantRevision),
      expectedPolicyRevision: Int32(connection.policyRevision)
    )
    let response = try await client.perform(mutation: NoemaAPI.StartAdapterOauthSetupMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    guard let attempt = response.data?.startAdapterOauthSetup,
          let url = URL(string: attempt.authorizationUrl) else { throw ChatModelError.emptyResponse }
    return AdapterOAuthSetupAttempt(
      attemptID: attempt.attemptId,
      authorizationURL: url,
      expiresAt: Date(timeIntervalSince1970: TimeInterval(attempt.expiresAtEpochSeconds))
    )
  }

  static func savePolicy(_ connection: AdapterConnectionModel, sharing: String, unsafeActions: String, client: ApolloClient) async throws {
    let input = NoemaAPI.SaveCapabilityConnectionPolicyInput(
      kind: GraphQLEnum(.api),
      connectionId: connection.connectionID,
      expectedConnectionRevision: String(connection.connectionRevision),
      expectedPolicyRevision: Int32(connection.policyRevision),
      dataSharingPolicy: sharing,
      unsafeActionPolicy: unsafeActions
    )
    let response = try await client.perform(mutation: NoemaAPI.ChatSaveCapabilityConnectionPolicyMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func resolve(_ action: GovernedActionModel, decision: String, client: ApolloClient) async throws {
    let value = GraphQLEnum(NoemaAPI.GovernedActionDecision(rawValue: decision) ?? .approve)
    let input = NoemaAPI.ResolveGovernedActionInput(
      actionId: action.actionID,
      expectedRevision: Int32(action.revision),
      decision: value
    )
    let response = try await client.perform(mutation: NoemaAPI.ResolveChatGovernedActionMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func startMcpAuthentication(_ auth: McpAuthModel, client: ApolloClient, profile: NoemaProfile?) async throws -> URL? {
    let redirectURI = profile?.origin.appending(path: "mcp/oauth/callback").absoluteString ?? "http://localhost/mcp/oauth/callback"
    let input = NoemaAPI.StartMcpAuthenticationInput(
      requestId: auth.requestID,
      expectedRevision: Int32(auth.revision),
      redirectUri: redirectURI
    )
    let response = try await client.perform(mutation: NoemaAPI.StartMcpAuthenticationMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    return response.data?.startMcpAuthentication.authorizationUrl.flatMap(URL.init(string:))
  }

  static func skipMcpAuthentication(_ auth: McpAuthModel, client: ApolloClient) async throws {
    let input = NoemaAPI.SkipMcpAuthenticationInput(requestId: auth.requestID, expectedRevision: Int32(auth.revision))
    let response = try await client.perform(mutation: NoemaAPI.SkipMcpAuthenticationMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func startAdapterAuthentication(_ auth: AdapterAuthModel, client: ApolloClient) async throws -> URL? {
    let input = NoemaAPI.StartAdapterAuthenticationInput(requestId: auth.requestID, expectedRevision: Int32(auth.revision))
    let response = try await client.perform(mutation: NoemaAPI.StartAdapterAuthenticationMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
    guard let authorizationURL = response.data?.startAdapterAuthentication.authorizationUrl else { return nil }
    return URL(string: authorizationURL)
  }

  static func skipAdapterAuthentication(_ auth: AdapterAuthModel, client: ApolloClient) async throws {
    let input = NoemaAPI.SkipAdapterAuthenticationInput(requestId: auth.requestID, expectedRevision: Int32(auth.revision))
    let response = try await client.perform(mutation: NoemaAPI.SkipAdapterAuthenticationMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }

  static func resolveMcpSetup(_ setup: McpSetupModel, mcpServerID: String, client: ApolloClient) async throws {
    let input = NoemaAPI.ResolveMcpSetupInterventionInput(itemId: setup.itemID, mcpServerId: mcpServerID)
    let response = try await client.perform(mutation: NoemaAPI.ResolveMcpSetupInterventionMutation(input: input))
    if let message = response.errors?.first?.message { throw ChatModelError.server(message) }
  }
}
