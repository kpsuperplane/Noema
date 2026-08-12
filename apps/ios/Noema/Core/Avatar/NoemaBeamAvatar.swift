import SwiftUI

// This native port derives from boringdesigners/boring-avatars.
// See LICENSE.boring-avatars.txt in this folder.

private struct BeamTransform: Sendable {
  var x = 0.0
  var y = 0.0
  var rotation = 0.0
  var scaleX = 1.0
  var scaleY = 1.0

  static func interpolate(_ first: Self, _ second: Self, progress: Double) -> Self {
    Self(
      x: first.x + (second.x - first.x) * progress,
      y: first.y + (second.y - first.y) * progress,
      rotation: first.rotation + (second.rotation - first.rotation) * progress,
      scaleX: first.scaleX + (second.scaleX - first.scaleX) * progress,
      scaleY: first.scaleY + (second.scaleY - first.scaleY) * progress
    )
  }

  func adding(_ other: Self) -> Self {
    Self(
      x: x + other.x,
      y: y + other.y,
      rotation: rotation + other.rotation,
      scaleX: scaleX * other.scaleX,
      scaleY: scaleY * other.scaleY
    )
  }

  func affine(origin: CGPoint) -> CGAffineTransform {
    CGAffineTransform(translationX: origin.x + x, y: origin.y + y)
      .rotated(by: rotation * .pi / 180)
      .scaledBy(x: scaleX, y: scaleY)
      .translatedBy(x: -origin.x, y: -origin.y)
  }
}

private struct BeamMotionPose: Sendable {
  var character = BeamTransform()
  var shadow = BeamTransform()
  var shadowOpacity = 0.24
  var body = BeamTransform()
  var face = BeamTransform()
  var gaze = BeamTransform()
  var mouthOpen = 0.0
  var blink = 0.0

  static func interpolate(_ first: Self, _ second: Self, progress: Double) -> Self {
    Self(
      character: .interpolate(first.character, second.character, progress: progress),
      shadow: .interpolate(first.shadow, second.shadow, progress: progress),
      shadowOpacity: first.shadowOpacity + (second.shadowOpacity - first.shadowOpacity) * progress,
      body: .interpolate(first.body, second.body, progress: progress),
      face: .interpolate(first.face, second.face, progress: progress),
      gaze: .interpolate(first.gaze, second.gaze, progress: progress),
      mouthOpen: first.mouthOpen + (second.mouthOpen - first.mouthOpen) * progress,
      blink: first.blink + (second.blink - first.blink) * progress
    )
  }
}

private struct BeamKeyframe: Sendable {
  let offset: Double
  let pose: BeamMotionPose
}

private struct BeamEvent: Sendable {
  let duration: Double
  let gap: Double
  let frames: [BeamKeyframe]
}

private enum BeamMotion {
  static func base(_ activity: NoemaAvatarActivity) -> BeamMotionPose {
    switch activity {
    case .idle:
      BeamMotionPose()
    case .listening:
      BeamMotionPose(
        character: BeamTransform(scaleX: 1.42, scaleY: 1.39),
        shadow: BeamTransform(y: 5, scaleX: 0.72, scaleY: 0.62),
        shadowOpacity: 0.08,
        body: BeamTransform(y: 0.4, rotation: -0.7, scaleX: 1.018, scaleY: 0.982),
        face: BeamTransform(y: 0.65, rotation: -0.4, scaleX: 1.075, scaleY: 0.955),
        gaze: BeamTransform(y: -0.2)
      )
    case .thinking:
      BeamMotionPose(
        character: BeamTransform(x: -3, y: 0.8, scaleX: 0.965, scaleY: 0.985),
        shadow: BeamTransform(x: -3, scaleX: 0.8, scaleY: 0.82),
        shadowOpacity: 0.18,
        body: BeamTransform(y: -0.9, rotation: -3, scaleX: 0.99, scaleY: 1.02),
        face: BeamTransform(x: 0.65, y: -2.4, rotation: 1.2),
        gaze: BeamTransform(x: 0.9, y: -2.7, scaleX: 0.98)
      )
    case .speaking:
      BeamMotionPose(
        character: BeamTransform(y: -1, scaleX: 1.02, scaleY: 1.02),
        shadow: BeamTransform(scaleX: 0.92, scaleY: 0.9),
        shadowOpacity: 0.24,
        body: BeamTransform(y: -0.35, rotation: 0.8, scaleX: 1.008, scaleY: 0.992),
        face: BeamTransform(y: 0.25),
        gaze: BeamTransform(x: 0.15),
        mouthOpen: 0.45
      )
    }
  }

