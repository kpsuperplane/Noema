import Apollo
import ApolloAPI
import Foundation
import Network
import NoemaAPI
import Testing
@testable import Noema

@Suite("Connection recovery")
struct NoemaConnectionRecoveryTests {
  @Test("Only an unsatisfied path blocks attempts")
  func networkPathMapping() {
    #expect(NoemaNetworkMonitor.availability(for: .satisfied) == .available)
    #expect(NoemaNetworkMonitor.availability(for: .requiresConnection) == .available)
    #expect(NoemaNetworkMonitor.availability(for: .unsatisfied) == .unavailable)
  }

  @Test("Network recovery starts an immediate attempt")
  @MainActor
  func networkRecoveryStartsAttempt() async throws {
    let fixture = recoveryFixture(active: true, availability: .unavailable)
    let task = consume(fixture.stream)

    try await Task.sleep(for: .milliseconds(50))
    #expect(fixture.transport.callCount == 0)
    fixture.status.networkChanged(.available)
    try await waitForCall(fixture.transport)
    #expect(fixture.transport.callCount == 1)
    task.cancel()
  }

  @Test("Retry delays use the capped schedule")
  func retryDelaySchedule() {
    let delays = (0...8).map(NoemaConnectionRetryPolicy.delaySeconds)
    #expect(delays == [0, 1, 2, 4, 8, 16, 30, 30, 30])
  }

  @Test("Background state blocks attempts")
  @MainActor
  func backgroundBlocksAttempt() async throws {
    let fixture = recoveryFixture(active: false, availability: .available)
    let task = consume(fixture.stream)

    try await Task.sleep(for: .milliseconds(50))
    #expect(fixture.transport.callCount == 0)
    fixture.status.foregroundAttemptStarted()
    try await waitForCall(fixture.transport)
    #expect(fixture.transport.callCount == 1)
    task.cancel()
  }

  @Test("Confirmation advances once and resets presentation")
  @MainActor
  func connectionConfirmation() {
    let status = NoemaConnectionStatus()
    #expect(status.isDisconnected)
    var callbacks = 0
    status.onConnectionConfirmed = { callbacks += 1 }
    status.networkChanged(.available)
    status.foregroundAttemptStarted()
    status.confirmationFailed()

    status.connectionConfirmed()
    status.connectionConfirmed()

    #expect(status.outcome == .connected)
    #expect(status.confirmationGeneration == 1)
    #expect(callbacks == 1)
  }

  @Test("Connection outcomes select exact banner content")
  @MainActor
  func connectionPresentation() {
    let status = NoemaConnectionStatus()
    status.foregroundAttemptStarted()
    status.networkChanged(.unavailable)
    #expect(status.bannerMessage == "No internet connection.")
    #expect(status.bannerSymbol == "wifi.slash")

    status.networkChanged(.available)
    #expect(status.bannerMessage == "Noema cannot connect to your server.")
    #expect(status.bannerSymbol == "externaldrive.badge.exclamationmark")

    status.connectionConfirmed()
    #expect(status.bannerMessage == nil)
  }

  @Test("Cancellation prevents another subscription attempt")
  @MainActor
  func cancellationStopsRetry() async throws {
    let fixture = recoveryFixture(active: true, availability: .available)
    let task = consume(fixture.stream)
    try await waitForCall(fixture.transport)

    fixture.status.networkChanged(.unavailable)
    task.cancel()
    _ = await task.result
    fixture.status.networkChanged(.available)
    try await Task.sleep(for: .milliseconds(100))

    #expect(fixture.transport.callCount == 1)
  }

  @MainActor
  private func recoveryFixture(
    active: Bool,
    availability: NoemaNetworkAvailability
  ) -> (
    status: NoemaConnectionStatus,
    transport: FailingSubscriptionTransport,
    stream: SubscriptionStream<GraphQLResponse<MemoryEventsSubscription>>
  ) {
    let status = NoemaConnectionStatus()
    status.networkChanged(availability)
    if active { status.foregroundAttemptStarted() }
    let transport = FailingSubscriptionTransport()
    let recovering = NoemaRecoveringSubscriptionTransport(
      underlying: transport,
      recovery: NoemaConnectionRecovery(status: status),
      connectionStatus: status
    )
    let stream = try! recovering.send(
      subscription: MemoryEventsSubscription(),
      fetchBehavior: .NetworkOnly,
      requestConfiguration: RequestConfiguration()
    )
    return (status, transport, stream)
  }

  private func consume<Element: Sendable>(
    _ stream: SubscriptionStream<Element>
  ) -> Task<Void, Never> {
    Task {
      do {
        for try await _ in stream {}
      } catch {
      }
    }
  }

  private func waitForCall(_ transport: FailingSubscriptionTransport) async throws {
    for _ in 0..<100 where transport.callCount == 0 {
      try await Task.sleep(for: .milliseconds(10))
    }
  }
}

private enum SubscriptionTestError: Error {
  case unavailable
}

private final class FailingSubscriptionTransport: SubscriptionNetworkTransport, @unchecked Sendable {
  private let lock = NSLock()
  private var calls = 0

  var callCount: Int {
    lock.lock()
    defer { lock.unlock() }
    return calls
  }

  func send<Subscription: GraphQLSubscription>(
    subscription: Subscription,
    fetchBehavior: FetchBehavior,
    requestConfiguration: RequestConfiguration
  ) throws -> SubscriptionStream<GraphQLResponse<Subscription>> {
    lock.lock()
    calls += 1
    lock.unlock()
    let stream = AsyncThrowingStream<GraphQLResponse<Subscription>, any Error> { continuation in
      continuation.finish(throwing: SubscriptionTestError.unavailable)
    }
    return SubscriptionStream(stream: stream, stateProvider: { .finished(.error(SubscriptionTestError.unavailable)) })
  }
}
