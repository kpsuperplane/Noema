import AuthenticationServices
import CryptoKit
import Foundation
import Security
import UIKit

enum ConnectionServiceError: Error, LocalizedError {
  case busy
  case cancelled
  case authorizationExpired
  case invalidCallback
  case serverRejected(Int)
  case malformedResponse

  var errorDescription: String? {
    switch self {
    case .busy: "A connection request is already in progress."
    case .cancelled: "The connection request was cancelled."
    case .authorizationExpired: "This client authorization has expired. Connect this device again."
    case .invalidCallback: "Noema rejected an invalid authorization response."
    case .serverRejected(let status): "The Noema server rejected authorization (HTTP \(status))."
    case .malformedResponse: "The Noema server returned an invalid OAuth response."
    }
  }
}

@MainActor
final class ConnectionService: NSObject, ASWebAuthenticationPresentationContextProviding {
  private let profileStore: KeychainProfileStore
  private var isWorking = false
  private var browserSession: ASWebAuthenticationSession?

  init(profileStore: KeychainProfileStore) {
    self.profileStore = profileStore
  }

  func authorize(payload: ConnectionPayload) async throws -> NoemaProfile {
    guard !isWorking else { throw ConnectionServiceError.busy }
    isWorking = true
    defer { isWorking = false }

    let clientID = "noema-ios:\(try Self.randomBase64URL(bytes: 18))"
    let verifier = try Self.randomBase64URL(bytes: 32)
    let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncoded
    let state = try Self.randomBase64URL(bytes: 32)
    var components = URLComponents(
      url: payload.origin.url.appending(path: "oauth/authorize"),
      resolvingAgainstBaseURL: false
    )
    components?.queryItems = [
      URLQueryItem(name: "client_id", value: clientID),
      URLQueryItem(name: "redirect_uri", value: "noema://oauth/callback"),
      URLQueryItem(name: "response_type", value: "code"),
      URLQueryItem(name: "state", value: state),
      URLQueryItem(name: "code_challenge", value: challenge),
      URLQueryItem(name: "code_challenge_method", value: "S256")
    ]
    guard let authorizationURL = components?.url else {
      throw ConnectionServiceError.malformedResponse
    }
    let callback = try await browserCallback(for: authorizationURL)
    guard callback.scheme?.lowercased() == "noema",
          callback.host?.lowercased() == "oauth",
          callback.path == "/callback",
          let callbackComponents = URLComponents(url: callback, resolvingAgainstBaseURL: false),
          callbackComponents.queryItems?.filter({ $0.name == "state" }).single?.value == state,
          let code = callbackComponents.queryItems?.filter({ $0.name == "code" }).single?.value,
          (1...128).contains(code.utf8.count)
    else { throw ConnectionServiceError.invalidCallback }

    return try await exchange(
      origin: payload.origin.url,
      clientID: clientID,
      fields: [
        "grant_type": "authorization_code",
        "client_id": clientID,
        "redirect_uri": "noema://oauth/callback",
        "code": code,
        "code_verifier": verifier
      ]
    )
  }

  func refresh(_ stored: StoredNoemaProfile) async throws -> NoemaProfile {
    try await exchange(
      origin: stored.origin,
      clientID: stored.clientId,
      fields: [
        "grant_type": "refresh_token",
        "refresh_token": stored.refreshToken
      ]
    )
  }

  func refresh(_ profile: NoemaProfile) async throws -> NoemaProfile {
    try await refresh(StoredNoemaProfile(
      origin: profile.origin,
      clientId: profile.clientId,
      refreshToken: profile.refreshToken
    ))
  }

  func revoke(_ profile: NoemaProfile) async throws {
    var request = URLRequest(url: profile.origin.appending(path: "oauth/revoke"))
    request.httpMethod = "POST"
    request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
    request.httpBody = Self.formBody(["token": profile.refreshToken])
    let (_, response) = try await URLSession.shared.data(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
      throw ConnectionServiceError.serverRejected((response as? HTTPURLResponse)?.statusCode ?? 0)
    }
  }

