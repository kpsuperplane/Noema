import SafariServices
import SwiftUI

struct OnboardingRootView: View {
  @Bindable var model: OnboardingModel
  let onComplete: () -> Void

  var body: some View {
    NavigationStack {
      Group {
        switch model.stage {
        case .loading:
          ProgressView("Checking setup…")
        case .chooseProvider:
          ProviderChoiceView(model: model)
        case .authenticate:
          ProviderAuthView(model: model)
        case .localModel:
          LocalModelOnboardingView(model: model)
        case .models:
          ModelConfirmationView(model: model, onComplete: onComplete)
        case let .failed(message):
          ContentUnavailableView {
            Label("Setup needs attention", systemImage: "exclamationmark.triangle")
          } description: {
            Text(message)
          } actions: {
            Button("Try again") { Task { await model.retry() } }
              .buttonStyle(.borderedProminent)
          }
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
      .background(NoemaColor.surface)
      .navigationTitle("Set up Noema")
      .navigationBarTitleDisplayMode(.inline)
    }
  }
}

private struct ProviderChoiceView: View {
  @Bindable var model: OnboardingModel

  var body: some View {
    List {
      Section {
        Text("Connect an assistant or install a local model before starting chat.")
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      Section("Assistant connections") {
        ForEach(model.accounts) { account in
          Button {
            Task { await model.choose(account) }
          } label: {
            HStack {
              VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                Text(account.displayName).font(NoemaFont.bodyEmphasized)
                Text("\(account.kind) · \(readableStatus(account.status))")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
              Spacer()
              Image(systemName: account.status == "AUTHENTICATED" ? "checkmark.circle.fill" : "arrow.right.circle")
                .foregroundStyle(account.status == "AUTHENTICATED" ? NoemaColor.success : NoemaColor.accent)
            }
          }
          .buttonStyle(.plain)
        }
      }
      let accountKinds = Set(model.accounts.map(\.kind))
      let catalogProviders = model.catalog.filter { !accountKinds.contains($0.kind) }
      if !catalogProviders.isEmpty {
        Section(model.accounts.isEmpty ? "Available providers" : "Add another provider") {
          ForEach(catalogProviders) { provider in
            Button {
              Task { await model.choose(provider) }
            } label: {
              HStack {
                VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
                  Text(provider.displayName).font(NoemaFont.bodyEmphasized)
                  Text("\(provider.kind) · Sign-in required")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
                Spacer()
                Image(systemName: "arrow.right.circle").foregroundStyle(NoemaColor.accent)
              }
            }
            .buttonStyle(.plain)
          }
        }
      } else if model.accounts.isEmpty {
        Section("Assistant connections") {
          Text("No provider account is ready yet.").foregroundStyle(NoemaColor.contentSecondary)
        }
      }
      if model.localModel?.modelName != nil {
        Section("Local inference") {
          Button {
            Task { await model.installRecommendedLocalModel() }
          } label: {
            Label("Install \(model.localModel?.modelName ?? "recommended model")", systemImage: "arrow.down.circle")
          }
        }
      }
      if let error = model.errorMessage {
        Section { Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(NoemaColor.danger) }
      }
    }
    .listStyle(.insetGrouped)
  }
}

private struct ProviderAuthView: View {
  @Bindable var model: OnboardingModel
  @State private var showSafari = false

  var body: some View {
    Group {
      if let auth = model.auth {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          Label("Sign in to continue", systemImage: "person.badge.key")
            .font(NoemaFont.title)
          Text("Noema will resume setup when the provider confirms this sign-in.")
            .foregroundStyle(NoemaColor.contentSecondary)
          if let instructions = auth.instructions { Text(instructions) }
          if let code = auth.userCode {
            LabeledContent("Code", value: code)
              .font(NoemaFont.bodyEmphasized)
          }
          if auth.status == "FAILED" || auth.status == "EXPIRED" {
            Label(auth.errorMessage ?? "The provider sign-in failed.", systemImage: "xmark.octagon")
              .foregroundStyle(NoemaColor.danger)
          } else {
            ProgressView("Waiting for provider…")
          }
          HStack {
            if auth.verificationURL != nil {
              Button("Open sign-in") { showSafari = true }
                .buttonStyle(.borderedProminent)
            }
            Button("Cancel", role: .cancel) { Task { await model.cancelAuthentication() } }
          }
          Spacer()
        }
        .padding(NoemaSpacing.lg)
        .sheet(isPresented: $showSafari, onDismiss: {
          Task { await model.refreshAuthentication() }
        }) {
          if let url = auth.verificationURL { SafariView(url: url) }
        }
        .onAppear { showSafari = auth.verificationURL != nil && auth.method == "OAUTH_PKCE" }
        .onChange(of: model.stage) { _, stage in
          if stage != .authenticate { showSafari = false }
        }
      } else {
        ProgressView()
      }
    }
  }
}

private struct LocalModelOnboardingView: View {
  @Bindable var model: OnboardingModel

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      Label("Use a local model", systemImage: "internaldrive")
        .font(NoemaFont.title)
      if let local = model.localModel {
        Text(local.modelName ?? "Recommended GGUF")
          .font(NoemaFont.bodyEmphasized)
        if let license = local.license { Text("License: \(license)").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary) }
        if let error = local.errorMessage { Label(error, systemImage: "xmark.octagon").foregroundStyle(NoemaColor.danger) }
        if let total = local.totalBytes, total > 0, local.completedBytes > 0 {
          ProgressView(value: Double(local.completedBytes), total: Double(total))
        } else if local.installationStatus == "DOWNLOADING" || local.installationStatus == "VERIFYING" {
          ProgressView()
        }
        if local.installationID != nil && local.installationStatus != "INSTALLED" {
          Button("Cancel download", role: .cancel) { Task { await model.cancelLocalInstall() } }
        } else if local.isReady {
          Button("Continue") { Task { await model.continueWithLocalModel() } }.buttonStyle(.borderedProminent)
        } else {
          Button("Install recommended model") { Task { await model.installRecommendedLocalModel() } }.buttonStyle(.borderedProminent)
        }
      }
      Spacer()
    }
    .padding(NoemaSpacing.lg)
  }
}

