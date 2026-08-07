// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksProjectsQuery: GraphQLQuery {
  public static let operationName: String = "TasksProjects"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksProjects($workspaceId: String!, $includeArchived: Boolean! = false, $first: Int = 100, $after: String) { projects( workspaceId: $workspaceId includeArchived: $includeArchived first: $first after: $after ) { __typename edges { __typename cursor node { __typename ...TasksProjectFields } } pageInfo { __typename ...TasksPageInfoFields } } }"#,
      fragments: [TasksPageInfoFields.self, TasksProjectFields.self]
    ))

  public var workspaceId: String
  public var includeArchived: Bool
  public var first: GraphQLNullable<Int32>
  public var after: GraphQLNullable<String>

  public init(
    workspaceId: String,
    includeArchived: Bool = false,
    first: GraphQLNullable<Int32> = 100,
    after: GraphQLNullable<String>
  ) {
    self.workspaceId = workspaceId
    self.includeArchived = includeArchived
    self.first = first
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "workspaceId": workspaceId,
    "includeArchived": includeArchived,
    "first": first,
    "after": after
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("projects", Projects.self, arguments: [
        "workspaceId": .variable("workspaceId"),
        "includeArchived": .variable("includeArchived"),
        "first": .variable("first"),
        "after": .variable("after")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksProjectsQuery.Data.self
    ] }

    /// List owner-authorized projects in stable update order.
    public var projects: Projects { __data["projects"] }

    /// Projects
    ///
    /// Parent Type: `ProjectConnection`
    nonisolated public struct Projects: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProjectConnection }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("edges", [Edge].self),
        .field("pageInfo", PageInfo.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksProjectsQuery.Data.Projects.self
      ] }

      /// Ordered project edges.
      public var edges: [Edge] { __data["edges"] }
      /// Pagination metadata.
      public var pageInfo: PageInfo { __data["pageInfo"] }

      /// Projects.Edge
      ///
      /// Parent Type: `ProjectEdge`
      nonisolated public struct Edge: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProjectEdge }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("cursor", String.self),
          .field("node", Node.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksProjectsQuery.Data.Projects.Edge.self
        ] }

        /// Opaque keyset cursor.
        public var cursor: String { __data["cursor"] }
        /// Connection node.
        public var node: Node { __data["node"] }

        /// Projects.Edge.Node
        ///
        /// Parent Type: `Project`
        nonisolated public struct Node: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Project }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksProjectFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksProjectsQuery.Data.Projects.Edge.Node.self,
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
          /// Optional absolute project working folder.
          public var folder: String? { __data["folder"] }
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

      /// Projects.PageInfo
      ///
      /// Parent Type: `PageInfo`
      nonisolated public struct PageInfo: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.PageInfo }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksPageInfoFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksProjectsQuery.Data.Projects.PageInfo.self,
          TasksPageInfoFields.self
        ] }

        /// Cursor for the final edge in this page.
        public var endCursor: String? { __data["endCursor"] }
        /// Whether another page exists.
        public var hasNextPage: Bool { __data["hasNextPage"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksPageInfoFields: TasksPageInfoFields { _toFragment() }
        }
      }
    }
  }
}
