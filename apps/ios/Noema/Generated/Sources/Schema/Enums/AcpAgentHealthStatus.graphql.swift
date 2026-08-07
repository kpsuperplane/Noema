// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Readiness of a configured ACP executable.
nonisolated public enum AcpAgentHealthStatus: String, EnumType {
  case unknown = "UNKNOWN"
  case healthy = "HEALTHY"
  case unavailable = "UNAVAILABLE"
}
