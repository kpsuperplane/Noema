import Apollo
import ApolloAPI
import ApolloWebSocket
import Foundation

final class NoemaGraphQLClient: @unchecked Sendable {
  let client: ApolloClient
  private let webSocketTransport: WebSocketTransport

  init(profile: NoemaProfile) {
    let store = ApolloStore(cache: InMemoryNormalizedCache())
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
    Task {
      await websocket.updateHeaderValues(["Authorization": "Bearer \(profile.token)"], reconnectIfConnected: false)
    }
  }

  func pauseSubscriptions() async {
    await webSocketTransport.pause()
  }

  func resumeSubscriptionsAndRecover(refetch: (@Sendable () async -> Void)? = nil) async {
    await webSocketTransport.resume()
    await refetch?()
  }

  func clearInMemoryCache() async throws {
    try await client.clearCache()
  }

  private static func websocketURL(for origin: URL) -> URL {
    var components = URLComponents(url: origin.appending(path: "graphql/ws"), resolvingAgainstBaseURL: false)
    components?.scheme = "wss"
    return components?.url ?? origin
  }
}
