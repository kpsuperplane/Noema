// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ChatSaveCapabilityConnectionPolicyMutation: GraphQLMutation {
  public static let operationName: String = "ChatSaveCapabilityConnectionPolicy"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ChatSaveCapabilityConnectionPolicy($input: SaveCapabilityConnectionPolicyInput!) { saveCapabilityConnectionPolicy(input: $input) { __typename kind connectionId connectionRevision policyRevision status dataSharingPolicy unsafeActionPolicy } }"#
    ))

  public var input: SaveCapabilityConnectionPolicyInput

  public init(input: SaveCapabilityConnectionPolicyInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("saveCapabilityConnectionPolicy", SaveCapabilityConnectionPolicy.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ChatSaveCapabilityConnectionPolicyMutation.Data.self
    ] }

    /// Save both sharing and unsafe-call choices for one exact connection.
    public var saveCapabilityConnectionPolicy: SaveCapabilityConnectionPolicy { __data["saveCapabilityConnectionPolicy"] }

    /// SaveCapabilityConnectionPolicy
    ///
    /// Parent Type: `CapabilityConnection`
    nonisolated public struct SaveCapabilityConnectionPolicy: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityConnection }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("kind", GraphQLEnum<NoemaAPI.CapabilityIntegrationKind>.self),
        .field("connectionId", String.self),
        .field("connectionRevision", String.self),
        .field("policyRevision", Int.self),
        .field("status", String.self),
        .field("dataSharingPolicy", String?.self),
        .field("unsafeActionPolicy", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ChatSaveCapabilityConnectionPolicyMutation.Data.SaveCapabilityConnectionPolicy.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
      public var connectionId: String { __data["connectionId"] }
      public var connectionRevision: String { __data["connectionRevision"] }
      public var policyRevision: Int { __data["policyRevision"] }
      public var status: String { __data["status"] }
      public var dataSharingPolicy: String? { __data["dataSharingPolicy"] }
      public var unsafeActionPolicy: String? { __data["unsafeActionPolicy"] }
    }
  }
}
