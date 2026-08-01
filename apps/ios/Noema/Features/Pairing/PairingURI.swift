import Foundation

struct TrustedOrigin: Codable, Hashable, Sendable {
  let url: URL
  let displayValue: String

  init(url: URL) throws {
    guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
          components.scheme?.lowercased() == "https",
          let rawHost = components.host,
          !rawHost.isEmpty,
          components.user == nil,
          components.password == nil,
          components.query == nil,
          components.fragment == nil,
          components.path.isEmpty || components.path == "/",
          !Self.isLocalHost(rawHost)
    else { throw PairingURIError.untrustedOrigin }

    let host = rawHost.lowercased()
    var value = "https://\(host)"
    if let port = components.port, port != 443 {
      guard (1...65535).contains(port) else { throw PairingURIError.untrustedOrigin }
      value += ":\(port)"
    }
    guard let canonicalURL = URL(string: value) else { throw PairingURIError.untrustedOrigin }
    self.url = canonicalURL
    self.displayValue = value
  }

  private static func isLocalHost(_ host: String) -> Bool {
    let normalized = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
    return normalized == "localhost"
      || normalized.hasSuffix(".localhost")
      || normalized == "127.0.0.1"
      || normalized == "0.0.0.0"
      || normalized == "::1"
      || normalized == "::ffff:127.0.0.1"
  }
}

struct PairingPayload: Hashable, Sendable {
  let origin: TrustedOrigin
  let pairingID: String
  let secret: String

  init(string: String) throws {
    guard let url = URL(string: string.trimmingCharacters(in: .whitespacesAndNewlines)) else {
      throw PairingURIError.invalidURI
    }
    try self.init(url: url)
  }

  init(url: URL) throws {
    guard url.scheme?.lowercased() == "noema",
          url.host?.lowercased() == "pair",
          url.user == nil,
          url.password == nil,
          url.fragment == nil,
          url.path.isEmpty || url.path == "/",
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
    else { throw PairingURIError.invalidURI }

    let values = Dictionary(grouping: components.queryItems ?? [], by: \.name)
    guard values.keys.count == 3,
          let originValue = values["origin"]?.single?.value,
          let pairingID = values["pairingId"]?.single?.value,
          let secret = values["secret"]?.single?.value
    else { throw PairingURIError.missingField }

    guard (1...128).contains(pairingID.utf8.count), Self.isBase64URL(pairingID) else {
      throw PairingURIError.invalidPairingID
    }
    guard Self.isBase64URL(secret),
          Data(base64URLEncoded: secret)?.count == 32,
          secret.count == 43
    else { throw PairingURIError.invalidSecret }

    self.origin = try TrustedOrigin(url: try Self.originURL(from: originValue))
    self.pairingID = pairingID
    self.secret = secret
  }

  private static func originURL(from value: String) throws -> URL {
    guard let url = URL(string: value),
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
          components.query == nil,
          components.fragment == nil
    else { throw PairingURIError.untrustedOrigin }
    return url
  }

  private static func isBase64URL(_ value: String) -> Bool {
    !value.isEmpty && value.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }
  }
}

enum PairingURIError: Error, LocalizedError {
  case invalidURI
  case missingField
  case invalidPairingID
  case invalidSecret
  case untrustedOrigin

  var errorDescription: String? {
    switch self {
    case .invalidURI: "Paste a Noema pairing link."
    case .missingField: "The pairing link is missing a required field."
    case .invalidPairingID: "The pairing link has an invalid pairing id."
    case .invalidSecret: "The pairing link has an invalid pairing secret."
    case .untrustedOrigin: "Pairing requires a canonical HTTPS origin that is not localhost."
    }
  }
}

private extension Array where Element == URLQueryItem {
  var single: Element? { count == 1 ? first : nil }
}
