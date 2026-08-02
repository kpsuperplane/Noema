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

/// A compact native equivalent of the web's bounded mobile intervention sheet.
/// The explicit handle keeps the system drag indicator from changing between iOS releases.
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
    VStack(alignment: .leading, spacing: 0) {
      Capsule()
        .fill(NoemaColor.contentTertiary.opacity(0.5))
        .frame(width: 32, height: 4)
        .frame(maxWidth: .infinity)
        .padding(.top, NoemaSpacing.sm)
        .padding(.bottom, NoemaSpacing.xs)
        .accessibilityHidden(true)
      HStack(alignment: .top, spacing: NoemaSpacing.md) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text(title).font(NoemaFont.mobileTitle).foregroundStyle(NoemaColor.content)
          if let subtitle {
            Text(subtitle).font(NoemaFont.body).foregroundStyle(NoemaColor.contentSecondary)
          }
        }
        Spacer(minLength: 0)
        Button(action: onClose) {
          Image(systemName: "xmark").font(.system(size: 15, weight: .medium)).frame(width: 32, height: 32)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Close")
      }
      .padding(.horizontal, NoemaSpacing.lg)
      .padding(.top, NoemaSpacing.lg)
      .padding(.bottom, NoemaSpacing.lg)
      ScrollView {
        content
          .padding(.horizontal, NoemaSpacing.lg)
          .padding(.bottom, NoemaSpacing.lg)
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    .background(NoemaColor.surface)
    .presentationDetents(detents)
    .presentationDragIndicator(.hidden)
    .presentationCornerRadius(NoemaRadius.sheet)
    .presentationBackground(NoemaColor.surface)
  }
}
