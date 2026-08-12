import SwiftUI

// This native port derives from boringdesigners/boring-avatars.
// See LICENSE.boring-avatars.txt in this folder.

private let marblePointCount = 8
private let marbleFlowSteps = 5

private struct MarblePoint: Sendable {
  var x: Double
  var y: Double

  static func interpolate(_ first: Self, _ second: Self, progress: Double) -> Self {
    Self(
      x: first.x + (second.x - first.x) * progress,
      y: first.y + (second.y - first.y) * progress
    )
  }
}

private struct MarbleNode: Sendable {
  var point: MarblePoint
  var incoming: MarblePoint
  var outgoing: MarblePoint
}

private struct MarbleContour: Sendable {
  var nodes: [MarbleNode]
  var targetArea: Double

  static func interpolate(_ first: Self, _ second: Self, progress: Double) -> Self {
    Self(
      nodes: zip(first.nodes, second.nodes).map { firstNode, secondNode in
        MarbleNode(
          point: .interpolate(firstNode.point, secondNode.point, progress: progress),
          incoming: .interpolate(firstNode.incoming, secondNode.incoming, progress: progress),
          outgoing: .interpolate(firstNode.outgoing, secondNode.outgoing, progress: progress)
        )
      },
      targetArea: second.targetArea
    )
  }
}

private struct MarbleFlowState: Sendable {
  var contours: [MarbleContour]

  static func interpolate(_ first: Self, _ second: Self, progress: Double) -> Self {
    Self(contours: zip(first.contours, second.contours).map {
      MarbleContour.interpolate($0, $1, progress: progress)
    })
  }
}

private struct MarbleVelocityField: Sendable {
  struct Vortex: Sendable {
    let center: MarblePoint
    let radius: Double
    let strength: Double
  }

  let vortices: [Vortex]
  let shear: Double
  let strain: Double
  let verticalDrift: Double
}

private struct MarbleFlowProfile {
  let duration: ClosedRange<Double>
  let strength: ClosedRange<Double>
  let shear: Double
  let strain: Double
  let verticalDrift: Double
}

private enum MarbleFlow {
  static func profile(_ activity: NoemaAvatarActivity) -> MarbleFlowProfile {
    switch activity {
    case .idle: MarbleFlowProfile(duration: 7...10, strength: 2.7...3.9, shear: 0.22, strain: 0.004, verticalDrift: 0.08)
    case .listening: MarbleFlowProfile(duration: 5.5...7.5, strength: 3...4.5, shear: 0.42, strain: 0.018, verticalDrift: -0.04)
    case .thinking: MarbleFlowProfile(duration: 4.5...6.5, strength: 4.3...6.1, shear: 0.58, strain: 0.012, verticalDrift: -0.12)
    case .speaking: MarbleFlowProfile(duration: 5...7, strength: 3.2...4.7, shear: 0.34, strain: 0.006, verticalDrift: -0.06)
    }
  }

  static func initial(seed: Int, activity: NoemaAvatarActivity) -> MarbleFlowState {
    var random = NoemaSeededRandom(NoemaAvatarSeed.hash("\(seed):\(activity.rawValue):marble-contours"))
    let activityPhase = switch activity {
    case .idle: 0.0
    case .listening: 0.7
    case .thinking: 1.4
    case .speaking: 2.1
    }
    let phase = random.between(0, .pi * 2) + activityPhase
    return MarbleFlowState(contours: [
      contour(
        random: &random,
        center: MarblePoint(x: 30 + random.between(-3, 3), y: 42 + random.between(-3, 3)),
        radiusX: 48,
        radiusY: 31,
        phase: phase
      ),
      contour(
        random: &random,
        center: MarblePoint(x: 69 + random.between(-3, 3), y: 61 + random.between(-3, 3)),
        radiusX: 46,
        radiusY: 34,
        phase: phase + 1.9
      ),
      contour(
        random: &random,
        center: MarblePoint(x: 52 + random.between(-4, 4), y: 29 + random.between(-2, 3)),
        radiusX: 39,
        radiusY: 27,
        phase: phase + 3.7
      ),
    ])
  }