  static func sample(
    name: String,
    activity: NoemaAvatarActivity,
    previousActivity: NoemaAvatarActivity,
    audioLevel: Double?,
    elapsed: Double,
    isCircle: Bool
  ) -> BeamMotionPose {
    let target = base(activity)
    if elapsed < 0.36 {
      return .interpolate(base(previousActivity), target, progress: easeOut(elapsed / 0.36))
    }

    var random = NoemaSeededRandom(NoemaAvatarSeed.hash("\(name):\(activity.rawValue):beam"))
    var remaining = max(0, elapsed - 0.48)
    var event = nextEvent(activity: activity, random: &random, isCircle: isCircle, base: target)
    for _ in 0..<128 {
      if remaining <= event.duration {
        var pose = interpolate(event.frames, progress: remaining / event.duration)
        pose.blink = max(pose.blink, blink(name: name, elapsed: elapsed))
        if activity == .speaking {
          pose.mouthOpen = mouthLevel(name: name, audioLevel: audioLevel, elapsed: elapsed)
          let pulse = pose.mouthOpen
          pose.body = pose.body.adding(BeamTransform(y: -0.7 * pulse, scaleX: 1 + 0.012 * pulse, scaleY: 1 - 0.008 * pulse))
        }
        return pose
      }
      remaining -= event.duration
      if remaining < event.gap {
        var pose = target
        pose.blink = blink(name: name, elapsed: elapsed)
        if activity == .speaking { pose.mouthOpen = mouthLevel(name: name, audioLevel: audioLevel, elapsed: elapsed) }
        return pose
      }
      remaining -= event.gap
      event = nextEvent(activity: activity, random: &random, isCircle: isCircle, base: target)
    }
    return target
  }

  private static func nextEvent(
    activity: NoemaAvatarActivity,
    random: inout NoemaSeededRandom,
    isCircle: Bool,
    base: BeamMotionPose
  ) -> BeamEvent {
    if activity == .thinking { return thinking(random: &random, base: base) }
    if activity == .speaking { return speaking(random: &random, base: base) }
    let direction = random.next() < 0.5 ? -1.0 : 1.0
    let action = random.next()
    if activity == .idle, action < 0.16 { return roll(random: &random, direction: direction, isCircle: isCircle, base: base) }
    if activity == .idle, action < 0.34 { return peek(random: &random, direction: direction, base: base) }
    let followChance = activity == .idle ? 0.34 : 0.28
    if random.next() < followChance { return follow(random: &random, direction: direction, activity: activity, base: base) }
    return quickLook(random: &random, direction: direction, activity: activity, base: base)
  }

  private static func quickLook(
    random: inout NoemaSeededRandom,
    direction: Double,
    activity: NoemaAvatarActivity,
    base: BeamMotionPose
  ) -> BeamEvent {
    let duration = activity == .listening ? random.between(1.4, 1.9) : random.between(1.8, 2.4)
    let gazeX = direction * random.between(1.1, 1.75)
    let gazeY = random.between(-1.05, 0.7)
    let roamX = direction * random.between(3.5, 6)
    let hopY = -random.between(1.4, 2.6)
    var middle = base
    middle.gaze = middle.gaze.adding(BeamTransform(x: gazeX, y: gazeY, scaleX: 0.94))
    middle.face = middle.face.adding(BeamTransform(x: gazeX * 3.2, y: gazeY * 0.85, rotation: 2.4 * direction, scaleX: 0.95))
    if activity == .idle {
      middle.character = middle.character.adding(BeamTransform(x: roamX, y: hopY, scaleX: 1.01, scaleY: 0.99))
      middle.shadow = middle.shadow.adding(BeamTransform(x: roamX, scaleX: 0.72, scaleY: 0.78))
      middle.shadowOpacity *= 0.62
      middle.body = middle.body.adding(BeamTransform(x: 0.3 * direction, y: -0.22, rotation: 1.6 * direction, scaleX: 1.005, scaleY: 0.995))
    } else {
      middle.character = BeamTransform(x: -0.35, y: 0.2, scaleX: 1.435, scaleY: 1.375)
      middle.shadow = BeamTransform(y: 5, scaleX: 0.66, scaleY: 0.56)
      middle.shadowOpacity = 0.055
    }
    return BeamEvent(
      duration: duration,
      gap: activity == .idle ? random.between(1.8, 3.4) : random.between(1.4, 2.6),
      frames: [.init(offset: 0, pose: base), .init(offset: 0.5, pose: middle), .init(offset: 1, pose: base)]
    )
  }

