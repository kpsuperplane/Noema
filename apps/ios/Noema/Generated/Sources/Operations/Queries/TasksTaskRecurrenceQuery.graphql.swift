// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksTaskRecurrenceQuery: GraphQLQuery {
  public static let operationName: String = "TasksTaskRecurrence"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksTaskRecurrence($recurrenceId: String!, $first: Int = 30) { taskRecurrence(recurrenceId: $recurrenceId, first: $first) { __typename recurrenceId title taskDocument taskDocumentDigest startsAt cronExpression timeZone missedRunPolicy overlapPolicy lifecycle revision nextRunAt pendingCoalescedAt occurrences { __typename recurrenceRevision scheduledFor localSlot trigger resolution taskId createdAt } } }"#
    ))

  public var recurrenceId: String
  public var first: GraphQLNullable<Int32>

  public init(
    recurrenceId: String,
    first: GraphQLNullable<Int32> = 30
  ) {
    self.recurrenceId = recurrenceId
    self.first = first
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "recurrenceId": recurrenceId,
    "first": first
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("taskRecurrence", TaskRecurrence.self, arguments: [
        "recurrenceId": .variable("recurrenceId"),
        "first": .variable("first")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskRecurrenceQuery.Data.self
    ] }

    /// Return recurring authority with newest occurrence history.
    public var taskRecurrence: TaskRecurrence { __data["taskRecurrence"] }

    /// TaskRecurrence
    ///
    /// Parent Type: `TaskRecurrence`
    nonisolated public struct TaskRecurrence: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRecurrence }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("recurrenceId", String.self),
        .field("title", String.self),
        .field("taskDocument", String.self),
        .field("taskDocumentDigest", String.self),
        .field("startsAt", String.self),
        .field("cronExpression", String.self),
        .field("timeZone", String.self),
        .field("missedRunPolicy", GraphQLEnum<NoemaAPI.MissedRunPolicy>.self),
        .field("overlapPolicy", GraphQLEnum<NoemaAPI.OverlapPolicy>.self),
        .field("lifecycle", GraphQLEnum<NoemaAPI.RecurrenceLifecycle>.self),
        .field("revision", Int.self),
        .field("nextRunAt", String?.self),
        .field("pendingCoalescedAt", String?.self),
        .field("occurrences", [Occurrence].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksTaskRecurrenceQuery.Data.TaskRecurrence.self
      ] }

      /// Recurring template identity.
      public var recurrenceId: String { __data["recurrenceId"] }
      /// Current title for future occurrences.
      public var title: String { __data["title"] }
      /// Current template TASK.md content.
      public var taskDocument: String { __data["taskDocument"] }
      /// Transient SHA-256 of the current template.
      public var taskDocumentDigest: String { __data["taskDocumentDigest"] }
      /// Inclusive UTC lower bound.
      public var startsAt: String { __data["startsAt"] }
      /// Five-field cron expression.
      public var cronExpression: String { __data["cronExpression"] }
      /// Authoring IANA timezone.
      public var timeZone: String { __data["timeZone"] }
      /// Missed-window behavior.
      public var missedRunPolicy: GraphQLEnum<NoemaAPI.MissedRunPolicy> { __data["missedRunPolicy"] }
      /// Overlap behavior.
      public var overlapPolicy: GraphQLEnum<NoemaAPI.OverlapPolicy> { __data["overlapPolicy"] }
      /// Lifecycle.
      public var lifecycle: GraphQLEnum<NoemaAPI.RecurrenceLifecycle> { __data["lifecycle"] }
      /// Optimistic template revision.
      public var revision: Int { __data["revision"] }
      /// Next projected UTC slot.
      public var nextRunAt: String? { __data["nextRunAt"] }
      /// Retained coalesced UTC slot.
      public var pendingCoalescedAt: String? { __data["pendingCoalescedAt"] }
      /// Newest occurrence history.
      public var occurrences: [Occurrence] { __data["occurrences"] }

      /// TaskRecurrence.Occurrence
      ///
      /// Parent Type: `RecurrenceOccurrence`
      nonisolated public struct Occurrence: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.RecurrenceOccurrence }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("recurrenceRevision", Int.self),
          .field("scheduledFor", String.self),
          .field("localSlot", String.self),
          .field("trigger", GraphQLEnum<NoemaAPI.RecurrenceOccurrenceTrigger>.self),
          .field("resolution", GraphQLEnum<NoemaAPI.RecurrenceOccurrenceResolution>.self),
          .field("taskId", String?.self),
          .field("createdAt", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksTaskRecurrenceQuery.Data.TaskRecurrence.Occurrence.self
        ] }

        /// Template revision used for this slot.
        public var recurrenceRevision: Int { __data["recurrenceRevision"] }
        /// Exact UTC slot.
        public var scheduledFor: String { __data["scheduledFor"] }
        /// Deduplicated local wall-clock minute.
        public var localSlot: String { __data["localSlot"] }
        /// Whether cron or an explicit request created this occurrence.
        public var trigger: GraphQLEnum<NoemaAPI.RecurrenceOccurrenceTrigger> { __data["trigger"] }
        /// Materialized, skipped, or coalesced disposition.
        public var resolution: GraphQLEnum<NoemaAPI.RecurrenceOccurrenceResolution> { __data["resolution"] }
        /// Ordinary child task when materialized.
        public var taskId: String? { __data["taskId"] }
        /// Audit timestamp.
        public var createdAt: String { __data["createdAt"] }
      }
    }
  }
}
