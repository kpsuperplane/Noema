import Apollo
import ApolloAPI
import Foundation
import Network
import Observation

enum NoemaNetworkAvailability: String, Sendable {
  case unknown
  case available
  case unavailable
}

@MainActor
final class NoemaNetworkMonitor {
  static let shared = NoemaNetworkMonitor()

  typealias Observer = @MainActor @Sendable (NoemaNetworkAvailability) -> Void

  private let monitor = NWPathMonitor()
  private let queue = DispatchQueue(label: "dev.noema.app.ios.network-path")
  private var observers: [UUID: Observer] = [:]
  private(set) var availability = NoemaNetworkAvailability.unknown

  private init() {
    monitor.pathUpdateHandler = { [weak self] path in
      let availability = Self.availability(for: path.status)
      let fields = [
        "status": Self.pathStatus(path.status),
        "interface": Self.interfaceName(path),
        "expensive": String(path.isExpensive),
        "constrained": String(path.isConstrained),
        "dns": String(path.supportsDNS),
        "ipv4": String(path.supportsIPv4),
        "ipv6": String(path.supportsIPv6)
      ]
      Task { @MainActor [weak self] in
        self?.publish(availability, fields: fields)
      }
    }
    monitor.start(queue: queue)
  }

  deinit {
    monitor.cancel()
  }

  func observe(_ observer: @escaping Observer) -> UUID {
    let id = UUID()
    observers[id] = observer
    observer(availability)
    return id
  }

  func removeObserver(_ id: UUID) {
    observers[id] = nil
  }

  nonisolated static func availability(for status: NWPath.Status) -> NoemaNetworkAvailability {
    status == .unsatisfied ? .unavailable : .available
  }

  private func publish(
    _ next: NoemaNetworkAvailability,
    fields: [String: String]
  ) {
    NoemaDiagnosticTrace.shared.record(category: "network", event: "path_changed", fields: fields)
    guard availability != next else { return }
    availability = next
    for observer in observers.values { observer(next) }
  }

  nonisolated private static func pathStatus(_ status: NWPath.Status) -> String {
    switch status {
    case .satisfied: "satisfied"
    case .unsatisfied: "unsatisfied"
    case .requiresConnection: "requires_connection"
    @unknown default: "unknown"
    }
  }

  nonisolated private static func interfaceName(_ path: NWPath) -> String {
    if path.usesInterfaceType(.wifi) { return "wifi" }
    if path.usesInterfaceType(.cellular) { return "cellular" }
    if path.usesInterfaceType(.wiredEthernet) { return "wired" }
    if path.usesInterfaceType(.loopback) { return "loopback" }
    if path.usesInterfaceType(.other) { return "other" }
    return "none"
  }
}

@MainActor
@Observable
final class NoemaConnectionStatus {
  enum Outcome: String, Sendable {
    case presumedConnected
    case connected
    case networkUnavailable
    case serverUnavailable
  }

  struct RetryState: Equatable, Sendable {
    let active: Bool
    let network: NoemaNetworkAvailability

    var allowsAttempt: Bool { active && network != .unavailable }
  }

  private(set) var outcome: Outcome = .presumedConnected
  private(set) var isActive = false
  private(set) var networkAvailability = NoemaNetworkAvailability.unknown
  private(set) var confirmationGeneration = 0
  @ObservationIgnored private var confirmationPending = true
  @ObservationIgnored private var retryObservers: [UUID: AsyncStream<RetryState>.Continuation] = [:]
  @ObservationIgnored var onConnectionConfirmed: (@MainActor () -> Void)?

  var isDisconnected: Bool {
    outcome != .connected
  }

  var bannerMessage: String? {
    switch outcome {
    case .networkUnavailable: "No internet connection."
    case .serverUnavailable: "Noema cannot connect to your server."
    case .presumedConnected, .connected: nil
    }
  }

  var bannerSymbol: String {
    outcome == .networkUnavailable ? "wifi.slash" : "externaldrive.badge.exclamationmark"
  }

  var retryState: RetryState {
    RetryState(active: isActive, network: networkAvailability)
  }

  func foregroundAttemptStarted() {
    isActive = true
    confirmationPending = true
    if networkAvailability == .unavailable {
      setOutcome(.networkUnavailable)
    } else if outcome == .networkUnavailable {
      setOutcome(.serverUnavailable)
    }
    publishRetryState()
  }

  func backgrounded() {
    isActive = false
    publishRetryState()
  }

  func networkChanged(_ availability: NoemaNetworkAvailability) {
    let previous = networkAvailability
    networkAvailability = availability
    if isActive, availability == .unavailable {
      confirmationPending = true
      setOutcome(.networkUnavailable)
    } else if isActive, previous == .unavailable, outcome == .networkUnavailable {
      setOutcome(.serverUnavailable)
    }
    publishRetryState()
  }

  func transportDisconnected() {
    guard isActive else { return }
    confirmationPending = true
    setOutcome(networkAvailability == .unavailable ? .networkUnavailable : .serverUnavailable)
  }

  func confirmationFailed() {
    guard isActive else { return }
    confirmationPending = true
    setOutcome(networkAvailability == .unavailable ? .networkUnavailable : .serverUnavailable)
  }

  func connectionConfirmed() {
    guard retryState.allowsAttempt, confirmationPending else { return }
    confirmationPending = false
    setOutcome(.connected)
    confirmationGeneration &+= 1
    onConnectionConfirmed?()
  }