  private static func follow(
    random: inout NoemaSeededRandom,
    direction: Double,
    activity: NoemaAvatarActivity,
    base: BeamMotionPose
  ) -> BeamEvent {
    let duration = random.between(3.8, 5.6)
    let travel = direction * random.between(5.5, 9)
    var first = base
    first.gaze = first.gaze.adding(BeamTransform(x: direction * 2.7, y: -2.1, scaleX: 0.94))
    first.face = first.face.adding(BeamTransform(x: direction * 5.2, y: -1.4, rotation: direction * 3.2, scaleX: 0.96))
    var second = first
    second.character = second.character.adding(BeamTransform(x: travel, y: activity == .idle ? -1.8 : 0.3, scaleX: 1.01, scaleY: 0.99))
    second.shadow = second.shadow.adding(BeamTransform(x: travel, scaleX: 0.82, scaleY: 0.88))
    second.gaze = second.gaze.adding(BeamTransform(x: -direction * 1.1, y: 0.5))
    return BeamEvent(
      duration: duration,
      gap: random.between(1.2, 2.8),
      frames: [
        .init(offset: 0, pose: base),
        .init(offset: 0.2, pose: first),
        .init(offset: 0.62, pose: second),
        .init(offset: 0.82, pose: first),
        .init(offset: 1, pose: base),
      ]
    )
  }

  private static func roll(
    random: inout NoemaSeededRandom,
    direction: Double,
    isCircle: Bool,
    base: BeamMotionPose
  ) -> BeamEvent {
    let duration = random.between(2.8, 3.6)
    let startX = -direction * random.between(13, 17)
    let endX = direction * random.between(14, 18)
    func pose(x: Double, y: Double, rotation: Double, scaleX: Double = 1, scaleY: Double = 1) -> BeamMotionPose {
      var pose = base
      pose.character = pose.character.adding(BeamTransform(x: x, y: y))
      pose.shadow = pose.shadow.adding(BeamTransform(x: x, scaleX: 0.78, scaleY: 0.82))
      pose.body = pose.body.adding(BeamTransform(rotation: rotation, scaleX: scaleX, scaleY: scaleY))
      return pose
    }
    return BeamEvent(
      duration: duration,
      gap: random.between(1.6, 3),
      frames: [
        .init(offset: 0, pose: base),
        .init(offset: 0.16, pose: pose(x: startX, y: 0, rotation: isCircle ? 0 : -8 * direction)),
        .init(offset: 0.34, pose: pose(x: startX * 0.4, y: -2.8, rotation: isCircle ? 105 * direction : 24 * direction, scaleX: 0.98, scaleY: 1.02)),
        .init(offset: 0.62, pose: pose(x: endX * 0.38, y: -1.2, rotation: isCircle ? 255 * direction : -18 * direction)),
        .init(offset: 0.82, pose: pose(x: endX, y: 0, rotation: isCircle ? 360 * direction : 28 * direction, scaleX: 1.025, scaleY: 0.975)),
        .init(offset: 1, pose: base),
      ]
    )
  }

