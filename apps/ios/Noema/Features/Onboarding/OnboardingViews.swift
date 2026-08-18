import SafariServices
import SwiftUI

/// The onboarding surface intentionally uses the same content grammar as the
/// web setup frame: a centered heading followed by opaque bordered cards. It
/// does not use NavigationStack/List so the setup flow keeps its compact web
/// geometry on iPhone as well as iPad.
struct OnboardingRootView: View {
  @Bindable var model: OnboardingModel
  let onComplete: () -> Void

  var body: some View {
    Group {
      switch model.stage {
      case .loading:
        OnboardingLoadingView()
      case .chooseProvider, .authenticate, .localModel:
        ProviderChoiceView(model: model)
      case .models:
        ModelConfirmationView(model: model, onComplete: onComplete)
      case let .failed(message):
        OnboardingFailureView(message: message) { Task { await model.retry() } }
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(NoemaColor.surface)
  }
}

private struct OnboardingLoadingView: View {
  var body: some View {
    VStack(spacing: NoemaSpacing.md) {
      ProgressView()
        .tint(NoemaColor.pine500)
      Text("Checking setup…")
        .font(NoemaFont.body)
        .foregroundStyle(NoemaColor.contentSecondary)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

private struct ProviderChoiceView: View {
  @Bindable var model: OnboardingModel

  private var openRouter: ProviderCatalogModel? {
    model.catalog.first { $0.kind == "openrouter" }
  }

  private var codex: ProviderCatalogModel? {
    model.catalog.first { $0.kind == "codex" }
  }

  var body: some View {
    GeometryReader { proxy in
      ScrollView(.vertical) {
        VStack(spacing: NoemaSpacing.xl) {
          OnboardingHeading(
            eyebrow: "Choose your model provider",
            title: "Set up Noema",
            description: "Start locally, connect OpenRouter, or use Codex. You can add the other options later."
          )

          if proxy.size.width >= NoemaBreakpoint.compactMaximum {
            HStack(alignment: .top, spacing: NoemaSpacing.md) {
              localCard
              cloudCard(kind: "openrouter", provider: openRouter)
              cloudCard(kind: "codex", provider: codex)
            }
          } else {
            VStack(spacing: NoemaSpacing.sm) {
              localCard
              cloudCard(kind: "openrouter", provider: openRouter)
              cloudCard(kind: "codex", provider: codex)
            }
          }

          if let error = model.errorMessage {
            OnboardingErrorMarker(message: error)
          }
        }
        .frame(maxWidth: 1_100)
        .padding(.horizontal, proxy.size.width < NoemaBreakpoint.compactMaximum ? NoemaSpacing.lg : NoemaSpacing.xxl)
        .padding(.vertical, proxy.size.width < NoemaBreakpoint.compactMaximum ? NoemaSpacing.xxl : NoemaSpacing.xxl + NoemaSpacing.sm)
        .frame(maxWidth: .infinity)
      }
      .scrollIndicators(.hidden)
    }
  }

  private var localCard: some View {
    let local = model.localModel
    let localAccount = model.accounts.first { $0.kind == "local_models" && $0.status == "AUTHENTICATED" }
    let installationActive = isLocalTransferActive(local?.installationStatus)
    let otherChoiceActive = activeChoice != nil && activeChoice != "local_models"

    return OnboardingCard {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          HStack(alignment: .top, spacing: NoemaSpacing.md) {
            Text("Local")
              .font(NoemaFont.providerTitle)
              .onboardingTextMetrics()
            Spacer(minLength: 0)
            Image(systemName: "lock")
              .font(.system(size: 18, weight: .medium))
              .foregroundStyle(NoemaColor.pine600)
              .accessibilityHidden(true)
          }
          Text("Download a curated model and keep model traffic on this machine.")
            .font(NoemaFont.onboardingBody)
            .onboardingTextMetrics()
            .foregroundStyle(NoemaColor.contentSecondary)
        }

        localSetupContent(local)

        Spacer(minLength: 0)

        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          if local?.modelName != nil,
             !installationActive,
             local?.isReady == false,
             local?.modelID != nil {
            OnboardingButton(
              "Download \(local?.modelName ?? "recommended model")",
              symbol: "arrow.down",
              tone: .secondary,
              loading: model.localSaving,
              disabled: otherChoiceActive || model.localSaving
            ) { Task { await model.installRecommendedLocalModel() } }
          }
          if installationActive {
            OnboardingButton(
              "Cancel download",
              symbol: "xmark.circle",
              tone: .secondary,
              loading: model.localSaving,
              disabled: model.localSaving
            ) { Task { await model.cancelLocalInstall() } }
          }
          if local?.isReady == true, let localAccount {
            OnboardingButton(
              "Continue with Local",
              tone: .primary,
              loading: model.modelSetupLoadingKind == "local_models",
              disabled: otherChoiceActive || model.modelSetupLoadingKind != nil
            ) { Task { await model.choose(localAccount) } }
          }
        }
      }
    }
    .opacity(otherChoiceActive ? 0.52 : 1)
  }

  @ViewBuilder
  private func localSetupContent(_ local: LocalModelSetupModel?) -> some View {
    if model.localModel == nil {
      Text("Checking this machine…")
        .font(NoemaFont.onboardingBody)
        .onboardingTextMetrics()
        .foregroundStyle(NoemaColor.contentSecondary)
    } else if let local {
      if let modelName = local.modelName {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          Text(modelName)
            .font(NoemaFont.onboardingBodyEmphasized)
            .onboardingTextMetrics()
          HStack(spacing: NoemaSpacing.sm) {
            if let downloadGB = local.downloadGB {
              Text(formatGigabytes(downloadGB))
            }
            if let license = local.license, !license.isEmpty {
              Text(license)
            }
            if let backend = local.backend, !backend.isEmpty {
              Text(backend)
            }
          }
          .font(NoemaFont.onboardingMono)
          .onboardingTextMetrics()
          .foregroundStyle(NoemaColor.contentSecondary)
          .lineLimit(2)
          if let hardwareExplanation = local.hardwareExplanation, !hardwareExplanation.isEmpty {
            Text(hardwareExplanation)
              .font(NoemaFont.onboardingBody)
              .onboardingTextMetrics()
              .foregroundStyle(NoemaColor.contentSecondary)
          }
          if let error = local.errorMessage {
            OnboardingErrorMarker(message: error)
          }
          if let total = local.totalBytes, total > 0,
             isLocalTransferActive(local.installationStatus) {
            ProgressView(value: Double(local.completedBytes), total: Double(total))
              .tint(NoemaColor.pine500)
            Text("\(formatByteCount(local.completedBytes)) of \(formatByteCount(total))")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          } else if local.installationStatus == "DOWNLOADING" || local.installationStatus == "VERIFYING" {
            ProgressView()
              .tint(NoemaColor.pine500)
          }
          if local.installationStatus == "VERIFYING" {
            Text("Verifying model integrity…")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
      } else if !local.isReady {
        Text("No curated model fits this machine yet.")
          .font(NoemaFont.onboardingBody)
          .onboardingTextMetrics()
          .foregroundStyle(NoemaColor.contentSecondary)
      }
    }
  }

  @ViewBuilder
  private func cloudCard(kind: String, provider: ProviderCatalogModel?) -> some View {
    let account = model.accounts.first { $0.kind == kind }
    let displayName = provider?.displayName ?? account?.displayName ?? readableProvider(kind)
    let pending = model.auth?.providerKind == kind && isPending(model.auth?.status)
    let failed = model.auth?.providerKind == kind && isTerminal(model.auth?.status)
    let modelLoading = model.modelSetupLoadingKind == kind
    let disabled = activeChoice != nil && activeChoice != kind

    OnboardingCard {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        VStack(alignment: .leading, spacing: NoemaSpacing.compact) {
          Text(displayName)
            .font(NoemaFont.providerTitle)
            .onboardingTextMetrics()
          Text(description(for: kind))
            .font(NoemaFont.onboardingBody)
            .onboardingTextMetrics()
            // Core Text needs this small compensation to match the browser's two-line card rhythm.
            .frame(minHeight: kind == "codex" ? 53 : 48)
            .foregroundStyle(NoemaColor.contentSecondary)
        }

        if let account, account.status == "AUTHENTICATED" {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            Text("Connected.")
              .font(NoemaFont.onboardingBody)
              .onboardingTextMetrics()
              .foregroundStyle(NoemaColor.pine700)
            OnboardingButton(
              "Continue with \(displayName)",
              tone: .primary,
              loading: modelLoading,
              disabled: disabled || modelLoading
            ) { Task { await model.choose(account) } }
          }
        } else if pending, let auth = model.auth {
          OnboardingAuthCard(model: model, auth: auth)
        } else {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            OnboardingButton(
              "Connect \(displayName)",
              tone: kind == "openrouter" ? .primary : .secondary,
              disabled: (provider == nil && account == nil) || disabled || model.isSaving || modelLoading
            ) {
              Task {
                if let account { await model.choose(account) }
                else if let provider { await model.choose(provider) }
              }
            }
            if kind == "openrouter" {
              OpenRouterAPIKeyFallback(model: model, disabled: disabled)
            }
          }
        }

        if failed, let auth = model.auth {
          VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
            Text(auth.errorMessage ?? "Provider sign-in did not complete.")
              .font(NoemaFont.body)
              .foregroundStyle(NoemaColor.contentSecondary)
            OnboardingButton("Try again", tone: .secondary, disabled: disabled) {
              Task {
                if let account { await model.choose(account) }
                else if let provider { await model.choose(provider) }
              }
            }
          }
        }
      }
    }
    .opacity(disabled ? 0.52 : 1)
  }

  private var activeChoice: String? {
    if let modelLoading = model.modelSetupLoadingKind { return modelLoading }
    if let auth = model.auth, isPending(auth.status) { return auth.providerKind }
    if let local = model.localModel,
       isLocalTransferActive(local.installationStatus) { return "local_models" }
    return nil
  }

  private func description(for kind: String) -> String {
    switch kind {
    case "openrouter": "Connect once with PKCE and use the models allowed by your OpenRouter account."
    case "codex": "Sign in with the existing Codex device authorization flow."
    default: "Connect this provider and use its available models in Noema."
    }
  }

  private func readableProvider(_ kind: String) -> String {
    kind.replacingOccurrences(of: "_", with: " ").capitalized
  }
}

