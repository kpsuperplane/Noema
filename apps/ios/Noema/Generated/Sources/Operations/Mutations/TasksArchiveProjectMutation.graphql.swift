// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksArchiveProjectMutation: GraphQLMutation {
  public static let operationName: String = "TasksArchiveProject"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksArchiveProject($input: ArchiveProjectInput!) { archiveProject(input: $input) { __typename project { __typename ...TasksProjectFields } eventCursor clientMutationId } }"#,
      fragments: [TasksProjectFields.self]
    ))

  public var input: ArchiveProjectInput

  public init(input: ArchiveProjectInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("archiveProject", ArchiveProject.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksArchiveProjectMutation.Data.self
    ] }

    /// Archive a project through the semantic Work command service.
    public var archiveProject: ArchiveProject { __data["archiveProject"] }

    /// ArchiveProject
    ///
    /// Parent Type: `ProjectCommandPayload`
    nonisolated public struct ArchiveProject: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProjectCommandPayload }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("project", Project.self),
        .field("eventCursor", String.self),
        .field("clientMutationId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksArchiveProjectMutation.Data.ArchiveProject.self
      ] }

      /// Authoritative project projection.
      public var project: Project { __data["project"] }
      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }

      /// ArchiveProject.Project
      ///
      /// Parent Type: `Project`
      nonisolated public struct Project: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Project }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksProjectFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksArchiveProjectMutation.Data.ArchiveProject.Project.self,
          TasksProjectFields.self
        ] }

        /// Opaque project identity.
        public var projectId: String { __data["projectId"] }
        /// Owning workspace.
        public var workspaceId: String { __data["workspaceId"] }
        /// Project name.
        public var name: String { __data["name"] }
        /// Project description.
        public var description: String { __data["description"] }
        /// Optimistic project revision.
        public var revision: Int { __data["revision"] }
        /// Archive timestamp, if archived.
        public var archivedAt: String? { __data["archivedAt"] }
        /// Creation timestamp.
        public var createdAt: String { __data["createdAt"] }
        /// Last update timestamp.
        public var updatedAt: String { __data["updatedAt"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksProjectFields: TasksProjectFields { _toFragment() }
        }
      }
    }
  }
}
