import Foundation
import Security

struct NoemaProfile: Codable, Hashable, Sendable {
  let origin: URL
  let clientId: String
  let token: String
}

enum KeychainProfileError: Error, LocalizedError {
  case invalidProfile
  case read(OSStatus)
  case write(OSStatus)

  var errorDescription: String? {
    switch self {
    case .invalidProfile: "The returned client credential was invalid."
    case .read(let status): "Unable to read the Noema client profile (\(status))."
    case .write(let status): "Unable to store the Noema client profile (\(status))."
    }
  }
}

actor KeychainProfileStore {
  private let service = "dev.noema.app.ios.profile"
  private let account = "active"

  func read() throws -> NoemaProfile? {
    var query = baseQuery
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne

    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess else { throw KeychainProfileError.read(status) }
    guard let data = result as? Data else { throw KeychainProfileError.invalidProfile }
    return try JSONDecoder().decode(NoemaProfile.self, from: data)
  }

  func replace(with profile: NoemaProfile) throws {
    let data = try JSONEncoder().encode(profile)
    let updates: [String: Any] = [kSecValueData as String: data]
    let updateStatus = SecItemUpdate(baseQuery as CFDictionary, updates as CFDictionary)
    if updateStatus == errSecSuccess { return }
    guard updateStatus == errSecItemNotFound else { throw KeychainProfileError.write(updateStatus) }

    var attributes = baseQuery
    attributes[kSecValueData as String] = data
    attributes[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
    let addStatus = SecItemAdd(attributes as CFDictionary, nil)
    guard addStatus == errSecSuccess else { throw KeychainProfileError.write(addStatus) }
  }

  func disconnect() throws {
    let status = SecItemDelete(baseQuery as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw KeychainProfileError.write(status)
    }
  }

  private var baseQuery: [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
      kSecAttrAccount as String: account
    ]
  }
}
