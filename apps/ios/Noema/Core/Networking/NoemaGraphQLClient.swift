import Apollo
import ApolloAPI
import ApolloSQLite
import ApolloWebSocket
import CryptoKit
import Foundation
import NoemaAPI
import Observation

@MainActor
@Observable
final class NoemaConnectionStatus {
  enum Outcome: String {
    case presumedConnected
    case connected
    case disconnected
  }

  private(set) var outcome: Outcome = .presumedConnected
  private(set) var isActive = false
  private var reconnectAttemptPending = false

  var isDisconnected: Bool { outcome == .disconnected }

  func foregroundAttemptStarted() {
    isActive = true
    reconnectAttemptPending = true
  }

  func backgrounded() {
    isActive = false
  }

  func transportDisconnected() {
    guard isActive else { return }
    if reconnectAttemptPending || outcome == .presumedConnected {
      setOutcome(.disconnected)
    } else {
      reconnectAttemptPending = true
    }
  }

  func confirmationFailed() {
    guard isActive else { return }
    setOutcome(.disconnected)
  }

  func connectionConfirmed() {
    guard isActive else { return }
    reconnectAttemptPending = false
    setOutcome(.connected)
  }

  private func setOutcome(_ next: Outcome) {
    guard outcome != next else { return }
    outcome = next
    NoemaDiagnosticTrace.shared.record(
      category: "graphql",
      event: "connection_outcome_changed",
      fields: ["outcome": next.rawValue]
    )
  }
}

final class NoemaGraphQLClient: @unchecked Sendable {
  private static let reconnectionInterval: TimeInterval = 2

  let client: ApolloClient
  @MainActor let connectionStatus: NoemaConnectionStatus
  private let webSocketTransport: WebSocketTransport
  @MainActor private var healthCheckTask: Task<Void, Never>?

  @MainActor init(profile: NoemaProfile) {
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "client_initializing")
    self.connectionStatus = NoemaConnectionStatus()
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
      reconnectionInterval: Self.reconnectionInterval,
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
    Task { await websocket.setDelegate(self) }
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "client_initialized")
  }

  @MainActor func pauseSubscriptions() async {
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "subscriptions_pausing")
    connectionStatus.backgrounded()
    healthCheckTask?.cancel()
    await webSocketTransport.pause()
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "subscriptions_paused")
  }

  @MainActor func resumeSubscriptionsAndRecover() async {
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "subscriptions_resuming")
    connectionStatus.foregroundAttemptStarted()
    await webSocketTransport.resume()
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "subscriptions_resumed")
  }

  @MainActor private func confirmConnection() {
    guard connectionStatus.isActive else { return }
    healthCheckTask?.cancel()
    healthCheckTask = Task { [weak self] in
      guard let self else { return }
      while !Task.isCancelled, connectionStatus.isActive {
        do {
          let response = try await client.fetch(
            query: NoemaAPI.NativeConnectionHealthQuery(),
            cachePolicy: .networkOnly
          )
          guard response.data != nil, response.errors?.isEmpty != false else {
            throw ConnectionHealthError.invalidResponse
          }
          guard !Task.isCancelled, connectionStatus.isActive else { return }
          connectionStatus.connectionConfirmed()
          return
        } catch is CancellationError {
          return
        } catch {
          NoemaDiagnosticTrace.shared.record(category: "graphql", event: "connection_health_failed", error: error)
          connectionStatus.confirmationFailed()
          try? await Task.sleep(for: .seconds(Self.reconnectionInterval))
        }
      }
    }
  }

  @MainActor private func transportDisconnected() {
    healthCheckTask?.cancel()
    connectionStatus.transportDisconnected()
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
    let trace = NoemaDiagnosticTrace.shared
    let fileURL: URL
    do {
      fileURL = try cacheURL(for: profile)
    } catch {
      trace.record(category: "cache", event: "directory_failed", error: error)
      return InMemoryNormalizedCache()
    }
    do {
      let cache = try SQLiteNormalizedCache(fileURL: fileURL, shouldVacuumOnClear: true)
      trace.record(category: "cache", event: "sqlite_opened")
      return cache
    } catch {
      trace.record(category: "cache", event: "sqlite_open_failed", error: error)
    }
    removeCacheFiles(at: fileURL)
    do {
      let cache = try SQLiteNormalizedCache(fileURL: fileURL, shouldVacuumOnClear: true)
      trace.record(category: "cache", event: "sqlite_recreated")
      return cache
    } catch {
      trace.record(category: "cache", event: "memory_fallback", error: error)
      return InMemoryNormalizedCache()
    }
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

extension NoemaGraphQLClient: WebSocketTransportDelegate {
  func webSocketTransportDidConnect(_ webSocketTransport: isolated WebSocketTransport) {
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "websocket_connected")
    Task { @MainActor [weak self] in self?.confirmConnection() }
  }

  func webSocketTransportDidReconnect(_ webSocketTransport: isolated WebSocketTransport) {
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "websocket_reconnected")
    Task { @MainActor [weak self] in self?.confirmConnection() }
  }

  func webSocketTransport(
    _ webSocketTransport: isolated WebSocketTransport,
    didDisconnectWithError error: (any Error)?
  ) {
    if let error {
      NoemaDiagnosticTrace.shared.record(category: "graphql", event: "websocket_disconnected", error: error)
    } else {
      NoemaDiagnosticTrace.shared.record(category: "graphql", event: "websocket_disconnected")
    }
    Task { @MainActor [weak self] in self?.transportDisconnected() }
  }
}

private enum ConnectionHealthError: Error {
  case invalidResponse
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
