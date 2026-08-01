// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for submitting one durable A2UI action.
nonisolated public struct ProviderInteractionActionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    conversationId: String,
    interactionId: String,
    expectedRevision: Int32,
    surfaceId: String,
    sourceComponentId: String,
    actionName: String,
    context: GraphQLNullable<JSON> = nil,
    dataModel: GraphQLNullable<JSON> = nil,
    clientMessageId: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "conversationId": conversationId,
      "interactionId": interactionId,
      "expectedRevision": expectedRevision,
      "surfaceId": surfaceId,
      "sourceComponentId": sourceComponentId,
      "actionName": actionName,
      "context": context,
      "dataModel": dataModel,
      "clientMessageId": clientMessageId
    ])
  }

  /// Durable Noema conversation id.
  public var conversationId: String {
    get { __data["conversationId"] }
    set { __data["conversationId"] = newValue }
  }

  /// Durable A2UI interaction id.
  public var interactionId: String {
    get { __data["interactionId"] }
    set { __data["interactionId"] = newValue }
  }

  /// Pending interaction revision being answered.
  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  /// Provider-authored surface id.
  public var surfaceId: String {
    get { __data["surfaceId"] }
    set { __data["surfaceId"] = newValue }
  }

  /// Source component that exposes the action.
  public var sourceComponentId: String {
    get { __data["sourceComponentId"] }
    set { __data["sourceComponentId"] = newValue }
  }

  /// Exact action name advertised by the source component.
  public var actionName: String {
    get { __data["actionName"] }
    set { __data["actionName"] = newValue }
  }

  /// Optional resolved event context.
  public var context: GraphQLNullable<JSON> {
    get { __data["context"] }
    set { __data["context"] = newValue }
  }

  /// Synchronized data model only when the surface requests it.
  public var dataModel: GraphQLNullable<JSON> {
    get { __data["dataModel"] }
    set { __data["dataModel"] = newValue }
  }

  /// Frontend-generated id for optimistic UI correlation.
  public var clientMessageId: GraphQLNullable<String> {
    get { __data["clientMessageId"] }
    set { __data["clientMessageId"] = newValue }
  }
}