  private static func peek(
    random: inout NoemaSeededRandom,
    direction: Double,
    base: BeamMotionPose
  ) -> BeamEvent {
    let vertical = random.next() < 0.5 ? -1.0 : 1.0
    let duration = random.between(4.8, 6.4)
    let cornerX = direction * random.between(27, 31)
    let cornerY = vertical * random.between(23, 27)
    var outside = base
    outside.character = outside.character.adding(BeamTransform(x: cornerX * 1.62, y: cornerY * 1.72, scaleX: 0.88, scaleY: 0.88))
    outside.shadowOpacity = 0
    var watching = base
    watching.character = watching.character.adding(BeamTransform(x: cornerX, y: cornerY, scaleX: 0.9, scaleY: 0.9))
    watching.shadowOpacity = 0
    watching.body = watching.body.adding(BeamTransform(rotation: -7 * direction))
    watching.gaze = watching.gaze.adding(BeamTransform(x: -direction * 1.8, y: -vertical * 1.15, scaleX: 0.95))
    watching.face = watching.face.adding(BeamTransform(x: -direction * 3.6, y: -vertical * 2.2, rotation: -direction * 2, scaleX: 0.97))
    return BeamEvent(
      duration: duration,
      gap: random.between(1.5, 2.8),
      frames: [
        .init(offset: 0, pose: base),
        .init(offset: 0.21, pose: outside),
        .init(offset: 0.43, pose: watching),
        .init(offset: 0.72, pose: watching),
        .init(offset: 0.84, pose: outside),
        .init(offset: 1, pose: base),
      ]
    )
  }

  private static func thinking(random: inout NoemaSeededRandom, base: BeamMotionPose) -> BeamEvent {
    let aha = random.next() < 0.2
    let duration = aha ? random.between(3.8, 5.2) : random.between(3.8, 5.8)
    let direction = random.next() < 0.5 ? -1.0 : 1.0
    if aha {
      var dip = base
      dip.character = dip.character.adding(BeamTransform(x: 0.7 * direction, y: 1.2, scaleX: 1.026, scaleY: 0.974))
      dip.face = dip.face.adding(BeamTransform(x: 5.4 * direction, y: -2, rotation: 4.2 * direction, scaleX: 0.945))
      dip.gaze = BeamTransform(x: 2.9 * direction, y: -4, scaleX: 0.94)
      var lift = base
      lift.character = lift.character.adding(BeamTransform(x: -0.35 * direction, y: -1.8, scaleX: 0.984, scaleY: 1.016))
      lift.face = lift.face.adding(BeamTransform(y: -1.6, scaleX: 1.045, scaleY: 0.955))
      return BeamEvent(duration: duration, gap: random.between(0.8, 1.6), frames: [
        .init(offset: 0, pose: base), .init(offset: 0.29, pose: dip),
        .init(offset: 0.55, pose: lift), .init(offset: 1, pose: base),
      ])
    }
    let count = random.next() < 0.48 ? 2 : 3
    var frames = [BeamKeyframe(offset: 0, pose: base)]
    let offsets = count == 2 ? [0.16, 0.58] : [0.13, 0.43, 0.72]
    for index in 0..<count {
      let sign = index % 2 == 0 ? direction : -direction
      var pose = base
      pose.gaze = BeamTransform(x: sign * random.between(2.2, 3.15), y: -random.between(2.5, 4.3), scaleX: 0.95)
      pose.face = pose.face.adding(BeamTransform(x: sign * random.between(4.5, 6.5), y: -random.between(2.1, 3.6), rotation: sign * random.between(3, 5), scaleX: 0.955))
      pose.body = pose.body.adding(BeamTransform(rotation: sign * random.between(3.3, 4.6), scaleX: index == 1 ? 0.972 : 1.035, scaleY: index == 1 ? 1.028 : 0.965))
      frames.append(.init(offset: offsets[index], pose: pose))
    }
    frames.append(.init(offset: 1, pose: base))
    return BeamEvent(duration: duration, gap: random.between(0.6, 1.4), frames: frames)
  }

