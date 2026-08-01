// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct LocalModelSetupQuery: GraphQLQuery {
  public static let operationName: String = "LocalModelSetup"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query LocalModelSetup { localModelSetup { __typename isReady runtimeStatus recommendedModel { __typename ...NativeLocalModelCatalogEntryFields } installation { __typename ...NativeLocalModelInstallationFields } } }"#,
      fragments: [NativeLocalModelCatalogEntryFields.self, NativeLocalModelInstallationFields.self]
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("localModelSetup", LocalModelSetup.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      LocalModelSetupQuery.Data.self
    ] }

    /// Return the first-run local-model recommendation and readiness state.
    public var localModelSetup: LocalModelSetup { __data["localModelSetup"] }

    /// LocalModelSetup
    ///
    /// Parent Type: `LocalModelSetup`
    nonisolated public struct LocalModelSetup: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelSetup }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("isReady", Bool.self),
        .field("runtimeStatus", GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus>.self),
        .field("recommendedModel", RecommendedModel?.self),
        .field("installation", Installation?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        LocalModelSetupQuery.Data.LocalModelSetup.self
      ] }

      /// Whether setup is complete and local inference is usable.
      public var isReady: Bool { __data["isReady"] }
      /// Current supervised llama.cpp process state.
      public var runtimeStatus: GraphQLEnum<NoemaAPI.LocalModelRuntimeStatus> { __data["runtimeStatus"] }
      /// Top curated recommendation for this machine.
      public var recommendedModel: RecommendedModel? { __data["recommendedModel"] }
      /// Installation currently satisfying local setup, when present.
      public var installation: Installation? { __data["installation"] }

      /// LocalModelSetup.RecommendedModel
      ///
      /// Parent Type: `LocalModelCatalogEntry`
      nonisolated public struct RecommendedModel: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelCatalogEntry }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(NativeLocalModelCatalogEntryFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          LocalModelSetupQuery.Data.LocalModelSetup.RecommendedModel.self,
          NativeLocalModelCatalogEntryFields.self
        ] }

        /// Stable curated model identifier.
        public var modelId: String { __data["modelId"] }
        /// Product-facing model name.
        public var name: String { __data["name"] }
        /// License shown before download.
        public var license: String { __data["license"] }
        /// Recommendation priority from the bundled catalog.
        public var priority: Int { __data["priority"] }
        /// Pinned Hugging Face repository.
        public var repo: String { __data["repo"] }
        /// Immutable source revision.
        public var revision: String { __data["revision"] }
        /// Whether this is the top recommendation for the current machine.
        public var isRecommended: Bool { __data["isRecommended"] }
        /// Backend used by the selected artifact.
        public var compatibleBackend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["compatibleBackend"] }
        /// Best compatible artifact for this machine, when one fits.
        public var selectedBuild: SelectedBuild? { __data["selectedBuild"] }
        /// Detected hardware values that satisfy the artifact thresholds.
        public var hardwareFit: HardwareFit? { __data["hardwareFit"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var nativeLocalModelCatalogEntryFields: NativeLocalModelCatalogEntryFields { _toFragment() }
        }

        public typealias SelectedBuild = NativeLocalModelCatalogEntryFields.SelectedBuild

        public typealias HardwareFit = NativeLocalModelCatalogEntryFields.HardwareFit
      }

      /// LocalModelSetup.Installation
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
          LocalModelSetupQuery.Data.LocalModelSetup.Installation.self,
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
