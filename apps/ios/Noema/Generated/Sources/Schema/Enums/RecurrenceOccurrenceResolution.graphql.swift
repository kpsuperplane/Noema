// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Immutable recurrence slot disposition.
nonisolated public enum RecurrenceOccurrenceResolution: String, EnumType {
  case materialized = "MATERIALIZED"
  case skipped = "SKIPPED"
  case coalesced = "COALESCED"
}
