import SwiftUI

enum NoemaCornerTreatment {
  case element
  case container
  case page
  case chat
  case full

  // Keep these values aligned with --corner-shape-* in apps/web/src/theme/noema-neutral.css.
  fileprivate var parameter: CGFloat {
    switch self {
    case .element, .page: 1.75
    case .container: 2
    case .chat: 1.5
    case .full: 1.25
    }
  }
}

struct NoemaSuperellipse: InsettableShape {
  private static let cornerSegmentCount = 16
  private static let fullRadius: CGFloat = 9_999

  private let cornerRadii: NoemaCornerRadii
  private let treatment: NoemaCornerTreatment
  private var insetAmount: CGFloat = 0

  init(cornerRadius: CGFloat, treatment: NoemaCornerTreatment = .element) {
    cornerRadii = NoemaCornerRadii(all: cornerRadius)
    self.treatment = treatment
  }

  init(
    topLeftRadius: CGFloat,
    topRightRadius: CGFloat,
    bottomRightRadius: CGFloat,
    bottomLeftRadius: CGFloat,
    treatment: NoemaCornerTreatment
  ) {
    cornerRadii = NoemaCornerRadii(
      topLeftRadius,
      topRightRadius,
      bottomRightRadius,
      bottomLeftRadius
    )
    self.treatment = treatment
  }

  static var full: NoemaSuperellipse {
    NoemaSuperellipse(cornerRadius: fullRadius, treatment: .full)
  }

  static var composer: NoemaSuperellipse {
    NoemaSuperellipse(cornerRadius: 30, treatment: .full)
  }

  func path(in rect: CGRect) -> Path {
    let insetRect = rect.insetBy(dx: insetAmount, dy: insetAmount)
    let radii = cornerRadii.inset(by: insetAmount).scaled(to: insetRect.size)
    var path = Path()
    path.move(to: CGPoint(x: insetRect.minX + radii.topLeft, y: insetRect.minY))
    path.addLine(to: CGPoint(x: insetRect.maxX - radii.topRight, y: insetRect.minY))
    addCorner(
      to: &path,
      center: CGPoint(x: insetRect.maxX - radii.topRight, y: insetRect.minY + radii.topRight),
      radius: radii.topRight,
      from: -.pi / 2,
      to: 0
    )
    path.addLine(to: CGPoint(x: insetRect.maxX, y: insetRect.maxY - radii.bottomRight))
    addCorner(
      to: &path,
      center: CGPoint(x: insetRect.maxX - radii.bottomRight, y: insetRect.maxY - radii.bottomRight),
      radius: radii.bottomRight,
      from: 0,
      to: .pi / 2
    )
    path.addLine(to: CGPoint(x: insetRect.minX + radii.bottomLeft, y: insetRect.maxY))
    addCorner(
      to: &path,
      center: CGPoint(x: insetRect.minX + radii.bottomLeft, y: insetRect.maxY - radii.bottomLeft),
      radius: radii.bottomLeft,
      from: .pi / 2,
      to: .pi
    )
    path.addLine(to: CGPoint(x: insetRect.minX, y: insetRect.minY + radii.topLeft))
    addCorner(
      to: &path,
      center: CGPoint(x: insetRect.minX + radii.topLeft, y: insetRect.minY + radii.topLeft),
      radius: radii.topLeft,
      from: .pi,
      to: .pi * 1.5
    )
    path.closeSubpath()
    return path
  }

  func inset(by amount: CGFloat) -> NoemaSuperellipse {
    var shape = self
    shape.insetAmount += amount
    return shape
  }

  private func addCorner(
    to path: inout Path,
    center: CGPoint,
    radius: CGFloat,
    from startAngle: CGFloat,
    to endAngle: CGFloat
  ) {
    guard radius > 0 else { return }
    let power = pow(2.0, treatment.parameter)
    for step in 1...Self.cornerSegmentCount {
      let progress = CGFloat(step) / CGFloat(Self.cornerSegmentCount)
      let angle = startAngle + (endAngle - startAngle) * progress
      let x = signedPower(cos(angle), power: power)
      let y = signedPower(sin(angle), power: power)
      path.addLine(to: CGPoint(x: center.x + radius * x, y: center.y + radius * y))
    }
  }

  private func signedPower(_ value: CGFloat, power: CGFloat) -> CGFloat {
    value.sign == .minus
      ? -pow(abs(value), 2 / power)
      : pow(value, 2 / power)
  }
}

private struct NoemaCornerRadii {
  let topLeft: CGFloat
  let topRight: CGFloat
  let bottomRight: CGFloat
  let bottomLeft: CGFloat

  init(_ topLeft: CGFloat, _ topRight: CGFloat, _ bottomRight: CGFloat, _ bottomLeft: CGFloat) {
    self.topLeft = topLeft
    self.topRight = topRight
    self.bottomRight = bottomRight
    self.bottomLeft = bottomLeft
  }

  init(all radius: CGFloat) {
    self.init(radius, radius, radius, radius)
  }

  func inset(by amount: CGFloat) -> NoemaCornerRadii {
    NoemaCornerRadii(
      max(0, topLeft - amount),
      max(0, topRight - amount),
      max(0, bottomRight - amount),
      max(0, bottomLeft - amount)
    )
  }

  func scaled(to size: CGSize) -> NoemaCornerRadii {
    let scale = min(
      1,
      size.width / max(topLeft + topRight, 1),
      size.width / max(bottomLeft + bottomRight, 1),
      size.height / max(topLeft + bottomLeft, 1),
      size.height / max(topRight + bottomRight, 1)
    )
    return NoemaCornerRadii(
      topLeft * scale,
      topRight * scale,
      bottomRight * scale,
      bottomLeft * scale
    )
  }
}
