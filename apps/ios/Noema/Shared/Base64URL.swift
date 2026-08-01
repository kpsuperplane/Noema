import Foundation

extension Data {
  init?(base64URLEncoded value: String) {
    var normalized = value.replacingOccurrences(of: "-", with: "+")
      .replacingOccurrences(of: "_", with: "/")
    let remainder = normalized.count % 4
    if remainder != 0 { normalized += String(repeating: "=", count: 4 - remainder) }
    self.init(base64Encoded: normalized)
  }
}

