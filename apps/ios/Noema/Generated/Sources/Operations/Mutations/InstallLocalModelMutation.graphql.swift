// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct InstallLocalModelMutation: GraphQLMutation {
  public static let operationName: String = "InstallLocalModel"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation InstallLocalModel($input: InstallLocalModelInput!) { installLocalModel(input: $input) { __typename ...NativeLocalModelInstallationFields } }"#,
      fragments: [NativeLocalModelInstallationFields.self]
    ))

  public var input: InstallLocalModelInput

  public init(input: InstallLocalModelInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("installLocalModel", InstallLocalModel.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      InstallLocalModelMutation.Data.self
    ] }

    /// Install one curated local model using the selected machine build.
    public var installLocalModel: InstallLocalModel { __data["installLocalModel"] }

    /// InstallLocalModel
    ///
    /// Parent Type: `LocalModelInstallation`
    nonisolated public struct InstallLocalModel: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelInstallation }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(NativeLocalModelInstallationFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        InstallLocalModelMutation.Data.InstallLocalModel.self,
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
