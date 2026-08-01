import SwiftUI

enum NoemaMotion {
  static func animation(_ animation: Animation, reduceMotion: Bool) -> Animation? {
    reduceMotion ? nil : animation
  }
}