private struct OpenRouterAPIKeyFallback: View {
  @Bindable var model: OnboardingModel
  let disabled: Bool
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var isOpen = false
  @State private var key = ""

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Button {
        withAnimation(NoemaMotion.animation(NoemaSpring.micro, reduceMotion: reduceMotion)) { isOpen.toggle() }
      } label: {
        HStack(spacing: NoemaSpacing.xs) {
          Text("Use an API key instead")
          Image(systemName: isOpen ? "chevron.up" : "chevron.down")
            .font(.system(size: 10, weight: .semibold))
        }
        .font(.custom("Hanken Grotesk", size: 17, relativeTo: .body).weight(.semibold))
        .foregroundStyle(NoemaColor.content)
        .frame(maxWidth: .infinity, minHeight: 18, alignment: .leading)
      }
      .buttonStyle(.plain)
      .disabled(disabled)

      if isOpen {
        SecureField("OpenRouter API key", text: $key)
          .font(NoemaFont.body)
          .textInputAutocapitalization(.never)
          .autocorrectionDisabled()
          .padding(.horizontal, NoemaSpacing.sm)
          .frame(height: 32)
          .background(NoemaColor.paper100, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
          .overlay {
            NoemaSuperellipse(cornerRadius: NoemaRadius.element)
              .stroke(NoemaColor.separator, lineWidth: 1)
          }
        OnboardingButton(
          "Connect with API key",
          tone: .secondary,
          loading: model.isSaving,
          disabled: key.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isSaving
        ) {
          Task { await model.connectOpenRouterAPIKey(key) }
        }
      }
    }
  }
}

