import Apollo
import Foundation
import NoemaAPI

enum ChatModelError: LocalizedError {
  case emptyResponse
  case offline
  case server(String)

  var errorDescription: String? {
    switch self {
    case .emptyResponse: "Noema returned an empty response."
    case .offline: "Noema is offline. Reconnect and try again."
    case .server(let message): message
    }
  }
}

enum ChatIntervention: Identifiable, Equatable {
  case governed(GovernedActionModel)
  case mcpAuth(McpAuthModel)
  case adapterAuth(AdapterAuthModel)
  case setup(McpSetupModel)
  case attention(ChatTaskAttentionModel)
  case adapterDefinition(AdapterDefinitionModel)

  var id: String {
    switch self {
    case let .governed(value): value.actionID
    case let .mcpAuth(value): value.requestID
    case let .adapterAuth(value): value.requestID
    case let .setup(value): value.itemID
    case let .attention(value): "task-" + value.taskID + "-" + (value.gate?.id ?? "attention")
    case let .adapterDefinition(value): "adapter-" + value.semanticDigest
    }
  }
}

struct GovernedActionModel: Equatable {
  let actionID: String
  let revision: Int
  let summary: String
  let consequence: String
  let state: String
  let capabilityName: String
  let reviewRoute: String
  let readOnly: Bool?
  let idempotent: Bool?
  let destructive: Bool?
  let openWorld: Bool?
  let serviceName: String?
  let connectionLabel: String?
  let serviceID: String?
  let sharedContent: String?
  let taskID: String?
  let failureCode: String?
  let arguments: String

  var targetName: String {
    connectionLabel ?? serviceName ?? serviceID ?? capabilityName
  }

  var question: String {
    if readOnly == true { return "Share request data with \(targetName)?" }
    if destructive == true { return "Allow a destructive change in \(targetName)?" }
    return "Allow this change in \(targetName)?"
  }

  var effect: String {
    if readOnly == nil, idempotent == nil, destructive == nil, openWorld == nil {
      return "No behavior evidence is available."
    }
    var values = [readOnly == true ? "Read only" : "Can make changes"]
    if destructive == true { values.append("Destructive") }
    if openWorld == true { values.append("External system") }
    values.append(idempotent == true ? "Safe to repeat" : "May repeat the effect")
    return values.joined(separator: " · ")
  }
}

struct McpAuthModel: Equatable {
  let requestID: String
  let revision: Int
  let serverName: String
  let capabilityName: String
  let taskID: String?
  let state: String
  let failureCode: String?
}

struct AdapterAuthModel: Equatable {
  let requestID: String
  let revision: Int
  let serviceName: String
  let capabilityName: String
  let taskID: String?
  let state: String
  let failureCode: String?
}

struct McpSetupModel: Equatable {
  let itemID: String
  let conversationID: String
  let serverID: String?
  let status: String
  let displayName: String
  let serviceURL: URL?
  let endpointURL: URL?
  let oauthSupported: Bool
  let description: String?
  let discoveredToolCount: Int
  let connectionRevision: String?
  let policyRevision: Int?
  let toolCount: Int?
}

struct AdapterDefinitionModel: Equatable {
  let semanticDigest: String
  let definitionID: String
  let adapterID: String
  let displayName: String
  let definitionRevision: String
  let sourceReference: URL?
  let credentialSetup: AdapterCredentialSetupModel?
  let scopes: [String]
  let operations: [String]
  let reviewed: Bool
  let superseded: Bool
  let connectionCount: Int
  let origin: String
  let authenticationMode: String
  let accountIdentityOperationID: String?
  let operationDetails: [AdapterOperationModel]
  let connections: [AdapterConnectionModel]

  init(
    semanticDigest: String,
    definitionID: String,
    adapterID: String,
    displayName: String,
    definitionRevision: String,
    sourceReference: URL?,
    credentialSetup: AdapterCredentialSetupModel?,
    scopes: [String],
    operations: [String],
    reviewed: Bool,
    superseded: Bool,
    connectionCount: Int,
    origin: String = "",
    authenticationMode: String = "",
    accountIdentityOperationID: String? = nil,
    operationDetails: [AdapterOperationModel] = [],
    connections: [AdapterConnectionModel] = []
  ) {
    self.semanticDigest = semanticDigest
    self.definitionID = definitionID
    self.adapterID = adapterID
    self.displayName = displayName
    self.definitionRevision = definitionRevision
    self.sourceReference = sourceReference
    self.credentialSetup = credentialSetup
    self.scopes = scopes
    self.operations = operations
    self.reviewed = reviewed
    self.superseded = superseded
    self.connectionCount = connectionCount
    self.origin = origin
    self.authenticationMode = authenticationMode
    self.accountIdentityOperationID = accountIdentityOperationID
    self.operationDetails = operationDetails
    self.connections = connections
  }
}

struct AdapterOperationModel: Equatable, Identifiable {
  let operationID: String
  let method: String
  let path: String
  let readOnly: Bool?
  let idempotent: Bool?
  let destructive: Bool?
  let openWorld: Bool?
  let argumentNames: [String]
  let responseTransform: AdapterResponseTransformModel?

  var id: String { operationID }
  var summary: String { method + " " + path }
}

struct AdapterResponseTransformModel: Equatable {
  let language: String
  let sourceDigest: String
  let source: String
  let acceptedContentTypes: [String]
  let outputSchemaJSON: String
}

struct AdapterConnectionModel: Equatable, Identifiable {
  let connectionID: String
  let status: String
  let accountKind: String
  let connectionRevision: Int
  let credentialRevision: Int
  let grantRevision: Int
  let policyRevision: Int
  let grantedScopes: [String]
  let allowedOperations: [String]
  let policyConfigured: Bool

