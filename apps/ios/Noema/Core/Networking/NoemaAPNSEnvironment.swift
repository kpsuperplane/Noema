import Foundation
import NoemaAPI

enum NoemaAPNSEnvironment {
  static var current: NoemaAPI.ApnsEnvironment? {
    guard let value = Bundle.main.object(
      forInfoDictionaryKey: "NoemaAPNSEnvironment"
    ) as? String
    else { return nil }

    switch value {
    case "development": return .development
    case "production": return .production
    default: return nil
    }
  }
}
