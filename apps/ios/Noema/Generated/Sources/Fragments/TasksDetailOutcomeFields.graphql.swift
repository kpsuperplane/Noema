// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailOutcomeFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksDetailOutcomeFields on TaskDetail { __typename taskId taskDocument taskDocumentDigest resultDocument resultMetadata reviewDocument workspaceFiles { __typename path isDirectory } workspaceFilesTruncated }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("taskId", String.self),
    .field("taskDocument", String.self),
    .field("taskDocumentDigest", String.self),
    .field("resultDocument", String?.self),
    .field("resultMetadata", NoemaAPI.JSON.self),
    .field("reviewDocument", String?.self),
    .field("workspaceFiles", [WorkspaceFile].self),
    .field("workspaceFilesTruncated", Bool.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksDetailOutcomeFields.self
  ] }

  /// Task identity.
  public var taskId: String { __data["taskId"] }
  /// Current mutable TASK.md content.
  public var taskDocument: String { __data["taskDocument"] }
  /// Transient SHA-256 of the current Task document.
  public var taskDocumentDigest: String { __data["taskDocumentDigest"] }
  /// Readable current RESULT.md text, when it exists.
  public var resultDocument: String? { __data["resultDocument"] }
  /// Provider citation metadata for the projected result.
  public var resultMetadata: NoemaAPI.JSON { __data["resultMetadata"] }
  /// Current mutable REVIEW.md content, when it exists.
  public var reviewDocument: String? { __data["reviewDocument"] }
  /// Bounded recursive manifest of the current Task working directory.
  public var workspaceFiles: [WorkspaceFile] { __data["workspaceFiles"] }
  /// Whether the workspace manifest exceeded its safe entry limit.
  public var workspaceFilesTruncated: Bool { __data["workspaceFilesTruncated"] }

  /// WorkspaceFile
  ///
  /// Parent Type: `TaskWorkspaceFile`
  nonisolated public struct WorkspaceFile: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskWorkspaceFile }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("path", String.self),
      .field("isDirectory", Bool.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.WorkspaceFile.self
    ] }

    /// Path relative to the Task working directory.
    public var path: String { __data["path"] }
    /// Whether the entry is a directory.
    public var isDirectory: Bool { __data["isDirectory"] }
  }
}