private struct ModelConfirmationView: View {
  @Bindable var model: OnboardingModel
  let onComplete: () -> Void

  private let rows: [(String, String, String)] = [
    ("noema", "Chat", "Your main conversational model"),
    ("simpleTasks", "Simple tasks", "Fast, routine task execution"),
    ("mediumTasks", "Medium tasks", "General task execution"),
    ("difficultTasks", "High tasks", "Complex task execution"),
    ("taskReviewer", "Task reviewer", "Reviews completed task work"),
    ("webFetchSummarizer", "Web summaries", "Condenses fetched pages"),
    ("toolProgressAudit", "Progress checks", "Checks long-running task progress"),
    ("actionReviewer", "Action reviews", "Reviews governed actions"),
    ("memoryConsolidation", "Memory updates", "Maintains long-term memory")
  ]

  var body: some View {
    List {
      Section {
        Text("Connected to \(model.modelProviderName). Review the defaults Noema will use across chat, tasks, and supporting work.")
          .foregroundStyle(NoemaColor.contentSecondary)
      }
      ForEach(rows, id: \.0) { key, title, description in
        if let draft = model.draft[key] {
          Section {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              Text(title).font(NoemaFont.bodyEmphasized)
              Text(description).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
              Picker("Model", selection: Binding<String?>(
                get: { draft.mode == "NOEMA_RECOMMENDED" ? nil : draft.profile },
                set: { value in
                  if let value { model.updateSelection(key, profile: value) }
                  else { model.updateRecommended(key) }
                }
              )) {
                Text("Noema recommended").tag(Optional<String>.none)
                ForEach(model.modelOptions.filter { $0.disabledReason == nil }) { option in
                  Text(option.label).tag(Optional(option.id))
                }
              }
              .pickerStyle(.menu)
            }
          }
        }
      }
      if let error = model.errorMessage {
        Section { Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(NoemaColor.danger) }
      }
      Section {
        Button("Confirm models and start chat") {
          Task {
            await model.confirmModels()
            if model.stage == .chooseProvider { onComplete() }
          }
        }
        .buttonStyle(.borderedProminent)
        .disabled(model.isSaving || model.draft["noema"] == nil)
      }
    }
    .listStyle(.insetGrouped)
  }
}

struct SafariView: UIViewControllerRepresentable {
  let url: URL
  func makeUIViewController(context: Context) -> SFSafariViewController { SFSafariViewController(url: url) }
  func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}

private func readableStatus(_ status: String) -> String {
  switch status {
  case "AUTHENTICATED": return "Connected"
  case "UNAUTHENTICATED": return "Sign-in required"
  case "CHECKING": return "Checking"
  case "UNAVAILABLE": return "Unavailable"
  default: return "Not checked"
  }
}