  static func sample(seed: Int, activity: NoemaAvatarActivity, elapsed: Double) -> MarbleFlowState {
    var state = initial(seed: seed, activity: activity)
    var remaining = max(0, elapsed - 0.12)
    var eventIndex = 0
    while eventIndex < 64 {
      let event = flowEvent(current: state, seed: seed, activity: activity, eventIndex: eventIndex)
      if remaining <= event.duration {
        let scaled = min(1, remaining / event.duration) * Double(marbleFlowSteps)
        let index = min(marbleFlowSteps - 1, Int(scaled))
        return .interpolate(
          event.frames[index],
          event.frames[index + 1],
          progress: scaled - Double(index)
        )
      }
      remaining -= event.duration
      state = event.frames[marbleFlowSteps]
      eventIndex += 1
    }
    return state
  }

  static func pressure(seed: Int, activity: NoemaAvatarActivity, level: Double) -> MarbleContour {
    var random = NoemaSeededRandom(NoemaAvatarSeed.hash("\(seed):\(activity.rawValue):marble-pressure"))
    let phase = random.between(0, .pi * 2)
    let direction = random.next() < 0.5 ? -1.0 : 1.0
    let normalized = min(1, max(0, level))
    let anchors = (0..<marblePointCount).map { index in
      let angle = .pi * 2 * Double(index) / Double(marblePointCount)
      let pressure = max(0, cos(angle - phase)) * normalized
      let radiusX = 25 + normalized * 8 + pressure * 7
      let radiusY = 20 - normalized * 2 + (1 - pressure) * normalized * 5
      return MarblePoint(
        x: 50 + cos(angle) * radiusX + direction * sin(angle * 2) * normalized * 2.5,
        y: 50 + sin(angle) * radiusY - pressure * normalized * 3
      )
    }
    return contour(anchors: anchors)
  }

  static func rippleLevel(
    name: String,
    seed: Int,
    activity: NoemaAvatarActivity,
    audioLevel: Double?,
    elapsed: Double
  ) -> Double {
    if let audioLevel { return audioLevel }
    guard activity == .speaking else { return 0 }
    var random = NoemaSeededRandom(seed ^ 0x6D617262)
    var remaining = max(0, elapsed - 0.12)
    for _ in 0..<128 {
      let level = (0.42 + Double(NoemaAvatarSeed.hash(name) % 23) / 100) * random.between(0.82, 1.08)
      let duration = random.between(1.2, 2)
      if remaining <= duration {
        let progress = remaining / duration
        let envelope: Double
        switch progress {
        case ..<0.28: envelope = progress / 0.28 * 0.62
        case ..<0.52: envelope = 0.62 + (progress - 0.28) / 0.24 * 0.38
        case ..<0.74: envelope = 1 - (progress - 0.52) / 0.22 * 0.52
        default: envelope = 0.48 * (1 - (progress - 0.74) / 0.26)
        }
        return level * envelope
      }
      remaining -= duration + random.between(0.18, 0.52)
    }
    return 0
  }

  static func path(_ contour: MarbleContour) -> Path {
    var path = Path()
    guard let first = contour.nodes.first else { return path }
    path.move(to: CGPoint(x: first.point.x, y: first.point.y))
    for index in contour.nodes.indices {
      let node = contour.nodes[index]
      let next = contour.nodes[(index + 1) % contour.nodes.count]
      path.addCurve(
        to: CGPoint(x: next.point.x, y: next.point.y),
        control1: CGPoint(x: node.outgoing.x, y: node.outgoing.y),
        control2: CGPoint(x: next.incoming.x, y: next.incoming.y)
      )
    }
    path.closeSubpath()
    return path
  }

  private static func contour(
    random: inout NoemaSeededRandom,
    center: MarblePoint,
    radiusX: Double,
    radiusY: Double,
    phase: Double
  ) -> MarbleContour {
    let anchors = (0..<marblePointCount).map { index in
      let angle = .pi * 2 * Double(index) / Double(marblePointCount)
      let wobble = 1
        + 0.12 * sin(angle * 3 + phase)
        + 0.06 * sin(angle * 5 - phase * 0.7)
        + random.between(-0.035, 0.035)
      return MarblePoint(
        x: center.x + cos(angle) * radiusX * wobble,
        y: center.y + sin(angle) * radiusY * wobble
      )
    }
    return contour(anchors: anchors)
  }