  private static func speaking(random: inout NoemaSeededRandom, base: BeamMotionPose) -> BeamEvent {
    let duration = random.between(1.2, 2)
    var emphasis = base
    emphasis.character = emphasis.character.adding(BeamTransform(y: -1.2, scaleX: 1.015, scaleY: 0.985))
    emphasis.face = emphasis.face.adding(BeamTransform(y: -0.6, scaleX: 1.02, scaleY: 0.98))
    return BeamEvent(
      duration: duration,
      gap: random.between(0.18, 0.52),
      frames: [.init(offset: 0, pose: base), .init(offset: 0.52, pose: emphasis), .init(offset: 1, pose: base)]
    )
  }

  private static func interpolate(_ frames: [BeamKeyframe], progress: Double) -> BeamMotionPose {
    let clamped = min(1, max(0, progress))
    guard let nextIndex = frames.indices.dropFirst().first(where: { clamped <= frames[$0].offset }) else {
      return frames.last?.pose ?? BeamMotionPose()
    }
    let first = frames[nextIndex - 1]
    let second = frames[nextIndex]
    let local = (clamped - first.offset) / max(0.0001, second.offset - first.offset)
    return .interpolate(first.pose, second.pose, progress: smooth(local))
  }

  private static func blink(name: String, elapsed: Double) -> Double {
    let period = 2.8 + Double(NoemaAvatarSeed.hash(name + ":blink") % 240) / 100
    let local = elapsed.truncatingRemainder(dividingBy: period)
    guard local < 0.17 else { return 0 }
    let progress = local / 0.17
    if progress < 0.38 { return progress / 0.38 }
    if progress < 0.55 { return 1 }
    return 1 - (progress - 0.55) / 0.45
  }

  private static func mouthLevel(name: String, audioLevel: Double?, elapsed: Double) -> Double {
    if let audioLevel {
      if audioLevel < 0.09 { return 0 }
      if audioLevel > 0.66 { return 1 }
      return 0.45
    }
    let fallback = 0.42 + Double(NoemaAvatarSeed.hash(name) % 23) / 100
    let pulse = (sin(elapsed * 11.7 + Double(NoemaAvatarSeed.hash(name) % 17)) + 1) / 2
    return min(1, fallback * (0.35 + pulse * 1.15))
  }

  private static func smooth(_ value: Double) -> Double { value * value * (3 - 2 * value) }
  private static func easeOut(_ value: Double) -> Double { 1 - pow(1 - min(1, max(0, value)), 3) }
}

struct NoemaBeamAvatar: View {
  let name: String
  let colors: [UInt32]
  let activity: NoemaAvatarActivity
  let audioLevel: Double?
  let animated: Bool
  let square: Bool

