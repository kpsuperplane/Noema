// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksSubmissionFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksSubmissionFields on TaskSubmission { __typename submissionId contractId executorRunId reviewRound summary resultMarkdown criteria { __typename criterionId evidenceMarkdown } artifacts { __typename artifactId artifactVersionId title artifactKind storageKind mediaType downloadUrl externalUrl } createdAt }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSubmission }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("submissionId", String.self),
    .field("contractId", String.self),
    .field("executorRunId", String.self),
    .field("reviewRound", Int.self),
    .field("summary", String.self),
    .field("resultMarkdown", String.self),
    .field("criteria", [Criterium].self),
    .field("artifacts", [Artifact].self),
    .field("createdAt", String.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksSubmissionFields.self
  ] }

  /// Submission identity.
  public var submissionId: String { __data["submissionId"] }
  /// Contract evaluated.
  public var contractId: String { __data["contractId"] }
  /// Executor run.
  public var executorRunId: String { __data["executorRunId"] }
  /// Review round.
  public var reviewRound: Int { __data["reviewRound"] }
  /// Short summary.
  public var summary: String { __data["summary"] }
  /// Complete result Markdown.
  public var resultMarkdown: String { __data["resultMarkdown"] }
  /// Criterion evidence.
  public var criteria: [Criterium] { __data["criteria"] }
  /// Linked immutable artifact versions.
  public var artifacts: [Artifact] { __data["artifacts"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }

  /// Criterium
  ///
  /// Parent Type: `TaskSubmissionCriterion`
  nonisolated public struct Criterium: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSubmissionCriterion }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("criterionId", String.self),
      .field("evidenceMarkdown", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksSubmissionFields.Criterium.self
    ] }

    /// Criterion identity.
    public var criterionId: String { __data["criterionId"] }
    /// Evidence Markdown.
    public var evidenceMarkdown: String { __data["evidenceMarkdown"] }
  }

  /// Artifact
  ///
  /// Parent Type: `TaskSubmissionArtifact`
  nonisolated public struct Artifact: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSubmissionArtifact }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("artifactId", String.self),
      .field("artifactVersionId", String.self),
      .field("title", String.self),
      .field("artifactKind", String.self),
      .field("storageKind", GraphQLEnum<NoemaAPI.ArtifactStorageKind>.self),
      .field("mediaType", String?.self),
      .field("downloadUrl", String?.self),
      .field("externalUrl", String?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksSubmissionFields.Artifact.self
    ] }

    /// Artifact identity.
    public var artifactId: String { __data["artifactId"] }
    /// Artifact version identity.
    public var artifactVersionId: String { __data["artifactVersionId"] }
    /// Artifact title.
    public var title: String { __data["title"] }
    /// Artifact kind.
    public var artifactKind: String { __data["artifactKind"] }
    /// Storage kind.
    public var storageKind: GraphQLEnum<NoemaAPI.ArtifactStorageKind> { __data["storageKind"] }
    /// Media type, when present.
    public var mediaType: String? { __data["mediaType"] }
    /// Local download route, when present.
    public var downloadUrl: String? { __data["downloadUrl"] }
    /// External URL, when present.
    public var externalUrl: String? { __data["externalUrl"] }
  }
}
