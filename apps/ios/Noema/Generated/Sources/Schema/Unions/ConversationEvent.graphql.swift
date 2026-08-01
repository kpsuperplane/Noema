// @generated
// This file was automatically generated and should not be edited.

import ApolloAPI

public extension Unions {
  /// Conversation subscription event union.
  nonisolated static let ConversationEvent = Union(
    name: "ConversationEvent",
    possibleTypes: [
      Objects.SubscriptionReadyEvent.self,
      Objects.HumanInterventionsChangedEvent.self,
      Objects.ConversationItemEvent.self,
      Objects.AssistantTextDeltaEvent.self,
      Objects.AgentStatusEvent.self,
      Objects.TurnCompletedEvent.self
    ]
  )
}