  var id: String { connectionID }
}

struct AdapterOAuthSetupAttempt: Equatable {
  let attemptID: String
  let authorizationURL: URL
  let expiresAt: Date
}

struct McpSetupServerModel: Equatable {
  let serverID: String
  let connectionRevision: String
  let policyRevision: Int
  let toolCount: Int
}

extension ChatIntervention {
  init?(data: NoemaAPI.PendingChatInterventionsQuery.Data.PendingHumanIntervention) {
    if let action = data.asGovernedAction {
      self = .governed(GovernedActionModel(
        actionID: action.actionId,
        revision: action.revision,
        summary: action.safeSummary,
        consequence: action.consequence,
        state: action.governedState.rawValue,
        capabilityName: action.capabilityName,
        reviewRoute: action.reviewRoute.rawValue,
        readOnly: action.behavior?.readOnly,
        idempotent: action.behavior?.idempotent,
        destructive: action.behavior?.destructive,
        openWorld: action.behavior?.openWorld,
        serviceName: action.target?.serviceName,
        connectionLabel: action.target?.connectionLabel,
        serviceID: action.target?.serviceId,
        sharedContent: action.disclosure?.contentSummary,
        taskID: action.taskId,
        failureCode: action.failureCode,
        arguments: action.arguments.encodedString
      ))
    } else if let auth = data.asMcpAuthenticationIntervention {
      self = .mcpAuth(McpAuthModel(requestID: auth.requestId, revision: auth.revision, serverName: auth.serverDisplayName, capabilityName: auth.capabilityName, taskID: auth.taskId, state: auth.mcpAuthState.rawValue, failureCode: auth.failureCode))
    } else if let auth = data.asAdapterAuthenticationIntervention {
      self = .adapterAuth(AdapterAuthModel(requestID: auth.requestId, revision: auth.revision, serviceName: auth.serviceDisplayName, capabilityName: auth.capabilityName, taskID: auth.taskId, state: auth.adapterAuthState.rawValue, failureCode: auth.failureCode))
    } else if let setup = data.asMcpSetupIntervention {
      self = .setup(McpSetupModel(itemID: setup.itemId, conversationID: setup.setupConversationId, serverID: setup.setupMcpServerId, status: setup.setupStatus, displayName: setup.displayName, serviceURL: URL(string: setup.serviceUrl), endpointURL: URL(string: setup.endpointUrl), oauthSupported: setup.oauthSupported, description: setup.description, discoveredToolCount: setup.discoveredToolCount, connectionRevision: setup.connectionRevision, policyRevision: setup.policyRevision, toolCount: setup.toolCount))
    } else if let attention = data.asTaskAttention {
      let task = attention.task
      let gate = attention.gate.map { mapChatTaskGate($0.fragments.tasksGateFields) } ?? task.activeGate.map { mapChatTaskGate($0.fragments.tasksGateFields) }
      self = .attention(ChatTaskAttentionModel(taskID: task.taskId, title: attention.title, summary: attention.summary, revision: task.revision, generation: task.generation, gate: gate, validActions: Set(attention.validActions.map(\.rawValue))))
    } else if let definition = data.asAdapterDefinition {
      self = .adapterDefinition(AdapterDefinitionModel(
        semanticDigest: definition.semanticDigest,
        definitionID: definition.definitionId,
        adapterID: definition.adapterId,
        displayName: definition.displayName,
        definitionRevision: definition.definitionRevision,
        sourceReference: URL(string: definition.sourceReference),
        credentialSetup: definition.credentialSetup.map {
          AdapterCredentialSetupModel($0.fragments.adapterCredentialSetupFields)
        },
        scopes: definition.scopes,
        operations: definition.operations.map { $0.method + " " + $0.path },
        reviewed: definition.reviewed,
        superseded: definition.superseded,
        connectionCount: definition.connectionCount,
        origin: definition.origin,
        authenticationMode: definition.authenticationMode,
        accountIdentityOperationID: definition.accountIdentityOperationId,
        operationDetails: definition.operations.map { operation in
          AdapterOperationModel(
            operationID: operation.operationId,
            method: operation.method,
            path: operation.path,
            readOnly: operation.readOnly,
            idempotent: operation.idempotent,
            destructive: operation.destructive,
            openWorld: operation.openWorld,
            argumentNames: operation.argumentNames,
            responseTransform: operation.responseTransform.map {
              AdapterResponseTransformModel(
                language: $0.language,
                sourceDigest: $0.sourceDigest,
                source: $0.source,
                acceptedContentTypes: $0.acceptedContentTypes,
                outputSchemaJSON: $0.outputSchemaJson
              )
            }
          )
        },
        connections: definition.connections.map { connection in
          AdapterConnectionModel(
            connectionID: connection.connectionId,
            status: connection.status,
            accountKind: connection.accountKind,
            connectionRevision: connection.connectionRevision,
            credentialRevision: connection.credentialRevision,
            grantRevision: connection.grantRevision,
            policyRevision: connection.policyRevision,
            grantedScopes: connection.grantedScopes,
            allowedOperations: connection.allowedOperations,
            policyConfigured: connection.policyConfigured
          )
        }
      ))
    } else {
      return nil
    }
  }
}

private func mapChatTaskGate(_ source: TasksGateFields) -> ChatTaskGateModel {
  ChatTaskGateModel(id: source.gateId, kind: source.kind.rawValue, prompt: source.prompt, context: source.contextMarkdown, suggestedAnswers: source.suggestedAnswers, recoveryReason: source.recoveryReason?.rawValue)
}

func encodeJSON(_ value: Any?) -> NoemaAPI.JSON? {
  guard let value else { return nil }
  return NoemaAPI.JSON(foundationValue: value)
}