  private static func contour(anchors: [MarblePoint]) -> MarbleContour {
    let nodes = anchors.indices.map { index in
      let anchor = anchors[index]
      let previous = anchors[(index - 1 + anchors.count) % anchors.count]
      let next = anchors[(index + 1) % anchors.count]
      let tangent = MarblePoint(x: (next.x - previous.x) / 6, y: (next.y - previous.y) / 6)
      return MarbleNode(
        point: anchor,
        incoming: MarblePoint(x: anchor.x - tangent.x, y: anchor.y - tangent.y),
        outgoing: MarblePoint(x: anchor.x + tangent.x, y: anchor.y + tangent.y)
      )
    }
    var result = MarbleContour(nodes: nodes, targetArea: 0)
    result.targetArea = area(result)
    return result
  }

  private static func flowEvent(
    current: MarbleFlowState,
    seed: Int,
    activity: NoemaAvatarActivity,
    eventIndex: Int
  ) -> (duration: Double, frames: [MarbleFlowState]) {
    let profile = profile(activity)
    var durationRandom = NoemaSeededRandom(
      NoemaAvatarSeed.hash("\(seed):\(activity.rawValue):marble-duration:\(eventIndex)")
    )
    let duration = durationRandom.between(profile.duration.lowerBound, profile.duration.upperBound)
    let field = velocityField(seed: seed, activity: activity, eventIndex: eventIndex)
    var next = current
    var frames = [next]
    for _ in 0..<marbleFlowSteps {
      for index in next.contours.indices { advect(&next.contours[index], field: field) }
      frames.append(next)
    }
    return (duration, frames)
  }

  private static func velocityField(
    seed: Int,
    activity: NoemaAvatarActivity,
    eventIndex: Int
  ) -> MarbleVelocityField {
    let profile = profile(activity)
    var random = NoemaSeededRandom(NoemaAvatarSeed.hash("\(seed):\(activity.rawValue):marble-flow:\(eventIndex)"))
    let firstDirection = random.next() < 0.5 ? -1.0 : 1.0
    let secondDirection = random.next() < 0.72 ? -firstDirection : firstDirection
    let multiplier = activity == .idle && eventIndex % 4 == 3 ? 1.32 : 1
    return MarbleVelocityField(
      vortices: [
        .init(
          center: MarblePoint(x: random.between(25, 48), y: random.between(28, 72)),
          radius: random.between(28, 42),
          strength: firstDirection * random.between(profile.strength.lowerBound, profile.strength.upperBound) * multiplier
        ),
        .init(
          center: MarblePoint(x: random.between(54, 79), y: random.between(25, 75)),
          radius: random.between(30, 46),
          strength: secondDirection * random.between(profile.strength.lowerBound, profile.strength.upperBound) * multiplier
        ),
      ],
      shear: profile.shear * (random.next() < 0.5 ? -1 : 1),
      strain: profile.strain * (random.next() < 0.5 ? -1 : 1),
      verticalDrift: profile.verticalDrift * (random.next() < 0.5 ? -1 : 1)
    )
  }

  private static func velocity(_ field: MarbleVelocityField, at point: MarblePoint) -> MarblePoint {
    var result = MarblePoint(x: 0, y: 0)
    for vortex in field.vortices {
      let dx = point.x - vortex.center.x
      let dy = point.y - vortex.center.y
      let falloff = exp(-(dx * dx + dy * dy) / (vortex.radius * vortex.radius))
      result.x -= dy / vortex.radius * vortex.strength * falloff
      result.y += dx / vortex.radius * vortex.strength * falloff
    }
    result.x += (point.y - 50) / 50 * field.shear
    result.y += (50 - point.x) / 50 * field.shear + field.verticalDrift
    result.x -= (point.x - 50) * field.strain
    result.y += (point.y - 50) * field.strain
    return result
  }

