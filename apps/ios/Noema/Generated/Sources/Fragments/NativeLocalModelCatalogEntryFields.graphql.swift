// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeLocalModelCatalogEntryFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment NativeLocalModelCatalogEntryFields on LocalModelCatalogEntry { __typename modelId name license priority repo revision isRecommended compatibleBackend selectedBuild { __typename file sha256 downloadGb backends minRamGb minVramGb } hardwareFit { __typename backend ramGb vramGb unifiedMemory explanation } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelCatalogEntry }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("modelId", String.self),
    .field("name", String.self),
    .field("license", String.self),
    .field("priority", Int.self),
    .field("repo", String.self),
    .field("revision", String.self),
    .field("isRecommended", Bool.self),
    .field("compatibleBackend", GraphQLEnum<NoemaAPI.LocalModelBackend>?.self),
    .field("selectedBuild", SelectedBuild?.self),
    .field("hardwareFit", HardwareFit?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    NativeLocalModelCatalogEntryFields.self
  ] }

  /// Stable curated model identifier.
  public var modelId: String { __data["modelId"] }
  /// Product-facing model name.
  public var name: String { __data["name"] }
  /// License shown before download.
  public var license: String { __data["license"] }
  /// Recommendation priority from the bundled catalog.
  public var priority: Int { __data["priority"] }
  /// Pinned Hugging Face repository.
  public var repo: String { __data["repo"] }
  /// Immutable source revision.
  public var revision: String { __data["revision"] }
  /// Whether this is the top recommendation for the current machine.
  public var isRecommended: Bool { __data["isRecommended"] }
  /// Backend used by the selected artifact.
  public var compatibleBackend: GraphQLEnum<NoemaAPI.LocalModelBackend>? { __data["compatibleBackend"] }
  /// Best compatible artifact for this machine, when one fits.
  public var selectedBuild: SelectedBuild? { __data["selectedBuild"] }
  /// Detected hardware values that satisfy the artifact thresholds.
  public var hardwareFit: HardwareFit? { __data["hardwareFit"] }

  /// SelectedBuild
  ///
  /// Parent Type: `LocalModelBuild`
  nonisolated public struct SelectedBuild: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelBuild }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("file", String.self),
      .field("sha256", String.self),
      .field("downloadGb", Double.self),
      .field("backends", [GraphQLEnum<NoemaAPI.LocalModelBackend>].self),
      .field("minRamGb", Int.self),
      .field("minVramGb", Int?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeLocalModelCatalogEntryFields.SelectedBuild.self
    ] }

    /// Artifact filename in the pinned source repository.
    public var file: String { __data["file"] }
    /// Verified SHA-256 digest.
    public var sha256: String { __data["sha256"] }
    /// Rounded UI-facing download size in decimal gigabytes.
    public var downloadGb: Double { __data["downloadGb"] }
    /// Backends compatible with this artifact.
    public var backends: [GraphQLEnum<NoemaAPI.LocalModelBackend>] { __data["backends"] }
    /// Minimum system or unified memory in whole gigabytes.
    public var minRamGb: Int { __data["minRamGb"] }
    /// Minimum discrete or unified accelerator memory in whole gigabytes.
    public var minVramGb: Int? { __data["minVramGb"] }
  }

  /// HardwareFit
  ///
  /// Parent Type: `LocalModelHardwareFit`
  nonisolated public struct HardwareFit: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalModelHardwareFit }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("backend", GraphQLEnum<NoemaAPI.LocalModelBackend>.self),
      .field("ramGb", Int.self),
      .field("vramGb", Int?.self),
      .field("unifiedMemory", Bool.self),
      .field("explanation", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeLocalModelCatalogEntryFields.HardwareFit.self
    ] }

    /// Backend selected for the build.
    public var backend: GraphQLEnum<NoemaAPI.LocalModelBackend> { __data["backend"] }
    /// Detected system or unified memory in whole gigabytes.
    public var ramGb: Int { __data["ramGb"] }
    /// Detected discrete accelerator memory in whole gigabytes.
    public var vramGb: Int? { __data["vramGb"] }
    /// Whether the accelerator shares system memory.
    public var unifiedMemory: Bool { __data["unifiedMemory"] }
    /// Product-facing explanation generated from the match.
    public var explanation: String { __data["explanation"] }
  }
}
