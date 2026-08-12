// @generated
// This file was automatically generated and should not be edited.

import ApolloAPI

public extension Unions {
  /// Human intervention variants share presentation, but retain separate authorities.
  nonisolated static let HumanIntervention = Union(
    name: "HumanIntervention",
    possibleTypes: [
      Objects.TaskAttention.self,
      Objects.GovernedAction.self,
      Objects.McpAuthenticationIntervention.self,
      Objects.AdapterAuthenticationIntervention.self,
      Objects.McpSetupIntervention.self,
      Objects.AdapterOauthClientSetupIntervention.self,
      Objects.AdapterDefinition.self
    ]
  )
}