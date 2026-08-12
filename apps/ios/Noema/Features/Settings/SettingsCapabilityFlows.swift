import Apollo
import ApolloAPI
import Foundation
import NoemaAPI

struct SettingsAdapterConnection: Identifiable, Hashable {
  let id: String
  let status: String
  let grantID: String?
  let accountID: String?
  let connectionRevision: Int
  let credentialRevision: Int?
  let grantRevision: Int?
  let policyRevision: Int
  let policyConfigured: Bool
  let operationAccess: [SettingsAdapterOperationAccess]
}

struct SettingsAdapterOperationAccess: Hashable {
  let operationID: String
  let status: String
  let missingScopes: [String]
}

struct SettingsAdapterNextAction: Hashable {
  let kind: String
  let semanticDigest: String
  let applicationID: String?
  let applicationRevision: Int?
  let grantID: String?
  let grantRevision: Int?
  let connectionID: String?
  let connectionRevision: Int?
  let policyRevision: Int?
  let operationIDs: [String]
  let missingScopes: [String]
}

struct SettingsAdapterOperation: Identifiable, Hashable {
  let id: String
  let method: String
  let path: String
  let readOnly: Bool?
  let idempotent: Bool?
  let destructive: Bool?
  let openWorld: Bool?
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
  let adapterId: String
  let displayName: String
  let definitionRevision: String
  let sourceReference: URL?
  let origin: String
  let authenticationMode: String
  let oauthProfileDigest: String?
  let accountIdentityOperationID: String?
  let manifestJSON: String
  let scopes: [String]
  let credentialSetup: AdapterCredentialSetupModel?
  let operations: [SettingsAdapterOperation]
  let reviewed: Bool
  let superseded: Bool
  let connections: [SettingsAdapterConnection]
  let nextAction: SettingsAdapterNextAction?
  let connectionActions: [SettingsAdapterNextAction]
}

struct SettingsAdapterOAuthProfile: Identifiable, Hashable {
  var id: String { profileDigest }
  let profileDigest: String
  let displayName: String
  let audience: String
  let credentialSetup: AdapterCredentialSetupModel?
}

struct SettingsAdapterOAuthApplication: Identifiable, Hashable {
  var id: String { applicationID }
  let applicationID: String
  let profileDigest: String
  let providerName: String
  let callbackMode: String
  let clientID: String
  let projectLabel: String?
  let revision: Int
  let status: String
  let grantCount: Int
  let accountCount: Int
}

struct SettingsAdapterOAuthGrant: Identifiable, Hashable {
  var id: String { grantID }
  let grantID: String
  let applicationID: String
  let accountID: String?
  let accountLabel: String?
  let providerName: String
  let desiredScopes: [String]
  let grantedScopes: [String]
  let authorityRevision: Int
  let status: String
  let connectionIDs: [String]
}

struct SettingsAdapterOAuthState: Hashable {
  let profiles: [SettingsAdapterOAuthProfile]
  let applications: [SettingsAdapterOAuthApplication]
  let grants: [SettingsAdapterOAuthGrant]
}

