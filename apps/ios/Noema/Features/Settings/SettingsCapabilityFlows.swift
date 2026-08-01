import Apollo
import ApolloAPI
import Foundation
import NoemaAPI

struct SettingsAdapterConnection: Identifiable, Hashable {
  let id: String
  let status: String
  let connectionRevision: Int
  let credentialRevision: Int
  let grantRevision: Int
  let policyRevision: Int
  let policyConfigured: Bool
}

struct SettingsAdapterOperation: Identifiable, Hashable {
  let id: String
  let method: String
  let path: String
  let arguments: [String]
  let responseTransform: SettingsAdapterResponseTransform?
}

struct SettingsAdapterResponseTransform: Hashable {
  let language: String
  let sourceDigest: String
  let source: String
  let acceptedContentTypes: [String]
  let outputSchemaJSON: String
}

struct SettingsAdapterDefinition: Identifiable, Hashable {
  let id: String
  let semanticDigest: String
  let definitionId: String
  let displayName: String
  let definitionRevision: String
  let sourceReference: URL?
  let authenticationMode: String
  let accountIdentityOperationID: String?
  let scopes: [String]
  let clientSetupURL: URL?
  let operations: [SettingsAdapterOperation]
  let manifestJSON: String
  let acceptsOAuthClientJSON: Bool
  let reviewed: Bool
  let superseded: Bool
  let connections: [SettingsAdapterConnection]
}

struct SettingsMCPServer: Hashable {
  let id: String
  let connectionRevision: String
  let policyRevision: Int
  let displayName: String
  let transportKind: String
  let healthStatus: String
  let authStatus: String
  let toolCount: Int
  let browserOAuthReauthenticationSupported: Bool
}

struct SettingsMCPSetupResult: Hashable {
  let server: SettingsMCPServer?
  let setupStatus: String
  let discoveryStatus: String?
  let discoveredToolCount: Int
  let setupError: String?
  let oauthClientCredentialsSupported: Bool
  let oauthAuthorizationSupported: Bool
  let scopes: [String]
}

struct SettingsMCPAuthAttempt: Hashable {
  let attemptID: String
  let status: String
  let authorizationURL: URL?
  let errorMessage: String?
  let setupResult: SettingsMCPSetupResult?
}

extension SettingsModel {
  func loadAdapterDefinitions(client: ApolloClient? = nil) async {
    guard let client = client ?? self.client else { return }
    do {
      let stream = try client.fetch(query: NoemaAPI.SettingsAdapterDefinitionsQuery(), cachePolicy: .cacheAndNetwork)
      for try await response in stream {
        if let data = response.data {
          adapterDefinitions = data.adapterDefinitions.map(Self.adapterDefinition(from:))
        }
        if let message = response.errors?.first?.message, adapterDefinitions.isEmpty { errorMessage = message }
      }
    } catch {
      if adapterDefinitions.isEmpty { errorMessage = "API definitions could not be loaded." }
    }
  }