private struct OnboardingAuthCard: View {
  @Bindable var model: OnboardingModel
  let auth: ProviderAuthModel
  @State private var showSafari = false

  private var verificationURL: URL? {
    guard let url = auth.verificationURL, !url.absoluteString.isEmpty else { return nil }
    return url
  }

  var body: some View {
    OnboardingCard {
      VStack(alignment: .leading, spacing: NoemaSpacing.md) {
        Text(auth.status == "STARTING" ? "Starting login" : "Waiting for login")
          .font(NoemaFont.bodyEmphasized)
        if let verificationURL {
          OnboardingButton("Open login page", tone: .secondary) { showSafari = true }
            .noemaSheet(isPresented: $showSafari, onDismiss: { Task { await model.refreshAuthentication() } }) {
              SafariView(url: verificationURL)
            }
        }
        if let code = auth.userCode {
          Text(code)
            .font(.custom("JetBrains Mono", size: 18, relativeTo: .body))
            .foregroundStyle(NoemaColor.content)
            .padding(.horizontal, NoemaSpacing.sm)
            .padding(.vertical, NoemaSpacing.xs)
            .background(NoemaColor.paper100, in: NoemaSuperellipse(cornerRadius: NoemaRadius.inner))
            .textSelection(.enabled)
        }
        if let instructions = auth.instructions {
          Text(instructions)
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.content)
        }
        Text("Noema will continue automatically.")
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
        OnboardingButton("Cancel connection", tone: .ghost) {
          Task { await model.cancelAuthentication() }
        }
      }
    }
    .onAppear {
      if auth.method == "OAUTH_PKCE", verificationURL != nil { showSafari = true }
    }
    .onChange(of: model.stage) { _, stage in
      if stage != .authenticate { showSafari = false }
    }
  }
}

