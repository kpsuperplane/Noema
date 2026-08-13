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
  private(set) var pairingPayload: PairingPayload?
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
  private let pairingService: PairingService
  private var graphQL: NoemaGraphQLClient?
  private var registrationCleanupComplete = false
  private var subscriptionLifecycleTask: Task<Void, Never>?

  init() {
    let store = KeychainProfileStore()
    let notifications = NoemaNotificationService()
    let liveActivities = NoemaLiveActivityService()
    self.profileStore = store
    self.notifications = notifications
    self.liveActivities = liveActivities
    self.pairingService = PairingService(profileStore: store)
    NoemaApplicationDelegate.notifications = notifications
    notifications.onChatTap = { [weak self] in
      self?.openChatFromNotification()
    }
    notifications.onTaskTap = { [weak self] taskID in
      self?.openTaskFromNotification(taskID)
    }
  }

  var graphQLClient: NoemaGraphQLClient? { graphQL }

  func bootstrap() async {
    NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_started")
    do {
      if let stored = try await profileStore.read() {
        NoemaDiagnosticTrace.shared.record(category: "app", event: "profile_restored")
        NoemaGraphQLClient.discardStaleCaches(keeping: stored)
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
        await notifications.configure(profile: stored, client: graphQL?.client)
        await liveActivities.configure(profile: stored, client: graphQL?.client)
        notifications.markModelReady()
        NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_finished", fields: ["state": "paired"])
      } else {
        NoemaGraphQLClient.discardStaleCaches(keeping: nil)
        notifications.markModelNotReady()
        await liveActivities.configure(profile: nil, client: nil)
        state = .unpaired
        NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_finished", fields: ["state": "unpaired"])
      }
    } catch {
      NoemaDiagnosticTrace.shared.record(category: "app", event: "bootstrap_failed", error: error)
      notifications.markModelNotReady()
      await liveActivities.configure(profile: nil, client: nil)
      pairingError = "The saved connection could not be read. Pair this device again."
      state = .unpaired
    }
  }

  func ingestPairingText(_ value: String) {
    pairingInput = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !pairingInput.isEmpty else {
      pairingPayload = nil
      pairingError = "Paste a Noema pairing link."
      return
    }
    do {
      pairingPayload = try PairingPayload(string: pairingInput)
      pairingError = nil
    } catch {
      pairingPayload = nil
      pairingError = error.localizedDescription
    }
  }

  func ingestPairingURL(_ url: URL) {
    pairingInput = url.absoluteString
    do {
      pairingPayload = try PairingPayload(url: url)
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

  func completePairing(displayName: String) {
    guard let payload = pairingPayload, !isPairing else { return }
    let replacingExistingProfile = profile != nil
    isPairing = true
    pairingError = nil
    Task {
      do {
        if replacingExistingProfile {
          let notificationsRemoved = await notifications.disable()
          let activitiesRemoved = await liveActivities.disable()
          guard notificationsRemoved, activitiesRemoved else {
            pairingError = "Reconnect to the current server before pairing a replacement."
            isPairing = false
            return
          }
        }
        let previousGraphQL = graphQL
        let stored = try await pairingService.complete(payload: payload, displayName: displayName)
        try? await previousGraphQL?.clearCache()
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
        pairingPayload = nil
        await notifications.configure(profile: stored, client: graphQL?.client)
        await liveActivities.configure(profile: stored, client: graphQL?.client)
        notifications.markModelReady()
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
          ?? "Reconnect to this server before unpairing."
        return
      }
      registrationCleanupComplete = true
      do {
        try await graphQL?.clearCache()
      } catch {
        disconnectError = "Noema could not remove the offline cache. Try unpairing again."
        return
      }
      do {
        try await profileStore.disconnect()
      } catch {
        disconnectError = "Noema could not remove the saved connection. Try unpairing again."
        return
      }
      notifications.markModelNotReady()
      await notifications.configure(profile: nil, client: nil)
      await liveActivities.configure(profile: nil, client: nil)
      graphQL = nil
      profile = nil
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
    guard let graphQL else { return }
    subscriptionLifecycleTask?.cancel()
    switch phase {
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
