// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct DismissClientLiveActivityMutation: GraphQLMutation {
  public static let operationName: String = "DismissClientLiveActivity"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation DismissClientLiveActivity($activityId: String!) { dismissClientLiveActivity(activityId: $activityId) }"#
    ))

  public var activityId: String

  public init(activityId: String) {
    self.activityId = activityId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["activityId": activityId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("dismissClientLiveActivity", Bool.self, arguments: ["activityId": .variable("activityId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      DismissClientLiveActivityMutation.Data.self
    ] }

    public var dismissClientLiveActivity: Bool { __data["dismissClientLiveActivity"] }
  }
}
