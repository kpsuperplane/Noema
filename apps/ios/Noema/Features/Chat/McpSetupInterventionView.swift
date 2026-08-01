import SwiftUI

struct McpSetupInterventionCard: View {
  enum PolicyStep { case sharing, unsafeActions }

  let setup: McpSetupModel
  let isOffline: Bool
  let onOpenBrowser: (URL) -> Void
  let onRefresh: () async -> Void
  let onConnectPublicly: () async throws -> McpSetupServerModel
  let onStartOAuth: () async throws -> URL
  let onSavePolicy: (McpSetupServerModel, String, String) async throws -> Void
  let onResolve: (McpSetupServerModel) async throws -> Void

  @State private var status: String
  @State private var server: McpSetupServerModel?
  @State private var policyStep: PolicyStep = .sharing
  @State private var sharing = "allow_automatically"
  @State private var unsafeActions = "reviewer_may_approve"
  @State private var isWorking = false
  @State private var connected = false
  @State private var policySaved = false
  @State private var errorMessage: String?

  init(
    setup: McpSetupModel,
    isOffline: Bool,
    onOpenBrowser: @escaping (URL) -> Void,
    onRefresh: @escaping () async -> Void,
    onConnectPublicly: @escaping () async throws -> McpSetupServerModel,
    onStartOAuth: @escaping () async throws -> URL,
    onSavePolicy: @escaping (McpSetupServerModel, String, String) async throws -> Void,
    onResolve: @escaping (McpSetupServerModel) async throws -> Void
  ) {
    self.setup = setup
    self.isOffline = isOffline
    self.onOpenBrowser = onOpenBrowser
    self.onRefresh = onRefresh
    self.onConnectPublicly = onConnectPublicly
    self.onStartOAuth = onStartOAuth
    self.onSavePolicy = onSavePolicy
    self.onResolve = onResolve
    _status = State(initialValue: setup.status)
    _server = State(initialValue: Self.server(from: setup))
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text(connected ? "MCP connected" : status == "ready_for_policy" ? "Tool permissions · \(policyStep == .sharing ? "1" : "2") of 2" : "MCP setup")
        .font(NoemaFont.captionEmphasized)
      Text(title).font(NoemaFont.bodyEmphasized)
      Text(context).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
      if status == "ready_for_policy", !connected { policyChoices }
      DisclosureGroup("Connection details") {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          if let description = setup.description { Text(description) }
          if let serviceURL = setup.serviceURL { Link("Official website", destination: serviceURL) }
          if let endpointURL = setup.endpointURL { Text(endpointURL.absoluteString).font(NoemaFont.monoTiny).textSelection(.enabled) }
        }
        .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
      }
      if let errorMessage { NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .error) }
      actions
    }
    .onChange(of: setup) { _, value in
      status = value.status
      server = Self.server(from: value) ?? server
    }
  }

  private var title: String {
    if connected { return "\(setup.displayName) is connected" }
    return status == "ready_for_policy" ? "Enable \(setup.displayName)" : "Connect \(setup.displayName)"
  }

  private var context: String {
    if connected { return "\(server?.toolCount ?? setup.discoveredToolCount) MCP tools are ready under your policy." }
    if status == "ready_for_policy" {
      return policyStep == .sharing
        ? "Your account is connected. Choose when Noema may share relevant conversation details."
        : "Choose who may approve calls that can change, delete, or send information."
    }
    if status == "authentication_available" {
      return "\(setup.discoveredToolCount) tools are public. Sign in for full access or continue with public tools only."
    }
    return "This service requires browser sign-in before Noema can discover its tools."
  }

  @ViewBuilder private var policyChoices: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      if policyStep == .sharing {
        Text("Share personal information with \(setup.displayName)?").font(NoemaFont.sectionTitle)
        option("Share when needed", selected: sharing == "allow_automatically") { sharing = "allow_automatically" }
        option("Review every time", selected: sharing == "review_every_call") {
          sharing = "review_every_call"
          if unsafeActions == "never_ask" { unsafeActions = "reviewer_may_approve" }
        }
      } else {
        Text("Who approves risky calls?").font(NoemaFont.sectionTitle)
        option("Always me", selected: unsafeActions == "always_ask") { unsafeActions = "always_ask" }
        option("Noema first", selected: unsafeActions == "reviewer_may_approve") { unsafeActions = "reviewer_may_approve" }
        option("Run automatically", selected: unsafeActions == "never_ask", disabled: sharing == "review_every_call") { unsafeActions = "never_ask" }
      }
    }
  }

  private func option(_ title: String, selected: Bool, disabled: Bool = false, action: @escaping () -> Void) -> some View {
    Button(action: action) {
      HStack { Text(title).font(NoemaFont.bodyEmphasized); Spacer(); if selected { Image(systemName: "checkmark") } }
        .padding(NoemaSpacing.sm).background(selected ? NoemaColor.pine50 : NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaRadius.inner))
        .overlay { RoundedRectangle(cornerRadius: NoemaRadius.inner).stroke(selected ? NoemaColor.accent : NoemaColor.separatorSubtle, lineWidth: 1) }
    }
    .buttonStyle(.plain).disabled(disabled || isWorking || isOffline).opacity(disabled ? 0.5 : 1)
  }

  @ViewBuilder private var actions: some View {
    if !connected {
      HStack(spacing: NoemaSpacing.sm) {
        Spacer(minLength: 0)
        if status == "ready_for_policy" {
          if policyStep == .sharing {
            Button("Continue") { policyStep = .unsafeActions }.buttonStyle(.borderedProminent)
          } else {
            Button("Back") { policyStep = .sharing }.buttonStyle(.bordered)
            Button("Enable \(setup.displayName)") { Task { await savePolicy() } }.buttonStyle(.borderedProminent)
          }
        } else {
          if status == "authentication_available" {
            Button("Use public tools only") { Task { await connectPublicly() } }.buttonStyle(.bordered)
          }
          if setup.oauthSupported {
            Button("Continue in browser") { Task { await startOAuth() } }.buttonStyle(.borderedProminent)
          }
        }
      }
      .disabled(isWorking || isOffline)
    }
  }

  private func connectPublicly() async {
    await perform { server = try await onConnectPublicly(); status = "ready_for_policy" }
  }

  private func startOAuth() async {
    await perform { onOpenBrowser(try await onStartOAuth()) }
  }

  private func savePolicy() async {
    guard let server else { errorMessage = "Noema has not finished MCP discovery."; return }
    await perform {
      if !policySaved {
        try await onSavePolicy(server, sharing, unsafeActions)
        policySaved = true
      }
      try await onResolve(server)
      connected = true
    }
  }

  private func perform(_ action: () async throws -> Void) async {
    isWorking = true; errorMessage = nil
    do { try await action(); await onRefresh() }
    catch { errorMessage = error.localizedDescription }
    isWorking = false
  }

  private static func server(from setup: McpSetupModel) -> McpSetupServerModel? {
    guard let id = setup.serverID, let revision = setup.connectionRevision, let policy = setup.policyRevision else { return nil }
    return McpSetupServerModel(serverID: id, connectionRevision: revision, policyRevision: policy, toolCount: setup.toolCount ?? setup.discoveredToolCount)
  }
}
