// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsRetryLocalModelRuntimeMutation: GraphQLMutation {
  public static let operationName: String = "SettingsRetryLocalModelRuntime"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsRetryLocalModelRuntime { retryLocalModelRuntime }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("retryLocalModelRuntime", GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus>.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsRetryLocalModelRuntimeMutation.Data.self
    ] }

    /// Retry the supervised local llama.cpp runtime.
    public var retryLocalModelRuntime: GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus> { __data["retryLocalModelRuntime"] }
  }
}
