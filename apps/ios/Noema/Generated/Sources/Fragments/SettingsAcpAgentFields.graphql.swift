// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAcpAgentFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment SettingsAcpAgentFields on AcpAgent { __typename agentId displayName command arguments enabled authStatus healthStatus implementationName implementationVersion capabilities connectionRevision lastError }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AcpAgent }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("agentId", String.self),
    .field("displayName", String.self),
    .field("command", String.self),
    .field("arguments", [String].self),
    .field("enabled", Bool.self),
    .field("authStatus", GraphQLEnum<NoemaAPI.AcpAgentAuthStatus>.self),
    .field("healthStatus", GraphQLEnum<NoemaAPI.AcpAgentHealthStatus>.self),
    .field("implementationName", String?.self),
    .field("implementationVersion", String?.self),
    .field("capabilities", NoemaAPI.JSON.self),
    .field("connectionRevision", Int.self),
    .field("lastError", String?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    SettingsAcpAgentFields.self
  ] }

  public var agentId: String { __data["agentId"] }
  public var displayName: String { __data["displayName"] }
  public var command: String { __data["command"] }
  public var arguments: [String] { __data["arguments"] }
  public var enabled: Bool { __data["enabled"] }
  public var authStatus: GraphQLEnum<NoemaAPI.AcpAgentAuthStatus> { __data["authStatus"] }
  public var healthStatus: GraphQLEnum<NoemaAPI.AcpAgentHealthStatus> { __data["healthStatus"] }
  public var implementationName: String? { __data["implementationName"] }
  public var implementationVersion: String? { __data["implementationVersion"] }
  public var capabilities: NoemaAPI.JSON { __data["capabilities"] }
  public var connectionRevision: Int { __data["connectionRevision"] }
  public var lastError: String? { __data["lastError"] }
}
