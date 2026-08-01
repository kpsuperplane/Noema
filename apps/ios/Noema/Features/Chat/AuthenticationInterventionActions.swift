import SwiftUI

struct AuthenticationInterventionActions: View {
  let primaryTitle: String
  let disabled: Bool
  var primaryFirst = true
  let onStart: () async -> Void
  let onSkip: () async -> Void

  @State private var isWorking = false

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      if primaryFirst { startButton; skipButton } else { skipButton; startButton }
    }
    .disabled(disabled || isWorking)
  }

  private var startButton: some View {
    Button(isWorking ? "Opening…" : primaryTitle) { run(onStart) }
      .buttonStyle(.borderedProminent)
  }

  private var skipButton: some View {
    Button("Skip this call") { run(onSkip) }
      .buttonStyle(.bordered)
  }

  private func run(_ action: @escaping () async -> Void) {
    guard !isWorking else { return }
    isWorking = true
    Task {
      await action()
      isWorking = false
    }
  }
}

struct GovernedInterventionActions: View {
  let disabled: Bool
  let onDecision: (String) async -> Void

  @State private var isWorking = false

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Button("Decline") { decide("DECLINE") }.buttonStyle(.bordered)
      Button("Approve once") { decide("APPROVE") }.buttonStyle(.borderedProminent)
    }
    .disabled(disabled || isWorking)
  }

  private func decide(_ value: String) {
    guard !isWorking else { return }
    isWorking = true
    Task {
      await onDecision(value)
      isWorking = false
    }
  }
}