  private static func advected(_ point: MarblePoint, field: MarbleVelocityField) -> MarblePoint {
    let first = velocity(field, at: point)
    let midpoint = MarblePoint(x: point.x + first.x * 0.5, y: point.y + first.y * 0.5)
    let second = velocity(field, at: midpoint)
    return MarblePoint(x: point.x + second.x, y: point.y + second.y)
  }

  private static func advect(_ contour: inout MarbleContour, field: MarbleVelocityField) {
    for index in contour.nodes.indices {
      contour.nodes[index].point = advected(contour.nodes[index].point, field: field)
      contour.nodes[index].incoming = advected(contour.nodes[index].incoming, field: field)
      contour.nodes[index].outgoing = advected(contour.nodes[index].outgoing, field: field)
    }
    preserveArea(&contour)
    keepInBounds(&contour)
  }

  private static func area(_ contour: MarbleContour) -> Double {
    abs(contour.nodes.indices.reduce(0) { result, index in
      let current = contour.nodes[index].point
      let next = contour.nodes[(index + 1) % contour.nodes.count].point
      return result + current.x * next.y - next.x * current.y
    } / 2)
  }

  private static func preserveArea(_ contour: inout MarbleContour) {
    let currentArea = area(contour)
    guard currentArea > 0 else { return }
    let count = Double(contour.nodes.count)
    let center = contour.nodes.reduce(MarblePoint(x: 0, y: 0)) { result, node in
      MarblePoint(x: result.x + node.point.x / count, y: result.y + node.point.y / count)
    }
    let scale = sqrt(contour.targetArea / currentArea)
    for index in contour.nodes.indices {
      contour.nodes[index].point = scaled(contour.nodes[index].point, around: center, scale: scale)
      contour.nodes[index].incoming = scaled(contour.nodes[index].incoming, around: center, scale: scale)
      contour.nodes[index].outgoing = scaled(contour.nodes[index].outgoing, around: center, scale: scale)
    }
  }

  private static func scaled(_ point: MarblePoint, around center: MarblePoint, scale: Double) -> MarblePoint {
    MarblePoint(x: center.x + (point.x - center.x) * scale, y: center.y + (point.y - center.y) * scale)
  }

  private static func keepInBounds(_ contour: inout MarbleContour) {
    let points = contour.nodes.flatMap { [$0.point, $0.incoming, $0.outgoing] }
    let minimumX = points.map(\.x).min() ?? 0
    let maximumX = points.map(\.x).max() ?? 0
    let minimumY = points.map(\.y).min() ?? 0
    let maximumY = points.map(\.y).max() ?? 0
    let shiftX = minimumX < -12 ? -12 - minimumX : maximumX > 112 ? 112 - maximumX : 0
    let shiftY = minimumY < -12 ? -12 - minimumY : maximumY > 112 ? 112 - maximumY : 0
    for index in contour.nodes.indices {
      contour.nodes[index].point.x += shiftX
      contour.nodes[index].point.y += shiftY
      contour.nodes[index].incoming.x += shiftX
      contour.nodes[index].incoming.y += shiftY
      contour.nodes[index].outgoing.x += shiftX
      contour.nodes[index].outgoing.y += shiftY
    }
  }
}

struct NoemaMarbleAvatar: View {
  let name: String
  let colors: [UInt32]
  let activity: NoemaAvatarActivity
  let audioLevel: Double?
  let animated: Bool
  let square: Bool

  @State private var startedAt = Date.now

  var body: some View {
    Group {
      if animated {
        TimelineView(.animation(minimumInterval: 1 / 30)) { timeline in
          canvas(elapsed: timeline.date.timeIntervalSince(startedAt))
        }
      } else {
        canvas(elapsed: 0)
      }
    }
    .onChange(of: activity) { _, _ in startedAt = .now }
    .onChange(of: animated) { _, _ in startedAt = .now }
    .onChange(of: name) { _, _ in startedAt = .now }
  }

