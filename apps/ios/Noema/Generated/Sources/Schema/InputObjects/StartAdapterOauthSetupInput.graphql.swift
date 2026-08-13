// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Start browser OAuth against one exact filesystem connection revision.
nonisolated public struct StartAdapterOauthSetupInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    applicationId: String,
    expectedApplicationRevision: Int32,
    grantId: GraphQLNullable<String> = nil,
    expectedGrantRevision: GraphQLNullable<Int32> = nil,
    semanticDigest: String,
    operationIds: [String],
    additionalServices: GraphQLNullable<[AdapterOauthServiceSelectionInput]> = nil
  ) {
    __data = InputDict([
      "applicationId": applicationId,
      "expectedApplicationRevision": expectedApplicationRevision,
      "grantId": grantId,
      "expectedGrantRevision": expectedGrantRevision,
      "semanticDigest": semanticDigest,
      "operationIds": operationIds,
      "additionalServices": additionalServices
    ])
  }

  public var applicationId: String {
    get { __data["applicationId"] }
    set { __data["applicationId"] = newValue }
  }

  public var expectedApplicationRevision: Int32 {
    get { __data["expectedApplicationRevision"] }
    set { __data["expectedApplicationRevision"] = newValue }
  }

  public var grantId: GraphQLNullable<String> {
    get { __data["grantId"] }
    set { __data["grantId"] = newValue }
  }

  public var expectedGrantRevision: GraphQLNullable<Int32> {
    get { __data["expectedGrantRevision"] }
    set { __data["expectedGrantRevision"] = newValue }
  }

  public var semanticDigest: String {
    get { __data["semanticDigest"] }
    set { __data["semanticDigest"] = newValue }
  }

  public var operationIds: [String] {
    get { __data["operationIds"] }
    set { __data["operationIds"] = newValue }
  }

  public var additionalServices: GraphQLNullable<[AdapterOauthServiceSelectionInput]> {
    get { __data["additionalServices"] }
    set { __data["additionalServices"] = newValue }
  }
}
