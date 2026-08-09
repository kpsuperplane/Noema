import Apollo
import ApolloAPI
import ApolloSQLite
import ApolloWebSocket
import CryptoKit
import Foundation

final class NoemaGraphQLClient: @unchecked Sendable {
  let client: ApolloClient
  private let webSocketTransport: WebSocketTransport

  init(profile: NoemaProfile) {
    let store = ApolloStore(cache: Self.normalizedCache(for: profile))
    let sessionConfiguration = URLSessionConfiguration.ephemeral
    sessionConfiguration.httpAdditionalHeaders = ["Authorization": "Bearer \(profile.token)"]
    let http = RequestChainNetworkTransport(
      urlSession: URLSession(configuration: sessionConfiguration),
      interceptorProvider: DefaultInterceptorProvider.shared,
      store: store,
      endpointURL: profile.origin.appending(path: "graphql"),
      additionalHeaders: ["Authorization": "Bearer \(profile.token)"]
    )

    let websocketConfiguration = WebSocketTransport.Configuration(
      reconnectionInterval: 2,
      connectingPayload: nil,
      pingInterval: 20
    )
    let websocket = WebSocketTransport(
      urlSession: URLSession(configuration: sessionConfiguration),
      store: store,
      endpointURL: Self.websocketURL(for: profile.origin),
      configuration: websocketConfiguration
    )
    self.webSocketTransport = websocket
    self.client = ApolloClient(
      networkTransport: SplitNetworkTransport(
        queryTransport: http,
        mutationTransport: http,
        subscriptionTransport: websocket
      ),
      store: store
    )
  }

  func pauseSubscriptions() async {
    await webSocketTransport.pause()
  }

  func resumeSubscriptionsAndRecover(refetch: (@Sendable () async -> Void)? = nil) async {
    await webSocketTransport.resume()
    await refetch?()
  }

  func clearCache() async throws {
    try await client.clearCache()
  }

  static func discardStaleCaches(keeping profile: NoemaProfile?) {
    guard let directory = try? cacheDirectory() else { return }
    let retainedPrefix = profile.map { "\(cacheBaseName(for: $0)).sqlite" }
    guard let files = try? FileManager.default.contentsOfDirectory(
      at: directory,
      includingPropertiesForKeys: nil
    ) else { return }
    for file in files {
      if let retainedPrefix, file.lastPathComponent.hasPrefix(retainedPrefix) { continue }
      try? FileManager.default.removeItem(at: file)
    }
  }

  private static func normalizedCache(for profile: NoemaProfile) -> any NormalizedCache {
    guard let fileURL = try? cacheURL(for: profile) else { return InMemoryNormalizedCache() }
    if let cache = try? SQLiteNormalizedCache(fileURL: fileURL, shouldVacuumOnClear: true) {
      return cache
    }
    removeCacheFiles(at: fileURL)
    return (try? SQLiteNormalizedCache(fileURL: fileURL, shouldVacuumOnClear: true))
      ?? InMemoryNormalizedCache()
  }

  private static func cacheURL(for profile: NoemaProfile) throws -> URL {
    try cacheDirectory().appending(path: "\(cacheBaseName(for: profile)).sqlite")
  }

  private static func cacheBaseName(for profile: NoemaProfile) -> String {
    let identity = "\(profile.origin.absoluteString)\u{0}\(profile.clientId)"
    return SHA256.hash(data: Data(identity.utf8))
      .map { String(format: "%02x", $0) }
      .joined()
  }

  private static func cacheDirectory() throws -> URL {
    let fileManager = FileManager.default
    guard let applicationSupport = fileManager.urls(
      for: .applicationSupportDirectory,
      in: .userDomainMask
    ).first else { throw CocoaError(.fileNoSuchFile) }
    var directory = applicationSupport
      .appending(path: "Noema", directoryHint: .isDirectory)
      .appending(path: "GraphQLCache", directoryHint: .isDirectory)
    try fileManager.createDirectory(
      at: directory,
      withIntermediateDirectories: true,
      attributes: [.protectionKey: FileProtectionType.complete]
    )
    try fileManager.setAttributes(
      [.protectionKey: FileProtectionType.complete],
      ofItemAtPath: directory.path
    )
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try directory.setResourceValues(values)
    return directory
  }

  private static func removeCacheFiles(at fileURL: URL) {
    for suffix in ["", "-journal", "-shm", "-wal"] {
      try? FileManager.default.removeItem(atPath: fileURL.path + suffix)
    }
  }

  private static func websocketURL(for origin: URL) -> URL {
    var components = URLComponents(url: origin.appending(path: "graphql/ws"), resolvingAgainstBaseURL: false)
    components?.scheme = "wss"
    return components?.url ?? origin
  }
}

extension ApolloClient {
  /// Apollo 2.3.0 skips its network-first cache fallback when a successful HTTP response has no body.
  func fetchNetworkFirst<Query: GraphQLQuery>(
    query: Query
  ) async throws -> GraphQLResponse<Query> where Query.ResponseFormat == SingleResponseFormat {
    do {
      return try await fetch(query: query, cachePolicy: .networkFirst)
    } catch ApolloClient.Error.noResults {
      if let cached = try await fetch(query: query, cachePolicy: .cacheOnly) {
        return cached
      }
      throw ApolloClient.Error.noResults
    }
  }
}
