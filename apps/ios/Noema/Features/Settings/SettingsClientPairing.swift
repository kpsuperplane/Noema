import Foundation

struct SettingsPairingLink: Hashable {
  let uri: String
}

extension SettingsModel {
  func startClientPairing(profile: NoemaProfile?) async {
    guard let profile, !isStartingPairing else { return }
    isStartingPairing = true
    defer { isStartingPairing = false }
    pairingErrorMessage = nil
    var components = URLComponents()
    components.scheme = "noema"
    components.host = "connect"
    components.queryItems = [
      URLQueryItem(name: "origin", value: profile.origin.absoluteString)
    ]
    guard let uri = components.url?.absoluteString else {
      pairingLink = nil
      pairingErrorMessage = "Noema could not create the connection link."
      return
    }
    pairingLink = SettingsPairingLink(uri: uri)
  }
}