  @discardableResult
  func approveAdapterDefinition(_ semanticDigest: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsApproveAdapterDefinitionMutation(
        input: NoemaAPI.ApproveAdapterDefinitionInput(semanticDigest: semanticDigest)
      ))
    }
  }

  @discardableResult
  func importAdapterOAuthClientJSON(_ semanticDigest: String, data: Data) async -> Bool {
    guard canMutate, let client else { return false }
    guard data.count <= 32 * 1024 else {
      errorMessage = "OAuth client JSON must be 32 KB or smaller."
      return false
    }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsImportAdapterOauthClientJsonMutation(
        input: NoemaAPI.ImportAdapterOauthClientJsonInput(
          semanticDigest: semanticDigest,
          clientJsonBase64: data.base64EncodedString()
        )
      ))
    }
  }

  func startAdapterOAuth(connection: SettingsIntegrationConnection) async -> URL? {
    guard canMutate, let client,
          let definition = adapterDefinitions.first(where: { $0.semanticDigest == connection.sourceRevision }),
          let adapterConnection = definition.connections.first(where: { $0.id == connection.id }),
          let expectedConnectionRevision = Int32(exactly: adapterConnection.connectionRevision) else {
      errorMessage = "The API connection revision is invalid. Refresh Settings and try again."
      return nil
    }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsStartAdapterOauthSetupMutation(
        input: NoemaAPI.StartAdapterOauthSetupInput(
          connectionId: connection.id,
          expectedConnectionRevision: expectedConnectionRevision,
          expectedCredentialRevision: Int32(adapterConnection.credentialRevision),
          expectedGrantRevision: Int32(adapterConnection.grantRevision),
          expectedPolicyRevision: Int32(connection.policyRevision)
        )
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      isOffline = false
      return URL(string: response.data?.startAdapterOauthSetup.authorizationUrl ?? "")
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  @discardableResult
  func deleteAdapterConnection(_ connection: SettingsIntegrationConnection) async -> Bool {
    guard canMutate, let client else { return false }
    guard let expectedConnectionRevision = Int32(connection.connectionRevision) else {
      errorMessage = "The API connection revision is invalid. Refresh Settings and try again."
      return false
    }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsDeleteAdapterConnectionMutation(
        input: NoemaAPI.DeleteAdapterConnectionInput(
          connectionId: connection.id,
          expectedConnectionRevision: expectedConnectionRevision
        )
      ))
    }
  }

  @discardableResult
  func deleteAdapterService(_ definition: SettingsIntegration) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsDeleteAdapterServiceMutation(
        input: NoemaAPI.DeleteAdapterServiceInput(
          definitionId: definition.id,
          expectedSourceRevision: definition.sourceRevision
        )
      ))
    }
  }

  func addMCPConnection(
    definitionID: String,
    sourceRevision: String,
    label: String,
    secretEnv: [String: String],
    secretHeaders: [String: String],
    oauthClientID: String,
    oauthClientSecret: String,
    oauthScopes: [String]
  ) async -> SettingsMCPSetupResult? {
    guard canMutate, let client else { return nil }
    let credentials = oauthClientID.nilIfBlank.map {
      NoemaAPI.McpOAuthClientCredentialsInput(clientId: $0, clientSecret: oauthClientSecret, scopes: oauthScopes)
    }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsAddMcpConnectionMutation(
        input: NoemaAPI.AddMcpConnectionInput(
          mcpDefinitionId: definitionID,
          expectedDefinitionRevision: sourceRevision,
          connectionLabel: Self.optional(label.nilIfBlank),
          secretEnv: Self.json(secretEnv),
          secretHeaders: Self.json(secretHeaders),
          oauthClientCredentials: credentials.map { .some($0) } ?? .none,
          authPreference: .some(GraphQLEnum(.useAnonymous))
        )
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      isOffline = false
      await load(client: client)
      return response.data.map(Self.mcpSetupResult(from:))
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  func createMCPServer(input: NoemaAPI.CreateMcpServerInput) async -> SettingsMCPSetupResult? {
    guard canMutate, let client else { return nil }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsCreateMcpServerMutation(input: input))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      isOffline = false
      await load(client: client)
      return response.data.map(Self.mcpSetupResult(from:))
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  func startMCPServerOAuth(input: NoemaAPI.CreateMcpServerInput, redirectURI: String) async -> SettingsMCPAuthAttempt? {
    guard canMutate, let client else { return nil }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsStartMcpServerOauthSetupMutation(
        input: NoemaAPI.StartMcpServerOAuthSetupInput(server: input, redirectUri: redirectURI)
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      isOffline = false
      return response.data.map(Self.mcpAuthAttempt(from:))
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  func startMCPReauthentication(serverID: String, redirectURI: String) async -> SettingsMCPAuthAttempt? {
    guard canMutate, let client else { return nil }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsStartMcpServerReauthenticationOauthSetupMutation(
        input: NoemaAPI.StartMcpServerReauthenticationOAuthSetupInput(
          mcpServerId: serverID,
          redirectUri: redirectURI
        )
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      isOffline = false
      return response.data.map(Self.mcpAuthAttempt(from:))
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  func loadMCPAuthAttempt(_ attemptID: String) async -> SettingsMCPAuthAttempt? {
    guard let client else { return nil }
    do {
      let response = try await client.fetch(query: NoemaAPI.SettingsMcpOauthSetupAttemptQuery(attemptId: attemptID), cachePolicy: .networkOnly)
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      return response.data.flatMap(Self.mcpAuthAttempt(from:))
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  func continueMCPServerSetup(serverID: String, secretEnv: [String: String], secretHeaders: [String: String], oauthClientID: String, oauthClientSecret: String, oauthScopes: [String]) async -> SettingsMCPSetupResult? {
    guard canMutate, let client else { return nil }
    let credentials = oauthClientID.nilIfBlank.map {
      NoemaAPI.McpOAuthClientCredentialsInput(clientId: $0, clientSecret: oauthClientSecret, scopes: oauthScopes)
    }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsContinueMcpServerSetupMutation(
        input: NoemaAPI.ContinueMcpServerSetupInput(
          mcpServerId: serverID,
          secretEnv: Self.json(secretEnv),
          secretHeaders: Self.json(secretHeaders),
          oauthClientCredentials: credentials.map { .some($0) } ?? .none
        )
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      isOffline = false
      await load(client: client)
      return response.data.map(Self.mcpSetupResult(from:))
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  @discardableResult
  func deleteMCPServer(_ serverID: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsDeleteMcpServerMutation(mcpServerId: serverID))
    }
  }

  static func json(_ value: [String: String]) -> GraphQLNullable<NoemaAPI.JSON> {
    guard !value.isEmpty else { return .none }
    guard let json = NoemaAPI.JSON(foundationValue: value) else { return .none }
    return .some(json)
  }

  private static func adapterDefinition(from value: NoemaAPI.SettingsAdapterDefinitionsQuery.Data.AdapterDefinition) -> SettingsAdapterDefinition {
    SettingsAdapterDefinition(
      id: value.semanticDigest,
      semanticDigest: value.semanticDigest,
      definitionId: value.definitionId,
      displayName: value.displayName,
      definitionRevision: value.definitionRevision,
      sourceReference: URL(string: value.sourceReference),
      authenticationMode: value.authenticationMode,
      accountIdentityOperationID: value.accountIdentityOperationId,
      scopes: value.scopes,
      clientSetupURL: value.clientSetupUrl.flatMap(URL.init(string:)),
      operations: value.operations.map {
        SettingsAdapterOperation(
          id: $0.operationId,
          method: $0.method,
          path: $0.path,
          arguments: $0.argumentNames,
          responseTransform: $0.responseTransform.map {
            SettingsAdapterResponseTransform(language: $0.language, sourceDigest: $0.sourceDigest, source: $0.source, acceptedContentTypes: $0.acceptedContentTypes, outputSchemaJSON: $0.outputSchemaJson)
          }
        )
      },
      manifestJSON: value.manifestJson,
      acceptsOAuthClientJSON: value.acceptsOauthClientJson,
      reviewed: value.reviewed,
      superseded: value.superseded,
      connections: value.connections.map { SettingsAdapterConnection(id: $0.connectionId, status: $0.status, connectionRevision: $0.connectionRevision, credentialRevision: $0.credentialRevision, grantRevision: $0.grantRevision, policyRevision: $0.policyRevision, policyConfigured: $0.policyConfigured) }
    )
  }

  private static func mcpServer(id: String, connectionRevision: String, policyRevision: Int, displayName: String, transportKind: String, healthStatus: String, authStatus: String, toolCount: Int, browserOAuthReauthenticationSupported: Bool) -> SettingsMCPServer {
    SettingsMCPServer(id: id, connectionRevision: connectionRevision, policyRevision: policyRevision, displayName: displayName, transportKind: transportKind, healthStatus: healthStatus, authStatus: authStatus, toolCount: toolCount, browserOAuthReauthenticationSupported: browserOAuthReauthenticationSupported)
  }

  private static func mcpSetupResult(server: SettingsMCPServer?, setupStatus: String, discoveryStatus: String?, discoveredToolCount: Int, setupError: String?, oauthClientCredentialsSupported: Bool, oauthAuthorizationSupported: Bool, scopes: [String]) -> SettingsMCPSetupResult {
    SettingsMCPSetupResult(server: server, setupStatus: setupStatus, discoveryStatus: discoveryStatus, discoveredToolCount: discoveredToolCount, setupError: setupError, oauthClientCredentialsSupported: oauthClientCredentialsSupported, oauthAuthorizationSupported: oauthAuthorizationSupported, scopes: scopes)
  }

  private static func mcpSetupResult(from value: NoemaAPI.SettingsAddMcpConnectionMutation.Data) -> SettingsMCPSetupResult {
    mcpSetupResult(from: value.addMcpConnection.fragments.settingsMcpSetupResultFields)
  }

  private static func mcpSetupResult(from value: NoemaAPI.SettingsCreateMcpServerMutation.Data) -> SettingsMCPSetupResult {
    mcpSetupResult(from: value.createMcpServer.fragments.settingsMcpSetupResultFields)
  }

  private static func mcpSetupResult(from value: NoemaAPI.SettingsContinueMcpServerSetupMutation.Data) -> SettingsMCPSetupResult {
    mcpSetupResult(from: value.continueMcpServerSetup.fragments.settingsMcpSetupResultFields)
  }

  private static func mcpSetupResult(from result: NoemaAPI.SettingsMcpSetupResultFields) -> SettingsMCPSetupResult {
    mcpSetupResult(server: result.server.map { mcpServer(id: $0.mcpServerId, connectionRevision: $0.connectionRevision, policyRevision: $0.policyRevision, displayName: $0.displayName, transportKind: $0.transportKind, healthStatus: $0.healthStatus, authStatus: $0.authStatus, toolCount: $0.toolCount, browserOAuthReauthenticationSupported: $0.browserOauthReauthenticationSupported) }, setupStatus: result.setupStatus, discoveryStatus: result.discoveryStatus, discoveredToolCount: result.discoveredToolCount, setupError: result.setupError, oauthClientCredentialsSupported: result.auth?.oauthClientCredentialsSupported ?? false, oauthAuthorizationSupported: result.auth?.oauthAuthorizationSupported ?? false, scopes: result.auth?.scopes ?? [])
  }

  private static func mcpAuthAttempt(from value: NoemaAPI.SettingsStartMcpServerOauthSetupMutation.Data) -> SettingsMCPAuthAttempt {
    let attempt = value.startMcpServerOauthSetup
    return SettingsMCPAuthAttempt(attemptID: attempt.attemptId, status: attempt.status, authorizationURL: attempt.authorizationUrl.flatMap(URL.init(string:)), errorMessage: attempt.errorMessage, setupResult: attempt.setupResult.map { mcpSetupResult(from: $0.fragments.settingsMcpSetupResultFields) })
  }

  private static func mcpAuthAttempt(from value: NoemaAPI.SettingsStartMcpServerReauthenticationOauthSetupMutation.Data) -> SettingsMCPAuthAttempt {
    let attempt = value.startMcpServerReauthenticationOauthSetup
    return SettingsMCPAuthAttempt(attemptID: attempt.attemptId, status: attempt.status, authorizationURL: attempt.authorizationUrl.flatMap(URL.init(string:)), errorMessage: attempt.errorMessage, setupResult: attempt.setupResult.map { mcpSetupResult(from: $0.fragments.settingsMcpSetupResultFields) })
  }

  private static func mcpAuthAttempt(from value: NoemaAPI.SettingsMcpOauthSetupAttemptQuery.Data) -> SettingsMCPAuthAttempt? {
    guard let attempt = value.mcpOauthSetupAttempt else { return nil }
    let result = attempt.setupResult.map { mcpSetupResult(from: $0.fragments.settingsMcpSetupResultFields) }
    return SettingsMCPAuthAttempt(attemptID: attempt.attemptId, status: attempt.status, authorizationURL: attempt.authorizationUrl.flatMap(URL.init(string:)), errorMessage: attempt.errorMessage, setupResult: result)
  }
}

private extension String {
  var nilIfBlank: String? {
    let value = trimmingCharacters(in: .whitespacesAndNewlines)
    return value.isEmpty ? nil : value
  }
}
