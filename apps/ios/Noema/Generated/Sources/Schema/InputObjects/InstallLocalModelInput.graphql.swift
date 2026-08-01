// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Install one curated model/build.
nonisolated public struct InstallLocalModelInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    modelId: String,
    file: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "modelId": modelId,
      "file": file
    ])
  }

  /// Stable model id from the bundled catalog.
  public var modelId: String {
    get { __data["modelId"] }
    set { __data["modelId"] = newValue }
  }

  /// Optional artifact filename; the machine-selected build is used when omitted.
  public var file: GraphQLNullable<String> {
    get { __data["file"] }
    set { __data["file"] = newValue }
  }
}
