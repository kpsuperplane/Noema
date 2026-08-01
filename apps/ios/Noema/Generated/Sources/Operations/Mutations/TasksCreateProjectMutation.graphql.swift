// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksCreateProjectMutation: GraphQLMutation {
  public static let operationName: String = "TasksCreateProject"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksCreateProject($input: CreateProjectInput!) { createProject(input: $input) { __typename project { __typename ...TasksProjectFields } eventCursor clientMutationId } }"#,
      fragments: [TasksProjectFields.self]
    ))

  public var input: CreateProjectInput

  public init(input: CreateProjectInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("createProject", CreateProject.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksCreateProjectMutation.Data.self
    ] }

    /// Create a project through the semantic Work command service.
    public var createProject: CreateProject { __data["createProject"] }

    /// CreateProject
    ///
    /// Parent Type: `ProjectCommandPayload`
    nonisolated public struct CreateProject: NoemaAPI.SelectionSet {
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
        TasksCreateProjectMutation.Data.CreateProject.self
      ] }

      /// Authoritative project projection.
      public var project: Project { __data["project"] }
      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }

      /// CreateProject.Project
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
          TasksCreateProjectMutation.Data.CreateProject.Project.self,
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
