// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Multiple-choice selection mode exposed through GraphQL.
nonisolated public enum MultipleChoiceSelectionMode: String, EnumType {
  /// One option may be selected.
  case pickOne = "PICK_ONE"
  /// One or more options may be selected.
  case pickMany = "PICK_MANY"
}
