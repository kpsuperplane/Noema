// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// One replacement validation criterion.
nonisolated public struct TaskValidationCriterionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    criterionId: GraphQLNullable<String> = nil,
    ordinal: Int32,
    description: String,
    expectedEvidence: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "criterionId": criterionId,
      "ordinal": ordinal,
      "description": description,
      "expectedEvidence": expectedEvidence
    ])
  }

  /// Optional stable criterion identity.
  public var criterionId: GraphQLNullable<String> {
    get { __data["criterionId"] }
    set { __data["criterionId"] = newValue }
  }

  /// One-based ordinal.
  public var ordinal: Int32 {
    get { __data["ordinal"] }
    set { __data["ordinal"] = newValue }
  }

  /// Criterion description.
  public var description: String {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Optional evidence guidance.
  public var expectedEvidence: GraphQLNullable<String> {
    get { __data["expectedEvidence"] }
    set { __data["expectedEvidence"] = newValue }
  }
}
