import SwiftUI

enum NoemaSpacing {
  static let xxs: CGFloat = 2
  static let xs: CGFloat = 4
  static let compact: CGFloat = 6
  static let sm: CGFloat = 8
  static let md: CGFloat = 12
  static let lg: CGFloat = 16
  static let xl: CGFloat = 20
  static let xxl: CGFloat = 24
}

enum NoemaColor {
  static let surface = Color(uiColor: .systemBackground)
  static let surfaceSecondary = Color(uiColor: .secondarySystemBackground)
  static let surfaceTertiary = Color(uiColor: .tertiarySystemBackground)
  static let content = Color(uiColor: .label)
  static let contentSecondary = Color(uiColor: .secondaryLabel)
  static let contentTertiary = Color(uiColor: .tertiaryLabel)
  static let separator = Color(uiColor: .separator)
  static let accent = Color.accentColor
  static let success = Color(uiColor: .systemGreen)
  static let warning = Color(uiColor: .systemOrange)
  static let danger = Color(uiColor: .systemRed)
}

enum NoemaFont {
  static let body = Font.system(.body, design: .default)
  static let bodyEmphasized = Font.system(.body, design: .default).weight(.semibold)
  static let caption = Font.system(.caption, design: .default)
  static let captionEmphasized = Font.system(.caption, design: .default).weight(.semibold)
  static let title = Font.system(.title2, design: .default).weight(.semibold)
  static let mono = Font.system(.footnote, design: .monospaced)
}

enum NoemaSpring {
  static let micro = Animation.spring(response: 0.18, dampingFraction: 0.8)
  static let standard = Animation.spring(response: 0.28, dampingFraction: 0.82)
  static let surface = Animation.spring(response: 0.42, dampingFraction: 0.86)
}

struct NoemaOpaqueSurface<Content: View>: View {
  private let content: Content

  init(@ViewBuilder content: () -> Content) {
    self.content = content()
  }

  var body: some View {
    content
      .background(NoemaColor.surface)
      .compositingGroup()
  }
}