struct SettingsAdapterOAuthAttempt: Hashable {
  let attemptID: String
  let authorizationURL: URL
  let expiresAt: Date
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
          adapterDefinitions = data.adapterDefinitions.map {
            Self.adapterDefinition(from: $0.fragments.settingsAdapterDefinitionFields)
          }
        }
        if let message = response.errors?.first?.message, adapterDefinitions.isEmpty { errorMessage = message }
      }
    } catch {
      if adapterDefinitions.isEmpty { errorMessage = "API definitions could not be loaded." }
    }
  }

  func loadAdapterOAuthState(client: ApolloClient? = nil) async {
    guard let client = client ?? self.client else { return }
    do {
      let stream = try client.fetch(query: NoemaAPI.SettingsAdapterOauthStateQuery(), cachePolicy: .cacheAndNetwork)
      for try await response in stream {
        guard let state = response.data?.adapterOauthState else { continue }
        adapterOAuthState = SettingsAdapterOAuthState(
          profiles: state.profiles.map { SettingsAdapterOAuthProfile(
            profileDigest: $0.profileDigest,
            displayName: $0.displayName,
            audience: $0.grantAudience,
            credentialSetup: $0.credentialSetup.map { AdapterCredentialSetupModel($0.fragments.adapterCredentialSetupFields) }
          ) },
          applications: state.applications.map { SettingsAdapterOAuthApplication(
            applicationID: $0.applicationId,
            profileDigest: $0.profileDigest,
            providerName: $0.providerDisplayName,
            callbackMode: $0.callbackMode,
            clientID: $0.clientId,
            projectLabel: $0.projectLabel,
            revision: $0.revision,
            status: $0.status,
            grantCount: $0.grantCount,
            accountCount: $0.accountCount
          ) },
          grants: state.grants.map { SettingsAdapterOAuthGrant(
            grantID: $0.grantId,
            applicationID: $0.applicationId,
            accountID: $0.accountId,
            accountLabel: $0.accountLabel,
            providerName: $0.providerDisplayName,
            desiredScopes: $0.desiredScopes,
            grantedScopes: $0.grantedScopes,
            authorityRevision: $0.authorityRevision,
            status: $0.status,
            connectionIDs: $0.connectionIds
          ) }
        )
      }
    } catch {
      if adapterOAuthState == nil { errorMessage = "API accounts could not be loaded." }
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
  func cancelAdapterDefinition(_ semanticDigest: String) async -> Bool {
    guard canMutate, let client else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsCancelAdapterDefinitionMutation(
        input: NoemaAPI.CancelAdapterDefinitionInput(semanticDigest: semanticDigest)
      ))
    }
  }

  @discardableResult
  func setupAdapterConnection(_ semanticDigest: String, submission: AdapterCredentialSubmission) async -> Bool {
    guard canMutate, let client else { return false }
    if let document = submission.document, document.isEmpty || document.count > 128 * 1024 {
      errorMessage = AdapterCredentialError.invalidDocument.localizedDescription
      return false
    }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsSetupAdapterConnectionMutation(
        input: NoemaAPI.SetupAdapterConnectionInput(
          semanticDigest: semanticDigest,
          fieldValues: submission.fieldValues.map {
            NoemaAPI.AdapterCredentialFieldValueInput(fieldId: $0.fieldID, value: $0.value)
          },
          documentBase64: submission.document.map { .some($0.base64EncodedString()) } ?? .none
        )
      ))
    }
  }

  func startAdapterOAuth(_ action: SettingsAdapterNextAction) async -> SettingsAdapterOAuthAttempt? {
    guard canMutate, let client, let applicationID = action.applicationID,
          let applicationRevision = action.applicationRevision,
          let exactApplicationRevision = Int32(exactly: applicationRevision) else { return nil }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsStartAdapterOauthSetupMutation(
        input: NoemaAPI.StartAdapterOauthSetupInput(
          applicationId: applicationID,
          expectedApplicationRevision: exactApplicationRevision,
          grantId: action.grantID.map { .some($0) } ?? .none,
          expectedGrantRevision: action.grantRevision.flatMap { Int32(exactly: $0) }.map { .some($0) } ?? .none,
          semanticDigest: action.semanticDigest,
          operationIds: action.operationIDs
        )
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      guard let attempt = response.data?.startAdapterOauthSetup,
            let url = URL(string: attempt.authorizationUrl) else {
        throw SettingsError.server("Noema did not return an authorization URL.")
      }
      isOffline = false
      return SettingsAdapterOAuthAttempt(
        attemptID: attempt.attemptId,
        authorizationURL: url,
        expiresAt: Date(timeIntervalSince1970: TimeInterval(attempt.expiresAtEpochSeconds))
      )
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  @discardableResult
  func importAdapterOAuthApplication(profileDigest: String, document: Data) async -> Bool {
    guard canMutate, let client, !document.isEmpty, document.count <= 128 * 1024 else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsImportAdapterOauthApplicationMutation(
        input: NoemaAPI.ImportAdapterOauthApplicationInput(
          profileDigest: profileDigest,
          projectLabel: .none,
          clientDocumentBase64: document.base64EncodedString()
        )
      ))
    }
  }

  @discardableResult
  func replaceAdapterOAuthApplication(_ application: SettingsAdapterOAuthApplication, document: Data) async -> Bool {
    guard canMutate, let client, !document.isEmpty, document.count <= 128 * 1024,
          let revision = Int32(exactly: application.revision) else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsReplaceAdapterOauthApplicationMutation(
        input: NoemaAPI.ReplaceAdapterOauthApplicationInput(
          applicationId: application.applicationID,
          expectedRevision: revision,
          clientDocumentBase64: document.base64EncodedString()
        )
      ))
    }
  }

  @discardableResult
  func deleteAdapterOAuthApplication(_ application: SettingsAdapterOAuthApplication) async -> Bool {
    guard canMutate, let client, let revision = Int32(exactly: application.revision) else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsDeleteAdapterOauthApplicationMutation(
        input: NoemaAPI.DeleteAdapterOauthApplicationInput(
          applicationId: application.applicationID,
          expectedRevision: revision
        )
      ))
    }
  }

  @discardableResult
  func attachAdapterGrant(
    _ action: SettingsAdapterNextAction,
    grantID: String? = nil,
    grantRevision: Int? = nil
  ) async -> String? {
    guard canMutate, let client, let grantID = grantID ?? action.grantID,
          let revision = grantRevision ?? action.grantRevision,
          let exactRevision = Int32(exactly: revision) else { return nil }
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.SettingsAttachAdapterOauthConnectionMutation(
        input: NoemaAPI.AttachAdapterOauthConnectionInput(
          semanticDigest: action.semanticDigest,
          grantId: grantID,
          expectedGrantRevision: exactRevision
        )
      ))
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      guard let connection = response.data?.attachAdapterOauthConnection.connections
        .first(where: { $0.grantId == grantID }) else {
        throw SettingsError.server("The attached API connection is unavailable.")
      }
      isOffline = false
      await load(client: client)
      return connection.connectionId
    } catch {
      errorMessage = error.localizedDescription
      return nil
    }
  }

  @discardableResult
  func disconnectAdapterGrant(_ grant: SettingsAdapterOAuthGrant) async -> Bool {
    guard canMutate, let client, let revision = Int32(exactly: grant.authorityRevision) else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsDisconnectAdapterOauthGrantMutation(
        input: NoemaAPI.DisconnectAdapterOauthGrantInput(
          grantId: grant.grantID,
          expectedAuthorityRevision: revision
        )
      ))
    }
  }

  @discardableResult
  func setAdapterConnectionActive(_ connection: SettingsAdapterConnection, active: Bool) async -> Bool {
    guard canMutate, let client, let revision = Int32(exactly: connection.connectionRevision) else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsSetAdapterConnectionActiveMutation(
        input: NoemaAPI.SetAdapterConnectionActiveInput(
          connectionId: connection.id,
          expectedConnectionRevision: revision,
          active: active
        )
      ))
    }
  }

  @discardableResult
  func labelAdapterGrant(_ grant: SettingsAdapterOAuthGrant, label: String?) async -> Bool {
    guard canMutate, let client, let revision = Int32(exactly: grant.authorityRevision) else { return false }
    return await performMutation {
      try await client.perform(mutation: NoemaAPI.SettingsSaveAdapterOauthGrantLabelMutation(
        input: NoemaAPI.SaveAdapterOauthGrantLabelInput(
          grantId: grant.grantID,
          expectedAuthorityRevision: revision,
          accountLabel: label.map { .some($0) } ?? .none
        )
      ))
    }
  }

  func waitForAdapterOAuth(attemptID: String) async -> (status: String, grantID: String?, grantRevision: Int?)? {
    guard let client else { return nil }
    do {
      let stream = try client.subscribe(
        subscription: NoemaAPI.SettingsAdapterOauthAttemptEventsSubscription(attemptId: attemptID)
      )
      for try await response in stream {
        guard let event = response.data?.adapterOauthAttemptEvents else { continue }
        if event.status != "authorizing" {
          return (event.status, event.grantId, event.grantRevision)
        }
      }
    } catch is CancellationError {
      return nil
    } catch {
      errorMessage = error.localizedDescription
    }
    return nil
  }

  func adapterOAuthAttempt(attemptID: String) async -> (status: String, grantID: String?, grantRevision: Int?)? {
    guard let client else { return nil }
    do {
      let stream = try client.fetch(
        query: NoemaAPI.SettingsAdapterOauthAttemptQuery(attemptId: attemptID),
        cachePolicy: .fetchIgnoringCacheData
      )
      for try await response in stream {
        guard let event = response.data?.adapterOauthAttempt else { continue }
        return (event.status, event.grantId, event.grantRevision)
      }
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

  private static func adapterDefinition(from value: NoemaAPI.SettingsAdapterDefinitionFields) -> SettingsAdapterDefinition {
    SettingsAdapterDefinition(
      id: value.semanticDigest,
      semanticDigest: value.semanticDigest,
      definitionId: value.definitionId,
      adapterId: value.adapterId,
      displayName: value.displayName,
      definitionRevision: value.definitionRevision,
      sourceReference: URL(string: value.sourceReference),
      origin: value.origin,
      authenticationMode: value.authenticationMode,
      oauthProfileDigest: value.oauthProfileDigest,
      accountIdentityOperationID: value.accountIdentityOperationId,
      manifestJSON: value.manifestJson,
      scopes: value.scopes,
      credentialSetup: value.credentialSetup.map {
        AdapterCredentialSetupModel($0.fragments.adapterCredentialSetupFields)
      },
      operations: value.operations.map {
        SettingsAdapterOperation(
          id: $0.operationId,
          method: $0.method,
          path: $0.path,
          readOnly: $0.readOnly,
          idempotent: $0.idempotent,
          destructive: $0.destructive,
          openWorld: $0.openWorld,
          arguments: $0.argumentNames,
          responseTransform: $0.responseTransform.map {
            SettingsAdapterResponseTransform(language: $0.language, sourceDigest: $0.sourceDigest, source: $0.source, acceptedContentTypes: $0.acceptedContentTypes, outputSchemaJSON: $0.outputSchemaJson)
          }
        )
      },
      reviewed: value.reviewed,
      superseded: value.superseded,
      connections: value.connections.map { SettingsAdapterConnection(
        id: $0.connectionId,
        status: $0.status,
        grantID: $0.grantId,
        accountID: $0.accountId,
        connectionRevision: $0.connectionRevision,
        credentialRevision: $0.credentialRevision,
        grantRevision: $0.grantRevision,
        policyRevision: $0.policyRevision,
        policyConfigured: $0.policyConfigured,
        operationAccess: $0.operationAccess.map { SettingsAdapterOperationAccess(
          operationID: $0.operationId,
          status: $0.status,
          missingScopes: $0.missingScopes
        ) }
      ) },
      nextAction: value.nextAction.map { SettingsAdapterNextAction(
        kind: $0.kind,
        semanticDigest: $0.semanticDigest,
        applicationID: $0.applicationId,
        applicationRevision: $0.expectedApplicationRevision,
        grantID: $0.grantId,
        grantRevision: $0.expectedGrantRevision,
        connectionID: $0.connectionId,
        connectionRevision: $0.expectedConnectionRevision,
        policyRevision: $0.expectedPolicyRevision,
        operationIDs: $0.operationIds,
        missingScopes: $0.missingScopes
      ) },
      connectionActions: value.connectionActions.map { SettingsAdapterNextAction(
        kind: $0.kind,
        semanticDigest: $0.semanticDigest,
        applicationID: $0.applicationId,
        applicationRevision: $0.expectedApplicationRevision,
        grantID: $0.grantId,
        grantRevision: $0.expectedGrantRevision,
        connectionID: $0.connectionId,
        connectionRevision: $0.expectedConnectionRevision,
        policyRevision: $0.expectedPolicyRevision,
        operationIDs: $0.operationIds,
        missingScopes: $0.missingScopes
      ) }
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