  func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
    let windowScenes = UIApplication.shared.connectedScenes
      .compactMap { $0 as? UIWindowScene }
    if let keyWindow = windowScenes.flatMap(\.windows).first(where: \.isKeyWindow) {
      return keyWindow
    }
    guard let windowScene = windowScenes.first else {
      preconditionFailure("No window scene is available for authorization.")
    }
    return ASPresentationAnchor(windowScene: windowScene)
  }

  private func browserCallback(for url: URL) async throws -> URL {
    try await withCheckedThrowingContinuation { continuation in
      let session = ASWebAuthenticationSession(url: url, callbackURLScheme: "noema") {
        [weak self] callback, error in
        self?.browserSession = nil
        if let callback {
          continuation.resume(returning: callback)
        } else if (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin {
          continuation.resume(throwing: ConnectionServiceError.cancelled)
        } else {
          continuation.resume(throwing: error ?? ConnectionServiceError.invalidCallback)
        }
      }
      session.presentationContextProvider = self
      session.prefersEphemeralWebBrowserSession = false
      browserSession = session
      guard session.start() else {
        browserSession = nil
        continuation.resume(throwing: ConnectionServiceError.invalidCallback)
        return
      }
    }
  }

  private func exchange(
    origin: URL,
    clientID: String,
    fields: [String: String]
  ) async throws -> NoemaProfile {
    var request = URLRequest(url: origin.appending(path: "oauth/token"))
    request.httpMethod = "POST"
    request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
    request.httpBody = Self.formBody(fields)
    let configuration = URLSessionConfiguration.ephemeral
    configuration.timeoutIntervalForRequest = 30
    configuration.timeoutIntervalForResource = 30
    let session = URLSession(configuration: configuration)
    defer { session.invalidateAndCancel() }
    let (data, response) = try await session.data(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
      if let error = try? JSONDecoder().decode(TokenErrorResponse.self, from: data),
         error.error == .invalidGrant {
        throw ConnectionServiceError.authorizationExpired
      }
      throw ConnectionServiceError.serverRejected((response as? HTTPURLResponse)?.statusCode ?? 0)
    }
    guard let token = try? JSONDecoder().decode(TokenResponse.self, from: data),
          (1...128).contains(token.accessToken.utf8.count),
          (1...128).contains(token.refreshToken.utf8.count),
          (1...86_400).contains(token.expiresIn)
    else { throw ConnectionServiceError.malformedResponse }
    let profile = NoemaProfile(
      origin: origin,
      clientId: clientID,
      refreshToken: token.refreshToken,
      accessToken: token.accessToken,
      accessExpiresAt: Date().addingTimeInterval(TimeInterval(token.expiresIn))
    )
    try await profileStore.replace(with: profile)
    return profile
  }

  private static func formBody(_ fields: [String: String]) -> Data? {
    var components = URLComponents()
    components.queryItems = fields.sorted(by: { $0.key < $1.key }).map {
      URLQueryItem(name: $0.key, value: $0.value)
    }
    return components.percentEncodedQuery?.data(using: .utf8)
  }

  private static func randomBase64URL(bytes: Int) throws -> String {
    var data = Data(count: bytes)
    let status = data.withUnsafeMutableBytes { buffer in
      SecRandomCopyBytes(kSecRandomDefault, bytes, buffer.baseAddress!)
    }
    guard status == errSecSuccess else { throw ConnectionServiceError.malformedResponse }
    return data.base64URLEncoded
  }

  private struct TokenResponse: Decodable {
    let accessToken: String
    let refreshToken: String
    let expiresIn: Int

    enum CodingKeys: String, CodingKey {
      case accessToken = "access_token"
      case refreshToken = "refresh_token"
      case expiresIn = "expires_in"
    }
  }

  private struct TokenErrorResponse: Decodable {
    let error: TokenError
  }

  private enum TokenError: String, Decodable {
    case invalidGrant = "invalid_grant"
  }
}

private extension Array {
  var single: Element? { count == 1 ? first : nil }
}
