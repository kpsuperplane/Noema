import SwiftUI

/// Non-task interventions use the same compact decision order as Chat.
/// TaskAttention remains attached to its existing task card in the list.
struct TasksHumanInterventionsView: View {
  @Bindable var model: TasksModel
  let interventions: [HumanIntervention]
  var attachedToDock = false
  @State private var browserURL: URL?

  var body: some View {
    Group {
      if attachedToDock {
        VStack(alignment: .leading, spacing: NoemaSpacing.md) {
          ForEach(interventions) { intervention in
            content(for: intervention)
            if intervention.id != interventions.last?.id {
              Divider().overlay(NoemaColor.separatorSubtle)
            }
          }
        }
        .padding(.horizontal, NoemaSpacing.md)
        .padding(.top, NoemaSpacing.md)
        .padding(.bottom, NoemaSpacing.xxl + NoemaSpacing.md)
        .background(attachedShape.fill(NoemaColor.surface))
        .overlay { attachedShape.stroke(NoemaColor.separatorSubtle, lineWidth: 1) }
        .shadow(color: NoemaColor.ink900.opacity(0.13), radius: 14, y: -NoemaSpacing.xs)
      } else {
        VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
          ForEach(interventions) { intervention in
            card(for: intervention)
          }
        }
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .noemaSheet(isPresented: Binding(
      get: { browserURL != nil },
      set: { if !$0 { browserURL = nil } }
    ), onDismiss: {
      Task { await model.recoverConnection() }
    }) {
      if let browserURL { SafariView(url: browserURL) }
    }
  }

  @ViewBuilder
  private func card(for intervention: HumanIntervention) -> some View {
    NoemaCard(padding: NoemaSpacing.md, cornerRadius: 18) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        content(for: intervention)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  @ViewBuilder
  private func content(for intervention: HumanIntervention) -> some View {
    switch intervention {
    case let .governed(action): governed(action, intervention: intervention)
    case let .mcpAuth(auth): mcpAuth(auth)
    case let .adapterAuth(auth): adapterAuth(auth)
    case let .setup(setup): setupCard(setup)
    case let .oauthClientSetup(setup): oauthClientSetup(setup)
    case let .adapterDefinition(definition): adapterDefinition(definition)
    case .attention:
      EmptyView()
    }
  }

  private var attachedShape: NoemaSuperellipse {
    NoemaSuperellipse(
      topLeftRadius: NoemaSpacing.xxl,
      topRightRadius: NoemaSpacing.xxl,
      bottomRightRadius: 0,
      bottomLeftRadius: 0,
      treatment: .page
    )
  }

  @ViewBuilder
  private func governed(_ action: GovernedActionModel, intervention: HumanIntervention) -> some View {
    if !action.isToolEnablement {
      Label(reviewLabel(action.reviewRoute, readOnly: action.readOnly), systemImage: "hand.raised")
        .font(NoemaFont.captionEmphasized)
      Text(action.taskID == nil ? "Primary conversation" : "Background task")
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentTertiary)
    }
    ActionRequestReviewContent(action: action)
    if let failureCode = action.failureCode {
      Text(failureCode)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.danger)
    }
    interventionError(action.actionID)
    GovernedInterventionActions(
      disabled: !model.isConnected,
      approveTitle: action.isToolEnablement ? "Enable tool" : "Approve once"
    ) {
      await model.resolve(intervention, decision: $0)
    }
  }

  @ViewBuilder
  private func mcpAuth(_ auth: McpAuthModel) -> some View {
    Label("Sign-in required", systemImage: "person.badge.key")
      .font(NoemaFont.captionEmphasized)
    Text("Sign in to \(auth.serverName)")
      .font(NoemaFont.bodyEmphasized)
    Text(auth.failureCode == nil
      ? (auth.taskID == nil ? "Your request is paused until you sign in." : "This task is paused until you sign in.")
      : "The previous sign-in did not finish. Try again to continue.")
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
    Text(auth.capabilityName)
      .font(NoemaFont.monoTiny)
      .foregroundStyle(NoemaColor.contentSecondary)
    interventionError(auth.requestID)
    AuthenticationInterventionActions(
      primaryTitle: auth.state == "AUTHORIZING" ? "Open sign-in again" : "Continue in browser",
      disabled: !model.isConnected,
      primaryFirst: false,
      onStart: { browserURL = await model.startMcpAuthentication(auth) },
      onSkip: { await model.skipMcpAuthentication(auth) }
    )
  }