private struct ModelConfirmationView: View {
  @Bindable var model: OnboardingModel
  let onComplete: () -> Void
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  private let groups: [(String, [(String, String, String)])] = [
    ("Chat", [("noema", "Noema", "Your main conversational model")]),
    ("Tasks", [
      ("simpleTasks", "Simple tasks", "Fast, routine task execution"),
      ("mediumTasks", "Medium tasks", "General task execution"),
      ("difficultTasks", "High tasks", "Complex task execution"),
      ("taskReviewer", "Task reviewer", "Reviews completed task work")
    ]),
    ("Supporting work", [
      ("webFetchSummarizer", "Web summaries", "Condenses fetched pages"),
      ("toolProgressAudit", "Progress checks", "Checks long-running task progress"),
      ("actionReviewer", "Action reviews", "Reviews governed actions"),
      ("memoryConsolidation", "Memory updates", "Maintains long-term memory")
    ])
  ]

  var body: some View {
    ScrollView(.vertical) {
      VStack(spacing: NoemaSpacing.lg) {
        OnboardingHeading(
          eyebrow: "Connected to \(model.modelProviderName)",
          title: "Review your models",
          description: "These defaults cover chat, tasks, and supporting work. You can change them now or later.",
          uppercaseEyebrow: false
        )

        VStack(spacing: NoemaSpacing.md) {
          ForEach(groups, id: \.0) { group in
            OnboardingCard(padding: 0) {
              VStack(alignment: .leading, spacing: 0) {
                Text(group.0)
                  .font(NoemaFont.body)
                  .foregroundStyle(NoemaColor.contentSecondary)
                  .padding(.bottom, NoemaSpacing.sm)
                ForEach(Array(group.1.enumerated()), id: \.element.0) { index, row in
                  if index > 0 { NoemaDivider() }
                  modelRow(row)
                }
              }
              .padding(NoemaSpacing.md)
            }
          }
        }

        if let error = model.errorMessage {
          OnboardingErrorMarker(message: error)
        }

        if horizontalSizeClass == .compact {
          VStack(alignment: .trailing, spacing: NoemaSpacing.sm) {
            actionButtons
          }
          .frame(maxWidth: .infinity, alignment: .trailing)
        } else {
          HStack(spacing: NoemaSpacing.sm) {
            Spacer(minLength: 0)
            actionButtons
          }
          .frame(maxWidth: .infinity, alignment: .trailing)
        }
      }
      .frame(maxWidth: 720)
      .padding(.vertical, NoemaSpacing.xxl)
      .frame(maxWidth: .infinity)
    }
    .scrollIndicators(.hidden)
  }

