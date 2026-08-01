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

  let profileStore: KeychainProfileStore
  private let pairingService: PairingService
  private var graphQL: NoemaGraphQLClient?

  init() {
    let store = KeychainProfileStore()
    self.profileStore = store
    self.pairingService = PairingService(profileStore: store)
  }

  var graphQLClient: NoemaGraphQLClient? { graphQL }

  func bootstrap() async {
    do {
      if let stored = try await profileStore.read() {
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
      } else {
        state = .unpaired
      }
    } catch {
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
    isPairing = true
    pairingError = nil
    Task {
      do {
        let stored = try await pairingService.complete(payload: payload, displayName: displayName)
        profile = stored
        graphQL = NoemaGraphQLClient(profile: stored)
        state = .paired
        pairingPayload = nil
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

  func disconnect() {
    Task {
      graphQL = nil
      profile = nil
      try? await profileStore.disconnect()
      state = .unpaired
      pairingPayload = nil
      pairingInput = ""
      pairingError = nil
    }
  }

  func scenePhaseChanged(_ phase: ScenePhase) {
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
}
