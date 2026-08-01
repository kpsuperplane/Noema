// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveCapabilityConnectionLabelMutation: GraphQLMutation {
  public static let operationName: String = "SaveCapabilityConnectionLabel"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SaveCapabilityConnectionLabel($input: SaveCapabilityConnectionLabelInput!) { saveCapabilityConnectionLabel(input: $input) { __typename kind definitionId connectionId name connectionLabel sourceRevision connectionRevision policyRevision status healthStatus authStatus dataSharingPolicy unsafeActionPolicy toolCount availableToolCount pendingToolCount defaultedToolCount disabledToolCount sourceDetails } }"#
    ))

  public var input: SaveCapabilityConnectionLabelInput

  public init(input: SaveCapabilityConnectionLabelInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("saveCapabilityConnectionLabel", SaveCapabilityConnectionLabel.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SaveCapabilityConnectionLabelMutation.Data.self
    ] }

    /// Save a human-visible label without rotating connection authority.
    public var saveCapabilityConnectionLabel: SaveCapabilityConnectionLabel { __data["saveCapabilityConnectionLabel"] }

    /// SaveCapabilityConnectionLabel
    ///
    /// Parent Type: `CapabilityConnection`
    nonisolated public struct SaveCapabilityConnectionLabel: NoemaAPI.SelectionSet {
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
        SaveCapabilityConnectionLabelMutation.Data.SaveCapabilityConnectionLabel.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
      public var definitionId: String { __data["definitionId"] }
      public var connectionId: String { __data["connectionId"] }
      public var name: String { __data["name"] }
      public var connectionLabel: String? { __data["connectionLabel"] }
      public var sourceRevision: String { __data["sourceRevision"] }
      public var connectionRevision: String { __data["connectionRevision"] }
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
}
