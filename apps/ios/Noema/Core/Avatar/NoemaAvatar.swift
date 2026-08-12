import Foundation
import SwiftUI

// This native port derives from boringdesigners/boring-avatars.
// See LICENSE.boring-avatars.txt in this folder.

enum NoemaAvatarVariant: Sendable {
  case marble
  case beam
}

enum NoemaAvatarActivity: String, Sendable {
  case idle
  case listening
  case thinking
  case speaking
}

enum NoemaAvatarActorType: Sendable {
  case agent
  case human
}

struct NoemaAvatar: View {
  var name = "Clara Barton"
  var colors = NoemaAvatarPalette.default
  var variant: NoemaAvatarVariant = .marble
  var activity: NoemaAvatarActivity = .idle
  var audioLevel: Double?
  var animated = true
  var square = false

  @Environment(\.accessibilityReduceMotion) private var reduceMotion

  var body: some View {
    Group {
      switch variant {
      case .marble:
        NoemaMarbleAvatar(
          name: name,
          colors: colors,
          activity: activity,
          audioLevel: normalizedAudioLevel,
          animated: animated && !reduceMotion,
          square: square
        )
      case .beam:
        NoemaBeamAvatar(
          name: name,
          colors: colors,
          activity: activity,
          audioLevel: normalizedAudioLevel,
          animated: animated && !reduceMotion,
          square: square
        )
      }
    }
    .aspectRatio(1, contentMode: .fit)
    .accessibilityLabel(name)
  }

  private var normalizedAudioLevel: Double? {
    guard let audioLevel, audioLevel.isFinite else { return audioLevel == nil ? nil : 0 }
    return min(1, max(0, audioLevel))
  }
}

struct NoemaIdentityAvatar: View {
  let actorID: String
  let actorType: NoemaAvatarActorType
  var activity: NoemaAvatarActivity = .idle
  var audioLevel: Double?
  var animated = false

  var body: some View {
    NoemaAvatar(
      name: NoemaAvatarSeed.actor(actorID),
      colors: NoemaAvatarPalette.noema,
      variant: actorType == .human ? .marble : .beam,
      activity: animated ? activity : .idle,
      audioLevel: audioLevel,
      animated: animated
    )
    .clipShape(Circle())
    .accessibilityLabel(actorType == .human ? "Human avatar" : "Agent avatar")
  }
}

enum NoemaAvatarPalette {
  static let `default`: [UInt32] = [0x92A1C6, 0x146A7C, 0xF0AB3D, 0xC271B4, 0xC20D90]
  static let noema: [UInt32] = [0x3B4A6B, 0x7D6A91, 0xB9786D, 0xD6AD6B, 0xE6D8C4, 0x2F3440]
}

enum NoemaAvatarSeed {
  static func hash(_ value: String) -> Int {
    var hash: Int32 = 0
    for unit in value.utf16 {
      hash = hash &* 31 &+ Int32(unit)
    }
    return Int(abs(Int64(hash)))
  }

  static func actor(_ actorID: String) -> String {
    var hash: UInt32 = 0x811C9DC5
    for unit in actorID.utf16 {
      hash ^= UInt32(unit)
      hash = hash &* 0x01000162
    }
    return "actor-" + String(format: "%08x", hash)
  }
}

func noemaTaskRunAvatarMotion(status: String) -> (activity: NoemaAvatarActivity, animated: Bool) {
  switch status.lowercased() {
  case "queued", "leased":
    (.listening, true)
  case "running":
    (.thinking, true)
  default:
    (.idle, false)
  }
}

struct NoemaSeededRandom {
  private var state: UInt32

  init(_ seed: Int) {
    state = UInt32(truncatingIfNeeded: seed == 0 ? 1 : seed)
  }

  mutating func next() -> Double {
    state ^= state &<< 13
    state ^= state &>> 17
    state ^= state &<< 5
    return Double(state) / 4_294_967_296
  }

  mutating func between(_ minimum: Double, _ maximum: Double) -> Double {
    minimum + (maximum - minimum) * next()
  }
}

extension Color {
  init(avatarHex: UInt32, opacity: Double = 1) {
    self.init(
      .sRGB,
      red: Double((avatarHex >> 16) & 0xFF) / 255,
      green: Double((avatarHex >> 8) & 0xFF) / 255,
      blue: Double(avatarHex & 0xFF) / 255,
      opacity: opacity
    )
  }
}

extension UInt32 {
  var avatarColor: Color { Color(avatarHex: self) }
}
