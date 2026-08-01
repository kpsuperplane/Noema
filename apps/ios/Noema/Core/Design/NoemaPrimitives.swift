import SwiftUI

struct NoemaCard<Content: View>: View {
  var padding: CGFloat = NoemaSpacing.md
  private let content: Content

  init(padding: CGFloat = NoemaSpacing.md, @ViewBuilder content: () -> Content) {
    self.padding = padding
    self.content = content()
  }

  var body: some View {
    content
      .padding(padding)
      .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
      .overlay {
        RoundedRectangle(cornerRadius: NoemaRadius.element)
          .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
      }
  }
}

struct NoemaSectionSurface<Content: View>: View {
  let title: String?
  private let content: Content

  init(_ title: String? = nil, @ViewBuilder content: () -> Content) {
    self.title = title
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      if let title {
        Text(title)
          .font(NoemaFont.captionEmphasized)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      content
    }
    .padding(NoemaSpacing.md)
    .background(NoemaColor.surface, in: RoundedRectangle(cornerRadius: NoemaRadius.container))
    .overlay {
      RoundedRectangle(cornerRadius: NoemaRadius.container)
        .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
  }
}

struct NoemaDivider: View {
  var body: some View {
    Rectangle()
      .fill(NoemaColor.separatorSubtle)
      .frame(height: 1)
  }
}

struct NoemaStatusToken: View {
  enum Tone { case neutral, success, warning, error }

  let text: String
  var tone: Tone = .neutral

  var body: some View {
    Text(text)
      .font(NoemaFont.metadata.weight(.semibold))
      .foregroundStyle(foreground)
      .padding(.horizontal, NoemaSpacing.compact)
      .padding(.vertical, NoemaSpacing.xxs)
      .background(background, in: Capsule())
  }

  private var foreground: Color {
    switch tone {
    case .neutral: NoemaColor.contentSecondary
    case .success: NoemaColor.pine700
    case .warning: NoemaColor.clay600
    case .error: NoemaColor.red700
    }
  }

  private var background: Color {
    switch tone {
    case .neutral: NoemaColor.paper100
    case .success: NoemaColor.pine50
    case .warning: NoemaColor.clay50
    case .error: NoemaColor.red100
    }
  }
}

struct NoemaInlineState: View {
  let message: String
  var symbol: String?
  var tone: NoemaStatusToken.Tone = .neutral

  var body: some View {
    HStack(spacing: NoemaSpacing.compact) {
      if let symbol { Image(systemName: symbol).accessibilityHidden(true) }
      Text(message)
    }
    .font(NoemaFont.caption)
    .foregroundStyle(color)
    .frame(maxWidth: .infinity, alignment: .leading)
  }

  private var color: Color {
    switch tone {
    case .neutral: NoemaColor.contentSecondary
    case .success: NoemaColor.pine700
    case .warning: NoemaColor.clay600
    case .error: NoemaColor.red700
    }
  }
}

struct NoemaPageTrack<Content: View>: View {
  var maxWidth: CGFloat = 860
  var horizontalPadding: CGFloat = NoemaSpacing.xl
  private let content: Content

  init(maxWidth: CGFloat = 860, horizontalPadding: CGFloat = NoemaSpacing.xl, @ViewBuilder content: () -> Content) {
    self.maxWidth = maxWidth
    self.horizontalPadding = horizontalPadding
    self.content = content()
  }

  var body: some View {
    content
      .frame(maxWidth: maxWidth, alignment: .leading)
      .padding(.horizontal, horizontalPadding)
      .frame(maxWidth: .infinity, alignment: .center)
  }
}