  @State private var startedAt = Date.now
  @State private var previousActivity: NoemaAvatarActivity = .idle

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
    .onChange(of: activity) { oldValue, _ in
      previousActivity = oldValue
      startedAt = .now
    }
    .onChange(of: animated) { _, _ in startedAt = .now }
    .onChange(of: name) { _, _ in startedAt = .now }
  }

  private func canvas(elapsed: Double) -> some View {
    let palette = colors.isEmpty ? NoemaAvatarPalette.default : colors
    let seed = NoemaAvatarSeed.hash(name)
    let bodyColor = palette[seed % palette.count]
    let backgroundColor = palette[(seed + 13) % palette.count]
    let shadowColor = palette[(seed + 29) % palette.count]
    let isCircle = digit(seed, place: 1).isMultiple(of: 2)
    let faceColor: Color = contrast(bodyColor)
    let eyeSpread = (10.5 + abs(unit(seed, range: 1.5, place: 1))) * 1.15
    let eyeSize = (3.2 + Double(seed % 3) * 0.3) * 0.8
    let mouthWidth = 7.5 + Double(seed % 4)
    let faceRotation = unit(seed, range: 4, place: 3)
    let pose = animated
      ? BeamMotion.sample(
          name: name,
          activity: activity,
          previousActivity: previousActivity,
          audioLevel: audioLevel,
          elapsed: elapsed,
          isCircle: isCircle
        )
      : BeamMotion.base(activity)

    return Canvas { context, size in
      let scale = min(size.width, size.height) / 100
      context.scaleBy(x: scale, y: scale)
      context.clip(to: Path(roundedRect: CGRect(x: 0, y: 0, width: 100, height: 100), cornerRadius: square ? 0 : 50))
      context.fill(Path(CGRect(x: 0, y: 0, width: 100, height: 100)), with: .color(backgroundColor.avatarColor))

      context.drawLayer { shadow in
        shadow.concatenate(pose.shadow.affine(origin: CGPoint(x: 50, y: 88)))
        shadow.opacity = pose.shadowOpacity
        shadow.fill(
          Path(ellipseIn: CGRect(x: 25, y: 83, width: 50, height: 10)),
          with: .radialGradient(
            Gradient(colors: [.black.opacity(0.3), .black.opacity(0)]),
            center: CGPoint(x: 50, y: 88),
            startRadius: 0,
            endRadius: 25
          )
        )
      }

      context.drawLayer { character in
        if !animated, activity == .idle {
          character.concatenate(originalCharacterTransform(seed: seed, isCircle: isCircle))
        } else {
          character.concatenate(pose.character.affine(origin: CGPoint(x: 50, y: 52)))
        }
        character.drawLayer { body in
          body.concatenate(pose.body.affine(origin: CGPoint(x: 50, y: 52)))
          let sphere = GraphicsContext.Shading.radialGradient(
            Gradient(stops: [
              .init(color: bodyColor.avatarColor, location: 0.14),
              .init(color: bodyColor.avatarColor, location: 0.7),
              .init(color: shadowColor.avatarColor, location: 1),
            ]),
            center: CGPoint(x: 31, y: 23),
            startRadius: 0,
            endRadius: 78
          )
          let bodyPath = isCircle
            ? Path(ellipseIn: CGRect(x: 16, y: 18, width: 68, height: 68))
            : Path(roundedRect: CGRect(x: 16, y: 18, width: 68, height: 68), cornerRadius: 12)
          body.fill(bodyPath, with: sphere)
          body.stroke(
            bodyPath,
            with: .linearGradient(
              Gradient(colors: [.white.opacity(0.5), .white.opacity(0), .black.opacity(0.2)]),
              startPoint: CGPoint(x: 16, y: 18),
              endPoint: CGPoint(x: 84, y: 86)
            ),
            lineWidth: 1.4
          )

          body.drawLayer { face in
            face.concatenate(CGAffineTransform(translationX: 50, y: 52).rotated(by: faceRotation * .pi / 180).translatedBy(x: -50, y: -52))
            face.concatenate(pose.face.affine(origin: CGPoint(x: 50, y: 52)))
            face.drawLayer { eyes in
              eyes.concatenate(pose.gaze.affine(origin: CGPoint(x: 50, y: 48)))
              eyes.opacity = 1 - pose.blink
              eyes.fill(Path(ellipseIn: CGRect(x: 50 - eyeSpread - eyeSize, y: 44.88, width: eyeSize * 2, height: 6.24)), with: .color(faceColor))
              eyes.fill(Path(ellipseIn: CGRect(x: 50 + eyeSpread - eyeSize, y: 44.88, width: eyeSize * 2, height: 6.24)), with: .color(faceColor))
            }
            if pose.blink > 0 {
              face.opacity = pose.blink
              face.stroke(closedEye(centerX: 50 - eyeSpread), with: .color(faceColor), style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
              face.stroke(closedEye(centerX: 50 + eyeSpread), with: .color(faceColor), style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
              face.opacity = 1
            }
            drawMouth(
              in: &face,
              width: mouthWidth,
              color: faceColor,
              activity: activity,
              animated: animated,
              useHalfMoon: seed.isMultiple(of: 2),
              openness: pose.mouthOpen
            )
          }
        }
      }
      context.stroke(
        Path(roundedRect: CGRect(x: 0.6, y: 0.6, width: 98.8, height: 98.8), cornerRadius: square ? 0 : 49.4),
        with: .color(.black.opacity(0.12)),
        lineWidth: 1.2
      )
    }
  }

  private func drawMouth(
    in context: inout GraphicsContext,
    width: Double,
    color: Color,
    activity: NoemaAvatarActivity,
    animated: Bool,
    useHalfMoon: Bool,
    openness: Double
  ) {
    let centerY = 61.6
    if activity == .speaking || openness > 0.08 {
      let value = max(activity == .speaking ? 0.45 : 0, openness)
      let height = 3.2 + value * 3
      context.fill(
        Path(ellipseIn: CGRect(x: 50 - width * 0.7, y: centerY - height, width: width * 1.4, height: height * 2)),
        with: .color(color)
      )
      return
    }
    var path = Path()
    if activity == .thinking {
      let half = width * 0.52
      path.move(to: CGPoint(x: 50 - half, y: 60.6))
      path.addCurve(to: CGPoint(x: 50 + half, y: 60.6), control1: CGPoint(x: 50 - half * 0.55, y: 58.4), control2: CGPoint(x: 50 + half * 0.55, y: 58.4))
      path.addCurve(to: CGPoint(x: 50 - half, y: 60.6), control1: CGPoint(x: 50 + half * 0.68, y: 63.8), control2: CGPoint(x: 50 - half * 0.68, y: 63.8))
    } else {
      let halfMoon = !animated || useHalfMoon
      path.move(to: CGPoint(x: 50 - width, y: 60.1))
      path.addCurve(
        to: CGPoint(x: 50 + width, y: 60.1),
        control1: CGPoint(x: 50 - width * 0.65, y: halfMoon ? 60.1 : 63.6),
        control2: CGPoint(x: 50 + width * 0.65, y: halfMoon ? 60.1 : 63.6)
      )
      path.addCurve(
        to: CGPoint(x: 50 - width, y: 60.1),
        control1: CGPoint(x: 50 + width * 0.8, y: halfMoon ? 69.6 : 65.1),
        control2: CGPoint(x: 50 - width * 0.8, y: halfMoon ? 69.6 : 65.1)
      )
    }
    path.closeSubpath()
    context.fill(path, with: .color(color))
    context.stroke(path, with: .color(color), style: StrokeStyle(lineWidth: 1.1, lineJoin: .round))
  }

  private func closedEye(centerX: Double) -> Path {
    var path = Path()
    path.move(to: CGPoint(x: centerX - 3, y: 48))
    path.addQuadCurve(to: CGPoint(x: centerX + 3, y: 48), control: CGPoint(x: centerX, y: 50))
    return path
  }

  private func originalCharacterTransform(seed: Int, isCircle: Bool) -> CGAffineTransform {
    let preX = unit(seed, range: 10, place: 1)
    let preY = unit(seed, range: 10, place: 2)
    let x = preX < 5 ? preX + 4 : preX
    let y = preY < 5 ? preY + 4 : preY
    let wrapperRotation = unit(seed, range: 360)
    let rotation = isCircle ? 0 : ((wrapperRotation + 45).truncatingRemainder(dividingBy: 90) + 90).truncatingRemainder(dividingBy: 90) - 45
    let originalScale = 1 + unit(seed, range: 3) / 10
    let expansion = 100.0 / 68.0
    let unitScale = 100.0 / 36.0
    let roomToOriginal = CGAffineTransform(translationX: -16 * expansion, y: -18 * expansion).scaledBy(x: expansion, y: expansion)
    let original = CGAffineTransform(translationX: x * unitScale, y: y * unitScale)
      .rotated(by: rotation * .pi / 180)
      .scaledBy(x: originalScale, y: originalScale)
    return roomToOriginal.concatenating(original)
  }

  private func contrast(_ color: UInt32) -> Color {
    let red = Double((color >> 16) & 0xFF)
    let green = Double((color >> 8) & 0xFF)
    let blue = Double(color & 0xFF)
    return (red * 299 + green * 587 + blue * 114) / 1000 >= 128
      ? Color(avatarHex: 0x17202A)
      : .white
  }

  private func digit(_ number: Int, place: Int) -> Int {
    Int(Double(number) / pow(10, Double(place))) % 10
  }

  private func unit(_ number: Int, range: Double, place: Int? = nil) -> Double {
    let value = Double(number).truncatingRemainder(dividingBy: range)
    if let place, digit(number, place: place).isMultiple(of: 2) { return -value }
    return value
  }
}
