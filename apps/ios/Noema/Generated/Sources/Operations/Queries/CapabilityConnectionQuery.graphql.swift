// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct CapabilityConnectionQuery: GraphQLQuery {
  public static let operationName: String = "CapabilityConnection"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query CapabilityConnection($ref: CapabilityConnectionRefInput!) { capabilityConnection(ref: $ref) { __typename kind definitionId connectionId name connectionLabel sourceRevision connectionRevision credentialRevision grantRevision policyRevision status healthStatus authStatus dataSharingPolicy unsafeActionPolicy toolCount availableToolCount pendingToolCount defaultedToolCount disabledToolCount sourceDetails } capabilityTools(ref: $ref) { __typename kind connectionId toolId name description enabled readOnly { __typename value source } idempotent { __typename value source } destructive { __typename value source } openWorld { __typename value source } status policyRevision sourceRevision decisionPreview sourceDetails } }"#
    ))

  public var ref: CapabilityConnectionRefInput

  public init(ref: CapabilityConnectionRefInput) {
    self.ref = ref
  }

  @_spi(Unsafe) public var __variables: Variables? { ["ref": ref] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("capabilityConnection", CapabilityConnection?.self, arguments: ["ref": .variable("ref")]),
      .field("capabilityTools", [CapabilityTool].self, arguments: ["ref": .variable("ref")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      CapabilityConnectionQuery.Data.self
    ] }

    /// Return one concrete source-owned connection by structured identity.
    public var capabilityConnection: CapabilityConnection? { __data["capabilityConnection"] }
    /// Return tools for one concrete source-owned connection.
    public var capabilityTools: [CapabilityTool] { __data["capabilityTools"] }

    /// CapabilityConnection
    ///
    /// Parent Type: `CapabilityConnection`
    nonisolated public struct CapabilityConnection: NoemaAPI.SelectionSet {
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
        .field("credentialRevision", Int?.self),
        .field("grantRevision", Int?.self),
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
        CapabilityConnectionQuery.Data.CapabilityConnection.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
      public var definitionId: String { __data["definitionId"] }
      public var connectionId: String { __data["connectionId"] }
      public var name: String { __data["name"] }
      public var connectionLabel: String? { __data["connectionLabel"] }
      public var sourceRevision: String { __data["sourceRevision"] }
      public var connectionRevision: String { __data["connectionRevision"] }
      public var credentialRevision: Int? { __data["credentialRevision"] }
      public var grantRevision: Int? { __data["grantRevision"] }
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

    /// CapabilityTool
    ///
    /// Parent Type: `CapabilityManagedTool`
    nonisolated public struct CapabilityTool: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityManagedTool }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("kind", GraphQLEnum<NoemaAPI.CapabilityIntegrationKind>.self),
        .field("connectionId", String.self),
        .field("toolId", String.self),
        .field("name", String.self),
        .field("description", String?.self),
        .field("enabled", Bool.self),
        .field("readOnly", ReadOnly.self),
        .field("idempotent", Idempotent.self),
        .field("destructive", Destructive.self),
        .field("openWorld", OpenWorld.self),
        .field("status", String.self),
        .field("policyRevision", Int.self),
        .field("sourceRevision", String.self),
        .field("decisionPreview", String?.self),
        .field("sourceDetails", NoemaAPI.JSON.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        CapabilityConnectionQuery.Data.CapabilityTool.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.CapabilityIntegrationKind> { __data["kind"] }
      public var connectionId: String { __data["connectionId"] }
      public var toolId: String { __data["toolId"] }
      public var name: String { __data["name"] }
      public var description: String? { __data["description"] }
      public var enabled: Bool { __data["enabled"] }
      public var readOnly: ReadOnly { __data["readOnly"] }
      public var idempotent: Idempotent { __data["idempotent"] }
      public var destructive: Destructive { __data["destructive"] }
      public var openWorld: OpenWorld { __data["openWorld"] }
      public var status: String { __data["status"] }
      public var policyRevision: Int { __data["policyRevision"] }
      public var sourceRevision: String { __data["sourceRevision"] }
      public var decisionPreview: String? { __data["decisionPreview"] }
      public var sourceDetails: NoemaAPI.JSON { __data["sourceDetails"] }

      /// CapabilityTool.ReadOnly
      ///
      /// Parent Type: `CapabilityManagedToolHint`
      nonisolated public struct ReadOnly: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityManagedToolHint }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("value", Bool?.self),
          .field("source", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          CapabilityConnectionQuery.Data.CapabilityTool.ReadOnly.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }

      /// CapabilityTool.Idempotent
      ///
      /// Parent Type: `CapabilityManagedToolHint`
      nonisolated public struct Idempotent: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityManagedToolHint }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("value", Bool?.self),
          .field("source", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          CapabilityConnectionQuery.Data.CapabilityTool.Idempotent.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }

      /// CapabilityTool.Destructive
      ///
      /// Parent Type: `CapabilityManagedToolHint`
      nonisolated public struct Destructive: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityManagedToolHint }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("value", Bool?.self),
          .field("source", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          CapabilityConnectionQuery.Data.CapabilityTool.Destructive.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }

      /// CapabilityTool.OpenWorld
      ///
      /// Parent Type: `CapabilityManagedToolHint`
      nonisolated public struct OpenWorld: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityManagedToolHint }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("value", Bool?.self),
          .field("source", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          CapabilityConnectionQuery.Data.CapabilityTool.OpenWorld.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }
    }
  }
}
