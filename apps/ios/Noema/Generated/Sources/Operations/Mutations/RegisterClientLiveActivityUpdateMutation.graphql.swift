// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RegisterClientLiveActivityUpdateMutation: GraphQLMutation {
  public static let operationName: String = "RegisterClientLiveActivityUpdate"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation RegisterClientLiveActivityUpdate($input: RegisterClientLiveActivityUpdateInput!) { registerClientLiveActivityUpdate(input: $input) }"#
    ))

  public var input: RegisterClientLiveActivityUpdateInput

  public init(input: RegisterClientLiveActivityUpdateInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("registerClientLiveActivityUpdate", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      RegisterClientLiveActivityUpdateMutation.Data.self
    ] }

    public var registerClientLiveActivityUpdate: Bool { __data["registerClientLiveActivityUpdate"] }
  }
}
