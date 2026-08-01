import Foundation

enum NoemaDisplayName {
  static let maxUTF8Length = 128

  static func normalized(_ value: String) -> String? {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty, trimmed.utf8.count <= maxUTF8Length else { return nil }
    return trimmed
  }

  static func isValid(_ value: String) -> Bool {
    normalized(value) != nil
  }
}