  @ViewBuilder
  private func adapterAuth(_ auth: AdapterAuthModel) -> some View {
    Label("Sign-in required", systemImage: "person.badge.key")
      .font(NoemaFont.captionEmphasized)
    Text("Sign in to \(auth.serviceName)")
      .font(NoemaFont.bodyEmphasized)
    Text(auth.state == "AUTHORIZING"
      ? "A sign-in was already opened. You can continue it or start again."
      : auth.taskID == nil ? "Your request is paused until you sign in." : "This task is paused until you sign in.")
      .font(NoemaFont.caption)
      .foregroundStyle(NoemaColor.contentSecondary)
    Text(auth.capabilityName)
      .font(NoemaFont.monoTiny)
      .foregroundStyle(NoemaColor.contentSecondary)
    interventionError(auth.requestID)
    AuthenticationInterventionActions(
      primaryTitle: auth.state == "AUTHORIZING" ? "Open sign-in again" : "Continue in browser",
      disabled: !model.isConnected,
      primaryFirst: false,
      onStart: { browserURL = await model.startAdapterAuthentication(auth) },
      onSkip: { await model.skipAdapterAuthentication(auth) }
    )
  }

  private func setupCard(_ setup: McpSetupModel) -> some View {
    McpSetupInterventionCard(
      setup: setup,
      isOffline: !model.isConnected,
      onOpenBrowser: { browserURL = $0 },
      onRefresh: { await model.refresh() },
      onConnectPublicly: { try await model.connectMcpPublicly(setup) },
      onStartOAuth: { try await model.startMcpSetupOAuth(setup) },
      onSavePolicy: { try await model.saveMcpPolicy($0, sharing: $1, unsafeActions: $2) },
      onResolve: { try await model.resolveMcpSetup(setup, server: $0) }
    )
  }

  private func oauthClientSetup(_ setup: AdapterOauthClientSetupModel) -> some View {
    AdapterOauthClientSetupInterventionCard(
      setup: setup,
      isOffline: !model.isConnected,
      onOpenBrowser: { browserURL = $0 },
      onDismiss: nil,
      onImport: { try await model.importAdapterOauthClient(setup, submission: $0) }
    )
  }

  private func adapterDefinition(_ definition: AdapterDefinitionModel) -> some View {
    AdapterDefinitionInterventionCard(
      definition: definition,
      isOffline: !model.isConnected,
      onOpenBrowser: { browserURL = $0 },
      onRefresh: { await model.refresh() },
      onDismiss: nil,
      onApprove: { try await model.approveAdapterDefinition(definition) },
      onCancel: { try await model.cancelAdapterDefinition(definition) },
      onSetup: { try await model.setupAdapterConnection(definition, submission: $0) },
      onStartOAuth: { try await model.startAdapterOAuth($0) },
      onWaitForOAuth: { try await model.completeAdapterOAuth($0, action: $1) },
      onAttach: { try await model.attachAdapterGrant($0) },
      onSavePolicy: { try await model.saveAdapterPolicy($0, sharing: $1, unsafeActions: $2) }
    )
  }

  private func reviewLabel(_ route: String, readOnly: Bool?) -> String {
    let behavior = readOnly == true ? "Read only" : "Can make changes"
    return route == "HUMAN_REVIEW" ? "\(behavior) · Human review" : "\(behavior) · LLM review"
  }

  @ViewBuilder
  private func interventionError(_ id: String) -> some View {
    if let message = model.interventionError(id: id) {
      Text(message)
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.danger)
        .accessibilityLabel("Action failed: \(message)")
    }
  }
}
