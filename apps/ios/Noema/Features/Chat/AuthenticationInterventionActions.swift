import SwiftUI

struct AuthenticationInterventionActions: View {
  let primaryTitle: String
  let disabled: Bool
  var skipDisabled = false
  /// Web cards keep the safe escape hatch before the primary browser action.
  var primaryFirst = false
  let onStart: () async -> Void
  let onSkip: () async -> Void

  @State private var isWorking = false

  var body: some View {
    HStack(spacing: NoemaSpacing.xs) {
      Spacer(minLength: 0)
      if primaryFirst { startButton; skipButton } else { skipButton; startButton }
    }
  }

  private var startButton: some View {
    Button { run(onStart) } label: {
      HStack(spacing: NoemaSpacing.xs) {
        if isWorking { ProgressView().controlSize(.small) }
        Text(isWorking ? "Opening…" : primaryTitle)
      }
    }
      .buttonStyle(NoemaActionButtonStyle(variant: .primary))
      .disabled(disabled || isWorking)
  }

  private var skipButton: some View {
    Button("Skip this call") { run(onSkip) }
      .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
      .disabled(disabled || skipDisabled || isWorking)
  }

  private func run(_ action: @escaping () async -> Void) {
    guard !isWorking else { return }
    isWorking = true
    Task {
      defer { isWorking = false }
      await action()
    }
  }
}

struct GovernedInterventionActions: View {
  let disabled: Bool
  let onDecision: (String) async -> Void

  @State private var isWorking = false

  var body: some View {
    HStack(spacing: NoemaSpacing.xs) {
      Spacer(minLength: 0)
      Button("Decline") { decide("DECLINE") }.buttonStyle(NoemaActionButtonStyle(variant: .ghost))
      Button("Approve once") { decide("APPROVE") }.buttonStyle(NoemaActionButtonStyle(variant: .primary))
    }
    .disabled(disabled || isWorking)
  }

  private func decide(_ value: String) {
    guard !isWorking else { return }
    isWorking = true
    Task {
      defer { isWorking = false }
      await onDecision(value)
    }
  }
}

struct ChatInterventionSheet<Content: View>: View {
  let title: String
  let subtitle: String?
  let detents: Set<PresentationDetent>
  let onClose: () -> Void
  private let content: Content

  init(
    title: String,
    subtitle: String? = nil,
    detents: Set<PresentationDetent> = [.medium, .large],
    onClose: @escaping () -> Void,
    @ViewBuilder content: () -> Content
  ) {
    self.title = title
    self.subtitle = subtitle
    self.detents = detents
    self.onClose = onClose
    self.content = content()
  }

  var body: some View {
    NoemaNativeSheet(title: title, onDismiss: onClose) {
      ScrollView {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          if let subtitle {
            Text(subtitle)
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          content
        }
        .padding(NoemaSpacing.lg)
      }
    }
    .presentationDetents(detents)
    .presentationDragIndicator(.visible)
  }
}
