// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Transient credential input for one exact reviewed definition.
nonisolated public struct SetupAdapterConnectionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    semanticDigest: String,
    fieldValues: [AdapterCredentialFieldValueInput]? = nil,
    documentBase64: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "semanticDigest": semanticDigest,
      "fieldValues": fieldValues ?? GraphQLNullable.none,
      "documentBase64": documentBase64
    ])
  }

  public var semanticDigest: String {
    get { __data["semanticDigest"] }
    set { __data["semanticDigest"] = newValue }
  }

  public var fieldValues: [AdapterCredentialFieldValueInput]? {
    get { __data["fieldValues"] }
    set { __data["fieldValues"] = newValue }
  }

  public var documentBase64: GraphQLNullable<String> {
    get { __data["documentBase64"] }
    set { __data["documentBase64"] = newValue }
  }
}
