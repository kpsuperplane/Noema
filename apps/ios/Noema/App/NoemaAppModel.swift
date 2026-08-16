import Foundation
import Observation
import SwiftUI

@MainActor
@Observable
final class NoemaAppModel {
  enum ConnectionState {
    case loading
    case unpaired
    case paired
  }

  private(set) var state: ConnectionState = .loading
  private(set) var profile: NoemaProfile?
  private(set) var pairingPayload: ConnectionPayload?
  private(set) var pairingError: String?
  private(set) var pairingInput = ""
  private(set) var isPairing = false
  private(set) var recoveryGeneration = 0
  private(set) var notificationTapGeneration = 0
  private(set) var pendingTaskID: String?
  private(set) var disconnectError: String?

  let profileStore: KeychainProfileStore
  let notifications: NoemaNotificationService
  let liveActivities: NoemaLiveActivityService
  private let connectionService: ConnectionService
  private var graphQL: NoemaGraphQLClient?
  private var registrationCleanupComplete = false
  private var latestScenePhase: ScenePhase?
  private var subscriptionLifecycleTask: Task<Void, Never>?
  private var tokenRefreshTask: Task<Void, Never>?
  private var hasStoredProfile = false

  init() {
    let store = KeychainProfileStore()
    let notifications = NoemaNotificationService()
    let liveActivities = NoemaLiveActivityService()
    self.profileStore = store
    self.notifications = notifications
    self.liveActivities = liveActivities
    self.connectionService = ConnectionService(profileStore: store)
    NoemaApplicationDelegate.notifications = notifications
    notifications.onChatTap = { [weak self] in
      self?.openChatFromNotification()
    }
    notifications.onTaskTap = { [weak self] taskID in
      self?.openTaskFromNotification(taskID)
    }
  }

  var graphQLClient: NoemaGraphQLClient? { graphQL }
  var isDisconnected: Bool { graphQL?.connectionStatus.isDisconnected ?? false }

  func bootstrap() async {
    NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_started")
    do {
      if let stored = try await profileStore.read() {
        hasStoredProfile = true
        let restored = try await connectionService.refresh(stored)
        NoemaDiagnosticTrace.shared.record(category: "app", event: "profile_restored")
        NoemaGraphQLClient.discardStaleCaches(keeping: restored)
        profile = restored
        graphQL = NoemaGraphQLClient(profile: restored)
        state = .paired
        await notifications.configure(profile: restored, client: graphQL?.client)
        await liveActivities.configure(profile: restored, client: graphQL?.client)
        notifications.markModelReady()
        applySubscriptionLifecycle()
        scheduleTokenRefresh()
        NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_finished", fields: ["state": "paired"])
      } else {
        hasStoredProfile = false
        NoemaGraphQLClient.discardStaleCaches(keeping: nil)
        notifications.markModelNotReady()
        await liveActivities.configure(profile: nil, client: nil)
        state = .unpaired
        NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_finished", fields: ["state": "unpaired"])
      }
    } catch ConnectionServiceError.authorizationExpired {
      try? await profileStore.disconnect()
      hasStoredProfile = false
      NoemaDiagnosticTrace.shared.record(category: "app", event: "authorization_expired")
      notifications.markModelNotReady()
      await liveActivities.configure(profile: nil, client: nil)
      pairingError = "The saved connection needs authorization again."
      state = .unpaired
    } catch {
      NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_failed", error: error)
      notifications.markModelNotReady()
      await liveActivities.configure(profile: nil, client: nil)
      pairingError = "The saved connection could not be restored. Connect this device again."
      state = .unpaired
    }
  }

  func ingestPairingText(_ value: String) {
    pairingInput = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !pairingInput.isEmpty else {
      pairingPayload = nil
      pairingError = "Enter a Noema server address or connection link."
      return
    }
    guard !hasStoredProfile else {
      pairingPayload = nil
      pairingError = "Disconnect this app before connecting it to another server."
      return
    }
    do {
      pairingPayload = try ConnectionPayload(string: pairingInput)
      pairingError = nil
    } catch {
      pairingPayload = nil
      pairingError = error.localizedDescription
    }
  }

  func ingestPairingURL(_ url: URL) {
    pairingInput = url.absoluteString
    guard !hasStoredProfile else {
      pairingPayload = nil
      pairingError = "Disconnect this app before connecting it to another server."
      return
    }
    do {
      pairingPayload = try ConnectionPayload(url: url)
      pairingError = nil
    } catch {
      pairingPayload = nil
      pairingError = error.localizedDescription
    }
  }

  func ingestURL(_ url: URL) {
    let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
    let queryTaskID = components?.queryItems?.first(where: { $0.name == "id" })?.value
    let pathTaskID = String(url.path.dropFirst()).removingPercentEncoding
    let taskID = queryTaskID ?? pathTaskID ?? ""
    if url.scheme?.lowercased() == "noema",
       url.host?.lowercased() == "task",
       (1...256).contains(taskID.utf8.count),
       taskID.hasPrefix("task:") {
      pendingTaskID = taskID
      return
    }
    ingestPairingURL(url)
  }

  func completePairing() {
    guard let payload = pairingPayload, !hasStoredProfile, !isPairing else { return }
    isPairing = true
    pairingError = nil
    Task {
      do {
        let previousGraphQL = graphQL
        let stored = try await connectionService.authorize(payload: payload)
        hasStoredProfile = true
        try? await previousGraphQL?.clearCache()
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
        pairingPayload = nil
        await notifications.configure(profile: stored, client: graphQL?.client)
        await liveActivities.configure(profile: stored, client: graphQL?.client)
        notifications.markModelReady()
        applySubscriptionLifecycle()
        scheduleTokenRefresh()
      } catch {
        pairingError = error.localizedDescription
      }
      isPairing = false
    }
  }

