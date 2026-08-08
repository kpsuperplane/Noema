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
  private(set) var disconnectError: String?

  let profileStore: KeychainProfileStore
  let notifications: NoemaNotificationService
  private let pairingService: PairingService
  private var graphQL: NoemaGraphQLClient?

  init() {
    let store = KeychainProfileStore()
    let notifications = NoemaNotificationService()
    self.profileStore = store
    self.notifications = notifications
    self.pairingService = PairingService(profileStore: store)
    NoemaApplicationDelegate.notifications = notifications
    notifications.onChatTap = { [weak self] in
      self?.openChatFromNotification()
    }
  }

  var graphQLClient: NoemaGraphQLClient? { graphQL }

  func bootstrap() async {
    do {
      if let stored = try await profileStore.read() {
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
        await notifications.configure(profile: stored, client: graphQL?.client)
        notifications.markModelReady()
      } else {
        notifications.markModelNotReady()
        state = .unpaired
      }
    } catch {
      notifications.markModelNotReady()
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

  func completePairing(displayName: String) {
    guard let payload = pairingPayload, !isPairing else { return }
    let replacingExistingProfile = profile != nil
    isPairing = true
    pairingError = nil
    Task {
      do {
        if replacingExistingProfile, !(await notifications.disable()) {
          pairingError = "Reconnect to the current server before pairing a replacement."
          isPairing = false
          return
        }
        let stored = try await pairingService.complete(payload: payload, displayName: displayName)
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
        pairingPayload = nil
        await notifications.configure(profile: stored, client: graphQL?.client)
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

  func disconnect(notificationsAlreadyRemoved: Bool = false) {
    Task {
      disconnectError = nil
      if notificationsAlreadyRemoved {
        notifications.clearLocalRegistration()
      } else if !(await notifications.disable()) {
        disconnectError = notifications.errorMessage ?? "Reconnect to this server before unpairing."
        return
      }
      notifications.markModelNotReady()
      await notifications.configure(profile: nil, client: nil)
      graphQL = nil
      profile = nil
      try? await profileStore.disconnect()
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
    notifications.scenePhaseChanged(phase == .active)
    guard let graphQL else { return }
    switch phase {
    case .background, .inactive:
      Task { await graphQL.pauseSubscriptions() }
    case .active:
      Task {
        await graphQL.resumeSubscriptionsAndRecover()
        recoveryGeneration &+= 1
      }
    @unknown default:
      break
    }
  }

  func openChatFromNotification() {
    notificationTapGeneration &+= 1
    recoveryGeneration &+= 1
  }
}
