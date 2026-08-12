import SwiftUI

private struct NoemaMobileDrawerDetent: CustomPresentationDetent {
  static func height(in context: Context) -> CGFloat? {
    max(0, context.maxDetentValue - NoemaSpacing.xxl)
  }
}

enum NoemaSheetDismissControl {
  case close
  case text(String)
}

struct NoemaNativeSheet<Content: View>: View {
  let title: String
  let dismissControl: NoemaSheetDismissControl
  let dismissDisabled: Bool
  let onDismiss: () -> Void
  private let content: Content

  init(
    title: String,
    dismissControl: NoemaSheetDismissControl = .close,
    dismissDisabled: Bool = false,
    onDismiss: @escaping () -> Void,
    @ViewBuilder content: () -> Content
  ) {
    self.title = title
    self.dismissControl = dismissControl
    self.dismissDisabled = dismissDisabled
    self.onDismiss = onDismiss
    self.content = content()
  }

  var body: some View {
    NavigationStack {
      content
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
          ToolbarItem(placement: .cancellationAction) {
            switch dismissControl {
            case .close:
              Button(action: onDismiss) {
                Image(systemName: "xmark")
              }
              .accessibilityLabel("Close")
              .disabled(dismissDisabled)
            case .text(let title):
              Button(title, action: onDismiss)
                .disabled(dismissDisabled)
            }
          }
        }
    }
    .tint(NoemaColor.accent)
  }
}

extension View {
  func settingsSheetControl(focused: Bool = false) -> some View {
    font(NoemaFont.body)
      .foregroundStyle(NoemaColor.content)
      .frame(maxWidth: .infinity, minHeight: 40, alignment: .leading)
      .padding(.horizontal, NoemaSpacing.md)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .overlay {
        NoemaSuperellipse(cornerRadius: NoemaRadius.element)
          .stroke(focused ? NoemaColor.pine500 : NoemaColor.separator, lineWidth: focused ? 2 : 1)
      }
  }

  func noemaMobileDrawerPresentation() -> some View {
    presentationDetents([.custom(NoemaMobileDrawerDetent.self)])
      .presentationDragIndicator(.visible)
  }
}

struct NoemaCard<Content: View>: View {
  var padding: CGFloat = NoemaSpacing.md
  var cornerRadius: CGFloat = NoemaRadius.element
  private let content: Content

  init(
    padding: CGFloat = NoemaSpacing.md,
    cornerRadius: CGFloat = NoemaRadius.element,
    @ViewBuilder content: () -> Content
  ) {
    self.padding = padding
    self.cornerRadius = cornerRadius
    self.content = content()
  }

  var body: some View {
    content
      .padding(padding)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: cornerRadius, treatment: .container))
      .overlay {
        NoemaSuperellipse(cornerRadius: cornerRadius, treatment: .container)
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
    .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.container, treatment: .container))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.container, treatment: .container)
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
      .background(background, in: NoemaSuperellipse.full)
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

struct NoemaDeckState: View {
  let title: String
  let message: String
  let symbol: String
  var tone: NoemaStatusToken.Tone = .neutral
  var actionTitle: String?
  var action: (() -> Void)?

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(spacing: NoemaSpacing.sm) {
        Image(systemName: symbol)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(iconColor)
          .accessibilityHidden(true)
        Text(title)
          .font(NoemaFont.bodyEmphasized)
          .foregroundStyle(NoemaColor.content)
      }
      Text(message)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
      if let actionTitle, let action {
        Button(actionTitle, action: action)
          .buttonStyle(NoemaActionButtonStyle(variant: .secondary))
      }
    }
    .padding(NoemaSpacing.md)
    .frame(maxWidth: 420, minHeight: 72, alignment: .leading)
    .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element, treatment: .container))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.element, treatment: .container)
        .stroke(NoemaColor.separatorSubtle, lineWidth: 1)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
    .padding(NoemaSpacing.lg)
  }

  private var iconColor: Color {
    switch tone {
    case .neutral: NoemaColor.contentTertiary
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

enum NoemaActionButtonVariant {
  case primary
  case secondary
  case danger
  case ghost
}

struct NoemaActionButtonStyle: ButtonStyle {
  let variant: NoemaActionButtonVariant
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(NoemaFont.bodyEmphasized)
      .foregroundStyle(foreground)
      .padding(.horizontal, variant == .ghost ? NoemaSpacing.sm : NoemaSpacing.md)
      .frame(minHeight: 28)
      .background(background.opacity(configuration.isPressed ? 0.78 : 1), in: shape)
      .contentShape(Rectangle())
      .opacity(isEnabled ? 1 : 0.46)
  }

  private var shape: NoemaSuperellipse {
    NoemaSuperellipse(cornerRadius: NoemaRadius.element)
  }

  private var foreground: Color {
    switch variant {
    case .primary, .danger: NoemaColor.white
    case .secondary, .ghost: NoemaColor.content
    }
  }

  private var background: Color {
    switch variant {
    case .primary: NoemaColor.clay600
    case .danger: NoemaColor.danger
    case .secondary: NoemaColor.controlFill
    case .ghost: .clear
    }
  }
}

struct NoemaTextFieldModifier: ViewModifier {
  @Environment(\.isEnabled) private var isEnabled

  func body(content: Content) -> some View {
    content
      .font(NoemaFont.body)
      .foregroundStyle(NoemaColor.content)
      .padding(.horizontal, NoemaSpacing.sm)
      .padding(.vertical, NoemaSpacing.xs)
      .frame(minHeight: 28)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
      .overlay {
        NoemaSuperellipse(cornerRadius: NoemaRadius.element)
          .stroke(NoemaColor.content.opacity(0.24), lineWidth: 1)
      }
      .opacity(isEnabled ? 1 : 0.55)
  }
}

extension View {
  func noemaTextField() -> some View {
    modifier(NoemaTextFieldModifier())
  }
}

struct NoemaCheckboxToggleStyle: ToggleStyle {
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    Button {
      configuration.isOn.toggle()
    } label: {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        NoemaSuperellipse(cornerRadius: NoemaRadius.inner)
          .fill(configuration.isOn ? NoemaColor.clay600 : NoemaColor.surface)
          .overlay {
            NoemaSuperellipse(cornerRadius: NoemaRadius.inner)
              .stroke(configuration.isOn ? NoemaColor.clay600 : NoemaColor.content.opacity(0.24), lineWidth: 1)
          }
          .overlay {
            if configuration.isOn {
              Image(systemName: "checkmark")
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(NoemaColor.white)
            }
          }
          .frame(width: 18, height: 18)
        configuration.label
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.content)
          .multilineTextAlignment(.leading)
        Spacer(minLength: 0)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .disabled(!isEnabled)
    .opacity(isEnabled ? 1 : 0.5)
    .accessibilityValue(configuration.isOn ? "Selected" : "Not selected")
  }
}
