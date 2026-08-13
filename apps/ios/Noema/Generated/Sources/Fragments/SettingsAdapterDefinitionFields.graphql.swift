// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAdapterDefinitionFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment SettingsAdapterDefinitionFields on AdapterDefinition { __typename semanticDigest definitionId adapterId displayName definitionRevision sourceReference origin authenticationMode oauthProfileDigest scopes credentialSetup { __typename ...AdapterCredentialSetupFields } accountIdentityOperationId manifestJson operations { __typename operationId method path readOnly idempotent destructive openWorld argumentNames responseTransform { __typename language sourceDigest source acceptedContentTypes outputSchemaJson } } transition { __typename addedOperations changedOperations removedOperations authenticationChanged affectedConnections affectedSchedules authenticationRequiredConnections consolidatedConnections } connectionCount connections { __typename connectionId status grantId accountId connectionRevision credentialRevision grantRevision policyRevision grantedScopes allowedOperations policyConfigured operationAccess { __typename operationId status missingScopes } } reviewed superseded nextAction { __typename kind semanticDigest applicationId expectedApplicationRevision grantId expectedGrantRevision connectionId expectedConnectionRevision expectedPolicyRevision operationIds missingScopes } connectionActions { __typename kind semanticDigest applicationId expectedApplicationRevision grantId expectedGrantRevision connectionId expectedConnectionRevision expectedPolicyRevision operationIds missingScopes } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("semanticDigest", String.self),
    .field("definitionId", String.self),
    .field("adapterId", String.self),
    .field("displayName", String.self),
    .field("definitionRevision", String.self),
    .field("sourceReference", String.self),
    .field("origin", String.self),
    .field("authenticationMode", String.self),
    .field("oauthProfileDigest", String?.self),
    .field("scopes", [String].self),
    .field("credentialSetup", CredentialSetup?.self),
    .field("accountIdentityOperationId", String?.self),
    .field("manifestJson", String.self),
    .field("operations", [Operation].self),
    .field("transition", Transition.self),
    .field("connectionCount", Int.self),
    .field("connections", [Connection].self),
    .field("reviewed", Bool.self),
    .field("superseded", Bool.self),
    .field("nextAction", NextAction?.self),
    .field("connectionActions", [ConnectionAction].self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    SettingsAdapterDefinitionFields.self
  ] }

  public var semanticDigest: String { __data["semanticDigest"] }
  public var definitionId: String { __data["definitionId"] }
  public var adapterId: String { __data["adapterId"] }
  public var displayName: String { __data["displayName"] }
  public var definitionRevision: String { __data["definitionRevision"] }
  public var sourceReference: String { __data["sourceReference"] }
  public var origin: String { __data["origin"] }
  public var authenticationMode: String { __data["authenticationMode"] }
  public var oauthProfileDigest: String? { __data["oauthProfileDigest"] }
  public var scopes: [String] { __data["scopes"] }
  public var credentialSetup: CredentialSetup? { __data["credentialSetup"] }
  public var accountIdentityOperationId: String? { __data["accountIdentityOperationId"] }
  public var manifestJson: String { __data["manifestJson"] }
  public var operations: [Operation] { __data["operations"] }
  public var transition: Transition { __data["transition"] }
  public var connectionCount: Int { __data["connectionCount"] }
  public var connections: [Connection] { __data["connections"] }
  public var reviewed: Bool { __data["reviewed"] }
  public var superseded: Bool { __data["superseded"] }
  public var nextAction: NextAction? { __data["nextAction"] }
  public var connectionActions: [ConnectionAction] { __data["connectionActions"] }

  /// CredentialSetup
  ///
  /// Parent Type: `AdapterCredentialSetup`
  nonisolated public struct CredentialSetup: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterCredentialSetup }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(AdapterCredentialSetupFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionFields.CredentialSetup.self,
      AdapterCredentialSetupFields.self
    ] }

    public var credentialType: String { __data["credentialType"] }
    public var setupUrl: String { __data["setupUrl"] }
    public var instructions: [String] { __data["instructions"] }
    public var inputKind: String { __data["inputKind"] }
    public var fields: [Field] { __data["fields"] }
    public var documentMediaType: String? { __data["documentMediaType"] }
    public var redirectUri: String? { __data["redirectUri"] }
    public var normalizationTransform: NormalizationTransform? { __data["normalizationTransform"] }
    public var requestAuthTransform: RequestAuthTransform? { __data["requestAuthTransform"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var adapterCredentialSetupFields: AdapterCredentialSetupFields { _toFragment() }
    }

    public typealias Field = AdapterCredentialSetupFields.Field

    public typealias NormalizationTransform = AdapterCredentialSetupFields.NormalizationTransform

    public typealias RequestAuthTransform = AdapterCredentialSetupFields.RequestAuthTransform
  }

  /// Operation
  ///
  /// Parent Type: `AdapterOperation`
  nonisolated public struct Operation: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOperation }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("operationId", String.self),
      .field("method", String.self),
      .field("path", String.self),
      .field("readOnly", Bool?.self),
      .field("idempotent", Bool?.self),
      .field("destructive", Bool?.self),
      .field("openWorld", Bool?.self),
      .field("argumentNames", [String].self),
      .field("responseTransform", ResponseTransform?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionFields.Operation.self
    ] }

    public var operationId: String { __data["operationId"] }
    public var method: String { __data["method"] }
    public var path: String { __data["path"] }
    public var readOnly: Bool? { __data["readOnly"] }
    public var idempotent: Bool? { __data["idempotent"] }
    public var destructive: Bool? { __data["destructive"] }
    public var openWorld: Bool? { __data["openWorld"] }
    public var argumentNames: [String] { __data["argumentNames"] }
    public var responseTransform: ResponseTransform? { __data["responseTransform"] }

    /// Operation.ResponseTransform
    ///
    /// Parent Type: `AdapterResponseTransform`
    nonisolated public struct ResponseTransform: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterResponseTransform }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("language", String.self),
        .field("sourceDigest", String.self),
        .field("source", String.self),
        .field("acceptedContentTypes", [String].self),
        .field("outputSchemaJson", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAdapterDefinitionFields.Operation.ResponseTransform.self
      ] }

      public var language: String { __data["language"] }
      public var sourceDigest: String { __data["sourceDigest"] }
      public var source: String { __data["source"] }
      public var acceptedContentTypes: [String] { __data["acceptedContentTypes"] }
      public var outputSchemaJson: String { __data["outputSchemaJson"] }
    }
  }

  /// Transition
  ///
  /// Parent Type: `AdapterDefinitionTransition`
  nonisolated public struct Transition: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinitionTransition }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("addedOperations", [String].self),
      .field("changedOperations", [String].self),
      .field("removedOperations", [String].self),
      .field("authenticationChanged", Bool.self),
      .field("affectedConnections", Int.self),
      .field("affectedSchedules", Int.self),
      .field("authenticationRequiredConnections", Int.self),
      .field("consolidatedConnections", Int.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionFields.Transition.self
    ] }

    public var addedOperations: [String] { __data["addedOperations"] }
    public var changedOperations: [String] { __data["changedOperations"] }
    public var removedOperations: [String] { __data["removedOperations"] }
    public var authenticationChanged: Bool { __data["authenticationChanged"] }
    public var affectedConnections: Int { __data["affectedConnections"] }
    public var affectedSchedules: Int { __data["affectedSchedules"] }
    public var authenticationRequiredConnections: Int { __data["authenticationRequiredConnections"] }
    public var consolidatedConnections: Int { __data["consolidatedConnections"] }
  }

  /// Connection
  ///
  /// Parent Type: `AdapterConnection`
  nonisolated public struct Connection: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterConnection }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("connectionId", String.self),
      .field("status", String.self),
      .field("grantId", String?.self),
      .field("accountId", String?.self),
      .field("connectionRevision", Int.self),
      .field("credentialRevision", Int?.self),
      .field("grantRevision", Int?.self),
      .field("policyRevision", Int.self),
      .field("grantedScopes", [String].self),
      .field("allowedOperations", [String].self),
      .field("policyConfigured", Bool.self),
      .field("operationAccess", [OperationAccess].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionFields.Connection.self
    ] }

    public var connectionId: String { __data["connectionId"] }
    public var status: String { __data["status"] }
    public var grantId: String? { __data["grantId"] }
    public var accountId: String? { __data["accountId"] }
    public var connectionRevision: Int { __data["connectionRevision"] }
    public var credentialRevision: Int? { __data["credentialRevision"] }
    public var grantRevision: Int? { __data["grantRevision"] }
    public var policyRevision: Int { __data["policyRevision"] }
    public var grantedScopes: [String] { __data["grantedScopes"] }
    public var allowedOperations: [String] { __data["allowedOperations"] }
    public var policyConfigured: Bool { __data["policyConfigured"] }
    public var operationAccess: [OperationAccess] { __data["operationAccess"] }

    /// Connection.OperationAccess
    ///
    /// Parent Type: `AdapterOperationAccess`
    nonisolated public struct OperationAccess: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOperationAccess }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("operationId", String.self),
        .field("status", String.self),
        .field("missingScopes", [String].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAdapterDefinitionFields.Connection.OperationAccess.self
      ] }

      public var operationId: String { __data["operationId"] }
      public var status: String { __data["status"] }
      public var missingScopes: [String] { __data["missingScopes"] }
    }
  }

  /// NextAction
  ///
  /// Parent Type: `AdapterNextAction`
  nonisolated public struct NextAction: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterNextAction }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("kind", String.self),
      .field("semanticDigest", String.self),
      .field("applicationId", String?.self),
      .field("expectedApplicationRevision", Int?.self),
      .field("grantId", String?.self),
      .field("expectedGrantRevision", Int?.self),
      .field("connectionId", String?.self),
      .field("expectedConnectionRevision", Int?.self),
      .field("expectedPolicyRevision", Int?.self),
      .field("operationIds", [String].self),
      .field("missingScopes", [String].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionFields.NextAction.self
    ] }

    public var kind: String { __data["kind"] }
    public var semanticDigest: String { __data["semanticDigest"] }
    public var applicationId: String? { __data["applicationId"] }
    public var expectedApplicationRevision: Int? { __data["expectedApplicationRevision"] }
    public var grantId: String? { __data["grantId"] }
    public var expectedGrantRevision: Int? { __data["expectedGrantRevision"] }
    public var connectionId: String? { __data["connectionId"] }
    public var expectedConnectionRevision: Int? { __data["expectedConnectionRevision"] }
    public var expectedPolicyRevision: Int? { __data["expectedPolicyRevision"] }
    public var operationIds: [String] { __data["operationIds"] }
    public var missingScopes: [String] { __data["missingScopes"] }
  }

  /// ConnectionAction
  ///
  /// Parent Type: `AdapterNextAction`
  nonisolated public struct ConnectionAction: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterNextAction }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("kind", String.self),
      .field("semanticDigest", String.self),
      .field("applicationId", String?.self),
      .field("expectedApplicationRevision", Int?.self),
      .field("grantId", String?.self),
      .field("expectedGrantRevision", Int?.self),
      .field("connectionId", String?.self),
      .field("expectedConnectionRevision", Int?.self),
      .field("expectedPolicyRevision", Int?.self),
      .field("operationIds", [String].self),
      .field("missingScopes", [String].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionFields.ConnectionAction.self
    ] }

    public var kind: String { __data["kind"] }
    public var semanticDigest: String { __data["semanticDigest"] }
    public var applicationId: String? { __data["applicationId"] }
    public var expectedApplicationRevision: Int? { __data["expectedApplicationRevision"] }
    public var grantId: String? { __data["grantId"] }
    public var expectedGrantRevision: Int? { __data["expectedGrantRevision"] }
    public var connectionId: String? { __data["connectionId"] }
    public var expectedConnectionRevision: Int? { __data["expectedConnectionRevision"] }
    public var expectedPolicyRevision: Int? { __data["expectedPolicyRevision"] }
    public var operationIds: [String] { __data["operationIds"] }
    public var missingScopes: [String] { __data["missingScopes"] }
  }
}
