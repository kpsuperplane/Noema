import SwiftUI

struct NoemaAgentMark: View {
  var size: CGFloat = 24

  var body: some View {
    Canvas { context, canvasSize in
      let scale = min(canvasSize.width, canvasSize.height) / 100
      let viewport = CGRect(origin: .zero, size: canvasSize)
      let faceColor = Color(red: 0.09, green: 0.125, blue: 0.165)

      context.clip(to: Path(ellipseIn: viewport))
      context.fill(
        Path(viewport),
        with: .color(Color(red: 0.902, green: 0.847, blue: 0.769))
      )

      let shadow = CGRect(x: 25 * scale, y: 83 * scale, width: 50 * scale, height: 10 * scale)
      context.fill(Path(ellipseIn: shadow), with: .color(Color.black.opacity(0.24)))

      var character = context
      character.translateBy(x: 75 * scale, y: 75 * scale)
      character.rotate(by: .degrees(9))
      character.scaleBy(x: 1.471, y: 1.471)
      character.translateBy(x: -50 * scale, y: -52 * scale)

      let body = Path(
        roundedRect: CGRect(x: 16 * scale, y: 18 * scale, width: 68 * scale, height: 68 * scale),
        cornerRadius: 12 * scale
      )
      character.fill(
        body,
        with: .radialGradient(
          Gradient(colors: [
            Color(red: 0.839, green: 0.678, blue: 0.42),
            Color(red: 0.725, green: 0.471, blue: 0.427),
          ]),
          center: CGPoint(x: 31 * scale, y: 23 * scale),
          startRadius: 0,
          endRadius: 78 * scale
        )
      )
      character.stroke(body, with: .color(Color.white.opacity(0.2)), lineWidth: 1.4 * scale)

      var face = character
      face.translateBy(x: -1.298 * scale, y: 20.872 * scale)
      face.rotate(by: .degrees(-15))
      face.scaleBy(x: 0.68, y: 0.68)
      face.translateBy(x: 50 * scale, y: 52 * scale)
      face.rotate(by: .degrees(-3))
      face.translateBy(x: -50 * scale, y: -52 * scale)

      for eyeX in [37.925, 62.075] {
        face.fill(
          Path(
            ellipseIn: CGRect(
              x: (eyeX - 2.56) * scale,
              y: (48 - 3.12) * scale,
              width: 5.12 * scale,
              height: 6.24 * scale
            )
          ),
          with: .color(faceColor)
        )
      }

      var mouth = Path()
      mouth.move(to: CGPoint(x: 39.5 * scale, y: 60.1 * scale))
      mouth.addCurve(
        to: CGPoint(x: 60.5 * scale, y: 60.1 * scale),
        control1: CGPoint(x: 43.175 * scale, y: 60.1 * scale),
        control2: CGPoint(x: 56.825 * scale, y: 60.1 * scale)
      )
      mouth.addCurve(
        to: CGPoint(x: 39.5 * scale, y: 60.1 * scale),
        control1: CGPoint(x: 58.4 * scale, y: 69.6 * scale),
        control2: CGPoint(x: 41.6 * scale, y: 69.6 * scale)
      )
      mouth.closeSubpath()
      face.fill(mouth, with: .color(faceColor))
      face.stroke(mouth, with: .color(faceColor), lineWidth: 1.1 * scale)

      context.stroke(
        Path(ellipseIn: viewport.insetBy(dx: 0.6 * scale, dy: 0.6 * scale)),
        with: .color(Color.black.opacity(0.12)),
        lineWidth: 1.2 * scale
      )
    }
    .frame(width: size, height: size)
  }
}
