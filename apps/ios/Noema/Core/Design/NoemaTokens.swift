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

enum NoemaRadius {
  static let inner: CGFloat = 6
  static let element: CGFloat = 10
  static let container: CGFloat = 12
  static let page: CGFloat = 28
}

enum NoemaColor {
  static let paper50 = Color(hex: 0xFCFAF5)
  static let paper100 = Color(hex: 0xF7F2E8)
  static let paper200 = Color(hex: 0xEFE7D7)
  static let ink900 = Color(hex: 0x17160F)
  static let ink600 = Color(hex: 0x5E5A4B)
  static let ink500 = Color(hex: 0x827D6B)
  static let ink400 = Color(hex: 0xA8A38E)
  static let white = Color.white
  static let pine50 = Color(hex: 0xE9F2EC)
  static let pine100 = Color(hex: 0xC9E4D6)
  static let pine500 = Color(hex: 0x1F7A57)
  static let pine600 = Color(hex: 0x176046)
  static let pine700 = Color(hex: 0x114A37)
  static let clay50 = Color(hex: 0xFBEDE3)
  static let clay600 = Color(hex: 0xBC4E2B)
  static let red100 = Color(hex: 0xF8DBD3)
  static let red700 = Color(hex: 0x8F2A1C)
  static let blue100 = Color(hex: 0xD7E8EF)
  static let blue700 = Color(hex: 0x1D4A60)
  static let agentAvatarFill = Color(hex: 0xD6AD6B)
  static let agentAvatarInk = Color(hex: 0x2F3440)

  static let surface = white
  static let surfaceSecondary = paper100
  static let surfaceTertiary = paper200
  static let controlFill = Color(hex: 0xEDEDEB)
  static let content = ink900
  static let contentSecondary = ink600
  static let contentTertiary = ink500
  static let separator = ink900.opacity(0.14)
  static let separatorSubtle = ink900.opacity(0.08)
  static let accent = pine500
  static let success = pine600
  static let warning = clay600
  static let danger = red700
}

enum NoemaFont {
  static let body = Font.custom("Hanken Grotesk", size: 14, relativeTo: .body)
  static let bodyEmphasized = Font.custom("Hanken Grotesk", size: 14, relativeTo: .body).weight(.semibold)
  static let composer = Font.custom("Hanken Grotesk", size: 16, relativeTo: .body)
  static let navigation = Font.custom("Hanken Grotesk", size: 14, relativeTo: .body).weight(.medium)
  static let mobileTitle = Font.custom("Hanken Grotesk", size: 18, relativeTo: .headline).weight(.semibold)
  static let caption = Font.custom("Hanken Grotesk", size: 12, relativeTo: .caption)
  static let captionEmphasized = Font.custom("Hanken Grotesk", size: 12, relativeTo: .caption).weight(.semibold)
  static let sectionTitle = Font.custom("Bricolage Grotesque", size: 16, relativeTo: .headline).weight(.semibold)
  static let metadata = Font.custom("Hanken Grotesk", size: 10, relativeTo: .caption2)
  static let taskTitle = Font.custom("Hanken Grotesk", size: 13, relativeTo: .body).weight(.semibold)
  static let taskPreview = Font.custom("Hanken Grotesk", size: 11, relativeTo: .caption)
  static let taskMeta = Font.custom("Hanken Grotesk", size: 10, relativeTo: .caption2)
  static let title = Font.custom("Bricolage Grotesque", size: 18, relativeTo: .headline).weight(.semibold)
  static let pageTitle = Font.custom("Bricolage Grotesque", size: 24, relativeTo: .title2).weight(.semibold)
  static let mono = Font.custom("JetBrains Mono", size: 11, relativeTo: .caption)
  static let monoTiny = Font.custom("JetBrains Mono", size: 9, relativeTo: .caption2)
  static let article = Font.custom("Georgia", size: 15, relativeTo: .body)
  static let articleTitle = Font.custom("Georgia", size: 24, relativeTo: .title2).weight(.bold)
  static let articleHeading = Font.custom("Georgia", size: 19, relativeTo: .title3).weight(.bold)
}

enum NoemaSpring {
  static let micro = Animation.spring(response: 0.168, dampingFraction: 1)
  static let standard = Animation.spring(response: 0.251, dampingFraction: 1)
  static let surface = Animation.spring(response: 0.335, dampingFraction: 1)
}

extension Color {
  init(hex: UInt32, alpha: Double = 1) {
    self.init(
      .sRGB,
      red: Double((hex >> 16) & 0xFF) / 255,
      green: Double((hex >> 8) & 0xFF) / 255,
      blue: Double(hex & 0xFF) / 255,
      opacity: alpha
    )
  }
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