  func retryStates() -> AsyncStream<RetryState> {
    let id = UUID()
    return AsyncStream { continuation in
      retryObservers[id] = continuation
      continuation.yield(retryState)
      continuation.onTermination = { @Sendable [weak self] _ in
        Task { @MainActor in self?.retryObservers[id] = nil }
      }
    }
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

  private func publishRetryState() {
    let state = retryState
    for observer in retryObservers.values { observer.yield(state) }
  }
}

struct NoemaConnectionRetryPolicy {
  static let maximumAttempt = 6

  static func delaySeconds(for attempt: Int) -> Int {
    guard attempt > 0 else { return 0 }
    return min(1 << min(attempt - 1, 5), 30)
  }
}

final class NoemaConnectionRecovery: @unchecked Sendable {
  private enum WakeReason: String {
    case timer
    case availability
  }

  private let status: NoemaConnectionStatus

  init(status: NoemaConnectionStatus) {
    self.status = status
  }

  func waitBeforeRetry(_ attempt: Int) async throws {
    let initial = await status.retryState
    if !initial.allowsAttempt {
      NoemaDiagnosticTrace.shared.record(
        category: "graphql",
        event: "retry_suspended",
        fields: ["attempt": String(attempt)]
      )
      try await waitUntilAllowed()
      recordRetryTrigger(.availability, attempt: attempt)
      return
    }

    let delay = NoemaConnectionRetryPolicy.delaySeconds(for: attempt)
    guard delay > 0 else { return }
    NoemaDiagnosticTrace.shared.record(
      category: "graphql",
      event: "retry_scheduled",
      fields: ["attempt": String(attempt), "delaySeconds": String(delay)]
    )

    let updates = await status.retryStates()
    let reason = try await withThrowingTaskGroup(of: WakeReason.self) { group in
      group.addTask {
        try await Task.sleep(for: .seconds(delay))
        return .timer
      }
      group.addTask {
        var blocked = false
        for await state in updates {
          try Task.checkCancellation()
          if state.allowsAttempt {
            if blocked { return .availability }
          } else {
            blocked = true
          }
        }
        throw CancellationError()
      }
      defer { group.cancelAll() }
      return try await group.next() ?? .timer
    }
    try await waitUntilAllowed()
    recordRetryTrigger(reason, attempt: attempt)
  }

  private func waitUntilAllowed() async throws {
    if await status.retryState.allowsAttempt { return }
    let updates = await status.retryStates()
    for await state in updates {
      try Task.checkCancellation()
      if state.allowsAttempt { return }
    }
    throw CancellationError()
  }

  private func recordRetryTrigger(_ reason: WakeReason, attempt: Int) {
    NoemaDiagnosticTrace.shared.record(
      category: "graphql",
      event: "retry_started",
      fields: ["attempt": String(attempt), "trigger": reason.rawValue]
    )
  }
}

private final class NoemaSubscriptionStateStore: @unchecked Sendable {
  private let lock = NSLock()
  private var stored = SubscriptionState.pending

  var state: SubscriptionState {
    lock.lock()
    defer { lock.unlock() }
    return stored
  }

  func set(_ next: SubscriptionState) {
    lock.lock()
    stored = next
    lock.unlock()
  }
}

final class NoemaRecoveringSubscriptionTransport: SubscriptionNetworkTransport, @unchecked Sendable {
  private let underlying: any SubscriptionNetworkTransport
  private let recovery: NoemaConnectionRecovery
  private let connectionStatus: NoemaConnectionStatus

  init(
    underlying: any SubscriptionNetworkTransport,
    recovery: NoemaConnectionRecovery,
    connectionStatus: NoemaConnectionStatus
  ) {
    self.underlying = underlying
    self.recovery = recovery
    self.connectionStatus = connectionStatus
  }

  func send<Subscription: GraphQLSubscription>(
    subscription: Subscription,
    fetchBehavior: FetchBehavior,
    requestConfiguration: RequestConfiguration
  ) throws -> SubscriptionStream<GraphQLResponse<Subscription>> {
    let state = NoemaSubscriptionStateStore()
    let stream = AsyncThrowingStream<GraphQLResponse<Subscription>, any Error> { continuation in
      let worker = Task { [underlying, recovery, connectionStatus] in
        var attempt = 0
        var confirmationGeneration = await connectionStatus.confirmationGeneration

        while !Task.isCancelled {
          do {
            state.set(attempt == 0 ? .pending : .reconnecting)
            try await recovery.waitBeforeRetry(attempt)
            try Task.checkCancellation()
            let source = try underlying.send(
              subscription: subscription,
              fetchBehavior: fetchBehavior,
              requestConfiguration: requestConfiguration
            )
            for try await response in source {
              try Task.checkCancellation()
              state.set(.active)
              continuation.yield(response)
            }
          } catch is CancellationError {
            break
          } catch {
            NoemaDiagnosticTrace.shared.record(
              category: "graphql",
              event: "subscription_failed",
              error: error
            )
          }

          guard !Task.isCancelled else { break }
          let currentGeneration = await connectionStatus.confirmationGeneration
          if currentGeneration != confirmationGeneration {
            confirmationGeneration = currentGeneration
            attempt = 1
          } else {
            attempt = min(attempt + 1, NoemaConnectionRetryPolicy.maximumAttempt)
          }
        }

        state.set(.finished(.cancelled))
        continuation.finish()
      }
      continuation.onTermination = { @Sendable _ in worker.cancel() }
    }
    return SubscriptionStream(stream: stream, stateProvider: { state.state })
  }
}
