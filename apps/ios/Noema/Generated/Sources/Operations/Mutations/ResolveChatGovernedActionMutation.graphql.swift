// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ResolveChatGovernedActionMutation: GraphQLMutation {
  public static let operationName: String = "ResolveChatGovernedAction"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ResolveChatGovernedAction($input: ResolveGovernedActionInput!) { resolveGovernedAction(input: $input) { __typename actionId revision state output failureCode } }"#
    ))

  public var input: ResolveGovernedActionInput

  public init(input: ResolveGovernedActionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("resolveGovernedAction", ResolveGovernedAction.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ResolveChatGovernedActionMutation.Data.self
    ] }

    /// Approve or decline one immutable governed-action revision.
    public var resolveGovernedAction: ResolveGovernedAction { __data["resolveGovernedAction"] }

    /// ResolveGovernedAction
    ///
    /// Parent Type: `GovernedAction`
    nonisolated public struct ResolveGovernedAction: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GovernedAction }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("actionId", String.self),
        .field("revision", Int.self),
        .field("state", GraphQLEnum<NoemaAPI.GovernedActionState>.self),
        .field("output", NoemaAPI.JSON?.self),
        .field("failureCode", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ResolveChatGovernedActionMutation.Data.ResolveGovernedAction.self
      ] }

      public var actionId: String { __data["actionId"] }
      public var revision: Int { __data["revision"] }
      public var state: GraphQLEnum<NoemaAPI.GovernedActionState> { __data["state"] }
      public var output: NoemaAPI.JSON? { __data["output"] }
      public var failureCode: String? { __data["failureCode"] }
    }
  }
}
