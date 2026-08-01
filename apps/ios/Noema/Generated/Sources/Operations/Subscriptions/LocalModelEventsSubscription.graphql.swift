// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct LocalModelEventsSubscription: GraphQLSubscription {
  public static let operationName: String = "LocalModelEvents"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription LocalModelEvents($after: String) { localModelEvents(after: $after) { __typename cursor kind installationId modelId runtimeStatus createdAt installation { __typename ...NativeLocalModelInstallationFields } } }"#,
      fragments: [NativeLocalModelInstallationFields.self]
    ))

  public var after: GraphQLNullable<String>

  public init(after: GraphQLNullable<String>) {
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { ["after": after] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("localModelEvents", LocalModelEvents.self, arguments: ["after": .variable("after")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      LocalModelEventsSubscription.Data.self
    ] }

    /// Stream cursor-bearing local-model transfer, selection, and runtime events.
    public var localModelEvents: LocalModelEvents { __data["localModelEvents"] }

    /// LocalModelEvents
    ///
    /// Parent Type: `LocalModelEvent`
    nonisolated public struct LocalModelEvents: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("cursor", String.self),
        .field("kind", GraphQLEnum<NoemaAPI.LocalModelEventKind>.self),
        .field("installationId", String?.self),
        .field("modelId", String?.self),
        .field("runtimeStatus", GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus>?.self),
        .field("createdAt", String.self),
        .field("installation", Installation?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        LocalModelEventsSubscription.Data.LocalModelEvents.self
      ] }

      /// Exclusive cursor for reconnect/backfill.
      public var cursor: String { __data["cursor"] }
      /// Typed event category.
      public var kind: GraphQLEnum<NoemaAPI.LocalModelEventKind> { __data["kind"] }
      /// Installation affected by the event, when applicable.
      public var installationId: String? { __data["installationId"] }
      /// Model affected by the event, when applicable.
      public var modelId: String? { __data["modelId"] }
      /// Current runtime state, when applicable.
      public var runtimeStatus: GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus>? { __data["runtimeStatus"] }
      /// Durable event creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Current installation projection, when applicable.
      public var installation: Installation? { __data["installation"] }

      /// LocalModelEvents.Installation
      ///
      /// Parent Type: `LocalModelInstallation`
      nonisolated public struct Installation: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelInstallation }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(NativeLocalModelInstallationFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          LocalModelEventsSubscription.Data.LocalModelEvents.Installation.self,
          NativeLocalModelInstallationFields.self
        ] }

        /// Stable installation identifier.
        public var installationId: String { __data["installationId"] }
        /// Catalog model id, or an import-specific stable id.
        public var modelId: String { __data["modelId"] }
        /// Product-facing model name.
        public var name: String { __data["name"] }
        /// Artifact filename.
        public var file: String { __data["file"] }
        /// Model provenance.
        public var sourceKind: GraphQLEnum<NoemaAPI.LocalModelSourceKind> { __data["sourceKind"] }
        /// Current operation state.
        public var status: GraphQLEnum<NoemaAPI.LocalModelInstallationStatus> { __data["status"] }
        /// Verified content digest when known.
        public var sha256: String? { __data["sha256"] }
        /// Downloaded or copied bytes.
        public var completedBytes: Int { __data["completedBytes"] }
        /// Expected total bytes when known.
        public var totalBytes: Int? { __data["totalBytes"] }
        /// Disk bytes owned by the installation after deduplication.
        public var diskBytes: Int { __data["diskBytes"] }
        /// Runtime backend selected for this installation.
        public var backend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["backend"] }
        /// Whether this installation is Noema's active local model.
        public var isActive: Bool { __data["isActive"] }
        /// Stable non-secret error code.
        public var errorCode: String? { __data["errorCode"] }
        /// UI-safe operation failure message.
        public var errorMessage: String? { __data["errorMessage"] }
        /// Durable creation timestamp.
        public var createdAt: String { __data["createdAt"] }
        /// Durable update timestamp.
        public var updatedAt: String { __data["updatedAt"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var nativeLocalModelInstallationFields: NativeLocalModelInstallationFields { _toFragment() }
        }
      }
    }
  }
}
