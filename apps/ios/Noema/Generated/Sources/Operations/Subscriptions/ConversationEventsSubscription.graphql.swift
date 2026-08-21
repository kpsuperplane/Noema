// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ConversationEventsSubscription: GraphQLSubscription {
  public static let operationName: String = "ConversationEvents"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription ConversationEvents($conversationId: String!) { conversationEvents(conversationId: $conversationId) { __typename ... on SubscriptionReadyEvent { conversationId } ... on HumanInterventionsChangedEvent { conversationId } ... on ConversationItemEvent { conversationId clientMessageId itemId cursor itemTurnId: turnId metadata item { __typename ... on UserText { text } ... on AssistantText { text } ... on Activity { id activityKind status title summary metadata } ... on A2UISurface { id interactionId surfaceId version revision interactionRevision lifecycle catalog snapshot hasActions } ... on MultipleChoicePrompt { prompt selectionMode options { __typename id label } } ... on MultipleChoiceSelection { promptItemId selectionMode selectedOptions { __typename id label } } ... on ErrorNotice { message recoverable } ... on ArtifactReference { artifactId artifactVersionId title artifactKind storageKind externalUrl downloadUrl mediaType } ... on TaskReference { taskId task { __typename ...TasksTaskReferenceSummaryFields } } } } ... on AssistantTextDeltaEvent { conversationId deltaTurnId: turnId streamId responseIndex delta } ... on AgentStatusEvent { conversationId status } ... on TurnCompletedEvent { conversationId clientMessageId } } }"#,
      fragments: [TasksTaskReferenceSummaryFields.self]
    ))

  public var conversationId: String

  public init(conversationId: String) {
    self.conversationId = conversationId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["conversationId": conversationId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("conversationEvents", ConversationEvents.self, arguments: ["conversationId": .variable("conversationId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ConversationEventsSubscription.Data.self
    ] }

    /// Stream conversation events.
    public var conversationEvents: ConversationEvents { __data["conversationEvents"] }

    /// ConversationEvents
    ///
    /// Parent Type: `ConversationEvent`
    nonisolated public struct ConversationEvents: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Unions.ConversationEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .inlineFragment(AsSubscriptionReadyEvent.self),
        .inlineFragment(AsHumanInterventionsChangedEvent.self),
        .inlineFragment(AsConversationItemEvent.self),
        .inlineFragment(AsAssistantTextDeltaEvent.self),
        .inlineFragment(AsAgentStatusEvent.self),
        .inlineFragment(AsTurnCompletedEvent.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ConversationEventsSubscription.Data.ConversationEvents.self
      ] }

      public var asSubscriptionReadyEvent: AsSubscriptionReadyEvent? { _asInlineFragment() }
      public var asHumanInterventionsChangedEvent: AsHumanInterventionsChangedEvent? { _asInlineFragment() }
      public var asConversationItemEvent: AsConversationItemEvent? { _asInlineFragment() }
      public var asAssistantTextDeltaEvent: AsAssistantTextDeltaEvent? { _asInlineFragment() }
      public var asAgentStatusEvent: AsAgentStatusEvent? { _asInlineFragment() }
      public var asTurnCompletedEvent: AsTurnCompletedEvent? { _asInlineFragment() }

      /// ConversationEvents.AsSubscriptionReadyEvent
      ///
      /// Parent Type: `SubscriptionReadyEvent`
      nonisolated public struct AsSubscriptionReadyEvent: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionReadyEvent }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("conversationId", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConversationEventsSubscription.Data.ConversationEvents.self,
          ConversationEventsSubscription.Data.ConversationEvents.AsSubscriptionReadyEvent.self
        ] }

        /// Durable Noema conversation id.
        public var conversationId: String { __data["conversationId"] }
      }

      /// ConversationEvents.AsHumanInterventionsChangedEvent
      ///
      /// Parent Type: `HumanInterventionsChangedEvent`
      nonisolated public struct AsHumanInterventionsChangedEvent: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.HumanInterventionsChangedEvent }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("conversationId", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConversationEventsSubscription.Data.ConversationEvents.self,
          ConversationEventsSubscription.Data.ConversationEvents.AsHumanInterventionsChangedEvent.self
        ] }

        /// Durable Noema conversation id.
        public var conversationId: String { __data["conversationId"] }
      }

      /// ConversationEvents.AsConversationItemEvent
      ///
      /// Parent Type: `ConversationItemEvent`
      nonisolated public struct AsConversationItemEvent: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ConversationItemEvent }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("conversationId", String.self),
          .field("clientMessageId", String?.self),
          .field("itemId", String.self),
          .field("cursor", String?.self),
          .field("turnId", alias: "itemTurnId", String?.self),
          .field("metadata", NoemaAPI.JSON.self),
          .field("item", Item.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConversationEventsSubscription.Data.ConversationEvents.self,
          ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.self
        ] }

        /// Durable Noema conversation id.
        public var conversationId: String { __data["conversationId"] }
        /// Frontend-generated id for optimistic UI correlation.
        public var clientMessageId: String? { __data["clientMessageId"] }
        /// Durable conversation item id.
        public var itemId: String { __data["itemId"] }
        /// Opaque durable pagination cursor, absent for transient runtime rows.
        public var cursor: String? { __data["cursor"] }
        /// Durable conversation turn id.
        public var itemTurnId: String? { __data["itemTurnId"] }
        /// Structured durable item metadata.
        public var metadata: NoemaAPI.JSON { __data["metadata"] }
        /// Transcript item to render.
        public var item: Item { __data["item"] }

        /// ConversationEvents.AsConversationItemEvent.Item
        ///
        /// Parent Type: `TranscriptItem`
        nonisolated public struct Item: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Unions.TranscriptItem }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .inlineFragment(AsUserText.self),
            .inlineFragment(AsAssistantText.self),
            .inlineFragment(AsActivity.self),
            .inlineFragment(AsA2UISurface.self),
            .inlineFragment(AsMultipleChoicePrompt.self),
            .inlineFragment(AsMultipleChoiceSelection.self),
            .inlineFragment(AsErrorNotice.self),
            .inlineFragment(AsArtifactReference.self),
            .inlineFragment(AsTaskReference.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self
          ] }

          public var asUserText: AsUserText? { _asInlineFragment() }
          public var asAssistantText: AsAssistantText? { _asInlineFragment() }
          public var asActivity: AsActivity? { _asInlineFragment() }
          public var asA2UISurface: AsA2UISurface? { _asInlineFragment() }
          public var asMultipleChoicePrompt: AsMultipleChoicePrompt? { _asInlineFragment() }
          public var asMultipleChoiceSelection: AsMultipleChoiceSelection? { _asInlineFragment() }
          public var asErrorNotice: AsErrorNotice? { _asInlineFragment() }
          public var asArtifactReference: AsArtifactReference? { _asInlineFragment() }
          public var asTaskReference: AsTaskReference? { _asInlineFragment() }

          /// ConversationEvents.AsConversationItemEvent.Item.AsUserText
          ///
          /// Parent Type: `UserText`
          nonisolated public struct AsUserText: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.UserText }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("text", String.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsUserText.self
            ] }

            /// Text authored by the user.
            public var text: String { __data["text"] }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsAssistantText
          ///
          /// Parent Type: `AssistantText`
          nonisolated public struct AsAssistantText: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AssistantText }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("text", String.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsAssistantText.self
            ] }

            /// Text to render as the assistant response.
            public var text: String { __data["text"] }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsActivity
          ///
          /// Parent Type: `Activity`
          nonisolated public struct AsActivity: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Activity }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("id", String.self),
              .field("activityKind", String.self),
              .field("status", GraphQLEnum<NoemaAPI.TurnActivityStatus>.self),
              .field("title", String.self),
              .field("summary", String?.self),
              .field("metadata", NoemaAPI.JSON.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsActivity.self
            ] }

            /// Stable activity id.
            public var id: String { __data["id"] }
            /// Activity kind.
            public var activityKind: String { __data["activityKind"] }
            /// Activity status.
            public var status: GraphQLEnum<NoemaAPI.TurnActivityStatus> { __data["status"] }
            /// Short display title.
            public var title: String { __data["title"] }
            /// Optional summary.
            public var summary: String? { __data["summary"] }
            /// Structured metadata as JSON.
            public var metadata: NoemaAPI.JSON { __data["metadata"] }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsA2UISurface
          ///
          /// Parent Type: `A2UISurface`
          nonisolated public struct AsA2UISurface: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.A2UISurface }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("id", String.self),
              .field("interactionId", String?.self),
              .field("surfaceId", String.self),
              .field("version", String.self),
              .field("revision", Int.self),
              .field("interactionRevision", Int?.self),
              .field("lifecycle", String.self),
              .field("catalog", NoemaAPI.JSON.self),
              .field("snapshot", NoemaAPI.JSON.self),
              .field("hasActions", Bool.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsA2UISurface.self
            ] }

            /// Stable transcript projection id.
            public var id: String { __data["id"] }
            /// Durable interaction id for action-bearing surfaces.
            public var interactionId: String? { __data["interactionId"] }
            /// Provider-authored surface id.
            public var surfaceId: String { __data["surfaceId"] }
            /// A2UI protocol version.
            public var version: String { __data["version"] }
            /// Reduced surface revision.
            public var revision: Int { __data["revision"] }
            /// Durable interaction revision used for action CAS.
            public var interactionRevision: Int? { __data["interactionRevision"] }
            /// Durable lifecycle such as pending, answered, or completed.
            public var lifecycle: String { __data["lifecycle"] }
            /// Advertised Noema A2UI catalog.
            public var catalog: NoemaAPI.JSON { __data["catalog"] }
            /// Complete validated reduced surface snapshot.
            public var snapshot: NoemaAPI.JSON { __data["snapshot"] }
            /// Whether the current snapshot exposes an action.
            public var hasActions: Bool { __data["hasActions"] }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoicePrompt
          ///
          /// Parent Type: `MultipleChoicePrompt`
          nonisolated public struct AsMultipleChoicePrompt: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MultipleChoicePrompt }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("prompt", String.self),
              .field("selectionMode", GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode>.self),
              .field("options", [Option].self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoicePrompt.self
            ] }

            /// Question or instruction shown above the options.
            public var prompt: String { __data["prompt"] }
            /// Whether one or many options may be selected.
            public var selectionMode: GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode> { __data["selectionMode"] }
            /// Ordered selectable options.
            public var options: [Option] { __data["options"] }

            /// ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoicePrompt.Option
            ///
            /// Parent Type: `MultipleChoiceOption`
            nonisolated public struct Option: NoemaAPI.SelectionSet {
              @_spi(Unsafe) public let __data: DataDict
              @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

              @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MultipleChoiceOption }
              @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
                .field("__typename", String.self),
                .field("id", String.self),
                .field("label", String.self),
              ] }
              @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
                ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoicePrompt.Option.self
              ] }

              /// Stable semantic option id.
              public var id: String { __data["id"] }
              /// Human-visible label.
              public var label: String { __data["label"] }
            }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoiceSelection
          ///
          /// Parent Type: `MultipleChoiceSelection`
          nonisolated public struct AsMultipleChoiceSelection: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MultipleChoiceSelection }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("promptItemId", String.self),
              .field("selectionMode", GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode>.self),
              .field("selectedOptions", [SelectedOption].self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoiceSelection.self
            ] }

            /// Prompt item this selection answers.
            public var promptItemId: String { __data["promptItemId"] }
            /// Selection mode from the prompt.
            public var selectionMode: GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode> { __data["selectionMode"] }
            /// Selected options in prompt order.
            public var selectedOptions: [SelectedOption] { __data["selectedOptions"] }

            /// ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoiceSelection.SelectedOption
            ///
            /// Parent Type: `MultipleChoiceOption`
            nonisolated public struct SelectedOption: NoemaAPI.SelectionSet {
              @_spi(Unsafe) public let __data: DataDict
              @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

              @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MultipleChoiceOption }
              @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
                .field("__typename", String.self),
                .field("id", String.self),
                .field("label", String.self),
              ] }
              @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
                ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsMultipleChoiceSelection.SelectedOption.self
              ] }

              /// Stable semantic option id.
              public var id: String { __data["id"] }
              /// Human-visible label.
              public var label: String { __data["label"] }
            }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsErrorNotice
          ///
          /// Parent Type: `ErrorNotice`
          nonisolated public struct AsErrorNotice: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ErrorNotice }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("message", String.self),
              .field("recoverable", Bool.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsErrorNotice.self
            ] }

            /// Human-readable error message.
            public var message: String { __data["message"] }
            /// Whether the chat turn can continue.
            public var recoverable: Bool { __data["recoverable"] }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsArtifactReference
          ///
          /// Parent Type: `ArtifactReference`
          nonisolated public struct AsArtifactReference: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ArtifactReference }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("artifactId", String.self),
              .field("artifactVersionId", String?.self),
              .field("title", String.self),
              .field("artifactKind", String.self),
              .field("storageKind", String.self),
              .field("externalUrl", String?.self),
              .field("downloadUrl", String?.self),
              .field("mediaType", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsArtifactReference.self
            ] }

            /// Stable artifact id.
            public var artifactId: String { __data["artifactId"] }
            /// Optional referenced artifact version id.
            public var artifactVersionId: String? { __data["artifactVersionId"] }
            /// Display title captured in the transcript row.
            public var title: String { __data["title"] }
            /// Product-defined artifact kind label.
            public var artifactKind: String { __data["artifactKind"] }
            /// Durable storage family for the referenced artifact.
            public var storageKind: String { __data["storageKind"] }
            /// External durable URL when the artifact is externally hosted.
            public var externalUrl: String? { __data["externalUrl"] }
            /// Local download route when the artifact bytes live in Noema.
            public var downloadUrl: String? { __data["downloadUrl"] }
            /// Optional media type for the referenced version payload.
            public var mediaType: String? { __data["mediaType"] }
          }

          /// ConversationEvents.AsConversationItemEvent.Item.AsTaskReference
          ///
          /// Parent Type: `TaskReference`
          nonisolated public struct AsTaskReference: NoemaAPI.InlineFragment {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item
            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReference }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("taskId", String.self),
              .field("task", Task?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.self,
              ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsTaskReference.self
            ] }

            /// Stable task id.
            public var taskId: String { __data["taskId"] }
            /// Current task summary for this reference.
            public var task: Task? { __data["task"] }

            /// ConversationEvents.AsConversationItemEvent.Item.AsTaskReference.Task
            ///
            /// Parent Type: `TaskSummary`
            nonisolated public struct Task: NoemaAPI.SelectionSet {
              @_spi(Unsafe) public let __data: DataDict
              @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

              @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSummary }
              @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
                .field("__typename", String.self),
                .fragment(TasksTaskReferenceSummaryFields.self),
              ] }
              @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
                ConversationEventsSubscription.Data.ConversationEvents.AsConversationItemEvent.Item.AsTaskReference.Task.self,
                TasksTaskReferenceSummaryFields.self
              ] }

              /// Task identity.
              public var taskId: String { __data["taskId"] }
              /// Human title.
              public var title: String { __data["title"] }
              /// The only task-level state.
              public var stage: Stage { __data["stage"] }
              /// Completion timestamp, when any.
              public var completedAt: String? { __data["completedAt"] }
              /// Current run projection.
              public var currentRun: CurrentRun? { __data["currentRun"] }
              /// Derived attention.
              public var attention: Attention? { __data["attention"] }

              public struct Fragments: FragmentContainer {
                @_spi(Unsafe) public let __data: DataDict
                @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

                public var tasksTaskReferenceSummaryFields: TasksTaskReferenceSummaryFields { _toFragment() }
              }

              public typealias Stage = TasksTaskReferenceSummaryFields.Stage

              public typealias CurrentRun = TasksTaskReferenceSummaryFields.CurrentRun

              public typealias Attention = TasksTaskReferenceSummaryFields.Attention
            }
          }
        }
      }

      /// ConversationEvents.AsAssistantTextDeltaEvent
      ///
      /// Parent Type: `AssistantTextDeltaEvent`
      nonisolated public struct AsAssistantTextDeltaEvent: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AssistantTextDeltaEvent }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("conversationId", String.self),
          .field("turnId", alias: "deltaTurnId", String.self),
          .field("streamId", String.self),
          .field("responseIndex", Int.self),
          .field("delta", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConversationEventsSubscription.Data.ConversationEvents.self,
          ConversationEventsSubscription.Data.ConversationEvents.AsAssistantTextDeltaEvent.self
        ] }

        /// Durable Noema conversation id.
        public var conversationId: String { __data["conversationId"] }
        /// Durable conversation turn id.
        public var deltaTurnId: String { __data["deltaTurnId"] }
        /// Runtime stream id.
        public var streamId: String { __data["streamId"] }
        /// Zero-based response item index within the provider response.
        public var responseIndex: Int { __data["responseIndex"] }
        /// Assistant text delta.
        public var delta: String { __data["delta"] }
      }

      /// ConversationEvents.AsAgentStatusEvent
      ///
      /// Parent Type: `AgentStatusEvent`
      nonisolated public struct AsAgentStatusEvent: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentStatusEvent }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("conversationId", String.self),
          .field("status", GraphQLEnum<NoemaAPI.AgentStatus>.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConversationEventsSubscription.Data.ConversationEvents.self,
          ConversationEventsSubscription.Data.ConversationEvents.AsAgentStatusEvent.self
        ] }

        /// Durable Noema conversation id.
        public var conversationId: String { __data["conversationId"] }
        /// Current agent status.
        public var status: GraphQLEnum<NoemaAPI.AgentStatus> { __data["status"] }
      }

      /// ConversationEvents.AsTurnCompletedEvent
      ///
      /// Parent Type: `TurnCompletedEvent`
      nonisolated public struct AsTurnCompletedEvent: NoemaAPI.InlineFragment {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ConversationEventsSubscription.Data.ConversationEvents
        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TurnCompletedEvent }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("conversationId", String.self),
          .field("clientMessageId", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ConversationEventsSubscription.Data.ConversationEvents.self,
          ConversationEventsSubscription.Data.ConversationEvents.AsTurnCompletedEvent.self
        ] }

        /// Durable Noema conversation id.
        public var conversationId: String { __data["conversationId"] }
        /// Frontend-generated id for optimistic UI correlation.
        public var clientMessageId: String? { __data["clientMessageId"] }
      }
    }
  }
}
