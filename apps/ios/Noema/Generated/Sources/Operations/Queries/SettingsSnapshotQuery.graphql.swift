// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsSnapshotQuery: GraphQLQuery {
  public static let operationName: String = "SettingsSnapshot"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query SettingsSnapshot { localStatus { __typename localService assistantConnection memoryStorage primaryAgentDisplayName } agents { __typename agentId displayName isPrimary modelPreference { __typename providerKind providerAccountId modelProfile reasoningEffort selectionMode } modelOptions { __typename providerKind providerAccountId providerDisplayName status disabledReason profiles { __typename id label reasoningEfforts defaultReasoningEffort disabledReason } recommendations { __typename useCase modelProfile reasoningEffort disabledReason } } } memorySettings { __typename modelPreference { __typename providerKind providerAccountId modelProfile reasoningEffort selectionMode } modelOptions { __typename providerKind providerAccountId providerDisplayName status disabledReason profiles { __typename id label reasoningEfforts defaultReasoningEffort disabledReason } recommendations { __typename useCase modelProfile reasoningEffort disabledReason } } } webFetchSettings { __typename summarizer { __typename modelPreference { __typename providerKind providerAccountId modelProfile reasoningEffort selectionMode } modelOptions { __typename providerKind providerAccountId providerDisplayName status disabledReason profiles { __typename id label reasoningEfforts defaultReasoningEffort disabledReason } recommendations { __typename useCase modelProfile reasoningEffort disabledReason } } } } webToolSettings { __typename search { __typename toolName capabilityId activeProviderAccountId providerOptions { __typename providerAccountId providerKind accountKey displayName capabilityId reliabilityContract dataFlowClass citations directUrlFetch } } fetch { __typename toolName capabilityId activeProviderAccountId providerOptions { __typename providerAccountId providerKind accountKey displayName capabilityId reliabilityContract dataFlowClass citations directUrlFetch } } } privacySettings { __typename reviewer { __typename modelPreference { __typename providerKind providerAccountId modelProfile reasoningEffort selectionMode } modelOptions { __typename providerKind providerAccountId providerDisplayName status disabledReason profiles { __typename id label reasoningEfforts defaultReasoningEffort disabledReason } recommendations { __typename useCase modelProfile reasoningEffort disabledReason } } } } usageSettings { __typename progressAudit { __typename modelPreference { __typename providerKind providerAccountId modelProfile reasoningEffort selectionMode } modelOptions { __typename providerKind providerAccountId providerDisplayName status disabledReason profiles { __typename id label reasoningEfforts defaultReasoningEffort disabledReason } recommendations { __typename useCase modelProfile reasoningEffort disabledReason } } } } taskExecutionPolicy { __typename maxProviderContinuations maxToolCalls maxActiveMinutes progressAuditInterval maxAutomaticRetries maxReviewRounds } localModelSetup { __typename isReady runtimeStatus recommendedModel { __typename modelId name license priority repo revision isRecommended compatibleBackend selectedBuild { __typename file sha256 downloadGb backends minRamGb minVramGb } hardwareFit { __typename backend ramGb vramGb unifiedMemory explanation } } installation { __typename installationId modelId name file sourceKind status sha256 completedBytes totalBytes diskBytes backend isActive errorCode errorMessage createdAt updatedAt } } localModelCatalog { __typename modelId name license priority repo revision isRecommended compatibleBackend selectedBuild { __typename file sha256 downloadGb backends minRamGb minVramGb } hardwareFit { __typename backend ramGb vramGb unifiedMemory explanation } } localModelInstallations { __typename installationId modelId name file sourceKind status sha256 completedBytes totalBytes diskBytes backend isActive errorCode errorMessage createdAt updatedAt } defaultModelPreference { __typename providerKind providerAccountId selectionMode modelProfile reasoningEffort } providerAccountCatalog { __typename providerKind displayName preferredAuthMethod supportedAuthMethods capabilities { __typename capabilityId status reliabilityContract dataFlowClass } } providerAccounts { __typename providerAccountId providerKind accountKey displayName authMethod status isActive isDefault lastCheckedAt lastAuthenticatedAt lastErrorCode lastErrorMessage capabilities { __typename capabilityId status reliabilityContract dataFlowClass } } mcps: capabilityIntegrations(kind: MCP) { __typename kind definitionId name sourceRevision reviewed sourceSummary connections { __typename kind definitionId connectionId name connectionLabel sourceRevision connectionRevision credentialRevision grantRevision policyRevision status healthStatus authStatus dataSharingPolicy unsafeActionPolicy toolCount availableToolCount pendingToolCount defaultedToolCount disabledToolCount sourceDetails } } apis: capabilityIntegrations(kind: API) { __typename kind definitionId name sourceRevision reviewed sourceSummary connections { __typename kind definitionId connectionId name connectionLabel sourceRevision connectionRevision credentialRevision grantRevision policyRevision status healthStatus authStatus dataSharingPolicy unsafeActionPolicy toolCount availableToolCount pendingToolCount defaultedToolCount disabledToolCount sourceDetails } } mcpServers { __typename mcpServerId connectionRevision policyRevision displayName transportKind healthStatus authStatus toolCount pendingToolCount browserOauthReauthenticationSupported } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("localStatus", LocalStatus.self),
      .field("agents", [Agent].self),
      .field("memorySettings", MemorySettings.self),
      .field("webFetchSettings", WebFetchSettings.self),
      .field("webToolSettings", WebToolSettings.self),
      .field("privacySettings", PrivacySettings.self),
      .field("usageSettings", UsageSettings.self),
      .field("taskExecutionPolicy", TaskExecutionPolicy.self),
      .field("localModelSetup", LocalModelSetup.self),
      .field("localModelCatalog", [LocalModelCatalog].self),
      .field("localModelInstallations", [LocalModelInstallation].self),
      .field("defaultModelPreference", DefaultModelPreference?.self),
      .field("providerAccountCatalog", [ProviderAccountCatalog].self),
      .field("providerAccounts", [ProviderAccount].self),
      .field("capabilityIntegrations", alias: "mcps", [Mcp].self, arguments: ["kind": "MCP"]),
      .field("capabilityIntegrations", alias: "apis", [Api].self, arguments: ["kind": "API"]),
      .field("mcpServers", [McpServer].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsSnapshotQuery.Data.self
    ] }

    /// Return local Noema status.
    public var localStatus: LocalStatus { __data["localStatus"] }
    /// List agent metadata safe to show in Settings.
    public var agents: [Agent] { __data["agents"] }
    /// Return the native memory model preference and selectable options.
    public var memorySettings: MemorySettings { __data["memorySettings"] }
    /// Return web fetch settings safe to show in Settings.
    public var webFetchSettings: WebFetchSettings { __data["webFetchSettings"] }
    /// Return web tool provider bindings safe to show in Settings.
    public var webToolSettings: WebToolSettings { __data["webToolSettings"] }
    /// Return Safety privacy settings safe to show in Settings.
    public var privacySettings: PrivacySettings { __data["privacySettings"] }
    /// Return Safety usage settings safe to show in Settings.
    public var usageSettings: UsageSettings { __data["usageSettings"] }
    /// Return provider-independent safety limits shared by every task tier.
    public var taskExecutionPolicy: TaskExecutionPolicy { __data["taskExecutionPolicy"] }
    /// Return the first-run local-model recommendation and readiness state.
    public var localModelSetup: LocalModelSetup { __data["localModelSetup"] }
    /// List curated models and machine-selected builds.
    public var localModelCatalog: [LocalModelCatalog] { __data["localModelCatalog"] }
    /// List durable local-model installations and transfer state.
    public var localModelInstallations: [LocalModelInstallation] { __data["localModelInstallations"] }
    /// Return Noema's system model default.
    public var defaultModelPreference: DefaultModelPreference? { __data["defaultModelPreference"] }
    /// List provider account types that can be added in Settings.
    public var providerAccountCatalog: [ProviderAccountCatalog] { __data["providerAccountCatalog"] }
    /// List provider account metadata safe to show in Settings.
    public var providerAccounts: [ProviderAccount] { __data["providerAccounts"] }
    /// List API or MCP definitions with their concrete connections.
    public var mcps: [Mcp] { __data["mcps"] }
    /// List API or MCP definitions with their concrete connections.
    public var apis: [Api] { __data["apis"] }
    /// List MCP server metadata safe to show in Settings.
    public var mcpServers: [McpServer] { __data["mcpServers"] }

    /// LocalStatus
    ///
    /// Parent Type: `LocalStatus`
    nonisolated public struct LocalStatus: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalStatus }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("localService", GraphQLEnum<NoemaAPI.LocalServiceStatus>.self),
        .field("assistantConnection", GraphQLEnum<NoemaAPI.AssistantConnection>.self),
        .field("memoryStorage", GraphQLEnum<NoemaAPI.MemoryStorageStatus>.self),
        .field("primaryAgentDisplayName", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.LocalStatus.self
      ] }

      /// Local service status.
      public var localService: GraphQLEnum<NoemaAPI.LocalServiceStatus> { __data["localService"] }
      /// Assistant connection status.
      public var assistantConnection: GraphQLEnum<NoemaAPI.AssistantConnection> { __data["assistantConnection"] }
      /// Memory storage status.
      public var memoryStorage: GraphQLEnum<NoemaAPI.MemoryStorageStatus> { __data["memoryStorage"] }
      /// Current primary agent display name, if the agent has been named.
      public var primaryAgentDisplayName: String? { __data["primaryAgentDisplayName"] }
    }

    /// Agent
    ///
    /// Parent Type: `Agent`
    nonisolated public struct Agent: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Agent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("agentId", String.self),
        .field("displayName", String?.self),
        .field("isPrimary", Bool.self),
        .field("modelPreference", ModelPreference?.self),
        .field("modelOptions", [ModelOption].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.Agent.self
      ] }

      /// Durable concrete agent id.
      public var agentId: String { __data["agentId"] }
      /// Optional human-visible agent name.
      public var displayName: String? { __data["displayName"] }
      /// Whether this is Noema's built-in primary agent.
      public var isPrimary: Bool { __data["isPrimary"] }
      /// Current model preference, when configured.
      public var modelPreference: ModelPreference? { __data["modelPreference"] }
      /// Provider/profile options available to this agent.
      public var modelOptions: [ModelOption] { __data["modelOptions"] }

      /// Agent.ModelPreference
      ///
      /// Parent Type: `AgentModelPreference`
      nonisolated public struct ModelPreference: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelPreference }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("providerKind", String.self),
          .field("providerAccountId", String.self),
          .field("modelProfile", String?.self),
          .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
          .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.Agent.ModelPreference.self
        ] }

        /// Provider kind selected for this agent.
        public var providerKind: String { __data["providerKind"] }
        /// Provider account id selected for this agent.
        public var providerAccountId: String { __data["providerAccountId"] }
        /// Provider-specific model id or profile id.
        public var modelProfile: String? { __data["modelProfile"] }
        /// Optional explicit reasoning effort for reasoning-capable model profiles.
        public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
        /// Whether Noema or the human chooses the concrete model.
        public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
      }

      /// Agent.ModelOption
      ///
      /// Parent Type: `AgentModelProviderOption`
      nonisolated public struct ModelOption: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProviderOption }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("providerKind", String.self),
          .field("providerAccountId", String.self),
          .field("providerDisplayName", String.self),
          .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
          .field("disabledReason", String?.self),
          .field("profiles", [Profile].self),
          .field("recommendations", [Recommendation].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.Agent.ModelOption.self
        ] }

        /// Provider kind.
        public var providerKind: String { __data["providerKind"] }
        /// Provider account id.
        public var providerAccountId: String { __data["providerAccountId"] }
        /// User-facing provider display name.
        public var providerDisplayName: String { __data["providerDisplayName"] }
        /// Provider account status.
        public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
        /// Why this provider is disabled, when unavailable.
        public var disabledReason: String? { __data["disabledReason"] }
        /// Available profiles or model ids.
        public var profiles: [Profile] { __data["profiles"] }
        /// Current product recommendations available through this account.
        public var recommendations: [Recommendation] { __data["recommendations"] }

        /// Agent.ModelOption.Profile
        ///
        /// Parent Type: `AgentModelProfileOption`
        nonisolated public struct Profile: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProfileOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("id", String.self),
            .field("label", String.self),
            .field("reasoningEfforts", [GraphQLEnum<NoemaAPI.ReasoningEffort>].self),
            .field("defaultReasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("disabledReason", String?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.Agent.ModelOption.Profile.self
          ] }

          /// Stable profile or model id.
          public var id: String { __data["id"] }
          /// User-facing label.
          public var label: String { __data["label"] }
          /// Reasoning efforts available for this profile.
          public var reasoningEfforts: [GraphQLEnum<NoemaAPI.ReasoningEffort>] { __data["reasoningEfforts"] }
          /// Default reasoning effort for this profile, when advertised by metadata.
          public var defaultReasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["defaultReasoningEffort"] }
          /// Why this option is disabled, when unavailable.
          public var disabledReason: String? { __data["disabledReason"] }
        }

        /// Agent.ModelOption.Recommendation
        ///
        /// Parent Type: `AgentModelRecommendation`
        nonisolated public struct Recommendation: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelRecommendation }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("useCase", GraphQLEnum<NoemaAPI.NoemaModelUseCase>.self),
            .field("modelProfile", String.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("disabledReason", String?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.Agent.ModelOption.Recommendation.self
          ] }

          public var useCase: GraphQLEnum<NoemaAPI.NoemaModelUseCase> { __data["useCase"] }
          public var modelProfile: String { __data["modelProfile"] }
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          public var disabledReason: String? { __data["disabledReason"] }
        }
      }
    }

    /// MemorySettings
    ///
    /// Parent Type: `GraphqlNativeMemorySettings`
    nonisolated public struct MemorySettings: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemorySettings }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("modelPreference", ModelPreference?.self),
        .field("modelOptions", [ModelOption].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.MemorySettings.self
      ] }

      public var modelPreference: ModelPreference? { __data["modelPreference"] }
      public var modelOptions: [ModelOption] { __data["modelOptions"] }

      /// MemorySettings.ModelPreference
      ///
      /// Parent Type: `AgentModelPreference`
      nonisolated public struct ModelPreference: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelPreference }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("providerKind", String.self),
          .field("providerAccountId", String.self),
          .field("modelProfile", String?.self),
          .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
          .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.MemorySettings.ModelPreference.self
        ] }

        /// Provider kind selected for this agent.
        public var providerKind: String { __data["providerKind"] }
        /// Provider account id selected for this agent.
        public var providerAccountId: String { __data["providerAccountId"] }
        /// Provider-specific model id or profile id.
        public var modelProfile: String? { __data["modelProfile"] }
        /// Optional explicit reasoning effort for reasoning-capable model profiles.
        public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
        /// Whether Noema or the human chooses the concrete model.
        public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
      }

      /// MemorySettings.ModelOption
      ///
      /// Parent Type: `AgentModelProviderOption`
      nonisolated public struct ModelOption: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProviderOption }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("providerKind", String.self),
          .field("providerAccountId", String.self),
          .field("providerDisplayName", String.self),
          .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
          .field("disabledReason", String?.self),
          .field("profiles", [Profile].self),
          .field("recommendations", [Recommendation].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.MemorySettings.ModelOption.self
        ] }

        /// Provider kind.
        public var providerKind: String { __data["providerKind"] }
        /// Provider account id.
        public var providerAccountId: String { __data["providerAccountId"] }
        /// User-facing provider display name.
        public var providerDisplayName: String { __data["providerDisplayName"] }
        /// Provider account status.
        public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
        /// Why this provider is disabled, when unavailable.
        public var disabledReason: String? { __data["disabledReason"] }
        /// Available profiles or model ids.
        public var profiles: [Profile] { __data["profiles"] }
        /// Current product recommendations available through this account.
        public var recommendations: [Recommendation] { __data["recommendations"] }

        /// MemorySettings.ModelOption.Profile
        ///
        /// Parent Type: `AgentModelProfileOption`
        nonisolated public struct Profile: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProfileOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("id", String.self),
            .field("label", String.self),
            .field("reasoningEfforts", [GraphQLEnum<NoemaAPI.ReasoningEffort>].self),
            .field("defaultReasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("disabledReason", String?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.MemorySettings.ModelOption.Profile.self
          ] }

          /// Stable profile or model id.
          public var id: String { __data["id"] }
          /// User-facing label.
          public var label: String { __data["label"] }
          /// Reasoning efforts available for this profile.
          public var reasoningEfforts: [GraphQLEnum<NoemaAPI.ReasoningEffort>] { __data["reasoningEfforts"] }
          /// Default reasoning effort for this profile, when advertised by metadata.
          public var defaultReasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["defaultReasoningEffort"] }
          /// Why this option is disabled, when unavailable.
          public var disabledReason: String? { __data["disabledReason"] }
        }

        /// MemorySettings.ModelOption.Recommendation
        ///
        /// Parent Type: `AgentModelRecommendation`
        nonisolated public struct Recommendation: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelRecommendation }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("useCase", GraphQLEnum<NoemaAPI.NoemaModelUseCase>.self),
            .field("modelProfile", String.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("disabledReason", String?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.MemorySettings.ModelOption.Recommendation.self
          ] }

          public var useCase: GraphQLEnum<NoemaAPI.NoemaModelUseCase> { __data["useCase"] }
          public var modelProfile: String { __data["modelProfile"] }
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          public var disabledReason: String? { __data["disabledReason"] }
        }
      }
    }

    /// WebFetchSettings
    ///
    /// Parent Type: `WebFetchSettings`
    nonisolated public struct WebFetchSettings: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebFetchSettings }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("summarizer", Summarizer.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.WebFetchSettings.self
      ] }

      /// Web fetch summarizer settings.
      public var summarizer: Summarizer { __data["summarizer"] }

      /// WebFetchSettings.Summarizer
      ///
      /// Parent Type: `WebFetchSummarizerSettings`
      nonisolated public struct Summarizer: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebFetchSummarizerSettings }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("modelPreference", ModelPreference?.self),
          .field("modelOptions", [ModelOption].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.self
        ] }

        /// Current persisted summarizer model preference, when configured.
        public var modelPreference: ModelPreference? { __data["modelPreference"] }
        /// Provider/profile options available for the summarizer.
        public var modelOptions: [ModelOption] { __data["modelOptions"] }

        /// WebFetchSettings.Summarizer.ModelPreference
        ///
        /// Parent Type: `AgentModelPreference`
        nonisolated public struct ModelPreference: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelPreference }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerKind", String.self),
            .field("providerAccountId", String.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.ModelPreference.self
          ] }

          /// Provider kind selected for this agent.
          public var providerKind: String { __data["providerKind"] }
          /// Provider account id selected for this agent.
          public var providerAccountId: String { __data["providerAccountId"] }
          /// Provider-specific model id or profile id.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Optional explicit reasoning effort for reasoning-capable model profiles.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
        }

        /// WebFetchSettings.Summarizer.ModelOption
        ///
        /// Parent Type: `AgentModelProviderOption`
        nonisolated public struct ModelOption: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProviderOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerKind", String.self),
            .field("providerAccountId", String.self),
            .field("providerDisplayName", String.self),
            .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
            .field("disabledReason", String?.self),
            .field("profiles", [Profile].self),
            .field("recommendations", [Recommendation].self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.ModelOption.self
          ] }

          /// Provider kind.
          public var providerKind: String { __data["providerKind"] }
          /// Provider account id.
          public var providerAccountId: String { __data["providerAccountId"] }
          /// User-facing provider display name.
          public var providerDisplayName: String { __data["providerDisplayName"] }
          /// Provider account status.
          public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
          /// Why this provider is disabled, when unavailable.
          public var disabledReason: String? { __data["disabledReason"] }
          /// Available profiles or model ids.
          public var profiles: [Profile] { __data["profiles"] }
          /// Current product recommendations available through this account.
          public var recommendations: [Recommendation] { __data["recommendations"] }

          /// WebFetchSettings.Summarizer.ModelOption.Profile
          ///
          /// Parent Type: `AgentModelProfileOption`
          nonisolated public struct Profile: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProfileOption }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("id", String.self),
              .field("label", String.self),
              .field("reasoningEfforts", [GraphQLEnum<NoemaAPI.ReasoningEffort>].self),
              .field("defaultReasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
              .field("disabledReason", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.ModelOption.Profile.self
            ] }

            /// Stable profile or model id.
            public var id: String { __data["id"] }
            /// User-facing label.
            public var label: String { __data["label"] }
            /// Reasoning efforts available for this profile.
            public var reasoningEfforts: [GraphQLEnum<NoemaAPI.ReasoningEffort>] { __data["reasoningEfforts"] }
            /// Default reasoning effort for this profile, when advertised by metadata.
            public var defaultReasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["defaultReasoningEffort"] }
            /// Why this option is disabled, when unavailable.
            public var disabledReason: String? { __data["disabledReason"] }
          }

          /// WebFetchSettings.Summarizer.ModelOption.Recommendation
          ///
          /// Parent Type: `AgentModelRecommendation`
          nonisolated public struct Recommendation: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelRecommendation }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("useCase", GraphQLEnum<NoemaAPI.NoemaModelUseCase>.self),
              .field("modelProfile", String.self),
              .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
              .field("disabledReason", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.ModelOption.Recommendation.self
            ] }

            public var useCase: GraphQLEnum<NoemaAPI.NoemaModelUseCase> { __data["useCase"] }
            public var modelProfile: String { __data["modelProfile"] }
            public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
            public var disabledReason: String? { __data["disabledReason"] }
          }
        }
      }
    }

    /// WebToolSettings
    ///
    /// Parent Type: `WebToolSettings`
    nonisolated public struct WebToolSettings: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebToolSettings }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("search", Search.self),
        .field("fetch", Fetch.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.WebToolSettings.self
      ] }

      public var search: Search { __data["search"] }
      public var fetch: Fetch { __data["fetch"] }

      /// WebToolSettings.Search
      ///
      /// Parent Type: `WebToolBindingSettings`
      nonisolated public struct Search: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebToolBindingSettings }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("toolName", String.self),
          .field("capabilityId", String.self),
          .field("activeProviderAccountId", String.self),
          .field("providerOptions", [ProviderOption].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.WebToolSettings.Search.self
        ] }

        public var toolName: String { __data["toolName"] }
        public var capabilityId: String { __data["capabilityId"] }
        public var activeProviderAccountId: String { __data["activeProviderAccountId"] }
        public var providerOptions: [ProviderOption] { __data["providerOptions"] }

        /// WebToolSettings.Search.ProviderOption
        ///
        /// Parent Type: `WebToolProviderOption`
        nonisolated public struct ProviderOption: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebToolProviderOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerAccountId", String.self),
            .field("providerKind", String.self),
            .field("accountKey", String.self),
            .field("displayName", String.self),
            .field("capabilityId", String.self),
            .field("reliabilityContract", String.self),
            .field("dataFlowClass", String.self),
            .field("citations", Bool.self),
            .field("directUrlFetch", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.WebToolSettings.Search.ProviderOption.self
          ] }

          public var providerAccountId: String { __data["providerAccountId"] }
          public var providerKind: String { __data["providerKind"] }
          public var accountKey: String { __data["accountKey"] }
          public var displayName: String { __data["displayName"] }
          public var capabilityId: String { __data["capabilityId"] }
          public var reliabilityContract: String { __data["reliabilityContract"] }
          public var dataFlowClass: String { __data["dataFlowClass"] }
          public var citations: Bool { __data["citations"] }
          public var directUrlFetch: Bool { __data["directUrlFetch"] }
        }
      }

      /// WebToolSettings.Fetch
      ///
      /// Parent Type: `WebToolBindingSettings`
      nonisolated public struct Fetch: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebToolBindingSettings }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("toolName", String.self),
          .field("capabilityId", String.self),
          .field("activeProviderAccountId", String.self),
          .field("providerOptions", [ProviderOption].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.WebToolSettings.Fetch.self
        ] }

        public var toolName: String { __data["toolName"] }
        public var capabilityId: String { __data["capabilityId"] }
        public var activeProviderAccountId: String { __data["activeProviderAccountId"] }
        public var providerOptions: [ProviderOption] { __data["providerOptions"] }

        /// WebToolSettings.Fetch.ProviderOption
        ///
        /// Parent Type: `WebToolProviderOption`
        nonisolated public struct ProviderOption: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebToolProviderOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerAccountId", String.self),
            .field("providerKind", String.self),
            .field("accountKey", String.self),
            .field("displayName", String.self),
            .field("capabilityId", String.self),
            .field("reliabilityContract", String.self),
            .field("dataFlowClass", String.self),
            .field("citations", Bool.self),
            .field("directUrlFetch", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.WebToolSettings.Fetch.ProviderOption.self
          ] }

          public var providerAccountId: String { __data["providerAccountId"] }
          public var providerKind: String { __data["providerKind"] }
          public var accountKey: String { __data["accountKey"] }
          public var displayName: String { __data["displayName"] }
          public var capabilityId: String { __data["capabilityId"] }
          public var reliabilityContract: String { __data["reliabilityContract"] }
          public var dataFlowClass: String { __data["dataFlowClass"] }
          public var citations: Bool { __data["citations"] }
          public var directUrlFetch: Bool { __data["directUrlFetch"] }
        }
      }
    }

    /// PrivacySettings
    ///
    /// Parent Type: `PrivacySettings`
    nonisolated public struct PrivacySettings: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.PrivacySettings }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("reviewer", Reviewer.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.PrivacySettings.self
      ] }

      /// Model used to review governed writes and exports.
      public var reviewer: Reviewer { __data["reviewer"] }

      /// PrivacySettings.Reviewer
      ///
      /// Parent Type: `ActionReviewerSettings`
      nonisolated public struct Reviewer: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ActionReviewerSettings }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("modelPreference", ModelPreference?.self),
          .field("modelOptions", [ModelOption].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.self
        ] }

        /// Explicit reviewer preference, or none when action review requires human approval.
        public var modelPreference: ModelPreference? { __data["modelPreference"] }
        /// Provider/profile options available for action review.
        public var modelOptions: [ModelOption] { __data["modelOptions"] }

        /// PrivacySettings.Reviewer.ModelPreference
        ///
        /// Parent Type: `AgentModelPreference`
        nonisolated public struct ModelPreference: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelPreference }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerKind", String.self),
            .field("providerAccountId", String.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.ModelPreference.self
          ] }

          /// Provider kind selected for this agent.
          public var providerKind: String { __data["providerKind"] }
          /// Provider account id selected for this agent.
          public var providerAccountId: String { __data["providerAccountId"] }
          /// Provider-specific model id or profile id.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Optional explicit reasoning effort for reasoning-capable model profiles.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
        }

        /// PrivacySettings.Reviewer.ModelOption
        ///
        /// Parent Type: `AgentModelProviderOption`
        nonisolated public struct ModelOption: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProviderOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerKind", String.self),
            .field("providerAccountId", String.self),
            .field("providerDisplayName", String.self),
            .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
            .field("disabledReason", String?.self),
            .field("profiles", [Profile].self),
            .field("recommendations", [Recommendation].self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.ModelOption.self
          ] }

          /// Provider kind.
          public var providerKind: String { __data["providerKind"] }
          /// Provider account id.
          public var providerAccountId: String { __data["providerAccountId"] }
          /// User-facing provider display name.
          public var providerDisplayName: String { __data["providerDisplayName"] }
          /// Provider account status.
          public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
          /// Why this provider is disabled, when unavailable.
          public var disabledReason: String? { __data["disabledReason"] }
          /// Available profiles or model ids.
          public var profiles: [Profile] { __data["profiles"] }
          /// Current product recommendations available through this account.
          public var recommendations: [Recommendation] { __data["recommendations"] }

          /// PrivacySettings.Reviewer.ModelOption.Profile
          ///
          /// Parent Type: `AgentModelProfileOption`
          nonisolated public struct Profile: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProfileOption }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("id", String.self),
              .field("label", String.self),
              .field("reasoningEfforts", [GraphQLEnum<NoemaAPI.ReasoningEffort>].self),
              .field("defaultReasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
              .field("disabledReason", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.ModelOption.Profile.self
            ] }

            /// Stable profile or model id.
            public var id: String { __data["id"] }
            /// User-facing label.
            public var label: String { __data["label"] }
            /// Reasoning efforts available for this profile.
            public var reasoningEfforts: [GraphQLEnum<NoemaAPI.ReasoningEffort>] { __data["reasoningEfforts"] }
            /// Default reasoning effort for this profile, when advertised by metadata.
            public var defaultReasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["defaultReasoningEffort"] }
            /// Why this option is disabled, when unavailable.
            public var disabledReason: String? { __data["disabledReason"] }
          }

          /// PrivacySettings.Reviewer.ModelOption.Recommendation
          ///
          /// Parent Type: `AgentModelRecommendation`
          nonisolated public struct Recommendation: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelRecommendation }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("useCase", GraphQLEnum<NoemaAPI.NoemaModelUseCase>.self),
              .field("modelProfile", String.self),
              .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
              .field("disabledReason", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.ModelOption.Recommendation.self
            ] }

            public var useCase: GraphQLEnum<NoemaAPI.NoemaModelUseCase> { __data["useCase"] }
            public var modelProfile: String { __data["modelProfile"] }
            public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
            public var disabledReason: String? { __data["disabledReason"] }
          }
        }
      }
    }

    /// UsageSettings
    ///
    /// Parent Type: `UsageSettings`
    nonisolated public struct UsageSettings: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.UsageSettings }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("progressAudit", ProgressAudit.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.UsageSettings.self
      ] }

      /// Tool-continuation progress audit model settings.
      public var progressAudit: ProgressAudit { __data["progressAudit"] }

      /// UsageSettings.ProgressAudit
      ///
      /// Parent Type: `ToolProgressAuditSettings`
      nonisolated public struct ProgressAudit: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ToolProgressAuditSettings }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("modelPreference", ModelPreference?.self),
          .field("modelOptions", [ModelOption].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.self
        ] }

        /// Current persisted audit model preference, when configured.
        public var modelPreference: ModelPreference? { __data["modelPreference"] }
        /// Provider/profile options available for progress audits.
        public var modelOptions: [ModelOption] { __data["modelOptions"] }

        /// UsageSettings.ProgressAudit.ModelPreference
        ///
        /// Parent Type: `AgentModelPreference`
        nonisolated public struct ModelPreference: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelPreference }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerKind", String.self),
            .field("providerAccountId", String.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.ModelPreference.self
          ] }

          /// Provider kind selected for this agent.
          public var providerKind: String { __data["providerKind"] }
          /// Provider account id selected for this agent.
          public var providerAccountId: String { __data["providerAccountId"] }
          /// Provider-specific model id or profile id.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Optional explicit reasoning effort for reasoning-capable model profiles.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
        }

        /// UsageSettings.ProgressAudit.ModelOption
        ///
        /// Parent Type: `AgentModelProviderOption`
        nonisolated public struct ModelOption: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProviderOption }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("providerKind", String.self),
            .field("providerAccountId", String.self),
            .field("providerDisplayName", String.self),
            .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
            .field("disabledReason", String?.self),
            .field("profiles", [Profile].self),
            .field("recommendations", [Recommendation].self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.ModelOption.self
          ] }

          /// Provider kind.
          public var providerKind: String { __data["providerKind"] }
          /// Provider account id.
          public var providerAccountId: String { __data["providerAccountId"] }
          /// User-facing provider display name.
          public var providerDisplayName: String { __data["providerDisplayName"] }
          /// Provider account status.
          public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
          /// Why this provider is disabled, when unavailable.
          public var disabledReason: String? { __data["disabledReason"] }
          /// Available profiles or model ids.
          public var profiles: [Profile] { __data["profiles"] }
          /// Current product recommendations available through this account.
          public var recommendations: [Recommendation] { __data["recommendations"] }

          /// UsageSettings.ProgressAudit.ModelOption.Profile
          ///
          /// Parent Type: `AgentModelProfileOption`
          nonisolated public struct Profile: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelProfileOption }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("id", String.self),
              .field("label", String.self),
              .field("reasoningEfforts", [GraphQLEnum<NoemaAPI.ReasoningEffort>].self),
              .field("defaultReasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
              .field("disabledReason", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.ModelOption.Profile.self
            ] }

            /// Stable profile or model id.
            public var id: String { __data["id"] }
            /// User-facing label.
            public var label: String { __data["label"] }
            /// Reasoning efforts available for this profile.
            public var reasoningEfforts: [GraphQLEnum<NoemaAPI.ReasoningEffort>] { __data["reasoningEfforts"] }
            /// Default reasoning effort for this profile, when advertised by metadata.
            public var defaultReasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["defaultReasoningEffort"] }
            /// Why this option is disabled, when unavailable.
            public var disabledReason: String? { __data["disabledReason"] }
          }

          /// UsageSettings.ProgressAudit.ModelOption.Recommendation
          ///
          /// Parent Type: `AgentModelRecommendation`
          nonisolated public struct Recommendation: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelRecommendation }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("useCase", GraphQLEnum<NoemaAPI.NoemaModelUseCase>.self),
              .field("modelProfile", String.self),
              .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
              .field("disabledReason", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.ModelOption.Recommendation.self
            ] }

            public var useCase: GraphQLEnum<NoemaAPI.NoemaModelUseCase> { __data["useCase"] }
            public var modelProfile: String { __data["modelProfile"] }
            public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
            public var disabledReason: String? { __data["disabledReason"] }
          }
        }
      }
    }

    /// TaskExecutionPolicy
    ///
    /// Parent Type: `TaskExecutionPolicy`
    nonisolated public struct TaskExecutionPolicy: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskExecutionPolicy }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("maxProviderContinuations", Int.self),
        .field("maxToolCalls", Int.self),
        .field("maxActiveMinutes", Int.self),
        .field("progressAuditInterval", Int.self),
        .field("maxAutomaticRetries", Int.self),
        .field("maxReviewRounds", Int.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.TaskExecutionPolicy.self
      ] }

      /// Provider continuation bound.
      public var maxProviderContinuations: Int { __data["maxProviderContinuations"] }
      /// Tool-call bound.
      public var maxToolCalls: Int { __data["maxToolCalls"] }
      /// Active-minute bound.
      public var maxActiveMinutes: Int { __data["maxActiveMinutes"] }
      /// Progress-audit interval.
      public var progressAuditInterval: Int { __data["progressAuditInterval"] }
      /// Automatic retry bound.
      public var maxAutomaticRetries: Int { __data["maxAutomaticRetries"] }
      /// Review-round bound.
      public var maxReviewRounds: Int { __data["maxReviewRounds"] }
    }

    /// LocalModelSetup
    ///
    /// Parent Type: `LocalModelSetup`
    nonisolated public struct LocalModelSetup: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelSetup }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("isReady", Bool.self),
        .field("runtimeStatus", GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus>.self),
        .field("recommendedModel", RecommendedModel?.self),
        .field("installation", Installation?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.LocalModelSetup.self
      ] }

      /// Whether setup is complete and local inference is usable.
      public var isReady: Bool { __data["isReady"] }
      /// Current supervised llama.cpp process state.
      public var runtimeStatus: GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus> { __data["runtimeStatus"] }
      /// Top curated recommendation for this machine.
      public var recommendedModel: RecommendedModel? { __data["recommendedModel"] }
      /// Installation currently satisfying local setup, when present.
      public var installation: Installation? { __data["installation"] }

      /// LocalModelSetup.RecommendedModel
      ///
      /// Parent Type: `LocalModelCatalogEntry`
      nonisolated public struct RecommendedModel: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelCatalogEntry }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("modelId", String.self),
          .field("name", String.self),
          .field("license", String.self),
          .field("priority", Int.self),
          .field("repo", String.self),
          .field("revision", String.self),
          .field("isRecommended", Bool.self),
          .field("compatibleBackend", GraphQLEnum<NoemaAPI.LocalModelBackend>?.self),
          .field("selectedBuild", SelectedBuild?.self),
          .field("hardwareFit", HardwareFit?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.LocalModelSetup.RecommendedModel.self
        ] }

        /// Stable curated model identifier.
        public var modelId: String { __data["modelId"] }
        /// Product-facing model name.
        public var name: String { __data["name"] }
        /// License shown before download.
        public var license: String { __data["license"] }
        /// Recommendation priority from the bundled catalog.
        public var priority: Int { __data["priority"] }
        /// Pinned Hugging Face repository.
        public var repo: String { __data["repo"] }
        /// Immutable source revision.
        public var revision: String { __data["revision"] }
        /// Whether this is the top recommendation for the current machine.
        public var isRecommended: Bool { __data["isRecommended"] }
        /// Backend used by the selected artifact.
        public var compatibleBackend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["compatibleBackend"] }
        /// Best compatible artifact for this machine, when one fits.
        public var selectedBuild: SelectedBuild? { __data["selectedBuild"] }
        /// Detected hardware values that satisfy the artifact thresholds.
        public var hardwareFit: HardwareFit? { __data["hardwareFit"] }

        /// LocalModelSetup.RecommendedModel.SelectedBuild
        ///
        /// Parent Type: `LocalModelBuild`
        nonisolated public struct SelectedBuild: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelBuild }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("file", String.self),
            .field("sha256", String.self),
            .field("downloadGb", Double.self),
            .field("backends", [GraphQLEnum<NoemaAPI.LocalModelBackend>].self),
            .field("minRamGb", Int.self),
            .field("minVramGb", Int?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.LocalModelSetup.RecommendedModel.SelectedBuild.self
          ] }

          /// Artifact filename in the pinned source repository.
          public var file: String { __data["file"] }
          /// Verified SHA-256 digest.
          public var sha256: String { __data["sha256"] }
          /// Rounded UI-facing download size in decimal gigabytes.
          public var downloadGb: Double { __data["downloadGb"] }
          /// Backends compatible with this artifact.
          public var backends: [GraphQLEnum<NoemaAPI.LocalModelBackend>] { __data["backends"] }
          /// Minimum system or unified memory in whole gigabytes.
          public var minRamGb: Int { __data["minRamGb"] }
          /// Minimum discrete or unified accelerator memory in whole gigabytes.
          public var minVramGb: Int? { __data["minVramGb"] }
        }

        /// LocalModelSetup.RecommendedModel.HardwareFit
        ///
        /// Parent Type: `LocalModelHardwareFit`
        nonisolated public struct HardwareFit: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelHardwareFit }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("backend", GraphQLEnum<NoemaAPI.LocalModelBackend>.self),
            .field("ramGb", Int.self),
            .field("vramGb", Int?.self),
            .field("unifiedMemory", Bool.self),
            .field("explanation", String.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsSnapshotQuery.Data.LocalModelSetup.RecommendedModel.HardwareFit.self
          ] }

          /// Backend selected for the build.
          public var backend: GraphQLEnum<NoemaAPI.LocalModelBackend> { __data["backend"] }
          /// Detected system or unified memory in whole gigabytes.
          public var ramGb: Int { __data["ramGb"] }
          /// Detected discrete accelerator memory in whole gigabytes.
          public var vramGb: Int? { __data["vramGb"] }
          /// Whether the accelerator shares system memory.
          public var unifiedMemory: Bool { __data["unifiedMemory"] }
          /// Product-facing explanation generated from the match.
          public var explanation: String { __data["explanation"] }
        }
      }

      /// LocalModelSetup.Installation
      ///
      /// Parent Type: `LocalModelInstallation`
      nonisolated public struct Installation: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelInstallation }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("installationId", String.self),
          .field("modelId", String.self),
          .field("name", String.self),
          .field("file", String.self),
          .field("sourceKind", GraphQLEnum<NoemaAPI.LocalModelSourceKind>.self),
          .field("status", GraphQLEnum<NoemaAPI.LocalModelInstallationStatus>.self),
          .field("sha256", String?.self),
          .field("completedBytes", Int.self),
          .field("totalBytes", Int?.self),
          .field("diskBytes", Int.self),
          .field("backend", GraphQLEnum<NoemaAPI.LocalModelBackend>?.self),
          .field("isActive", Bool.self),
          .field("errorCode", String?.self),
          .field("errorMessage", String?.self),
          .field("createdAt", String.self),
          .field("updatedAt", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.LocalModelSetup.Installation.self
        ] }

        /// Stable installation identifier.
        public var installationId: String { __data["installationId"] }
        /// Catalog model id, or an import-specific stable id.
        public var modelId: String { __data["modelId"] }
        /// Product-facing model name.
        public var name: String { __data["name"] }
        /// Artifact filename.
        public var file: String { __data["file"] }
        /// Model provenance.
        public var sourceKind: GraphQLEnum<NoemaAPI.LocalModelSourceKind> { __data["sourceKind"] }
        /// Current operation state.
        public var status: GraphQLEnum<NoemaAPI.LocalModelInstallationStatus> { __data["status"] }
        /// Verified content digest when known.
        public var sha256: String? { __data["sha256"] }
        /// Downloaded or copied bytes.
        public var completedBytes: Int { __data["completedBytes"] }
        /// Expected total bytes when known.
        public var totalBytes: Int? { __data["totalBytes"] }
        /// Disk bytes owned by the installation after deduplication.
        public var diskBytes: Int { __data["diskBytes"] }
        /// Runtime backend selected for this installation.
        public var backend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["backend"] }
        /// Whether this installation is Noema's active local model.
        public var isActive: Bool { __data["isActive"] }
        /// Stable non-secret error code.
        public var errorCode: String? { __data["errorCode"] }
        /// UI-safe operation failure message.
        public var errorMessage: String? { __data["errorMessage"] }
        /// Durable creation timestamp.
        public var createdAt: String { __data["createdAt"] }
        /// Durable update timestamp.
        public var updatedAt: String { __data["updatedAt"] }
      }
    }

    /// LocalModelCatalog
    ///
    /// Parent Type: `LocalModelCatalogEntry`
    nonisolated public struct LocalModelCatalog: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelCatalogEntry }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("modelId", String.self),
        .field("name", String.self),
        .field("license", String.self),
        .field("priority", Int.self),
        .field("repo", String.self),
        .field("revision", String.self),
        .field("isRecommended", Bool.self),
        .field("compatibleBackend", GraphQLEnum<NoemaAPI.LocalModelBackend>?.self),
        .field("selectedBuild", SelectedBuild?.self),
        .field("hardwareFit", HardwareFit?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.LocalModelCatalog.self
      ] }

      /// Stable curated model identifier.
      public var modelId: String { __data["modelId"] }
      /// Product-facing model name.
      public var name: String { __data["name"] }
      /// License shown before download.
      public var license: String { __data["license"] }
      /// Recommendation priority from the bundled catalog.
      public var priority: Int { __data["priority"] }
      /// Pinned Hugging Face repository.
      public var repo: String { __data["repo"] }
      /// Immutable source revision.
      public var revision: String { __data["revision"] }
      /// Whether this is the top recommendation for the current machine.
      public var isRecommended: Bool { __data["isRecommended"] }
      /// Backend used by the selected artifact.
      public var compatibleBackend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["compatibleBackend"] }
      /// Best compatible artifact for this machine, when one fits.
      public var selectedBuild: SelectedBuild? { __data["selectedBuild"] }
      /// Detected hardware values that satisfy the artifact thresholds.
      public var hardwareFit: HardwareFit? { __data["hardwareFit"] }

      /// LocalModelCatalog.SelectedBuild
      ///
      /// Parent Type: `LocalModelBuild`
      nonisolated public struct SelectedBuild: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelBuild }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("file", String.self),
          .field("sha256", String.self),
          .field("downloadGb", Double.self),
          .field("backends", [GraphQLEnum<NoemaAPI.LocalModelBackend>].self),
          .field("minRamGb", Int.self),
          .field("minVramGb", Int?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.LocalModelCatalog.SelectedBuild.self
        ] }

        /// Artifact filename in the pinned source repository.
        public var file: String { __data["file"] }
        /// Verified SHA-256 digest.
        public var sha256: String { __data["sha256"] }
        /// Rounded UI-facing download size in decimal gigabytes.
        public var downloadGb: Double { __data["downloadGb"] }
        /// Backends compatible with this artifact.
        public var backends: [GraphQLEnum<NoemaAPI.LocalModelBackend>] { __data["backends"] }
        /// Minimum system or unified memory in whole gigabytes.
        public var minRamGb: Int { __data["minRamGb"] }
        /// Minimum discrete or unified accelerator memory in whole gigabytes.
        public var minVramGb: Int? { __data["minVramGb"] }
      }

      /// LocalModelCatalog.HardwareFit
      ///
      /// Parent Type: `LocalModelHardwareFit`
      nonisolated public struct HardwareFit: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelHardwareFit }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("backend", GraphQLEnum<NoemaAPI.LocalModelBackend>.self),
          .field("ramGb", Int.self),
          .field("vramGb", Int?.self),
          .field("unifiedMemory", Bool.self),
          .field("explanation", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.LocalModelCatalog.HardwareFit.self
        ] }

        /// Backend selected for the build.
        public var backend: GraphQLEnum<NoemaAPI.LocalModelBackend> { __data["backend"] }
        /// Detected system or unified memory in whole gigabytes.
        public var ramGb: Int { __data["ramGb"] }
        /// Detected discrete accelerator memory in whole gigabytes.
        public var vramGb: Int? { __data["vramGb"] }
        /// Whether the accelerator shares system memory.
        public var unifiedMemory: Bool { __data["unifiedMemory"] }
        /// Product-facing explanation generated from the match.
        public var explanation: String { __data["explanation"] }
      }
    }

    /// LocalModelInstallation
    ///
    /// Parent Type: `LocalModelInstallation`
    nonisolated public struct LocalModelInstallation: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelInstallation }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("installationId", String.self),
        .field("modelId", String.self),
        .field("name", String.self),
        .field("file", String.self),
        .field("sourceKind", GraphQLEnum<NoemaAPI.LocalModelSourceKind>.self),
        .field("status", GraphQLEnum<NoemaAPI.LocalModelInstallationStatus>.self),
        .field("sha256", String?.self),
        .field("completedBytes", Int.self),
        .field("totalBytes", Int?.self),
        .field("diskBytes", Int.self),
        .field("backend", GraphQLEnum<NoemaAPI.LocalModelBackend>?.self),
        .field("isActive", Bool.self),
        .field("errorCode", String?.self),
        .field("errorMessage", String?.self),
        .field("createdAt", String.self),
        .field("updatedAt", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.LocalModelInstallation.self
      ] }

      /// Stable installation identifier.
      public var installationId: String { __data["installationId"] }
      /// Catalog model id, or an import-specific stable id.
      public var modelId: String { __data["modelId"] }
      /// Product-facing model name.
      public var name: String { __data["name"] }
      /// Artifact filename.
      public var file: String { __data["file"] }
      /// Model provenance.
      public var sourceKind: GraphQLEnum<NoemaAPI.LocalModelSourceKind> { __data["sourceKind"] }
      /// Current operation state.
      public var status: GraphQLEnum<NoemaAPI.LocalModelInstallationStatus> { __data["status"] }
      /// Verified content digest when known.
      public var sha256: String? { __data["sha256"] }
      /// Downloaded or copied bytes.
      public var completedBytes: Int { __data["completedBytes"] }
      /// Expected total bytes when known.
      public var totalBytes: Int? { __data["totalBytes"] }
      /// Disk bytes owned by the installation after deduplication.
      public var diskBytes: Int { __data["diskBytes"] }
      /// Runtime backend selected for this installation.
      public var backend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["backend"] }
      /// Whether this installation is Noema's active local model.
      public var isActive: Bool { __data["isActive"] }
      /// Stable non-secret error code.
      public var errorCode: String? { __data["errorCode"] }
      /// UI-safe operation failure message.
      public var errorMessage: String? { __data["errorMessage"] }
      /// Durable creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Durable update timestamp.
      public var updatedAt: String { __data["updatedAt"] }
    }

    /// DefaultModelPreference
    ///
    /// Parent Type: `DefaultModelPreference`
    nonisolated public struct DefaultModelPreference: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.DefaultModelPreference }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerKind", String.self),
        .field("providerAccountId", String.self),
        .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
        .field("modelProfile", String?.self),
        .field("reasoningEffort", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.DefaultModelPreference.self
      ] }

      /// Provider family used for new workloads.
      public var providerKind: String { __data["providerKind"] }
      /// Provider account used for new workloads.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider-specific model profile.
      public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
      public var modelProfile: String? { __data["modelProfile"] }
      /// Optional provider-specific reasoning effort.
      public var reasoningEffort: String? { __data["reasoningEffort"] }
    }

    /// ProviderAccountCatalog
    ///
    /// Parent Type: `ProviderAccountCatalogEntry`
    nonisolated public struct ProviderAccountCatalog: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAccountCatalogEntry }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerKind", String.self),
        .field("displayName", String.self),
        .field("preferredAuthMethod", GraphQLEnum<NoemaAPI.ProviderAuthMethod>.self),
        .field("supportedAuthMethods", [GraphQLEnum<NoemaAPI.ProviderAuthMethod>].self),
        .field("capabilities", [Capability].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.ProviderAccountCatalog.self
      ] }

      public var providerKind: String { __data["providerKind"] }
      public var displayName: String { __data["displayName"] }
      public var preferredAuthMethod: GraphQLEnum<NoemaAPI.ProviderAuthMethod> { __data["preferredAuthMethod"] }
      public var supportedAuthMethods: [GraphQLEnum<NoemaAPI.ProviderAuthMethod>] { __data["supportedAuthMethods"] }
      public var capabilities: [Capability] { __data["capabilities"] }

      /// ProviderAccountCatalog.Capability
      ///
      /// Parent Type: `ProviderCapability`
      nonisolated public struct Capability: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderCapability }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("capabilityId", String.self),
          .field("status", String.self),
          .field("reliabilityContract", String.self),
          .field("dataFlowClass", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.ProviderAccountCatalog.Capability.self
        ] }

        public var capabilityId: String { __data["capabilityId"] }
        public var status: String { __data["status"] }
        public var reliabilityContract: String { __data["reliabilityContract"] }
        public var dataFlowClass: String { __data["dataFlowClass"] }
      }
    }

    /// ProviderAccount
    ///
    /// Parent Type: `ProviderAccount`
    nonisolated public struct ProviderAccount: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAccount }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerAccountId", String.self),
        .field("providerKind", String.self),
        .field("accountKey", String.self),
        .field("displayName", String.self),
        .field("authMethod", String.self),
        .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
        .field("isActive", Bool.self),
        .field("isDefault", Bool.self),
        .field("lastCheckedAt", String?.self),
        .field("lastAuthenticatedAt", String?.self),
        .field("lastErrorCode", String?.self),
        .field("lastErrorMessage", String?.self),
        .field("capabilities", [Capability].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.ProviderAccount.self
      ] }

      /// Stable provider account id.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider family, such as `codex`.
      public var providerKind: String { __data["providerKind"] }
      /// Provider-local account key.
      public var accountKey: String { __data["accountKey"] }
      /// Human-readable account name.
      public var displayName: String { __data["displayName"] }
      /// Authentication method used for this account.
      public var authMethod: String { __data["authMethod"] }
      /// Last known account readiness status.
      public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
      /// Whether the account may be selected.
      public var isActive: Bool { __data["isActive"] }
      /// Whether the account is the default account for its provider.
      public var isDefault: Bool { __data["isDefault"] }
      /// Last time Noema checked the account status.
      public var lastCheckedAt: String? { __data["lastCheckedAt"] }
      /// Last time Noema observed successful authentication.
      public var lastAuthenticatedAt: String? { __data["lastAuthenticatedAt"] }
      /// Last non-secret provider error code.
      public var lastErrorCode: String? { __data["lastErrorCode"] }
      /// Last non-secret provider error message.
      public var lastErrorMessage: String? { __data["lastErrorMessage"] }
      /// Provider capabilities available through this account.
      public var capabilities: [Capability] { __data["capabilities"] }

      /// ProviderAccount.Capability
      ///
      /// Parent Type: `ProviderCapability`
      nonisolated public struct Capability: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderCapability }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("capabilityId", String.self),
          .field("status", String.self),
          .field("reliabilityContract", String.self),
          .field("dataFlowClass", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.ProviderAccount.Capability.self
        ] }

        public var capabilityId: String { __data["capabilityId"] }
        public var status: String { __data["status"] }
        public var reliabilityContract: String { __data["reliabilityContract"] }
        public var dataFlowClass: String { __data["dataFlowClass"] }
      }
    }

    /// Mcp
    ///
    /// Parent Type: `CapabilityIntegration`
    nonisolated public struct Mcp: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityIntegration }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("kind", GraphQLEnum<NoemaAPI.CapabilityIntegrationKind>.self),
        .field("definitionId", String.self),
        .field("name", String.self),
        .field("sourceRevision", String.self),
        .field("reviewed", Bool.self),
        .field("sourceSummary", String.self),
        .field("connections", [Connection].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.Mcp.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
      public var definitionId: String { __data["definitionId"] }
      public var name: String { __data["name"] }
      public var sourceRevision: String { __data["sourceRevision"] }
      public var reviewed: Bool { __data["reviewed"] }
      public var sourceSummary: String { __data["sourceSummary"] }
      public var connections: [Connection] { __data["connections"] }

      /// Mcp.Connection
      ///
      /// Parent Type: `CapabilityConnection`
      nonisolated public struct Connection: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityConnection }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("kind", GraphQLEnum<NoemaAPI.CapabilityIntegrationKind>.self),
          .field("definitionId", String.self),
          .field("connectionId", String.self),
          .field("name", String.self),
          .field("connectionLabel", String?.self),
          .field("sourceRevision", String.self),
          .field("connectionRevision", String.self),
          .field("credentialRevision", Int?.self),
          .field("grantRevision", Int?.self),
          .field("policyRevision", Int.self),
          .field("status", String.self),
          .field("healthStatus", String.self),
          .field("authStatus", String.self),
          .field("dataSharingPolicy", String?.self),
          .field("unsafeActionPolicy", String?.self),
          .field("toolCount", Int.self),
          .field("availableToolCount", Int.self),
          .field("pendingToolCount", Int.self),
          .field("defaultedToolCount", Int.self),
          .field("disabledToolCount", Int.self),
          .field("sourceDetails", NoemaAPI.JSON.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.Mcp.Connection.self
        ] }

        public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
        public var definitionId: String { __data["definitionId"] }
        public var connectionId: String { __data["connectionId"] }
        public var name: String { __data["name"] }
        public var connectionLabel: String? { __data["connectionLabel"] }
        public var sourceRevision: String { __data["sourceRevision"] }
        public var connectionRevision: String { __data["connectionRevision"] }
        public var credentialRevision: Int? { __data["credentialRevision"] }
        public var grantRevision: Int? { __data["grantRevision"] }
        public var policyRevision: Int { __data["policyRevision"] }
        public var status: String { __data["status"] }
        public var healthStatus: String { __data["healthStatus"] }
        public var authStatus: String { __data["authStatus"] }
        public var dataSharingPolicy: String? { __data["dataSharingPolicy"] }
        public var unsafeActionPolicy: String? { __data["unsafeActionPolicy"] }
        public var toolCount: Int { __data["toolCount"] }
        public var availableToolCount: Int { __data["availableToolCount"] }
        public var pendingToolCount: Int { __data["pendingToolCount"] }
        public var defaultedToolCount: Int { __data["defaultedToolCount"] }
        public var disabledToolCount: Int { __data["disabledToolCount"] }
        public var sourceDetails: NoemaAPI.JSON { __data["sourceDetails"] }
      }
    }

    /// Api
    ///
    /// Parent Type: `CapabilityIntegration`
    nonisolated public struct Api: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityIntegration }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("kind", GraphQLEnum<NoemaAPI.CapabilityIntegrationKind>.self),
        .field("definitionId", String.self),
        .field("name", String.self),
        .field("sourceRevision", String.self),
        .field("reviewed", Bool.self),
        .field("sourceSummary", String.self),
        .field("connections", [Connection].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.Api.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
      public var definitionId: String { __data["definitionId"] }
      public var name: String { __data["name"] }
      public var sourceRevision: String { __data["sourceRevision"] }
      public var reviewed: Bool { __data["reviewed"] }
      public var sourceSummary: String { __data["sourceSummary"] }
      public var connections: [Connection] { __data["connections"] }

      /// Api.Connection
      ///
      /// Parent Type: `CapabilityConnection`
      nonisolated public struct Connection: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityConnection }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("kind", GraphQLEnum<NoemaAPI.CapabilityIntegrationKind>.self),
          .field("definitionId", String.self),
          .field("connectionId", String.self),
          .field("name", String.self),
          .field("connectionLabel", String?.self),
          .field("sourceRevision", String.self),
          .field("connectionRevision", String.self),
          .field("credentialRevision", Int?.self),
          .field("grantRevision", Int?.self),
          .field("policyRevision", Int.self),
          .field("status", String.self),
          .field("healthStatus", String.self),
          .field("authStatus", String.self),
          .field("dataSharingPolicy", String?.self),
          .field("unsafeActionPolicy", String?.self),
          .field("toolCount", Int.self),
          .field("availableToolCount", Int.self),
          .field("pendingToolCount", Int.self),
          .field("defaultedToolCount", Int.self),
          .field("disabledToolCount", Int.self),
          .field("sourceDetails", NoemaAPI.JSON.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsSnapshotQuery.Data.Api.Connection.self
        ] }

        public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
        public var definitionId: String { __data["definitionId"] }
        public var connectionId: String { __data["connectionId"] }
        public var name: String { __data["name"] }
        public var connectionLabel: String? { __data["connectionLabel"] }
        public var sourceRevision: String { __data["sourceRevision"] }
        public var connectionRevision: String { __data["connectionRevision"] }
        public var credentialRevision: Int? { __data["credentialRevision"] }
        public var grantRevision: Int? { __data["grantRevision"] }
        public var policyRevision: Int { __data["policyRevision"] }
        public var status: String { __data["status"] }
        public var healthStatus: String { __data["healthStatus"] }
        public var authStatus: String { __data["authStatus"] }
        public var dataSharingPolicy: String? { __data["dataSharingPolicy"] }
        public var unsafeActionPolicy: String? { __data["unsafeActionPolicy"] }
        public var toolCount: Int { __data["toolCount"] }
        public var availableToolCount: Int { __data["availableToolCount"] }
        public var pendingToolCount: Int { __data["pendingToolCount"] }
        public var defaultedToolCount: Int { __data["defaultedToolCount"] }
        public var disabledToolCount: Int { __data["disabledToolCount"] }
        public var sourceDetails: NoemaAPI.JSON { __data["sourceDetails"] }
      }
    }

    /// McpServer
    ///
    /// Parent Type: `McpServer`
    nonisolated public struct McpServer: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpServer }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("mcpServerId", String.self),
        .field("connectionRevision", String.self),
        .field("policyRevision", Int.self),
        .field("displayName", String.self),
        .field("transportKind", String.self),
        .field("healthStatus", String.self),
        .field("authStatus", String.self),
        .field("toolCount", Int.self),
        .field("pendingToolCount", Int.self),
        .field("browserOauthReauthenticationSupported", Bool.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSnapshotQuery.Data.McpServer.self
      ] }

      /// Durable MCP server id.
      public var mcpServerId: String { __data["mcpServerId"] }
      /// Exact connection authority generation used for policy fencing.
      public var connectionRevision: String { __data["connectionRevision"] }
      /// Monotonic provider-policy revision used for policy fencing.
      public var policyRevision: Int { __data["policyRevision"] }
      /// Human-visible server name.
      public var displayName: String { __data["displayName"] }
      /// Transport used to connect to the server.
      public var transportKind: String { __data["transportKind"] }
      /// Last known server health.
      public var healthStatus: String { __data["healthStatus"] }
      /// Last known server authentication state.
      public var authStatus: String { __data["authStatus"] }
      /// Number of discovered tools for this server.
      public var toolCount: Int { __data["toolCount"] }
      /// Number of tools waiting for background classification.
      public var pendingToolCount: Int { __data["pendingToolCount"] }
      /// Whether this persisted server can restart browser OAuth authorization.
      public var browserOauthReauthenticationSupported: Bool { __data["browserOauthReauthenticationSupported"] }
    }
  }
}
