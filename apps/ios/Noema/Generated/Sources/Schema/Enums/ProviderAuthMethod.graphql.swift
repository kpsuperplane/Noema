// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Provider auth method exposed through GraphQL.
nonisolated public enum ProviderAuthMethod: String, EnumType {
  /// OAuth device-code flow.
  case oauthDeviceCode = "OAUTH_DEVICE_CODE"
  /// OAuth authorization-code flow with PKCE.
  case oauthPkce = "OAUTH_PKCE"
  /// Secret input flow.
  case secretInput = "SECRET_INPUT"
  /// External manual flow.
  case externalManual = "EXTERNAL_MANUAL"
  /// No auth required.
  case none = "NONE"
}
