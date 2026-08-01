// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SkipAdapterAuthenticationMutation: GraphQLMutation {
  public static let operationName: String = "SkipAdapterAuthentication"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SkipAdapterAuthentication($input: SkipAdapterAuthenticationInput!) { skipAdapterAuthentication(input: $input) { __typename requestId revision state failureCode } }"#
    ))

  public var input: SkipAdapterAuthenticationInput

  public init(input: SkipAdapterAuthenticationInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("skipAdapterAuthentication", SkipAdapterAuthentication.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SkipAdapterAuthenticationMutation.Data.self
    ] }

    /// Skip one exact API adapter call and continue its interrupted origin.
    public var skipAdapterAuthentication: SkipAdapterAuthentication { __data["skipAdapterAuthentication"] }

    /// SkipAdapterAuthentication
    ///
    /// Parent Type: `AdapterAuthenticationIntervention`
    nonisolated public struct SkipAdapterAuthentication: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterAuthenticationIntervention }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("requestId", String.self),
        .field("revision", Int.self),
        .field("state", GraphQLEnum<NoemaAPI.McpAuthenticationRequestState>.self),
        .field("failureCode", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SkipAdapterAuthenticationMutation.Data.SkipAdapterAuthentication.self
      ] }

      public var requestId: String { __data["requestId"] }
      public var revision: Int { __data["revision"] }
      public var state: GraphQLEnum<NoemaAPI.McpAuthenticationRequestState> { __data["state"] }
      public var failureCode: String? { __data["failureCode"] }
    }
  }
}
