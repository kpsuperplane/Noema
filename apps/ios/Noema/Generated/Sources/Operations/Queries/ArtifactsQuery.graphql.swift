// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ArtifactsQuery: GraphQLQuery {
  public static let operationName: String = "Artifacts"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query Artifacts($ownerObjectType: String!, $ownerObjectId: String!, $limit: Int) { artifacts( ownerObjectType: $ownerObjectType ownerObjectId: $ownerObjectId limit: $limit ) { __typename artifactId ownerObjectType ownerObjectId title description artifactKind storageKind currentVersion { __typename artifactVersionId versionIndex externalUrl downloadUrl mediaType } } }"#
    ))

  public var ownerObjectType: String
  public var ownerObjectId: String
  public var limit: GraphQLNullable<Int32>

  public init(
    ownerObjectType: String,
    ownerObjectId: String,
    limit: GraphQLNullable<Int32>
  ) {
    self.ownerObjectType = ownerObjectType
    self.ownerObjectId = ownerObjectId
    self.limit = limit
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "ownerObjectType": ownerObjectType,
    "ownerObjectId": ownerObjectId,
    "limit": limit
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("artifacts", [Artifact].self, arguments: [
        "ownerObjectType": .variable("ownerObjectType"),
        "ownerObjectId": .variable("ownerObjectId"),
        "limit": .variable("limit")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ArtifactsQuery.Data.self
    ] }

    /// List artifacts for one concrete owner.
    public var artifacts: [Artifact] { __data["artifacts"] }

    /// Artifact
    ///
    /// Parent Type: `Artifact`
    nonisolated public struct Artifact: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Artifact }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("artifactId", String.self),
        .field("ownerObjectType", String.self),
        .field("ownerObjectId", String.self),
        .field("title", String.self),
        .field("description", String?.self),
        .field("artifactKind", String.self),
        .field("storageKind", GraphQLEnum<NoemaAPI.ArtifactStorageKind>.self),
        .field("currentVersion", CurrentVersion.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ArtifactsQuery.Data.Artifact.self
      ] }

      /// Stable artifact id.
      public var artifactId: String { __data["artifactId"] }
      /// Concrete owner object type.
      public var ownerObjectType: String { __data["ownerObjectType"] }
      /// Concrete owner object id.
      public var ownerObjectId: String { __data["ownerObjectId"] }
      /// Human-readable artifact title.
      public var title: String { __data["title"] }
      /// Optional artifact description.
      public var description: String? { __data["description"] }
      /// Product-defined artifact kind label.
      public var artifactKind: String { __data["artifactKind"] }
      /// Durable storage family shared by every version.
      public var storageKind: GraphQLEnum<NoemaAPI.ArtifactStorageKind> { __data["storageKind"] }
      /// Current immutable version.
      public var currentVersion: CurrentVersion { __data["currentVersion"] }

      /// Artifact.CurrentVersion
      ///
      /// Parent Type: `ArtifactVersion`
      nonisolated public struct CurrentVersion: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ArtifactVersion }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("artifactVersionId", String.self),
          .field("versionIndex", Int.self),
          .field("externalUrl", String?.self),
          .field("downloadUrl", String?.self),
          .field("mediaType", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ArtifactsQuery.Data.Artifact.CurrentVersion.self
        ] }

        /// Stable artifact version id.
        public var artifactVersionId: String { __data["artifactVersionId"] }
        /// Monotonic version index within the artifact.
        public var versionIndex: Int { __data["versionIndex"] }
        /// Durable external URL when the version is externally hosted.
        public var externalUrl: String? { __data["externalUrl"] }
        /// Local download route when the version is stored in Noema.
        public var downloadUrl: String? { __data["downloadUrl"] }
        /// Optional media type for the version payload.
        public var mediaType: String? { __data["mediaType"] }
      }
    }
  }
}
