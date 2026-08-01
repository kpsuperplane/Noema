import Foundation

struct SettingsPairingLink: Hashable {
  let uri: String
  let expiresAt: Date
}

private struct PairingStartPayload: Decodable {
  let pairingUri: String
}

extension SettingsModel {
  func startClientPairing(profile: NoemaProfile?) async {
    guard let profile, !isStartingPairing else { return }
    isStartingPairing = true
    defer { isStartingPairing = false }
    pairingLink = nil
    pairingErrorMessage = nil
    do {
      let endpoint = profile.origin.appending(path: "auth/client/pairing/start")
      var request = URLRequest(url: endpoint)
      request.httpMethod = "POST"
      request.setValue("Bearer \(profile.token)", forHTTPHeaderField: "Authorization")
      request.setValue("application/json", forHTTPHeaderField: "Accept")
      let (data, response) = try await URLSession.shared.data(for: request)
      guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
        throw SettingsError.server("Client pairing could not be started.")
      }
      let payload = try JSONDecoder().decode(PairingStartPayload.self, from: data)
      guard payload.pairingUri.hasPrefix("noema://pair?") else { throw SettingsError.unavailable }
      pairingLink = SettingsPairingLink(
        uri: payload.pairingUri,
        expiresAt: Date().addingTimeInterval(600)
      )
    } catch {
      pairingErrorMessage = error.localizedDescription
    }
  }
}
