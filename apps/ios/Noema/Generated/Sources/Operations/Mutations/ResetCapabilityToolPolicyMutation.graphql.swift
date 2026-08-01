// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ResetCapabilityToolPolicyMutation: GraphQLMutation {
  public static let operationName: String = "ResetCapabilityToolPolicy"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ResetCapabilityToolPolicy($input: ResetCapabilityToolPolicyInput!) { resetCapabilityToolPolicy(input: $input) { __typename kind connectionId toolId name description enabled readOnly { __typename value source } idempotent { __typename value source } destructive { __typename value source } openWorld { __typename value source } status policyRevision sourceRevision decisionPreview sourceDetails } }"#
    ))

  public var input: ResetCapabilityToolPolicyInput

  public init(input: ResetCapabilityToolPolicyInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("resetCapabilityToolPolicy", ResetCapabilityToolPolicy.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ResetCapabilityToolPolicyMutation.Data.self
    ] }

    /// Reset one exact tool to its current source/default behavior.
    public var resetCapabilityToolPolicy: ResetCapabilityToolPolicy { __data["resetCapabilityToolPolicy"] }

    /// ResetCapabilityToolPolicy
    ///
    /// Parent Type: `CapabilityManagedTool`
    nonisolated public struct ResetCapabilityToolPolicy: NoemaAPI.SelectionSet {
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
        ResetCapabilityToolPolicyMutation.Data.ResetCapabilityToolPolicy.self
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

      /// ResetCapabilityToolPolicy.ReadOnly
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
          ResetCapabilityToolPolicyMutation.Data.ResetCapabilityToolPolicy.ReadOnly.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }

      /// ResetCapabilityToolPolicy.Idempotent
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
          ResetCapabilityToolPolicyMutation.Data.ResetCapabilityToolPolicy.Idempotent.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }

      /// ResetCapabilityToolPolicy.Destructive
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
          ResetCapabilityToolPolicyMutation.Data.ResetCapabilityToolPolicy.Destructive.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }

      /// ResetCapabilityToolPolicy.OpenWorld
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
          ResetCapabilityToolPolicyMutation.Data.ResetCapabilityToolPolicy.OpenWorld.self
        ] }

        public var value: Bool? { __data["value"] }
        public var source: String? { __data["source"] }
      }
    }
  }
}
