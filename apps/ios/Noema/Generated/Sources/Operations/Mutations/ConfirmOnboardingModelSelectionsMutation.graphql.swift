// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ConfirmOnboardingModelSelectionsMutation: GraphQLMutation {
  public static let operationName: String = "ConfirmOnboardingModelSelections"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ConfirmOnboardingModelSelections($input: ConfirmOnboardingModelSelectionsInput!) { confirmOnboardingModelSelections(input: $input) { __typename isUserOnboarded steps { __typename id status providerKind providerAccountId accountKey displayName } } }"#
    ))

  public var input: ConfirmOnboardingModelSelectionsInput

  public init(input: ConfirmOnboardingModelSelectionsInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("confirmOnboardingModelSelections", ConfirmOnboardingModelSelections.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ConfirmOnboardingModelSelectionsMutation.Data.self
    ] }

    /// Atomically confirm every first-run model assignment.
    public var confirmOnboardingModelSelections: ConfirmOnboardingModelSelections { __data["confirmOnboardingModelSelections"] }

    /// ConfirmOnboardingModelSelections
    ///
    /// Parent Type: `OnboardingStatus`
    nonisolated public struct ConfirmOnboardingModelSelections: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingStatus }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("isUserOnboarded", Bool.self),
        .field("steps", [Step].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ConfirmOnboardingModelSelectionsMutation.Data.ConfirmOnboardingModelSelections.self
      ] }

      /// Whether chat can start.
      public var isUserOnboarded: Bool { __data["isUserOnboarded"] }
      /// Ordered onboarding steps.
      public var steps: [Step] { __data["steps"] }

      /// ConfirmOnboardingModelSelections.Step
      ///
      /// Parent Type: `OnboardingStep`
      nonisolated public struct Step: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingStep }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("id", String.self),
          .field("status", GraphQLEnum<NoemaAPI.OnboardingStepStatus>.self),
          .field("providerKind", String?.self),
          .field("providerAccountId", String?.self),
          .field("accountKey", String?.self),
          .field("displayName", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConfirmOnboardingModelSelectionsMutation.Data.ConfirmOnboardingModelSelections.Step.self
        ] }

        /// Stable step id.
        public var id: String { __data["id"] }
        /// Step status.
        public var status: GraphQLEnum<NoemaAPI.OnboardingStepStatus> { __data["status"] }
        /// Provider family when the step is provider-backed.
        public var providerKind: String? { __data["providerKind"] }
        /// Provider account id when the step is provider-backed.
        public var providerAccountId: String? { __data["providerAccountId"] }
        /// Provider-local account key when the step is provider-backed.
        public var accountKey: String? { __data["accountKey"] }
        /// Human-readable account name when the step is provider-backed.
        public var displayName: String? { __data["displayName"] }
      }
    }
  }
}
