// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RuntimeDebugProfileQuery: GraphQLQuery {
  public static let operationName: String = "RuntimeDebugProfile"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query RuntimeDebugProfile($input: RuntimeDebugProfileInput!) { runtimeDebugProfile(input: $input) { __typename kind scopeId status startedAt endedAt elapsedMilliseconds accountedMilliseconds uninstrumentedMilliseconds spans { __typename id category name status startedAt endedAt startOffsetMilliseconds durationMilliseconds provider model phase responseIndex roundIndex toolName correlationId inputTokens cachedInputTokens outputTokens totalTokens } } }"#
    ))

  public var input: RuntimeDebugProfileInput

  public init(input: RuntimeDebugProfileInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("runtimeDebugProfile", RuntimeDebugProfile?.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      RuntimeDebugProfileQuery.Data.self
    ] }

    /// Return a live or durable runtime profile for one authorized turn or run.
    public var runtimeDebugProfile: RuntimeDebugProfile? { __data["runtimeDebugProfile"] }

    /// RuntimeDebugProfile
    ///
    /// Parent Type: `RuntimeDebugProfile`
    nonisolated public struct RuntimeDebugProfile: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.RuntimeDebugProfile }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("kind", GraphQLEnum<NoemaAPI.RuntimeDebugScopeKind>.self),
        .field("scopeId", String.self),
        .field("status", GraphQLEnum<NoemaAPI.RuntimeDebugStatus>.self),
        .field("startedAt", String.self),
        .field("endedAt", String?.self),
        .field("elapsedMilliseconds", Int.self),
        .field("accountedMilliseconds", Int.self),
        .field("uninstrumentedMilliseconds", Int.self),
        .field("spans", [Span].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        RuntimeDebugProfileQuery.Data.RuntimeDebugProfile.self
      ] }

      public var kind: GraphQLEnum<NoemaAPI.RuntimeDebugScopeKind> { __data["kind"] }
      public var scopeId: String { __data["scopeId"] }
      public var status: GraphQLEnum<NoemaAPI.RuntimeDebugStatus> { __data["status"] }
      public var startedAt: String { __data["startedAt"] }
      public var endedAt: String? { __data["endedAt"] }
      public var elapsedMilliseconds: Int { __data["elapsedMilliseconds"] }
      public var accountedMilliseconds: Int { __data["accountedMilliseconds"] }
      public var uninstrumentedMilliseconds: Int { __data["uninstrumentedMilliseconds"] }
      public var spans: [Span] { __data["spans"] }

      /// RuntimeDebugProfile.Span
      ///
      /// Parent Type: `RuntimeDebugSpan`
      nonisolated public struct Span: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.RuntimeDebugSpan }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("id", String.self),
          .field("category", GraphQLEnum<NoemaAPI.RuntimeDebugSpanCategory>.self),
          .field("name", String.self),
          .field("status", GraphQLEnum<NoemaAPI.RuntimeDebugStatus>.self),
          .field("startedAt", String.self),
          .field("endedAt", String?.self),
          .field("startOffsetMilliseconds", Int.self),
          .field("durationMilliseconds", Int.self),
          .field("provider", String?.self),
          .field("model", String?.self),
          .field("phase", String?.self),
          .field("responseIndex", Int?.self),
          .field("roundIndex", Int?.self),
          .field("toolName", String?.self),
          .field("correlationId", String?.self),
          .field("inputTokens", Int?.self),
          .field("cachedInputTokens", Int?.self),
          .field("outputTokens", Int?.self),
          .field("totalTokens", Int?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          RuntimeDebugProfileQuery.Data.RuntimeDebugProfile.Span.self
        ] }

        public var id: String { __data["id"] }
        public var category: GraphQLEnum<NoemaAPI.RuntimeDebugSpanCategory> { __data["category"] }
        public var name: String { __data["name"] }
        public var status: GraphQLEnum<NoemaAPI.RuntimeDebugStatus> { __data["status"] }
        public var startedAt: String { __data["startedAt"] }
        public var endedAt: String? { __data["endedAt"] }
        public var startOffsetMilliseconds: Int { __data["startOffsetMilliseconds"] }
        public var durationMilliseconds: Int { __data["durationMilliseconds"] }
        public var provider: String? { __data["provider"] }
        public var model: String? { __data["model"] }
        public var phase: String? { __data["phase"] }
        public var responseIndex: Int? { __data["responseIndex"] }
        public var roundIndex: Int? { __data["roundIndex"] }
        public var toolName: String? { __data["toolName"] }
        public var correlationId: String? { __data["correlationId"] }
        public var inputTokens: Int? { __data["inputTokens"] }
        public var cachedInputTokens: Int? { __data["cachedInputTokens"] }
        public var outputTokens: Int? { __data["outputTokens"] }
        public var totalTokens: Int? { __data["totalTokens"] }
      }
    }
  }
}