  @ViewBuilder
  private var actionButtons: some View {
    OnboardingButton(
      "Use a different provider",
      tone: .secondary,
      disabled: model.isSaving
    ) {
      model.chooseDifferentProvider()
    }
    OnboardingButton(
      "Confirm models and start chat",
      tone: .primary,
      loading: model.isSaving,
      disabled: model.isSaving || !model.canConfirmModels
    ) {
      Task {
        await model.confirmModels()
        if model.stage == .chooseProvider { onComplete() }
      }
    }
  }

  @ViewBuilder
  private func modelRow(_ row: (String, String, String)) -> some View {
    let key = row.0
    let draft = model.draft[key]
    let isHumanReview = key == "actionReviewer" && draft == nil
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
        Text(row.1)
          .font(NoemaFont.bodyEmphasized)
        Text(row.2)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      if isHumanReview {
        Text("Ask me for approval")
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
      } else if let draft {
        ModelSelectionMenu(model: model, key: key, draft: draft)
      }
    }
    .padding(.vertical, NoemaSpacing.sm)
  }
}

private struct ModelSelectionMenu: View {
  @Bindable var model: OnboardingModel
  let key: String
  let draft: ModelSelectionDraft
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass

  private var selectedLabel: String {
    if draft.mode == "NOEMA_RECOMMENDED" {
      let profileID = model.recommendation(for: key)?.modelProfile
      return model.modelOptions.first { $0.id == profileID }.map { modelLabel($0.label) }
        ?? "Noema recommended"
    }
    return model.modelOptions.first { $0.id == draft.profile }.map { modelLabel($0.label) }
      ?? "Select a model"
  }

  private var reasoningOptions: [String] {
    model.reasoningOptions(for: key, draft: draft)
  }

  private var selectedReasoning: String? {
    draft.reasoning ?? model.recommendedReasoning(for: key)
  }

  var body: some View {
    Group {
      if horizontalSizeClass == .compact {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          modelMenu
          reasoningMenu
          speedControl
        }
      } else {
        HStack(spacing: NoemaSpacing.sm) {
          modelMenu
          reasoningMenu
          speedControl
        }
      }
    }
    .disabled(model.isSaving)
  }

  @ViewBuilder
  private var modelMenu: some View {
      Menu {
        Button("Noema recommended") { model.updateRecommended(key) }
          .disabled(model.recommendation(for: key)?.disabledReason != nil)
        ForEach(model.modelOptions) { option in
          Button(modelLabel(option.label)) { model.updateSelection(key, profile: option.id) }
            .disabled(option.disabledReason != nil)
        }
      } label: {
        selectionLabel(selectedLabel)
      }
      .buttonStyle(.plain)
      .frame(maxWidth: .infinity, alignment: .leading)
  }

  @ViewBuilder
  private var reasoningMenu: some View {
    if let selectedReasoning, !reasoningOptions.isEmpty {
      Menu {
        ForEach(reasoningOptions, id: \.self) { effort in
          Button(reasoningLabel(effort)) { model.updateReasoning(key, effort: effort) }
        }
      } label: {
        selectionLabel(reasoningLabel(selectedReasoning))
      }
      .buttonStyle(.plain)
      .frame(
        minWidth: horizontalSizeClass == .compact ? 0 : 112,
        maxWidth: horizontalSizeClass == .compact ? .infinity : 132,
        alignment: .leading
      )
      .disabled(draft.mode == "NOEMA_RECOMMENDED" || model.isSaving)
    }
  }

  @ViewBuilder
  private var speedControl: some View {
    if model.supportsFastMode {
      Picker("Speed", selection: Binding(
        get: { draft.fastMode },
        set: { model.updateFastMode(key, enabled: $0) }
      )) {
        Text("Standard").tag(false)
        Text("Fast").tag(true)
      }
      .pickerStyle(.segmented)
      .frame(
        minWidth: horizontalSizeClass == .compact ? 0 : 160,
        maxWidth: horizontalSizeClass == .compact ? .infinity : 180
      )
      .accessibilityLabel("Model speed")
    }
  }

  private func selectionLabel(_ title: String) -> some View {
    HStack(spacing: NoemaSpacing.sm) {
      if draft.mode == "NOEMA_RECOMMENDED" {
        Image(systemName: "sparkles")
          .font(.system(size: 12, weight: .semibold))
          .foregroundStyle(NoemaColor.pine600)
          .accessibilityHidden(true)
      }
      Text(title)
        .font(NoemaFont.body)
        .foregroundStyle(NoemaColor.content)
        .lineLimit(1)
      Spacer(minLength: NoemaSpacing.xs)
      Image(systemName: "chevron.down")
        .font(.system(size: 11, weight: .semibold))
        .foregroundStyle(NoemaColor.contentSecondary)
    }
    .padding(.horizontal, NoemaSpacing.sm)
    .frame(minHeight: 32)
    .background(NoemaColor.paper100, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.element)
        .stroke(NoemaColor.separator, lineWidth: 1)
    }
  }

  private func modelLabel(_ label: String) -> String {
    guard model.modelProviderKind == "openrouter", let separator = label.range(of: ": ") else {
      return label
    }
    return String(label[separator.upperBound...])
  }
}

