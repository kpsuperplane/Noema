import Foundation
import Testing
@testable import Noema

@Suite("Credential refresh", .serialized)
struct NoemaCredentialRefreshTests {
  @Test("Concurrent refresh callers share one rotation")
  @MainActor
  func concurrentRefreshIsSerialized() async throws {
    RefreshURLProtocol.reset(with: [.success(tokenResponse(refresh: "successor-refresh"))])
    let fixture = try await makeFixture()

    async let first = fixture.service.refresh()
    async let second = fixture.service.refresh()
    let profiles = try await [first, second]

    #expect(RefreshURLProtocol.requestCount == 1)
    #expect(profiles.allSatisfy { $0.accessToken == "access-token" })
    let stored = try await fixture.store.read()
    #expect(stored?.refreshToken == "successor-refresh")
    #expect(stored?.pendingRefreshRequestId == nil)
    try await fixture.store.disconnect()
  }

  @Test("A failed response reuses its saved request identifier")
  @MainActor
  func responseLossReusesRequestIdentifier() async throws {
    RefreshURLProtocol.reset(with: [
      .failure(URLError(.networkConnectionLost)),
      .success(tokenResponse(refresh: "recovered-refresh"))
    ])
    let fixture = try await makeFixture()

    do {
      _ = try await fixture.service.refresh()
      Issue.record("The first refresh unexpectedly succeeded")
    } catch {
    }
    let pending = try #require(try await fixture.store.read()?.pendingRefreshRequestId)
    let recovered = try await fixture.service.refresh()

    #expect(recovered.accessToken == "access-token")
    #expect(RefreshURLProtocol.requestIDs == [pending, pending])
    #expect(try await fixture.store.read()?.pendingRefreshRequestId == nil)
    try await fixture.store.disconnect()
  }

  @Test("Inactive state blocks refresh before transmission")
  @MainActor
  func inactiveStateBlocksRefresh() async throws {
    RefreshURLProtocol.reset(with: [.success(tokenResponse(refresh: "unused-refresh"))])
    let fixture = try await makeFixture(allowRefresh: false)

    do {
      _ = try await fixture.service.refresh()
      Issue.record("Inactive refresh unexpectedly succeeded")
    } catch ConnectionServiceError.inactive {
    } catch {
      Issue.record("Inactive refresh returned \(error)")
    }

    #expect(RefreshURLProtocol.requestCount == 0)
    try await fixture.store.disconnect()
  }

  @Test("Existing Keychain profiles decode without pending state")
  func existingProfileDecodes() throws {
    let data = Data(#"{"origin":"https:\/\/noema.test","clientId":"client","refreshToken":"refresh"}"#.utf8)
    let profile = try JSONDecoder().decode(StoredNoemaProfile.self, from: data)
    #expect(profile.pendingRefreshRequestId == nil)
  }

  @MainActor
  private func makeFixture(
    allowRefresh: Bool = true
  ) async throws -> (service: ConnectionService, store: KeychainProfileStore) {
    let store = KeychainProfileStore(
      service: "dev.noema.app.ios.tests.\(UUID().uuidString)",
      account: "active"
    )
    try await store.replace(with: StoredNoemaProfile(
      origin: URL(string: "https://noema.test")!,
      clientId: "noema-ios:test-client",
      refreshToken: "initial-refresh"
    ))
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [RefreshURLProtocol.self]
    let service = ConnectionService(
      profileStore: store,
      sessionConfiguration: configuration
    )
    service.setRefreshAllowed(allowRefresh)
    return (service, store)
  }

  private func tokenResponse(refresh: String) -> Data {
    Data(
      #"{"access_token":"access-token","refresh_token":"\#(refresh)","expires_in":900}"#.utf8
    )
  }
}

private final class RefreshURLProtocol: URLProtocol, @unchecked Sendable {
  enum Result {
    case success(Data)
    case failure(Error)
  }

  private static let lock = NSLock()
  nonisolated(unsafe) private static var results: [Result] = []
  nonisolated(unsafe) private static var bodies: [String] = []

  static var requestCount: Int {
    lock.withLock { bodies.count }
  }

  static var requestIDs: [String] {
    lock.withLock {
      bodies.compactMap { body in
        URLComponents(string: "?\(body)")?.queryItems?
          .first(where: { $0.name == "refresh_request_id" })?.value
      }
    }
  }

  static func reset(with results: [Result]) {
    lock.withLock {
      self.results = results
      bodies = []
    }
  }

  override class func canInit(with request: URLRequest) -> Bool { true }

  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

  override func startLoading() {
    let result = Self.lock.withLock { () -> Result in
      Self.bodies.append(String(data: request.httpBody ?? Data(), encoding: .utf8) ?? "")
      return Self.results.isEmpty
        ? .failure(URLError(.badServerResponse))
        : Self.results.removeFirst()
    }
    switch result {
    case .success(let data):
      let response = HTTPURLResponse(
        url: request.url!,
        statusCode: 200,
        httpVersion: "HTTP/1.1",
        headerFields: ["Content-Type": "application/json"]
      )!
      client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
      client?.urlProtocol(self, didLoad: data)
      client?.urlProtocolDidFinishLoading(self)
    case .failure(let error):
      client?.urlProtocol(self, didFailWithError: error)
    }
  }

  override func stopLoading() {}
}
