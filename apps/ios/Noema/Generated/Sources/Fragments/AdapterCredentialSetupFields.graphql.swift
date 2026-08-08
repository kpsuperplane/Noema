// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct AdapterCredentialSetupFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment AdapterCredentialSetupFields on AdapterCredentialSetup { __typename credentialType setupUrl instructions inputKind fields { __typename fieldId label } documentMediaType redirectUri normalizationTransform { __typename language sourceDigest source } requestAuthTransform { __typename language sourceDigest source } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterCredentialSetup }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("credentialType", String.self),
    .field("setupUrl", String.self),
    .field("instructions", [String].self),
    .field("inputKind", String.self),
    .field("fields", [Field].self),
    .field("documentMediaType", String?.self),
    .field("redirectUri", String?.self),
    .field("normalizationTransform", NormalizationTransform?.self),
    .field("requestAuthTransform", RequestAuthTransform?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
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

  /// Field
  ///
  /// Parent Type: `AdapterCredentialField`
  nonisolated public struct Field: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterCredentialField }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("fieldId", String.self),
      .field("label", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      AdapterCredentialSetupFields.Field.self
    ] }

    public var fieldId: String { __data["fieldId"] }
    public var label: String { __data["label"] }
  }

  /// NormalizationTransform
  ///
  /// Parent Type: `AdapterCredentialTransform`
  nonisolated public struct NormalizationTransform: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterCredentialTransform }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("language", String.self),
      .field("sourceDigest", String.self),
      .field("source", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      AdapterCredentialSetupFields.NormalizationTransform.self
    ] }

    public var language: String { __data["language"] }
    public var sourceDigest: String { __data["sourceDigest"] }
    public var source: String { __data["source"] }
  }

  /// RequestAuthTransform
  ///
  /// Parent Type: `AdapterCredentialTransform`
  nonisolated public struct RequestAuthTransform: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterCredentialTransform }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("language", String.self),
      .field("sourceDigest", String.self),
      .field("source", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      AdapterCredentialSetupFields.RequestAuthTransform.self
    ] }

    public var language: String { __data["language"] }
    public var sourceDigest: String { __data["sourceDigest"] }
    public var source: String { __data["source"] }
  }
}
