// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeConnectionHealthQuery: GraphQLQuery {
  public static let operationName: String = "NativeConnectionHealth"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query NativeConnectionHealth { __typename }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeConnectionHealthQuery.Data.self
    ] }
  }
}
