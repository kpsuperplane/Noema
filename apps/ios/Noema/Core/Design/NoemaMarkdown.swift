import MarkdownUI
import SwiftUI
import UIKit

struct NoemaMarkdown: View {
  enum Role {
    case humanMessage
    case assistantMessage
    case body
    case secondary
    case document
  }

  private static let messageFontSize = NoemaFont.messageSize
  private static let listLineHeight: CGFloat = 20
  private static let messageLineSpacing: CGFloat = {
    let font = UIFont(name: "HankenGrotesk-Regular", size: messageFontSize)
      ?? UIFont.systemFont(ofSize: messageFontSize)
    return max(0, messageFontSize * 1.7 - font.lineHeight)
  }()
  private static let listLineSpacing: CGFloat = {
    let font = UIFont(name: "HankenGrotesk-Regular", size: messageFontSize)
      ?? UIFont.systemFont(ofSize: messageFontSize)
    return max(0, listLineHeight - font.lineHeight)
  }()

  let content: String
  let role: Role

  init(_ content: String, role: Role = .body) {
    self.content = content
    self.role = role
  }

  @ViewBuilder
  var body: some View {
    switch role {
    case .humanMessage: message(assistant: false)
    case .assistantMessage: message(assistant: true)
    case .body: body(color: NoemaColor.content)
    case .secondary: body(color: NoemaColor.contentSecondary)
    case .document: document
    }
  }

  private func message(assistant: Bool) -> some View {
    let color = assistant ? NoemaColor.content : NoemaColor.white

    return Markdown(content)
      .frame(alignment: .leading)
      .markdownTextStyle {
        FontFamily(.custom("Hanken Grotesk"))
        FontSize(Self.messageFontSize)
        TextKerning(-0.12)
        ForegroundColor(color)
      }
      .markdownTextStyle(\.link) {
        FontWeight(.semibold)
        ForegroundColor(color)
      }
      .markdownBlockStyle(\.paragraph) { configuration in
        NoemaMarkdownParagraph(
          label: configuration.label,
          lineSpacing: Self.messageLineSpacing,
          listLineSpacing: Self.listLineSpacing
        )
      }
      .noemaMarkdownListStyle(
        assistant: assistant,
        color: color,
        lineHeight: Self.listLineHeight
      )
  }

  private func body(color: Color) -> some View {
    Markdown(content)
      .markdownTextStyle {
        FontFamily(.custom("Hanken Grotesk"))
        FontSize(14)
        ForegroundColor(color)
      }
      .markdownBlockStyle(\.paragraph) { configuration in
        configuration.label
          .lineSpacing(5)
          .markdownMargin(top: 0, bottom: NoemaSpacing.sm)
      }
      .noemaMarkdownListStyle(
        assistant: true,
        color: color,
        lineHeight: Self.listLineHeight
      )
      .frame(maxWidth: .infinity, alignment: .leading)
      .textSelection(.enabled)
  }

  private var document: some View {
    Markdown(content)
      .markdownTextStyle {
        FontFamily(.custom("Georgia"))
        FontSize(15)
        ForegroundColor(NoemaColor.content)
      }
      .markdownTextStyle(\.link) {
        FontFamily(.system())
        FontSize(14)
        ForegroundColor(NoemaColor.clay600)
      }
      .markdownBlockStyle(\.paragraph) { configuration in
        configuration.label
          .lineSpacing(7.5)
          .markdownMargin(top: NoemaSpacing.lg, bottom: NoemaSpacing.sm)
      }
      .markdownBlockStyle(\.heading1) { configuration in
        NoemaMarkdownDocumentHeading(label: configuration.label, major: true)
      }
      .markdownBlockStyle(\.heading2) { configuration in
        NoemaMarkdownDocumentHeading(label: configuration.label, major: true)
      }
      .markdownBlockStyle(\.heading3) { configuration in
        NoemaMarkdownDocumentHeading(label: configuration.label, major: false)
      }
      .markdownBlockStyle(\.heading4) { configuration in
        NoemaMarkdownDocumentHeading(label: configuration.label, major: false)
      }
      .noemaMarkdownListStyle(
        assistant: true,
        color: NoemaColor.content,
        lineHeight: Self.listLineHeight
      )
      .frame(maxWidth: .infinity, alignment: .leading)
      .textSelection(.enabled)
  }
}

