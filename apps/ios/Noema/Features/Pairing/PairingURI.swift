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
    else { throw ConnectionURIError.untrustedOrigin }

    let host = rawHost.lowercased()
    var value = "https://\(host)"
    if let port = components.port, port != 443 {
      guard (1...65535).contains(port) else { throw ConnectionURIError.untrustedOrigin }
      value += ":\(port)"
    }
    guard let canonicalURL = URL(string: value) else { throw ConnectionURIError.untrustedOrigin }
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

struct ConnectionPayload: Hashable, Sendable {
  let origin: TrustedOrigin

  init(string: String) throws {
    guard let url = URL(string: string.trimmingCharacters(in: .whitespacesAndNewlines)) else {
      throw ConnectionURIError.invalidURI
    }
    try self.init(url: url)
  }

  init(url: URL) throws {
    if url.scheme?.lowercased() == "https" {
      self.origin = try TrustedOrigin(url: url)
      return
    }
    guard url.scheme?.lowercased() == "noema",
          url.host?.lowercased() == "connect",
          url.user == nil,
          url.password == nil,
          url.fragment == nil,
          url.path.isEmpty || url.path == "/",
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
    else { throw ConnectionURIError.invalidURI }

    let values = Dictionary(grouping: components.queryItems ?? [], by: \.name)
    guard values.keys.count == 1,
          let originValue = values["origin"]?.single?.value
    else { throw ConnectionURIError.missingField }

    self.origin = try TrustedOrigin(url: try Self.originURL(from: originValue))
  }

  private static func originURL(from value: String) throws -> URL {
    guard let url = URL(string: value),
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
          components.query == nil,
          components.fragment == nil
    else { throw ConnectionURIError.untrustedOrigin }
    return url
  }

}

enum ConnectionURIError: Error, LocalizedError {
  case invalidURI
  case missingField
  case untrustedOrigin

  var errorDescription: String? {
    switch self {
    case .invalidURI: "Enter a Noema HTTPS server or connection link."
    case .missingField: "The connection link is missing its server origin."
    case .untrustedOrigin: "Connection requires a canonical HTTPS origin that is not localhost."
    }
  }
}

private extension Array where Element == URLQueryItem {
  var single: Element? { count == 1 ? first : nil }
}