  func cancelPairingReplacement() {
    guard profile != nil, !isPairing else { return }
    pairingPayload = nil
    pairingInput = ""
    pairingError = nil
  }

  func disconnect(registrationsAlreadyRemoved: Bool = false) {
    Task {
      disconnectError = nil
      let notificationsRemoved: Bool
      let activitiesRemoved: Bool
      if registrationsAlreadyRemoved || registrationCleanupComplete {
        notifications.clearLocalRegistration()
        notificationsRemoved = true
        await liveActivities.clearLocalActivities()
        activitiesRemoved = true
      } else {
        notificationsRemoved = await notifications.disable()
        activitiesRemoved = await liveActivities.disable()
      }
      if !notificationsRemoved || !activitiesRemoved {
        disconnectError = notifications.errorMessage
          ?? liveActivities.errorMessage
          ?? "Reconnect to this server before disconnecting."
        return
      }
      registrationCleanupComplete = true
      guard let profile else {
        disconnectError = "No active server connection is available."
        return
      }
      do {
        try await connectionService.revoke(profile)
      } catch {
        disconnectError = "Noema could not revoke this client. Reconnect before disconnecting."
        return
      }
      do {
        try await graphQL?.clearCache()
      } catch {
        disconnectError = "Noema could not remove the offline cache. Try disconnecting again."
        return
      }
      do {
        try await profileStore.disconnect()
        hasStoredProfile = false
      } catch {
        disconnectError = "Noema could not remove the saved connection. Try disconnecting again."
        return
      }
      notifications.markModelNotReady()
      await notifications.configure(profile: nil, client: nil)
      await liveActivities.configure(profile: nil, client: nil)
      graphQL = nil
      profile = nil
      tokenRefreshTask?.cancel()
      tokenRefreshTask = nil
      registrationCleanupComplete = false
      state = .unpaired
      pairingPayload = nil
      pairingInput = ""
      pairingError = nil
    }
  }

  func clearDisconnectError() {
    disconnectError = nil
  }

  func scenePhaseChanged(_ phase: ScenePhase) {
    latestScenePhase = phase
    let phaseName: String
    switch phase {
    case .active: phaseName = "active"
    case .inactive: phaseName = "inactive"
    case .background: phaseName = "background"
    @unknown default: phaseName = "unknown"
    }
    NoemaDiagnosticTrace.shared.record(
      category: "app",
      event: "scene_phase_changed",
      fields: ["phase": phaseName, "recoveryGeneration": String(recoveryGeneration)]
    )
    notifications.scenePhaseChanged(phase == .active)
    liveActivities.scenePhaseChanged(phase == .active)
    applySubscriptionLifecycle()
  }

  private func applySubscriptionLifecycle() {
    guard let latestScenePhase, let graphQL else { return }
    subscriptionLifecycleTask?.cancel()
    switch latestScenePhase {
    case .background, .inactive:
      subscriptionLifecycleTask = Task {
        guard !Task.isCancelled else { return }
        await graphQL.pauseSubscriptions()
      }
    case .active:
      subscriptionLifecycleTask = Task {
        let startedAt = ProcessInfo.processInfo.systemUptime
        NoemaDiagnosticTrace.shared.record(category: "graphql", event: "foreground_recovery_started")
        await graphQL.resumeSubscriptionsAndRecover()
        guard !Task.isCancelled else { return }
        recoveryGeneration &+= 1
        NoemaDiagnosticTrace.shared.record(
          category: "graphql",
          event: "foreground_recovery_finished",
          fields: [
            "durationMilliseconds": String(Int((ProcessInfo.processInfo.systemUptime - startedAt) * 1_000)),
            "recoveryGeneration": String(recoveryGeneration)
          ]
        )
      }
    @unknown default:
      break
    }
  }

  private func scheduleTokenRefresh(after retryDelay: TimeInterval? = nil) {
    tokenRefreshTask?.cancel()
    guard let profile else { return }
    let delay = retryDelay ?? max(1, profile.accessExpiresAt.timeIntervalSinceNow - 60)
    tokenRefreshTask = Task { [weak self] in
      do {
        try await Task.sleep(for: .seconds(delay))
        guard let self, let current = self.profile else { return }
        let refreshed = try await self.connectionService.refresh(current)
        guard !Task.isCancelled else { return }
        await self.graphQL?.pauseSubscriptions()
        self.profile = refreshed
        self.graphQL = NoemaGraphQLClient(profile: refreshed)
        await self.notifications.configure(profile: refreshed, client: self.graphQL?.client)
        await self.liveActivities.configure(profile: refreshed, client: self.graphQL?.client)
        self.applySubscriptionLifecycle()
        self.scheduleTokenRefresh()
      } catch is CancellationError {
      } catch ConnectionServiceError.authorizationExpired {
        try? await self?.profileStore.disconnect()
        self?.hasStoredProfile = false
        self?.notifications.markModelNotReady()
        await self?.liveActivities.configure(profile: nil, client: nil)
        self?.graphQL = nil
        self?.profile = nil
        self?.state = .unpaired
        self?.pairingError = "The saved connection needs authorization again."
      } catch {
        self?.scheduleTokenRefresh(after: 15)
      }
    }
  }

  func openChatFromNotification() {
    notificationTapGeneration &+= 1
    recoveryGeneration &+= 1
  }

  func openTaskFromNotification(_ taskID: String) {
    pendingTaskID = taskID
    recoveryGeneration &+= 1
  }

  func clearPendingTaskID() {
    pendingTaskID = nil
  }
}
