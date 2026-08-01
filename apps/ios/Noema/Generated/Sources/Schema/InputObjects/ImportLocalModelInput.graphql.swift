// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Import a public GGUF or a GGUF already on this machine.
nonisolated public struct ImportLocalModelInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    name: String,
    sourceKind: GraphQLEnum<LocalModelSourceKind>,
    localPath: GraphQLNullable<String> = nil,
    repo: GraphQLNullable<String> = nil,
    revision: GraphQLNullable<String> = nil,
    file: GraphQLNullable<String> = nil,
    sha256: GraphQLNullable<String> = nil,
    license: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "name": name,
      "sourceKind": sourceKind,
      "localPath": localPath,
      "repo": repo,
      "revision": revision,
      "file": file,
      "sha256": sha256,
      "license": license
    ])
  }

  /// Product-facing name for the imported model.
  public var name: String {
    get { __data["name"] }
    set { __data["name"] = newValue }
  }

  /// Import provenance.
  public var sourceKind: GraphQLEnum<LocalModelSourceKind> {
    get { __data["sourceKind"] }
    set { __data["sourceKind"] = newValue }
  }

  /// Local file path for a local-file import.
  public var localPath: GraphQLNullable<String> {
    get { __data["localPath"] }
    set { __data["localPath"] = newValue }
  }

  /// Public Hugging Face repository for a public-GGUF import.
  public var repo: GraphQLNullable<String> {
    get { __data["repo"] }
    set { __data["repo"] = newValue }
  }

  /// Immutable Hugging Face revision for a public-GGUF import.
  public var revision: GraphQLNullable<String> {
    get { __data["revision"] }
    set { __data["revision"] = newValue }
  }

  /// GGUF filename in the public repository.
  public var file: GraphQLNullable<String> {
    get { __data["file"] }
    set { __data["file"] = newValue }
  }

  /// Required verified SHA-256 digest for a public-GGUF import.
  public var sha256: GraphQLNullable<String> {
    get { __data["sha256"] }
    set { __data["sha256"] = newValue }
  }

  /// License label shown in Settings when supplied by the user.
  public var license: GraphQLNullable<String> {
    get { __data["license"] }
    set { __data["license"] = newValue }
  }
}