private struct OnboardingHeading: View {
  let eyebrow: String
  let title: String
  let description: String
  var uppercaseEyebrow = true

  var body: some View {
    VStack(spacing: NoemaSpacing.sm) {
      Text(uppercaseEyebrow ? eyebrow.uppercased() : eyebrow)
        .font(.custom("JetBrains Mono", size: 16, relativeTo: .body))
        .onboardingTextMetrics()
        .foregroundStyle(NoemaColor.clay600)
        .tracking(1.6)
        .multilineTextAlignment(.center)
      Text(title)
        .font(NoemaFont.onboardingTitle)
        .onboardingTextMetrics()
        .foregroundStyle(NoemaColor.content)
      Text(description)
        .font(NoemaFont.onboardingBody)
        .onboardingTextMetrics()
        .foregroundStyle(NoemaColor.contentSecondary)
        .multilineTextAlignment(.center)
        .frame(maxWidth: 620)
    }
    .frame(maxWidth: .infinity)
  }
}

private struct OnboardingFailureView: View {
  let message: String
  let retry: () -> Void

  var body: some View {
    VStack(spacing: NoemaSpacing.md) {
      Image(systemName: "exclamationmark.triangle")
        .font(.system(size: 22, weight: .medium))
        .foregroundStyle(NoemaColor.clay600)
      Text("Setup needs attention")
        .font(NoemaFont.pageTitle)
      Text(message)
        .font(NoemaFont.body)
        .foregroundStyle(NoemaColor.contentSecondary)
        .multilineTextAlignment(.center)
      OnboardingButton("Try again", tone: .primary, action: retry)
    }
    .padding(NoemaSpacing.xl)
    .frame(maxWidth: 520)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

private struct OnboardingCard<Content: View>: View {
  private let content: Content
  private let padding: CGFloat

  init(padding: CGFloat = NoemaSpacing.lg, @ViewBuilder content: () -> Content) {
    self.padding = padding
    self.content = content()
  }

  var body: some View {
    content
      .padding(padding)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(NoemaColor.surface, in: NoemaSuperellipse(cornerRadius: NoemaRadius.container, treatment: .container))
      .overlay {
        NoemaSuperellipse(cornerRadius: NoemaRadius.container, treatment: .container)
          .stroke(NoemaColor.ink900.opacity(0.24), lineWidth: 1)
      }
  }
}

private struct OnboardingErrorMarker: View {
  let message: String

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
      Image(systemName: "exclamationmark.triangle.fill")
        .font(.system(size: 12, weight: .semibold))
      Text(message)
        .font(NoemaFont.body)
        .multilineTextAlignment(.leading)
    }
    .foregroundStyle(NoemaColor.red700)
    .padding(.horizontal, NoemaSpacing.sm)
    .padding(.vertical, NoemaSpacing.xs)
    .background(NoemaColor.red100.opacity(0.45), in: NoemaSuperellipse(cornerRadius: NoemaRadius.inner))
    .overlay {
      NoemaSuperellipse(cornerRadius: NoemaRadius.inner)
        .stroke(NoemaColor.red100.opacity(0.72), lineWidth: 1)
    }
  }
}