private struct NoemaMarkdownListKey: EnvironmentKey {
  static let defaultValue = false
}

private extension EnvironmentValues {
  var noemaMarkdownList: Bool {
    get { self[NoemaMarkdownListKey.self] }
    set { self[NoemaMarkdownListKey.self] = newValue }
  }
}

private struct NoemaMarkdownParagraph: View {
  @Environment(\.noemaMarkdownList) private var isList

  let label: BlockConfiguration.Label
  let lineSpacing: CGFloat
  let listLineSpacing: CGFloat

  var body: some View {
    label
      .lineSpacing(isList ? listLineSpacing : lineSpacing)
      .markdownMargin(top: 0, bottom: isList ? NoemaSpacing.xs : NoemaSpacing.sm)
  }
}

private struct NoemaMarkdownListLabelStyle: LabelStyle {
  func makeBody(configuration: Configuration) -> some View {
    HStack(alignment: .top, spacing: NoemaSpacing.sm) {
      configuration.icon
      configuration.title
    }
  }
}

private extension View {
  func noemaMarkdownListStyle(
    assistant: Bool,
    color: Color,
    lineHeight: CGFloat
  ) -> some View {
    self
      .markdownBlockStyle(\.list) { configuration in
        configuration.label
          .environment(\.noemaMarkdownList, true)
          .padding(.horizontal, -NoemaSpacing.sm)
          .markdownMargin(top: NoemaSpacing.xs, bottom: NoemaSpacing.xs)
      }
      .markdownBlockStyle(\.listItem) { configuration in
        configuration.label
          .labelStyle(NoemaMarkdownListLabelStyle())
          .padding(.horizontal, assistant ? 0 : NoemaSpacing.sm)
          .padding(.vertical, NoemaSpacing.xs)
          .markdownMargin(top: NoemaSpacing.xxs, bottom: NoemaSpacing.xxs)
      }
      .markdownBulletedListMarker(
        BlockStyle { _ in
          NoemaMarkdownBulletMarker(assistant: assistant, color: color)
        }
      )
      .markdownNumberedListMarker(
        BlockStyle { configuration in
          Text("\(configuration.itemNumber).")
            .font(NoemaFont.message)
            .monospacedDigit()
            .foregroundStyle(color)
            .frame(
              width: assistant ? NoemaSpacing.xxl : NoemaSpacing.lg,
              height: lineHeight,
              alignment: .topTrailing
            )
        }
      )
  }
}

private struct NoemaMarkdownBulletMarker: View {
  let assistant: Bool
  let color: Color

  var body: some View {
    let diameter: CGFloat = assistant ? NoemaSpacing.xs : NoemaSpacing.compact
    let width: CGFloat = assistant ? NoemaSpacing.xs : NoemaSpacing.lg
    ZStack {
      if assistant {
        Circle()
          .fill(NoemaColor.clay600.opacity(0.06))
          .frame(width: NoemaSpacing.sm, height: NoemaSpacing.sm)
      }
      Circle()
        .fill(assistant ? NoemaColor.clay600.opacity(0.72) : color)
        .frame(width: diameter, height: diameter)
    }
    .frame(width: width, height: NoemaSpacing.xl)
    .offset(x: assistant ? -NoemaSpacing.xxs : 0)
    .padding(.leading, assistant ? NoemaSpacing.sm + NoemaSpacing.xxs : 0)
  }
}

private struct NoemaMarkdownDocumentHeading<Label: View>: View {
  let label: Label
  let major: Bool

  var body: some View {
    label
      .markdownTextStyle {
        FontFamily(.custom("Georgia"))
        FontSize(major ? 24 : 19)
        FontWeight(.regular)
        ForegroundColor(NoemaColor.content)
      }
      .lineSpacing(2)
      .markdownMargin(top: major ? 32 : NoemaSpacing.lg, bottom: NoemaSpacing.sm)
      .padding(.bottom, 10)
      .overlay(alignment: .bottom) {
        Rectangle()
          .fill(NoemaColor.separator)
          .frame(height: 1)
      }
  }
}
