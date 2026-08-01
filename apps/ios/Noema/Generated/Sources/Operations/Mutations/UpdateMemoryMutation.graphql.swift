// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct UpdateMemoryMutation: GraphQLMutation {
  public static let operationName: String = "UpdateMemory"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation UpdateMemory { updateMemory { __typename accepted status { __typename state active lastConsolidatedSequence lastConsolidatedItem error updatedAt } } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("updateMemory", UpdateMemory.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      UpdateMemoryMutation.Data.self
    ] }

    /// Queue one native Markdown memory update for the local primary conversation.
    public var updateMemory: UpdateMemory { __data["updateMemory"] }

    /// UpdateMemory
    ///
    /// Parent Type: `GraphqlNativeMemoryUpdateResult`
    nonisolated public struct UpdateMemory: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryUpdateResult }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("accepted", Bool.self),
        .field("status", Status.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        UpdateMemoryMutation.Data.UpdateMemory.self
      ] }

      public var accepted: Bool { __data["accepted"] }
      public var status: Status { __data["status"] }

      /// UpdateMemory.Status
      ///
      /// Parent Type: `GraphqlNativeMemoryUpdateStatus`
      nonisolated public struct Status: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryUpdateStatus }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("state", String.self),
          .field("active", Bool.self),
          .field("lastConsolidatedSequence", Int.self),
          .field("lastConsolidatedItem", String?.self),
          .field("error", String?.self),
          .field("updatedAt", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          UpdateMemoryMutation.Data.UpdateMemory.Status.self
        ] }

        public var state: String { __data["state"] }
        public var active: Bool { __data["active"] }
        public var lastConsolidatedSequence: Int { __data["lastConsolidatedSequence"] }
        public var lastConsolidatedItem: String? { __data["lastConsolidatedItem"] }
        public var error: String? { __data["error"] }
        public var updatedAt: String? { __data["updatedAt"] }
      }
    }
  }
}
