import Foundation
import Security
import NoemaAPI

enum NoemaAPNSEnvironment {
  static var current: NoemaAPI.ApnsEnvironment? {
    guard let task = SecTaskCreateFromSelf(nil),
          let value = SecTaskCopyValueForEntitlement(
            task,
            "aps-environment" as CFString,
            nil
          ) as? String
    else { return nil }

    switch value {
    case "development": .development
    case "production": .production
    default: nil
    }
  }
}
