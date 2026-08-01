// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct PendingChatInterventionsQuery: GraphQLQuery {
  public static let operationName: String = "PendingChatInterventions"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query PendingChatInterventions($conversationId: String, $taskId: String, $projectId: String, $first: Int = 50) { pendingHumanInterventions( conversationId: $conversationId taskId: $taskId projectId: $projectId first: $first ) { __typename ... on TaskAttention { kind title summary gate { __typename ...TasksGateFields } task { __typename ...TasksTaskCardFields } validActions } ... on GovernedAction { actionId revision governedConversationId: conversationId taskId runId capabilityName reviewRoute behavior { __typename readOnly idempotent destructive openWorld } safeSummary arguments governedState: state output failureCode } ... on McpAuthenticationIntervention { requestId revision mcpAuthConversationId: conversationId taskId runId mcpAuthServerId: mcpServerId serverDisplayName capabilityName mcpAuthState: state failureCode } ... on AdapterAuthenticationIntervention { requestId revision adapterAuthConversationId: conversationId taskId runId adapterConnectionId serviceDisplayName capabilityName adapterAuthState: state failureCode } ... on McpSetupIntervention { itemId setupConversationId: conversationId setupStatus displayName description serviceUrl endpointUrl oauthSupported discoveredToolCount setupMcpServerId: mcpServerId connectionRevision policyRevision toolCount } ... on AdapterDefinition { semanticDigest definitionId displayName definitionRevision sourceReference origin authenticationMode scopes clientSetupUrl oauthRedirectUri accountIdentityOperationId manifestJson acceptsOauthClientJson connectionCount reviewed superseded connections { __typename connectionId status accountKind connectionRevision credentialRevision grantRevision policyRevision grantedScopes allowedOperations policyConfigured } operations { __typename operationId method path readOnly idempotent destructive openWorld argumentNames responseTransform { __typename language sourceDigest source acceptedContentTypes outputSchemaJson } } } } }"#,
      fragments: [TasksCurrentRunFields.self, TasksGateFields.self, TasksProjectFields.self, TasksReviewSummaryFields.self, TasksStageFields.self, TasksTaskCardFields.self, TasksWorkspaceFields.self]
    ))

  public var conversationId: GraphQLNullable<String>
  public var taskId: GraphQLNullable<String>
  public var projectId: GraphQLNullable<String>
  public var first: GraphQLNullable<Int32>

  public init(
    conversationId: GraphQLNullable<String>,
    taskId: GraphQLNullable<String>,
    projectId: GraphQLNullable<String>,
    first: GraphQLNullable<Int32> = 50
  ) {
    self.conversationId = conversationId
    self.taskId = taskId
    self.projectId = projectId
    self.first = first
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "conversationId": conversationId,
    "taskId": taskId,
    "projectId": projectId,
    "first": first
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("pendingHumanInterventions", [PendingHumanIntervention].self, arguments: [
        "conversationId": .variable("conversationId"),
        "taskId": .variable("taskId"),
        "projectId": .variable("projectId"),
        "first": .variable("first")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      PendingChatInterventionsQuery.Data.self
    ] }

    /// List unresolved task, permission, setup, and sign-in interventions for the current human.
    public var pendingHumanInterventions: [PendingHumanIntervention] { __data["pendingHumanInterventions"] }

    /// PendingHumanIntervention
    ///
    /// Parent Type: `HumanIntervention`
    nonisolated public struct PendingHumanIntervention: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Unions.HumanIntervention }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .inlineFragment(AsTaskAttention.self),
        .inlineFragment(AsGovernedAction.self),
        .inlineFragment(AsMcpAuthenticationIntervention.self),
        .inlineFragment(AsAdapterAuthenticationIntervention.self),
        .inlineFragment(AsMcpSetupIntervention.self),
        .inlineFragment(AsAdapterDefinition.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        PendingChatInterventionsQuery.Data.PendingHumanIntervention.self
      ] }

      public var asTaskAttention: AsTaskAttention? { _asInlineFragment() }
      public var asGovernedAction: AsGovernedAction? { _asInlineFragment() }
      public var asMcpAuthenticationIntervention: AsMcpAuthenticationIntervention? { _asInlineFragment() }
      public var asAdapterAuthenticationIntervention: AsAdapterAuthenticationIntervention? { _asInlineFragment() }
      public var asMcpSetupIntervention: AsMcpSetupIntervention? { _asInlineFragment() }
      public var asAdapterDefinition: AsAdapterDefinition? { _asInlineFragment() }

      /// PendingHumanIntervention.AsTaskAttention
      ///
      /// Parent Type: `TaskAttention`
      nonisolated public struct AsTaskAttention: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = PendingChatInterventionsQuery.Data.PendingHumanIntervention
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskAttention }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("kind", GraphQLEnum<NoemaAPI.TaskAttentionKind>.self),
          .field("title", String.self),
          .field("summary", String.self),
          .field("gate", Gate?.self),
          .field("task", Task.self),
          .field("validActions", [GraphQLEnum<NoemaAPI.ValidTaskAction>].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.self,
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsTaskAttention.self
        ] }

        /// Clarification, approval, recovery, or review readiness.
        public var kind: GraphQLEnum<NoemaAPI.TaskAttentionKind> { __data["kind"] }
        /// Stable UI title.
        public var title: String { __data["title"] }
        /// Safe summary.
        public var summary: String { __data["summary"] }
        /// Complete related gate evidence, when any.
        public var gate: Gate? { __data["gate"] }
        /// Authoritative task card without recursively embedding attention.
        public var task: Task { __data["task"] }
        /// Server-authorized actions.
        public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }

        /// PendingHumanIntervention.AsTaskAttention.Gate
        ///
        /// Parent Type: `TaskGate`
        nonisolated public struct Gate: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskGate }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksGateFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsTaskAttention.Gate.self,
            TasksGateFields.self
          ] }

          /// Gate identity.
          public var gateId: String { __data["gateId"] }
          /// Task generation.
          public var taskGeneration: Int { __data["taskGeneration"] }
          /// Gate kind.
          public var kind: GraphQLEnum<NoemaAPI.TaskGateKind> { __data["kind"] }
          /// Gate state.
          public var state: GraphQLEnum<NoemaAPI.TaskGateState> { __data["state"] }
          /// Recovery reason, when any.
          public var recoveryReason: GraphQLEnum<NoemaAPI.TaskRecoveryReason>? { __data["recoveryReason"] }
          /// Explicit recovery continuation role, when any.
          public var retryRunKind: GraphQLEnum<NoemaAPI.TaskRunKind>? { __data["retryRunKind"] }
          /// Human prompt.
          public var prompt: String { __data["prompt"] }
          /// Bounded context.
          public var contextMarkdown: String { __data["contextMarkdown"] }
          /// Optional direct answers.
          public var suggestedAnswers: [String] { __data["suggestedAnswers"] }
          /// Opener actor.
          public var openedBy: String { __data["openedBy"] }
          /// Opening run, when any.
          public var originatingRunId: String? { __data["originatingRunId"] }
          /// Open timestamp.
          public var openedAt: String { __data["openedAt"] }
          /// Resolver actor, when resolved.
          public var resolvedBy: String? { __data["resolvedBy"] }
          /// Resolution timestamp, when resolved.
          public var resolvedAt: String? { __data["resolvedAt"] }
          /// Resolution message identity, when resolved.
          public var resolution: String? { __data["resolution"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var tasksGateFields: TasksGateFields { _toFragment() }
          }
        }

        /// PendingHumanIntervention.AsTaskAttention.Task
        ///
        /// Parent Type: `TaskCard`
        nonisolated public struct Task: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCard }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksTaskCardFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsTaskAttention.Task.self,
            TasksTaskCardFields.self
          ] }

          /// Task identity.
          public var taskId: String { __data["taskId"] }
          /// Workspace placement.
          public var workspace: Workspace { __data["workspace"] }
          /// Nullable project placement.
          public var project: Project? { __data["project"] }
          /// Human title.
          public var title: String { __data["title"] }
          /// Bounded description preview.
          public var descriptionPreview: String { __data["descriptionPreview"] }
          /// The only task-level state.
          public var stage: Stage { __data["stage"] }
          /// Optimistic revision.
          public var revision: Int { __data["revision"] }
          /// Execution generation fence.
          public var generation: Int { __data["generation"] }
          /// Creation timestamp.
          public var createdAt: String { __data["createdAt"] }
          /// Last update timestamp.
          public var updatedAt: String { __data["updatedAt"] }
          /// Completion timestamp, when any.
          public var completedAt: String? { __data["completedAt"] }
          /// Current run projection.
          public var currentRun: CurrentRun? { __data["currentRun"] }
          /// Open gate projection.
          public var activeGate: ActiveGate? { __data["activeGate"] }
          /// Latest review projection.
          public var latestReview: LatestReview? { __data["latestReview"] }
          /// Server-authorized actions.
          public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var tasksTaskCardFields: TasksTaskCardFields { _toFragment() }
          }

          public typealias Workspace = TasksTaskCardFields.Workspace

          public typealias Project = TasksTaskCardFields.Project

          public typealias Stage = TasksTaskCardFields.Stage

          public typealias CurrentRun = TasksTaskCardFields.CurrentRun

          public typealias ActiveGate = TasksTaskCardFields.ActiveGate

          public typealias LatestReview = TasksTaskCardFields.LatestReview
        }
      }

      /// PendingHumanIntervention.AsGovernedAction
      ///
      /// Parent Type: `GovernedAction`
      nonisolated public struct AsGovernedAction: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = PendingChatInterventionsQuery.Data.PendingHumanIntervention
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GovernedAction }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("actionId", String.self),
          .field("revision", Int.self),
          .field("conversationId", alias: "governedConversationId", String?.self),
          .field("taskId", String?.self),
          .field("runId", String?.self),
          .field("capabilityName", String.self),
          .field("reviewRoute", GraphQLEnum<NoemaAPI.ExecutionReviewRoute>.self),
          .field("behavior", Behavior?.self),
          .field("safeSummary", String.self),
          .field("arguments", NoemaAPI.JSON.self),
          .field("state", alias: "governedState", GraphQLEnum<NoemaAPI.GovernedActionState>.self),
          .field("output", NoemaAPI.JSON?.self),
          .field("failureCode", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.self,
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsGovernedAction.self
        ] }

        public var actionId: String { __data["actionId"] }
        public var revision: Int { __data["revision"] }
        public var governedConversationId: String? { __data["governedConversationId"] }
        public var taskId: String? { __data["taskId"] }
        public var runId: String? { __data["runId"] }
        public var capabilityName: String { __data["capabilityName"] }
        public var reviewRoute: GraphQLEnum<NoemaAPI.ExecutionReviewRoute> { __data["reviewRoute"] }
        public var behavior: Behavior? { __data["behavior"] }
        public var safeSummary: String { __data["safeSummary"] }
        public var arguments: NoemaAPI.JSON { __data["arguments"] }
        public var governedState: GraphQLEnum<NoemaAPI.GovernedActionState> { __data["governedState"] }
        public var output: NoemaAPI.JSON? { __data["output"] }
        public var failureCode: String? { __data["failureCode"] }

        /// PendingHumanIntervention.AsGovernedAction.Behavior
        ///
        /// Parent Type: `ToolBehavior`
        nonisolated public struct Behavior: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ToolBehavior }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("readOnly", Bool.self),
            .field("idempotent", Bool.self),
            .field("destructive", Bool.self),
            .field("openWorld", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsGovernedAction.Behavior.self
          ] }

          public var readOnly: Bool { __data["readOnly"] }
          public var idempotent: Bool { __data["idempotent"] }
          public var destructive: Bool { __data["destructive"] }
          public var openWorld: Bool { __data["openWorld"] }
        }
      }

      /// PendingHumanIntervention.AsMcpAuthenticationIntervention
      ///
      /// Parent Type: `McpAuthenticationIntervention`
      nonisolated public struct AsMcpAuthenticationIntervention: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = PendingChatInterventionsQuery.Data.PendingHumanIntervention
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpAuthenticationIntervention }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("requestId", String.self),
          .field("revision", Int.self),
          .field("conversationId", alias: "mcpAuthConversationId", String?.self),
          .field("taskId", String?.self),
          .field("runId", String?.self),
          .field("mcpServerId", alias: "mcpAuthServerId", String.self),
          .field("serverDisplayName", String.self),
          .field("capabilityName", String.self),
          .field("state", alias: "mcpAuthState", GraphQLEnum<NoemaAPI.McpAuthenticationRequestState>.self),
          .field("failureCode", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.self,
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsMcpAuthenticationIntervention.self
        ] }

        public var requestId: String { __data["requestId"] }
        public var revision: Int { __data["revision"] }
        public var mcpAuthConversationId: String? { __data["mcpAuthConversationId"] }
        public var taskId: String? { __data["taskId"] }
        public var runId: String? { __data["runId"] }
        public var mcpAuthServerId: String { __data["mcpAuthServerId"] }
        public var serverDisplayName: String { __data["serverDisplayName"] }
        public var capabilityName: String { __data["capabilityName"] }
        public var mcpAuthState: GraphQLEnum<NoemaAPI.McpAuthenticationRequestState> { __data["mcpAuthState"] }
        public var failureCode: String? { __data["failureCode"] }
      }

      /// PendingHumanIntervention.AsAdapterAuthenticationIntervention
      ///
      /// Parent Type: `AdapterAuthenticationIntervention`
      nonisolated public struct AsAdapterAuthenticationIntervention: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = PendingChatInterventionsQuery.Data.PendingHumanIntervention
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterAuthenticationIntervention }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("requestId", String.self),
          .field("revision", Int.self),
          .field("conversationId", alias: "adapterAuthConversationId", String?.self),
          .field("taskId", String?.self),
          .field("runId", String?.self),
          .field("adapterConnectionId", String.self),
          .field("serviceDisplayName", String.self),
          .field("capabilityName", String.self),
          .field("state", alias: "adapterAuthState", GraphQLEnum<NoemaAPI.McpAuthenticationRequestState>.self),
          .field("failureCode", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.self,
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsAdapterAuthenticationIntervention.self
        ] }

        public var requestId: String { __data["requestId"] }
        public var revision: Int { __data["revision"] }
        public var adapterAuthConversationId: String? { __data["adapterAuthConversationId"] }
        public var taskId: String? { __data["taskId"] }
        public var runId: String? { __data["runId"] }
        public var adapterConnectionId: String { __data["adapterConnectionId"] }
        public var serviceDisplayName: String { __data["serviceDisplayName"] }
        public var capabilityName: String { __data["capabilityName"] }
        public var adapterAuthState: GraphQLEnum<NoemaAPI.McpAuthenticationRequestState> { __data["adapterAuthState"] }
        public var failureCode: String? { __data["failureCode"] }
      }

      /// PendingHumanIntervention.AsMcpSetupIntervention
      ///
      /// Parent Type: `McpSetupIntervention`
      nonisolated public struct AsMcpSetupIntervention: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = PendingChatInterventionsQuery.Data.PendingHumanIntervention
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpSetupIntervention }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("itemId", String.self),
          .field("conversationId", alias: "setupConversationId", String.self),
          .field("setupStatus", String.self),
          .field("displayName", String.self),
          .field("description", String?.self),
          .field("serviceUrl", String.self),
          .field("endpointUrl", String.self),
          .field("oauthSupported", Bool.self),
          .field("discoveredToolCount", Int.self),
          .field("mcpServerId", alias: "setupMcpServerId", String?.self),
          .field("connectionRevision", String?.self),
          .field("policyRevision", Int?.self),
          .field("toolCount", Int?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.self,
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsMcpSetupIntervention.self
        ] }

        public var itemId: String { __data["itemId"] }
        public var setupConversationId: String { __data["setupConversationId"] }
        public var setupStatus: String { __data["setupStatus"] }
        public var displayName: String { __data["displayName"] }
        public var description: String? { __data["description"] }
        public var serviceUrl: String { __data["serviceUrl"] }
        public var endpointUrl: String { __data["endpointUrl"] }
        public var oauthSupported: Bool { __data["oauthSupported"] }
        public var discoveredToolCount: Int { __data["discoveredToolCount"] }
        public var setupMcpServerId: String? { __data["setupMcpServerId"] }
        public var connectionRevision: String? { __data["connectionRevision"] }
        public var policyRevision: Int? { __data["policyRevision"] }
        public var toolCount: Int? { __data["toolCount"] }
      }

      /// PendingHumanIntervention.AsAdapterDefinition
      ///
      /// Parent Type: `AdapterDefinition`
      nonisolated public struct AsAdapterDefinition: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = PendingChatInterventionsQuery.Data.PendingHumanIntervention
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("semanticDigest", String.self),
          .field("definitionId", String.self),
          .field("displayName", String.self),
          .field("definitionRevision", String.self),
          .field("sourceReference", String.self),
          .field("origin", String.self),
          .field("authenticationMode", String.self),
          .field("scopes", [String].self),
          .field("clientSetupUrl", String?.self),
          .field("oauthRedirectUri", String?.self),
          .field("accountIdentityOperationId", String?.self),
          .field("manifestJson", String.self),
          .field("acceptsOauthClientJson", Bool.self),
          .field("connectionCount", Int.self),
          .field("reviewed", Bool.self),
          .field("superseded", Bool.self),
          .field("connections", [Connection].self),
          .field("operations", [Operation].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.self,
          PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsAdapterDefinition.self
        ] }

        public var semanticDigest: String { __data["semanticDigest"] }
        public var definitionId: String { __data["definitionId"] }
        public var displayName: String { __data["displayName"] }
        public var definitionRevision: String { __data["definitionRevision"] }
        public var sourceReference: String { __data["sourceReference"] }
        public var origin: String { __data["origin"] }
        public var authenticationMode: String { __data["authenticationMode"] }
        public var scopes: [String] { __data["scopes"] }
        public var clientSetupUrl: String? { __data["clientSetupUrl"] }
        public var oauthRedirectUri: String? { __data["oauthRedirectUri"] }
        public var accountIdentityOperationId: String? { __data["accountIdentityOperationId"] }
        public var manifestJson: String { __data["manifestJson"] }
        public var acceptsOauthClientJson: Bool { __data["acceptsOauthClientJson"] }
        public var connectionCount: Int { __data["connectionCount"] }
        public var reviewed: Bool { __data["reviewed"] }
        public var superseded: Bool { __data["superseded"] }
        public var connections: [Connection] { __data["connections"] }
        public var operations: [Operation] { __data["operations"] }

        /// PendingHumanIntervention.AsAdapterDefinition.Connection
        ///
        /// Parent Type: `AdapterConnection`
        nonisolated public struct Connection: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterConnection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("connectionId", String.self),
            .field("status", String.self),
            .field("accountKind", String.self),
            .field("connectionRevision", Int.self),
            .field("credentialRevision", Int.self),
            .field("grantRevision", Int.self),
            .field("policyRevision", Int.self),
            .field("grantedScopes", [String].self),
            .field("allowedOperations", [String].self),
            .field("policyConfigured", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsAdapterDefinition.Connection.self
          ] }

          public var connectionId: String { __data["connectionId"] }
          public var status: String { __data["status"] }
          public var accountKind: String { __data["accountKind"] }
          public var connectionRevision: Int { __data["connectionRevision"] }
          public var credentialRevision: Int { __data["credentialRevision"] }
          public var grantRevision: Int { __data["grantRevision"] }
          public var policyRevision: Int { __data["policyRevision"] }
          public var grantedScopes: [String] { __data["grantedScopes"] }
          public var allowedOperations: [String] { __data["allowedOperations"] }
          public var policyConfigured: Bool { __data["policyConfigured"] }
        }

        /// PendingHumanIntervention.AsAdapterDefinition.Operation
        ///
        /// Parent Type: `AdapterOperation`
        nonisolated public struct Operation: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOperation }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("operationId", String.self),
            .field("method", String.self),
            .field("path", String.self),
            .field("readOnly", Bool?.self),
            .field("idempotent", Bool?.self),
            .field("destructive", Bool?.self),
            .field("openWorld", Bool?.self),
            .field("argumentNames", [String].self),
            .field("responseTransform", ResponseTransform?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsAdapterDefinition.Operation.self
          ] }

          public var operationId: String { __data["operationId"] }
          public var method: String { __data["method"] }
          public var path: String { __data["path"] }
          public var readOnly: Bool? { __data["readOnly"] }
          public var idempotent: Bool? { __data["idempotent"] }
          public var destructive: Bool? { __data["destructive"] }
          public var openWorld: Bool? { __data["openWorld"] }
          public var argumentNames: [String] { __data["argumentNames"] }
          public var responseTransform: ResponseTransform? { __data["responseTransform"] }

          /// PendingHumanIntervention.AsAdapterDefinition.Operation.ResponseTransform
          ///
          /// Parent Type: `AdapterResponseTransform`
          nonisolated public struct ResponseTransform: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterResponseTransform }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("language", String.self),
              .field("sourceDigest", String.self),
              .field("source", String.self),
              .field("acceptedContentTypes", [String].self),
              .field("outputSchemaJson", String.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              PendingChatInterventionsQuery.Data.PendingHumanIntervention.AsAdapterDefinition.Operation.ResponseTransform.self
            ] }

            public var language: String { __data["language"] }
            public var sourceDigest: String { __data["sourceDigest"] }
            public var source: String { __data["source"] }
            public var acceptedContentTypes: [String] { __data["acceptedContentTypes"] }
            public var outputSchemaJson: String { __data["outputSchemaJson"] }
          }
        }
      }
    }
  }
}
