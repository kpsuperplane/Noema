// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeConversationItemFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment NativeConversationItemFields on ConversationItem { __typename itemId cursor turnId metadata item { __typename ... on UserText { text } ... on AssistantText { text } ... on Activity { id activityKind status title summary metadata } ... on A2UISurface { id interactionId surfaceId version revision interactionRevision lifecycle catalog snapshot hasActions } ... on MultipleChoicePrompt { prompt selectionMode options { __typename id label } } ... on MultipleChoiceSelection { promptItemId selectionMode selectedOptions { __typename id label } } ... on ErrorNotice { message recoverable } ... on ArtifactReference { artifactId artifactVersionId title artifactKind storageKind externalUrl downloadUrl mediaType } ... on TaskReference { taskId } } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ConversationItem }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("itemId", String.self),
    .field("cursor", String.self),
    .field("turnId", String?.self),
    .field("metadata", NoemaAPI.JSON.self),
    .field("item", Item.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    NativeConversationItemFields.self
  ] }

  /// Durable conversation item id.
  public var itemId: String { __data["itemId"] }
  /// Opaque durable pagination cursor.
  public var cursor: String { __data["cursor"] }
  /// Durable conversation turn id.
  public var turnId: String? { __data["turnId"] }
  /// Structured durable item metadata.
  public var metadata: NoemaAPI.JSON { __data["metadata"] }
  /// Transcript item to render.
  public var item: Item { __data["item"] }

  /// Item
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
      NativeConversationItemFields.Item.self
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

    /// Item.AsUserText
    ///
    /// Parent Type: `UserText`
    nonisolated public struct AsUserText: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.UserText }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("text", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsUserText.self
      ] }

      /// Text authored by the user.
      public var text: String { __data["text"] }
    }

    /// Item.AsAssistantText
    ///
    /// Parent Type: `AssistantText`
    nonisolated public struct AsAssistantText: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AssistantText }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("text", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsAssistantText.self
      ] }

      /// Text to render as the assistant response.
      public var text: String { __data["text"] }
    }

    /// Item.AsActivity
    ///
    /// Parent Type: `Activity`
    nonisolated public struct AsActivity: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
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
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsActivity.self
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

    /// Item.AsA2UISurface
    ///
    /// Parent Type: `A2UISurface`
    nonisolated public struct AsA2UISurface: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
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
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsA2UISurface.self
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

    /// Item.AsMultipleChoicePrompt
    ///
    /// Parent Type: `MultipleChoicePrompt`
    nonisolated public struct AsMultipleChoicePrompt: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MultipleChoicePrompt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("prompt", String.self),
        .field("selectionMode", GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode>.self),
        .field("options", [Option].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsMultipleChoicePrompt.self
      ] }

      /// Question or instruction shown above the options.
      public var prompt: String { __data["prompt"] }
      /// Whether one or many options may be selected.
      public var selectionMode: GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode> { __data["selectionMode"] }
      /// Ordered selectable options.
      public var options: [Option] { __data["options"] }

      /// Item.AsMultipleChoicePrompt.Option
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
          NativeConversationItemFields.Item.AsMultipleChoicePrompt.Option.self
        ] }

        /// Stable semantic option id.
        public var id: String { __data["id"] }
        /// Human-visible label.
        public var label: String { __data["label"] }
      }
    }

    /// Item.AsMultipleChoiceSelection
    ///
    /// Parent Type: `MultipleChoiceSelection`
    nonisolated public struct AsMultipleChoiceSelection: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MultipleChoiceSelection }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("promptItemId", String.self),
        .field("selectionMode", GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode>.self),
        .field("selectedOptions", [SelectedOption].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsMultipleChoiceSelection.self
      ] }

      /// Prompt item this selection answers.
      public var promptItemId: String { __data["promptItemId"] }
      /// Selection mode from the prompt.
      public var selectionMode: GraphQLEnum<NoemaAPI.MultipleChoiceSelectionMode> { __data["selectionMode"] }
      /// Selected options in prompt order.
      public var selectedOptions: [SelectedOption] { __data["selectedOptions"] }

      /// Item.AsMultipleChoiceSelection.SelectedOption
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
          NativeConversationItemFields.Item.AsMultipleChoiceSelection.SelectedOption.self
        ] }

        /// Stable semantic option id.
        public var id: String { __data["id"] }
        /// Human-visible label.
        public var label: String { __data["label"] }
      }
    }

    /// Item.AsErrorNotice
    ///
    /// Parent Type: `ErrorNotice`
    nonisolated public struct AsErrorNotice: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ErrorNotice }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("message", String.self),
        .field("recoverable", Bool.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsErrorNotice.self
      ] }

      /// Human-readable error message.
      public var message: String { __data["message"] }
      /// Whether the chat turn can continue.
      public var recoverable: Bool { __data["recoverable"] }
    }

    /// Item.AsArtifactReference
    ///
    /// Parent Type: `ArtifactReference`
    nonisolated public struct AsArtifactReference: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
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
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsArtifactReference.self
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

    /// Item.AsTaskReference
    ///
    /// Parent Type: `TaskReference`
    nonisolated public struct AsTaskReference: NoemaAPI.InlineFragment {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public typealias RootEntityType = NativeConversationItemFields.Item
      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReference }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("taskId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeConversationItemFields.Item.self,
        NativeConversationItemFields.Item.AsTaskReference.self
      ] }

      /// Stable task id.
      public var taskId: String { __data["taskId"] }
    }
  }
}
