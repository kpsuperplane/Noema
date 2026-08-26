import Apollo
import ApolloAPI
import ApolloSQLite
import ApolloWebSocket
import CryptoKit
import Foundation
import NoemaAPI

final class NoemaGraphQLClient: @unchecked Sendable {
  private static let normalizedCacheVersion = 2
  let client: ApolloClient
  @MainActor let connectionStatus: NoemaConnectionStatus
  private let credential: NoemaAccessCredential
  private let webSocketTransport: WebSocketTransport
  private let connectionRecovery: NoemaConnectionRecovery
  private var networkObserverID: UUID?
  @MainActor private var healthCheckTask: Task<Void, Never>?

  @MainActor init(profile: NoemaProfile) {
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "client_initializing")
    let connectionStatus = NoemaConnectionStatus()
    let connectionRecovery = NoemaConnectionRecovery(status: connectionStatus)
    self.connectionStatus = connectionStatus
    self.connectionRecovery = connectionRecovery
    self.credential = profile.credential
    let store = ApolloStore(cache: Self.normalizedCache(for: profile))
    let http = RequestChainNetworkTransport(
      urlSession: URLSession(configuration: .ephemeral),
      interceptorProvider: NoemaInterceptorProvider(credential: profile.credential),
      store: store,
      endpointURL: profile.origin.appending(path: "graphql")
    )

    let webSocketSessionConfiguration = URLSessionConfiguration.ephemeral
    webSocketSessionConfiguration.httpAdditionalHeaders = ["Authorization": "Bearer \(profile.accessToken)"]
    let websocketConfiguration = WebSocketTransport.Configuration(
      reconnectionInterval: -1,
      connectingPayload: nil,
      pingInterval: 20
    )
    let websocket = WebSocketTransport(
      urlSession: URLSession(configuration: webSocketSessionConfiguration),
      store: store,
      endpointURL: Self.websocketURL(for: profile.origin),
      configuration: websocketConfiguration
    )
    self.webSocketTransport = websocket
    let recoveringSubscriptions = NoemaRecoveringSubscriptionTransport(
      underlying: websocket,
      recovery: connectionRecovery,
      connectionStatus: connectionStatus
    )
    self.client = ApolloClient(
      networkTransport: SplitNetworkTransport(
        queryTransport: http,
        mutationTransport: http,
        subscriptionTransport: recoveringSubscriptions
      ),
      store: store
    )
    Task { await websocket.setDelegate(self) }
    networkObserverID = NoemaNetworkMonitor.shared.observe { [weak self] availability in
      Task { @MainActor [weak self] in await self?.networkChanged(availability) }
    }
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "client_initialized")
  }

  deinit {
    guard let networkObserverID else { return }
    Task { @MainActor in NoemaNetworkMonitor.shared.removeObserver(networkObserverID) }
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
    if connectionStatus.retryState.allowsAttempt {
      await webSocketTransport.resume()
    }
    NoemaDiagnosticTrace.shared.record(category: "graphql", event: "subscriptions_resumed")
  }

  @MainActor func refreshAuthorization() async {
    await webSocketTransport.updateHeaderValues(
      ["Authorization": "Bearer \(credential.token)"],
      reconnectIfConnected: true
    )
  }

  @MainActor private func confirmConnection() {
    guard connectionStatus.retryState.allowsAttempt else { return }
    healthCheckTask?.cancel()
    healthCheckTask = Task { [weak self] in
      guard let self else { return }
      var attempt = 0
      while !Task.isCancelled {
        do {
          try await connectionRecovery.waitBeforeRetry(attempt)
          let response = try await client.fetch(
            query: NoemaAPI.NativeConnectionHealthQuery(),
            cachePolicy: .networkOnly
          )
          guard response.data != nil, response.errors?.isEmpty != false else {
            throw ConnectionHealthError.invalidResponse
          }
          guard !Task.isCancelled, connectionStatus.retryState.allowsAttempt else { return }
          connectionStatus.connectionConfirmed()
          return
        } catch is CancellationError {
          return
        } catch {
          NoemaDiagnosticTrace.shared.record(category: "graphql", event: "connection_health_failed", error: error)
          connectionStatus.confirmationFailed()
          attempt = min(attempt + 1, NoemaConnectionRetryPolicy.maximumAttempt)
        }
      }
    }
  }

  @MainActor private func networkChanged(_ availability: NoemaNetworkAvailability) async {
    connectionStatus.networkChanged(availability)
    guard connectionStatus.isActive else { return }
    if availability == .unavailable {
      healthCheckTask?.cancel()
      await webSocketTransport.pause()
    } else {
      await webSocketTransport.resume()
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
    let identity = "v\(normalizedCacheVersion)\u{0}\(profile.origin.absoluteString)\u{0}\(profile.clientId)"
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

private struct NoemaInterceptorProvider: InterceptorProvider {
  let credential: NoemaAccessCredential

  func graphQLInterceptors<Operation: GraphQLOperation>(
    for operation: Operation
  ) -> [any GraphQLInterceptor] {
    [NoemaAuthorizationInterceptor(credential: credential)]
      + DefaultInterceptorProvider.shared.graphQLInterceptors(for: operation)
  }

  func cacheInterceptor<Operation: GraphQLOperation>(
    for operation: Operation
  ) -> any CacheInterceptor {
    DefaultInterceptorProvider.shared.cacheInterceptor(for: operation)
  }

  func httpInterceptors<Operation: GraphQLOperation>(
    for operation: Operation
  ) -> [any HTTPInterceptor] {
    DefaultInterceptorProvider.shared.httpInterceptors(for: operation)
  }

  func responseParser<Operation: GraphQLOperation>(
    for operation: Operation
  ) -> any ResponseParsingInterceptor {
    DefaultInterceptorProvider.shared.responseParser(for: operation)
  }
}

private struct NoemaAuthorizationInterceptor: GraphQLInterceptor {
  let credential: NoemaAccessCredential

  func intercept<Request: GraphQLRequest>(
    request: Request,
    next: NextInterceptorFunction<Request>
  ) async throws -> InterceptorResultStream<Request> {
    var request = request
    request.addHeader(name: "Authorization", value: "Bearer \(credential.token)")
    return await next(request)
  }
}

extension ApolloClient {
  func recoveringSubscribe<Subscription: GraphQLSubscription>(
    subscription: Subscription
  ) throws -> SubscriptionStream<GraphQLResponse<Subscription>> {
    try subscribe(subscription: subscription)
  }

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