  private func canvas(elapsed: Double) -> some View {
    let palette = colors.isEmpty ? NoemaAvatarPalette.default : colors
    let seed = NoemaAvatarSeed.hash(name)
    let data = (
      background: palette[seed % palette.count],
      middle: palette[(seed + 7) % palette.count],
      foreground: palette[(seed + 19) % palette.count],
      accent: palette[(seed + 31) % palette.count]
    )
    let state = animated
      ? MarbleFlow.sample(seed: seed, activity: activity, elapsed: elapsed)
      : MarbleFlow.initial(seed: seed, activity: activity)
    let level = MarbleFlow.rippleLevel(
      name: name,
      seed: seed,
      activity: activity,
      audioLevel: audioLevel,
      elapsed: elapsed
    )
    let pressure = MarbleFlow.pressure(seed: seed, activity: activity, level: level)
    let baseOpacity = activity == .speaking ? 0.1 : activity == .listening ? 0.045 : 0
    let pressureOpacity = baseOpacity + (activity == .speaking ? level * 0.22 : level * 0.13)
    let blendMode: GraphicsContext.BlendMode = medianLuminance(palette) < 0.45 ? .screen : .multiply

    return Canvas { context, size in
      let scale = min(size.width, size.height) / 100
      let viewport = CGRect(x: 0, y: 0, width: 100, height: 100)
      context.scaleBy(x: scale, y: scale)
      context.clip(to: Path(roundedRect: viewport, cornerRadius: square ? 0 : 50))
      context.fill(
        Path(CGRect(x: -20, y: -20, width: 140, height: 140)),
        with: .radialGradient(
          Gradient(stops: [
            .init(color: data.accent.avatarColor, location: 0),
            .init(color: data.background.avatarColor, location: 0.54),
            .init(color: data.middle.avatarColor, location: 1),
          ]),
          center: CGPoint(x: 28, y: 20),
          startRadius: 0,
          endRadius: 92
        )
      )
      context.drawLayer { layer in
        layer.addFilter(.blur(radius: 2.4))
        layer.opacity = 0.82
        layer.fill(
          MarbleFlow.path(state.contours[0]),
          with: .radialGradient(
            Gradient(colors: [data.accent.avatarColor.opacity(0.94), data.middle.avatarColor.opacity(0.52)]),
            center: CGPoint(x: 34, y: 36),
            startRadius: 0,
            endRadius: 72
          )
        )
        layer.opacity = 0.76
        layer.fill(
          MarbleFlow.path(state.contours[1]),
          with: .linearGradient(
            Gradient(colors: [data.foreground.avatarColor.opacity(0.9), data.background.avatarColor.opacity(0.76), data.accent.avatarColor.opacity(0.48)]),
            startPoint: CGPoint(x: 0, y: 100),
            endPoint: CGPoint(x: 100, y: 0)
          )
        )
        layer.blendMode = blendMode
        layer.opacity = 0.52
        layer.fill(
          MarbleFlow.path(state.contours[2]),
          with: .radialGradient(
            Gradient(colors: [data.background.avatarColor.opacity(0.88), data.accent.avatarColor.opacity(0.68), data.foreground.avatarColor.opacity(0.34)]),
            center: CGPoint(x: 68, y: 28),
            startRadius: 0,
            endRadius: 68
          )
        )
        layer.blendMode = .screen
        layer.opacity = pressureOpacity
        layer.fill(
          MarbleFlow.path(pressure),
          with: .radialGradient(
            Gradient(colors: [data.accent.avatarColor.opacity(0.62), data.foreground.avatarColor.opacity(0)]),
            center: CGPoint(x: 50, y: 50),
            startRadius: 0,
            endRadius: 50
          )
        )
      }
      context.opacity = 0.4
      context.fill(
        Path(viewport),
        with: .linearGradient(
          Gradient(colors: [.white.opacity(0.32), .white.opacity(0.04), .black.opacity(0.12)]),
          startPoint: .zero,
          endPoint: CGPoint(x: 100, y: 100)
        )
      )
    }
  }

  private func medianLuminance(_ palette: [UInt32]) -> Double {
    let values = palette.map { color -> Double in
      let channels = [16, 8, 0].map { shift -> Double in
        let channel = Double((color >> UInt32(shift)) & 0xFF) / 255
        return channel <= 0.04045 ? channel / 12.92 : pow((channel + 0.055) / 1.055, 2.4)
      }
      return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
    }.sorted()
    return values[values.count / 2]
  }
}
