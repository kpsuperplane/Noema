// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailOutcomeFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksDetailOutcomeFields on TaskDetail { __typename taskId taskDocument taskDocumentDigest resultDocument reviewDocument }"#
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
    .field("reviewDocument", String?.self),
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
  /// Current mutable RESULT.md content, when it exists.
  public var resultDocument: String? { __data["resultDocument"] }
  /// Current mutable REVIEW.md content, when it exists.
  public var reviewDocument: String? { __data["reviewDocument"] }
}
