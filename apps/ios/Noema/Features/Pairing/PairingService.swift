import Foundation

enum PairingServiceError: Error, LocalizedError {
  case busy
  case invalidDisplayName
  case serverRejected(Int)
  case malformedResponse

  var errorDescription: String? {
    switch self {
    case .busy: "A pairing request is already in progress."
    case .invalidDisplayName: "Enter a device name between 1 and 128 bytes."
    case .serverRejected(let status): "The Noema server rejected pairing (HTTP \(status))."
    case .malformedResponse: "The Noema server returned an invalid client credential."
    }
  }
}

actor PairingService {
  private let profileStore: KeychainProfileStore
  private var isCompleting = false

  init(profileStore: KeychainProfileStore) {
    self.profileStore = profileStore
  }

  func complete(payload: PairingPayload, displayName: String) async throws -> NoemaProfile {
    guard !isCompleting else { throw PairingServiceError.busy }
    guard let name = NoemaDisplayName.normalized(displayName) else {
      throw PairingServiceError.invalidDisplayName
    }

    isCompleting = true
    defer { isCompleting = false }

    var request = URLRequest(url: payload.origin.url.appending(path: "auth/client/pairing/complete"))
    request.httpMethod = "POST"
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.httpBody = try JSONEncoder().encode(CompleteRequest(
      pairingID: payload.pairingID,
      secret: payload.secret,
      displayName: name
    ))

    let delegate = PairingRedirectDelegate(origin: payload.origin.url)
    let session = URLSession(configuration: .ephemeral, delegate: delegate, delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    let (data, response) = try await session.data(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
      throw PairingServiceError.serverRejected((response as? HTTPURLResponse)?.statusCode ?? 0)
    }

    let result: CompleteResponse
    do {
      result = try JSONDecoder().decode(CompleteResponse.self, from: data)
    } catch {
      throw PairingServiceError.malformedResponse
    }
    guard Self.isValidClientID(result.clientID), Self.isValidBearer(result.token, clientID: result.clientID) else {
      throw PairingServiceError.malformedResponse
    }

    let profile = NoemaProfile(origin: payload.origin.url, clientId: result.clientID, token: result.token)
    // The credential is written only after the complete response passes every validation above.
    try await profileStore.replace(with: profile)
    return profile
  }

  private static func isValidClientID(_ value: String) -> Bool {
    (1...128).contains(value.utf8.count) && value.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }
  }

  private static func isValidBearer(_ value: String, clientID: String) -> Bool {
    guard value.hasPrefix(clientID + ".") else { return false }
    let encoded = String(value.dropFirst(clientID.count + 1))
    return encoded.count == 43
      && encoded.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }
      && Data(base64URLEncoded: encoded)?.count == 32
  }

  private struct CompleteRequest: Encodable {
    let pairingID: String
    let secret: String
    let displayName: String

    enum CodingKeys: String, CodingKey {
      case pairingID = "pairingId"
      case secret
      case displayName
    }
  }

  private struct CompleteResponse: Decodable {
    let clientID: String
    let token: String

    enum CodingKeys: String, CodingKey {
      case clientID = "clientId"
      case token
    }
  }
}

private final class PairingRedirectDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
  private let origin: URL

  init(origin: URL) {
    self.origin = origin
  }

  func urlSession(
    _ session: URLSession,
    task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) {
    guard let target = request.url,
          target.scheme?.lowercased() == "https",
          target.host?.lowercased() == origin.host?.lowercased(),
          (target.port ?? 443) == (origin.port ?? 443)
    else {
      completionHandler(nil)
      return
    }
    completionHandler(request)
  }
}