private struct OnboardingButton: View {
  enum Tone { case primary, secondary, ghost }

  let label: String
  let symbol: String?
  let tone: Tone
  let loading: Bool
  let disabled: Bool
  let action: () -> Void

  init(
    _ label: String,
    symbol: String? = nil,
    tone: Tone,
    loading: Bool = false,
    disabled: Bool = false,
    action: @escaping () -> Void
  ) {
    self.label = label
    self.symbol = symbol
    self.tone = tone
    self.loading = loading
    self.disabled = disabled
    self.action = action
  }

  var body: some View {
    Button(action: action) {
      HStack(spacing: NoemaSpacing.sm) {
        if loading {
          ProgressView()
            .controlSize(.small)
            .tint(tone == .primary ? NoemaColor.white : NoemaColor.content)
        } else if let symbol {
          Image(systemName: symbol)
            .font(.system(size: 14, weight: .medium))
        }
        Text(label)
          .lineLimit(1)
      }
      .font(NoemaFont.navigation)
      .frame(maxWidth: .infinity, minHeight: 32)
      .padding(.horizontal, NoemaSpacing.md)
      .foregroundStyle(foreground)
      .background(background, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
    }
    .buttonStyle(.plain)
    .opacity(disabled ? 0.5 : 1)
    .disabled(disabled)
    .accessibilityLabel(label)
  }

  private var foreground: Color {
    tone == .primary ? NoemaColor.white : NoemaColor.content
  }

  private var background: Color {
    switch tone {
    case .primary: NoemaColor.clay600
    case .secondary: NoemaColor.ink900.opacity(0.08)
    case .ghost: .clear
    }
  }
}

struct SafariView: UIViewControllerRepresentable {
  let url: URL
  func makeUIViewController(context: Context) -> SFSafariViewController { SFSafariViewController(url: url) }
  func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}

private extension NoemaFont {
  static let onboardingTitle = Font.custom("Hanken Grotesk", size: 16, relativeTo: .headline)
  static let onboardingBody = Font.custom("Hanken Grotesk", size: 16, relativeTo: .body)
  static let onboardingBodyEmphasized = Font.custom("Hanken Grotesk", size: 16, relativeTo: .body).weight(.bold)
  static let onboardingMono = Font.custom("JetBrains Mono", size: 16, relativeTo: .body)
  static let providerTitle = Font.custom("Hanken Grotesk", size: 16, relativeTo: .headline)
}

private extension View {
  func onboardingTextMetrics() -> some View {
    lineSpacing(NoemaSpacing.xs)
      .frame(minHeight: 24)
  }
}

private func isPending(_ status: String?) -> Bool {
  status == "STARTING" || status == "WAITING_FOR_USER"
}

private func isTerminal(_ status: String?) -> Bool {
  status == "FAILED" || status == "EXPIRED" || status == "CANCELLED"
}

private func isLocalTransferActive(_ status: String?) -> Bool {
  status == "QUEUED" || status == "DOWNLOADING" || status == "VERIFYING"
}

private func reasoningLabel(_ value: String) -> String {
  value == "XHIGH" ? "XHigh" : value.lowercased().capitalized
}

private func formatByteCount(_ value: Int) -> String {
  guard value > 0 else { return "0 bytes" }
  return ByteCountFormatter.string(fromByteCount: Int64(value), countStyle: .file)
}

private func formatGigabytes(_ value: Double) -> String {
  "\(value.formatted(.number.precision(.fractionLength(value >= 10 ? 0 : 1)))) GB download"
}
