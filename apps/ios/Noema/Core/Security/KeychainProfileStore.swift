import Foundation
import Security

final class NoemaAccessCredential: @unchecked Sendable {
  private struct Value {
    var token: String
    var expiresAt: Date
    var generation: Int
  }

  private let lock = NSLock()
  private var value = Value(token: "", expiresAt: .distantPast, generation: 0)

  var token: String { lock.withLock { value.token } }
  var expiresAt: Date { lock.withLock { value.expiresAt } }
  var generation: Int { lock.withLock { value.generation } }

  func replace(token: String, expiresAt: Date) {
    lock.withLock {
      value.token = token
      value.expiresAt = expiresAt
      value.generation &+= 1
    }
  }

  func clear() {
    replace(token: "", expiresAt: .distantPast)
  }
}

struct NoemaProfile: Hashable, Sendable {
  let origin: URL
  let clientId: String
  let credential: NoemaAccessCredential

  var accessToken: String { credential.token }
  var accessExpiresAt: Date { credential.expiresAt }
  var credentialGeneration: Int { credential.generation }

  static func == (lhs: Self, rhs: Self) -> Bool {
    lhs.origin == rhs.origin && lhs.clientId == rhs.clientId
  }

  func hash(into hasher: inout Hasher) {
    hasher.combine(origin)
    hasher.combine(clientId)
  }
}

struct StoredNoemaProfile: Codable, Hashable, Sendable {
  let origin: URL
  let clientId: String
  let refreshToken: String
  let pendingRefreshRequestId: String?

  init(
    origin: URL,
    clientId: String,
    refreshToken: String,
    pendingRefreshRequestId: String? = nil
  ) {
    self.origin = origin
    self.clientId = clientId
    self.refreshToken = refreshToken
    self.pendingRefreshRequestId = pendingRefreshRequestId
  }
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
  private let service: String
  private let account: String

  init(
    service: String = "dev.noema.app.ios.profile",
    account: String = "active"
  ) {
    self.service = service
    self.account = account
  }

  func read() throws -> StoredNoemaProfile? {
    var query = baseQuery
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne

    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess else { throw KeychainProfileError.read(status) }
    guard let data = result as? Data else { throw KeychainProfileError.invalidProfile }
    return try JSONDecoder().decode(StoredNoemaProfile.self, from: data)
  }

  func replace(with profile: StoredNoemaProfile) throws {
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
