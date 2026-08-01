// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ArtifactVersionDetailQuery: GraphQLQuery {
  public static let operationName: String = "ArtifactVersionDetail"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query ArtifactVersionDetail($artifactVersionId: String!) { artifactVersionDetail(artifactVersionId: $artifactVersionId) { __typename artifactVersionId artifactId versionIndex title artifactKind storageKind mediaType previewKind markdown plainText downloadUrl externalUrl versions { __typename artifactVersionId versionIndex downloadUrl externalUrl mediaType } } }"#
    ))

  public var artifactVersionId: String

  public init(artifactVersionId: String) {
    self.artifactVersionId = artifactVersionId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["artifactVersionId": artifactVersionId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("artifactVersionDetail", ArtifactVersionDetail?.self, arguments: ["artifactVersionId": .variable("artifactVersionId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ArtifactVersionDetailQuery.Data.self
    ] }

    /// Load one artifact version detail payload for the chat detail rail.
    public var artifactVersionDetail: ArtifactVersionDetail? { __data["artifactVersionDetail"] }

    /// ArtifactVersionDetail
    ///
    /// Parent Type: `ArtifactVersionDetail`
    nonisolated public struct ArtifactVersionDetail: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ArtifactVersionDetail }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("artifactVersionId", String.self),
        .field("artifactId", String.self),
        .field("versionIndex", Int.self),
        .field("title", String.self),
        .field("artifactKind", String.self),
        .field("storageKind", GraphQLEnum<NoemaAPI.ArtifactStorageKind>.self),
        .field("mediaType", String?.self),
        .field("previewKind", GraphQLEnum<NoemaAPI.ArtifactVersionPreviewKind>.self),
        .field("markdown", String?.self),
        .field("plainText", String?.self),
        .field("downloadUrl", String?.self),
        .field("externalUrl", String?.self),
        .field("versions", [Version].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ArtifactVersionDetailQuery.Data.ArtifactVersionDetail.self
      ] }

      /// Stable artifact version id.
      public var artifactVersionId: String { __data["artifactVersionId"] }
      /// Parent artifact id.
      public var artifactId: String { __data["artifactId"] }
      /// Monotonic version index within the artifact.
      public var versionIndex: Int { __data["versionIndex"] }
      /// Display title inherited from version title or artifact title.
      public var title: String { __data["title"] }
      /// Product-defined artifact kind label.
      public var artifactKind: String { __data["artifactKind"] }
      /// Durable storage family shared by the artifact.
      public var storageKind: GraphQLEnum<NoemaAPI.ArtifactStorageKind> { __data["storageKind"] }
      /// Optional media type for the version payload.
      public var mediaType: String? { __data["mediaType"] }
      /// Preview renderer selected by the server.
      public var previewKind: GraphQLEnum<NoemaAPI.ArtifactVersionPreviewKind> { __data["previewKind"] }
      /// Markdown content when previewKind is MARKDOWN.
      public var markdown: String? { __data["markdown"] }
      /// Literal text content when previewKind is PLAIN_TEXT.
      public var plainText: String? { __data["plainText"] }
      /// Local download route when the version is stored in Noema.
      public var downloadUrl: String? { __data["downloadUrl"] }
      /// External durable URL when the version is externally hosted.
      public var externalUrl: String? { __data["externalUrl"] }
      /// Full immutable version history in ascending version order.
      public var versions: [Version] { __data["versions"] }

      /// ArtifactVersionDetail.Version
      ///
      /// Parent Type: `ArtifactVersion`
      nonisolated public struct Version: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ArtifactVersion }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("artifactVersionId", String.self),
          .field("versionIndex", Int.self),
          .field("downloadUrl", String?.self),
          .field("externalUrl", String?.self),
          .field("mediaType", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ArtifactVersionDetailQuery.Data.ArtifactVersionDetail.Version.self
        ] }

        /// Stable artifact version id.
        public var artifactVersionId: String { __data["artifactVersionId"] }
        /// Monotonic version index within the artifact.
        public var versionIndex: Int { __data["versionIndex"] }
        /// Local download route when the version is stored in Noema.
        public var downloadUrl: String? { __data["downloadUrl"] }
        /// Durable external URL when the version is externally hosted.
        public var externalUrl: String? { __data["externalUrl"] }
        /// Optional media type for the version payload.
        public var mediaType: String? { __data["mediaType"] }
      }
    }
  }
}
