// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct OnboardingModelSetupQuery: GraphQLQuery {
  public static let operationName: String = "OnboardingModelSetup"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query OnboardingModelSetup($providerAccountId: String!) { onboardingModelSetup(providerAccountId: $providerAccountId) { __typename providerKind providerAccountId providerDisplayName profiles { __typename id label disabledReason reasoningEfforts defaultReasoningEffort } recommendations { __typename useCase modelProfile reasoningEffort disabledReason } proposedSelections { __typename noema { __typename selectionMode modelProfile reasoningEffort fastMode } simpleTasks { __typename selectionMode modelProfile reasoningEffort fastMode } mediumTasks { __typename selectionMode modelProfile reasoningEffort fastMode } difficultTasks { __typename selectionMode modelProfile reasoningEffort fastMode } taskReviewer { __typename selectionMode modelProfile reasoningEffort fastMode } webFetchSummarizer { __typename selectionMode modelProfile reasoningEffort fastMode } toolProgressAudit { __typename selectionMode modelProfile reasoningEffort fastMode } actionReviewer { __typename selectionMode modelProfile reasoningEffort fastMode } memoryConsolidation { __typename selectionMode modelProfile reasoningEffort fastMode } } } }"#
    ))

  public var providerAccountId: String

  public init(providerAccountId: String) {
    self.providerAccountId = providerAccountId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["providerAccountId": providerAccountId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("onboardingModelSetup", OnboardingModelSetup.self, arguments: ["providerAccountId": .variable("providerAccountId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      OnboardingModelSetupQuery.Data.self
    ] }

    /// Return selectable models and role-aware proposals for one ready account.
    public var onboardingModelSetup: OnboardingModelSetup { __data["onboardingModelSetup"] }

    /// OnboardingModelSetup
    ///
    /// Parent Type: `OnboardingModelSetup`
    nonisolated public struct OnboardingModelSetup: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSetup }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerKind", String.self),
        .field("providerAccountId", String.self),
        .field("providerDisplayName", String.self),
        .field("profiles", [Profile].self),
        .field("recommendations", [Recommendation].self),
        .field("proposedSelections", ProposedSelections.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        OnboardingModelSetupQuery.Data.OnboardingModelSetup.self
      ] }

      public var providerKind: String { __data["providerKind"] }
      public var providerAccountId: String { __data["providerAccountId"] }
      public var providerDisplayName: String { __data["providerDisplayName"] }
      public var profiles: [Profile] { __data["profiles"] }
      public var recommendations: [Recommendation] { __data["recommendations"] }
      public var proposedSelections: ProposedSelections { __data["proposedSelections"] }

      /// OnboardingModelSetup.Profile
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
          .field("disabledReason", String?.self),
          .field("reasoningEfforts", [GraphQLEnum<NoemaAPI.ReasoningEffort>].self),
          .field("defaultReasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          OnboardingModelSetupQuery.Data.OnboardingModelSetup.Profile.self
        ] }

        /// Stable profile or model id.
        public var id: String { __data["id"] }
        /// User-facing label.
        public var label: String { __data["label"] }
        /// Why this option is disabled, when unavailable.
        public var disabledReason: String? { __data["disabledReason"] }
        /// Reasoning efforts available for this profile.
        public var reasoningEfforts: [GraphQLEnum<NoemaAPI.ReasoningEffort>] { __data["reasoningEfforts"] }
        /// Default reasoning effort for this profile, when advertised by metadata.
        public var defaultReasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["defaultReasoningEffort"] }
      }

      /// OnboardingModelSetup.Recommendation
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
          OnboardingModelSetupQuery.Data.OnboardingModelSetup.Recommendation.self
        ] }

        public var useCase: GraphQLEnum<NoemaAPI.NoemaModelUseCase> { __data["useCase"] }
        public var modelProfile: String { __data["modelProfile"] }
        public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
        public var disabledReason: String? { __data["disabledReason"] }
      }

      /// OnboardingModelSetup.ProposedSelections
      ///
      /// Parent Type: `OnboardingModelSelections`
      nonisolated public struct ProposedSelections: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelections }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("noema", Noema.self),
          .field("simpleTasks", SimpleTasks.self),
          .field("mediumTasks", MediumTasks.self),
          .field("difficultTasks", DifficultTasks.self),
          .field("taskReviewer", TaskReviewer.self),
          .field("webFetchSummarizer", WebFetchSummarizer.self),
          .field("toolProgressAudit", ToolProgressAudit.self),
          .field("actionReviewer", ActionReviewer?.self),
          .field("memoryConsolidation", MemoryConsolidation.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.self
        ] }

        public var noema: Noema { __data["noema"] }
        public var simpleTasks: SimpleTasks { __data["simpleTasks"] }
        public var mediumTasks: MediumTasks { __data["mediumTasks"] }
        public var difficultTasks: DifficultTasks { __data["difficultTasks"] }
        public var taskReviewer: TaskReviewer { __data["taskReviewer"] }
        public var webFetchSummarizer: WebFetchSummarizer { __data["webFetchSummarizer"] }
        public var toolProgressAudit: ToolProgressAudit { __data["toolProgressAudit"] }
        public var actionReviewer: ActionReviewer? { __data["actionReviewer"] }
        public var memoryConsolidation: MemoryConsolidation { __data["memoryConsolidation"] }

        /// OnboardingModelSetup.ProposedSelections.Noema
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct Noema: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.Noema.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.SimpleTasks
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct SimpleTasks: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.SimpleTasks.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.MediumTasks
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct MediumTasks: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.MediumTasks.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.DifficultTasks
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct DifficultTasks: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.DifficultTasks.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.TaskReviewer
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct TaskReviewer: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.TaskReviewer.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.WebFetchSummarizer
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct WebFetchSummarizer: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.WebFetchSummarizer.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.ToolProgressAudit
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct ToolProgressAudit: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.ToolProgressAudit.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.ActionReviewer
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct ActionReviewer: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.ActionReviewer.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }

        /// OnboardingModelSetup.ProposedSelections.MemoryConsolidation
        ///
        /// Parent Type: `OnboardingModelSelection`
        nonisolated public struct MemoryConsolidation: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingModelSelection }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
            .field("modelProfile", String?.self),
            .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
            .field("fastMode", Bool.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            OnboardingModelSetupQuery.Data.OnboardingModelSetup.ProposedSelections.MemoryConsolidation.self
          ] }

          /// Whether Noema or the human chooses the concrete model.
          public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
          /// Provider-specific model profile.
          public var modelProfile: String? { __data["modelProfile"] }
          /// Provider-supported reasoning effort.
          public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
          /// Whether this assignment requests faster service.
          public var fastMode: Bool { __data["fastMode"] }
        }
      }
    }
  }
}
