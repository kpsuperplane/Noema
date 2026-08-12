import Foundation
import SwiftUI

/// Native counterpart of the mobile-web AdapterDefinition intervention card.
struct AdapterDefinitionInterventionCard: View {
  enum PolicyStep { case sharing, unsafeActions }

  let definition: AdapterDefinitionModel
  let isOffline: Bool
  let onOpenBrowser: (URL) -> Void
  let onRefresh: () async -> Void
  let onDismiss: (() -> Void)?
  let onApprove: () async throws -> Void
  let onCancel: () async throws -> Void
  let onSetup: (AdapterCredentialSubmission) async throws -> Void
  let onStartOAuth: (AdapterNextActionModel) async throws -> AdapterOAuthSetupAttempt
  let onWaitForOAuth: (AdapterOAuthSetupAttempt, AdapterNextActionModel) async throws -> String
  let onAttach: (AdapterNextActionModel) async throws -> Void
  let onSavePolicy: (AdapterConnectionModel, String, String) async throws -> Void
  @State private var policyStep: PolicyStep = .sharing
  @State private var dataSharingPolicy = "allow_automatically"
  @State private var unsafeActionPolicy = "reviewer_may_approve"
  @State private var errorMessage: String?
  @State private var authorizationExpiresAt: Date?
  @State private var authorizing = false
  @State private var isRefreshing = false
  @State private var isWorking = false
  @State private var authorizationExpired = false
  @State private var detailsPresented = false
  @State private var credentialSetupPresented = false
  @State private var policyPresented = false

  private var connection: AdapterConnectionModel? { definition.connections.first { $0.status == "authentication_required" } }
  private var policyConnection: AdapterConnectionModel? { definition.connections.first { $0.status == "active" && !$0.policyConfigured } }
  private var credentialSetup: AdapterCredentialSetupModel? { definition.credentialSetup }
  private var nextAction: AdapterNextActionModel? { definition.nextAction }
  private var oauthSetupUnavailable: Bool {
    definition.reviewed
      && nextAction?.kind == "import_application"
      && credentialSetup == nil
  }
  private var operationCount: Int { definition.operationDetails.isEmpty ? definition.operations.count : definition.operationDetails.count }
  private var isReadOnly: Bool { !definition.operationDetails.isEmpty && definition.operationDetails.allSatisfy { $0.readOnly == true } }
  private var accessLabel: String {
    if operationCount == 0 { return "No actions" }
    return isReadOnly ? "Read only" : "Can make changes"
  }
  private var visibleOperations: [AdapterOperationModel] {
    Array(definition.operationDetails.sorted { operationRiskRank($0) < operationRiskRank($1) }.prefix(5))
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      if definition.reviewed {
        HStack(spacing: NoemaSpacing.sm) {
          Text(eyebrow)
          NoemaStatusToken(text: accessLabel)
          Spacer(minLength: 0)
          if policyConnection == nil, let onDismiss {
            Button(action: onDismiss) {
              Image(systemName: "xmark")
                .font(NoemaFont.captionEmphasized)
                .frame(width: 28, height: 28)
            }
            .buttonStyle(.plain)
            .foregroundStyle(NoemaColor.contentSecondary)
            .accessibilityLabel("Hide \(definition.displayName) setup from chat")
          }
        }
        .font(NoemaFont.captionEmphasized)
      }
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        HStack(spacing: NoemaSpacing.sm) {
          Text(title).font(NoemaFont.bodyEmphasized).foregroundStyle(NoemaColor.content)
          if !definition.reviewed { NoemaStatusToken(text: accessLabel) }
        }
        if definition.reviewed {
          Text(context).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
      }
      if !definition.reviewed, !visibleOperations.isEmpty {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("What Noema can do").font(NoemaFont.captionEmphasized)
          ForEach(visibleOperations) { operation in
            HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
              Text(humanizeOperationID(operation.operationID))
              Spacer(minLength: NoemaSpacing.sm)
              Text(operationRiskLabel(operation)).foregroundStyle(NoemaColor.contentTertiary)
            }
            .font(NoemaFont.caption)
          }
          let remaining = operationCount - visibleOperations.count
          if remaining > 0 {
            Text("+\(remaining) more action\(remaining == 1 ? "" : "s") in technical details")
              .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
          }
        }
      }
      if !definition.reviewed {
        Text("This approves the setup only. You’ll connect your account next.")
          .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
      }
      if definition.reviewed, connection == nil, let redirectURI = credentialSetup?.redirectURI {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Authorized redirect URI").font(NoemaFont.captionEmphasized)
          Text("Copy this exact value into the provider's OAuth client form.")
            .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
          Text(redirectURI).font(NoemaFont.monoTiny).textSelection(.enabled)
            .padding(NoemaSpacing.sm).frame(maxWidth: .infinity, alignment: .leading)
            .background(NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaRadius.inner))
        }
      }
      Button {
        detailsPresented = true
      } label: {
        Label("Technical details", systemImage: "chevron.right")
          .font(NoemaFont.captionEmphasized)
      }
      .buttonStyle(.plain)
      .foregroundStyle(NoemaColor.contentSecondary)
      if definition.superseded {
        NoemaInlineState(message: "This definition was superseded by a newer reviewed revision.", symbol: "arrow.triangle.2.circlepath", tone: .warning)
      }
      if authorizing, let expiry = authorizationExpiresAt {
        Text("Authorization is open until \(expiry.formatted(date: .omitted, time: .shortened)).")
          .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
      }
      if authorizationExpired { NoemaInlineState(message: "Authorization expired. You can try again.", symbol: "clock.badge.exclamationmark", tone: .warning) }
      if let errorMessage { NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .error) }
      if authorizationExpired || errorMessage != nil {
        Button("Refresh interventions", systemImage: "arrow.clockwise") {
          Task { isRefreshing = true; await onRefresh(); isRefreshing = false }
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .secondary)).disabled(isRefreshing || isWorking || isOffline)
      }
      actions
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .task(id: authorizationExpiresAt) {
      guard let expiry = authorizationExpiresAt else { return }
      let wait = expiry.timeIntervalSinceNow
      if wait > 0 { try? await Task.sleep(for: .seconds(wait)) }
      guard !Task.isCancelled else { return }
      authorizing = false; authorizationExpiresAt = nil; authorizationExpired = true
    }
    .sheet(isPresented: $detailsPresented) {
      ChatInterventionSheet(title: "Technical details", subtitle: definition.displayName, detents: [.medium, .large], onClose: { detailsPresented = false }) {
        accessDetails
      }
    }
    .sheet(isPresented: $credentialSetupPresented) {
      if let credentialSetup {
        AdapterCredentialSetupSheet(
          serviceName: definition.displayName,
          setup: credentialSetup,
          scopes: definition.scopes,
          introduction: nextAction?.kind == "import_application"
            ? "Import this provider client document once. You can reuse it for more accounts and compatible APIs."
            : "Create the exact reviewed credential below. Noema stores only the declared private fields.",
          submitTitle: nextAction?.kind == "import_application" ? "Import application" : "Add connection",
          onClose: { credentialSetupPresented = false },
          onSubmit: onSetup
        )
      }
    }
    .sheet(isPresented: $policyPresented) {
      ChatInterventionSheet(
        title: "Tool permissions",
        subtitle: definition.displayName,
        detents: [.large],
        onClose: { guard !isWorking else { return }; policyPresented = false }
      ) {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          policyChoices
          policySheetActions
        }
      }
    }
  }

  private var eyebrow: String {
    if !definition.reviewed { return "Connection review" }
    if definition.reviewed, policyConnection != nil { return "Tool permissions · \(policyStep == .sharing ? "1" : "2") of 2" }
    return nextAction?.kind == "add_access" ? "Additional access" : nextAction?.kind == "reconnect_account" ? "Account recovery" : "OAuth setup"
  }

  private var title: String {
    if !definition.reviewed { return "Review \(definition.displayName)" }
    if definition.reviewed, policyConnection != nil { return "Enable \(definition.displayName)" }
    switch nextAction?.kind {
    case "add_access": return "Add \(definition.displayName) access"
    case "reconnect_account": return "Reconnect account"
    case "add_account": return "Add account"
    case "attach_account": return "Connect \(definition.displayName)"
    case "import_application": return "Set up OAuth application"
    default: return "Add credentials for \(definition.displayName)"
    }
  }

  private var context: String {
    if definition.superseded { return "A newer definition is available. Review the latest revision before changing access." }
    if definition.reviewed, policyConnection != nil { return policyStep == .sharing ? "Your account is connected. Choose when Noema may share relevant conversation details." : "Choose who may approve calls that can change, delete, or send information." }
    if oauthSetupUnavailable { return "This OAuth application cannot use the callback for this Noema app." }
    if nextAction?.kind == "attach_account" { return "Use an account that already has the required access." }
    if nextAction?.kind == "add_access" { return "Approve added access. Current account access stays available." }
    if nextAction?.kind == "add_account" { return "Use the existing OAuth application. No new client document is required." }
    if nextAction?.kind == "import_application" { return "Import one provider client document. You can reuse it later." }
    if let credentialSetup {
      return "Create a \(credentialSetup.credentialType) using the reviewed provider instructions, then add it here."
    }
    return "This definition does not require credentials."
  }

  private func operationRiskRank(_ operation: AdapterOperationModel) -> Int {
    if operation.destructive == true { return 0 }
    if operation.readOnly != true { return 1 }
    return 2
  }

  private func operationRiskLabel(_ operation: AdapterOperationModel) -> String {
    if operation.destructive == true { return "Can delete" }
    if operation.readOnly == true { return "View only" }
    return "Can make changes"
  }

  private func humanizeOperationID(_ operationID: String) -> String {
    let value = operationID
      .replacingOccurrences(of: "_", with: " ")
      .replacingOccurrences(of: "-", with: " ")
      .replacingOccurrences(of: ".", with: " ")
      .replacingOccurrences(of: ":", with: " ")
      .lowercased()
    return value.prefix(1).uppercased() + value.dropFirst()
  }

  @ViewBuilder private var accessDetails: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      technicalSection("Authentication") {
        technicalRow("Method", authenticationLabel(definition.authenticationMode))
        technicalRow("Credential", credentialLabel(definition.authenticationMode, credentialSetup?.credentialType))
        if let setupURL = credentialSetup?.setupURL {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("Setup page").font(NoemaFont.captionEmphasized)
            Link(setupURL.absoluteString, destination: setupURL)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.accent)
              .textSelection(.enabled)
          }
        }
        technicalRow("Scopes", definition.scopes.isEmpty ? "No scopes requested" : definition.scopes.joined(separator: "\n"))
        let identity = definition.operationDetails.first { $0.operationID == definition.accountIdentityOperationID }
        technicalRow(
          "Account label",
          identity.map { "\($0.method) \($0.path)" }
            ?? (definition.authenticationMode == "oauth2_authorization_code_pkce" ? "Generated connection label" : "Not configured")
        )
      }

      technicalSection("API operations") {
        Text(operationCount == 1 ? "1 operation" : "\(operationCount) operations")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        if definition.operationDetails.isEmpty {
          Text("No operations requested")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          ForEach(definition.operationDetails) { operation in
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
                Text(humanizeOperationID(operation.operationID))
                  .font(NoemaFont.captionEmphasized)
                  .foregroundStyle(NoemaColor.content)
                Text(operation.operationID)
                  .font(NoemaFont.monoTiny)
                  .foregroundStyle(NoemaColor.contentTertiary)
              }
              Text(operation.summary)
                .font(NoemaFont.monoTiny)
                .foregroundStyle(NoemaColor.content)
                .textSelection(.enabled)
              Text(operationBehaviorSummary(operation))
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              Text(operation.argumentNames.isEmpty ? "No arguments" : "Arguments: \(operation.argumentNames.joined(separator: ", "))")
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
              if operation.responseTransform != nil {
                Text("Response is normalized before it reaches the agent")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
            }
            .padding(.vertical, NoemaSpacing.xs)
          }
        }
      }

      let transformed = definition.operationDetails.filter { $0.responseTransform != nil }
      if !transformed.isEmpty {
        technicalSection("Response handling") {
          ForEach(transformed) { operation in
            if let transform = operation.responseTransform {
              DisclosureGroup("\(humanizeOperationID(operation.operationID)) · \(operation.operationID)") {
                VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                  technicalRow("Language", transform.language)
                  technicalRow("Accepted responses", transform.acceptedContentTypes.joined(separator: ", "))
                  technicalRow("Source SHA-256", transform.sourceDigest)
                  Text("Transform source").font(NoemaFont.captionEmphasized)
                  Text(transform.source).font(NoemaFont.monoTiny).textSelection(.enabled)
                  Text("Output schema").font(NoemaFont.captionEmphasized)
                  Text(transform.outputSchemaJSON).font(NoemaFont.monoTiny).textSelection(.enabled)
                }
                .padding(.top, NoemaSpacing.xs)
              }
            }
          }
        }
      }

      technicalSection("Definition") {
        technicalRow("Review status", definition.reviewed ? "Reviewed" : "Pending review")
        technicalRow("API origin", definition.origin)
        technicalRow("Revision", definition.definitionRevision)
        technicalRow("Adapter ID", definition.adapterID)
        technicalRow("Definition ID", definition.definitionID)
        technicalRow("Definition SHA-256", definition.semanticDigest)
        if let source = definition.sourceReference {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            Text("Source").font(NoemaFont.captionEmphasized)
            Link(source.absoluteString, destination: source)
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.accent)
              .textSelection(.enabled)
          }
        }
      }
    }
    .font(NoemaFont.caption)
    .foregroundStyle(NoemaColor.contentSecondary)
  }

  @ViewBuilder private func technicalSection(_ title: String, @ViewBuilder content: () -> some View) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text(title).font(NoemaFont.captionEmphasized).foregroundStyle(NoemaColor.content)
      content()
    }
  }

  @ViewBuilder private func technicalRow(_ label: String, _ value: String) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Text(label).font(NoemaFont.captionEmphasized).foregroundStyle(NoemaColor.contentSecondary)
      Text(value).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary).textSelection(.enabled)
    }
  }

  private func authenticationLabel(_ mode: String) -> String {
    switch mode {
    case "oauth2_authorization_code_pkce": "OAuth 2.0 authorization code with PKCE"
    case "credential": "Provider credential"
    case "none": "No authentication"
    default: humanizeOperationID(mode)
    }
  }

  private func credentialLabel(_ mode: String, _ type: String?) -> String {
    if let type, !type.isEmpty { return type }
    return switch mode {
    case "oauth2_authorization_code_pkce": "OAuth client"
    case "credential": "Provider credential"
    default: "None"
    }
  }

  private func operationBehaviorSummary(_ operation: AdapterOperationModel) -> String {
    [
      hintLabel(operation.readOnly, yes: "Read only", no: "Can change data", unknown: "Read behavior unknown"),
      hintLabel(operation.idempotent, yes: "Idempotent", no: "Not idempotent", unknown: "Retry behavior unknown"),
      hintLabel(operation.destructive, yes: "Destructive", no: "Non-destructive", unknown: "Destructive behavior unknown"),
      hintLabel(operation.openWorld, yes: "External interaction", no: "No external interaction", unknown: "External behavior unknown")
    ].joined(separator: " · ")
  }

  private func hintLabel(_ value: Bool?, yes: String, no: String, unknown: String) -> String {
    value == true ? yes : value == false ? no : unknown
  }

  @ViewBuilder private var policyChoices: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      if policyStep == .sharing {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Share personal information with \(definition.displayName)?").font(NoemaFont.sectionTitle)
          Text("Choose how Noema shares relevant conversation details.").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
        policyOption("Share when needed", "Relevant details → Tool runs", selected: dataSharingPolicy == "allow_automatically") { dataSharingPolicy = "allow_automatically" }
        policyOption("Review every time", "Relevant details → Approval check → Tool runs", selected: dataSharingPolicy == "review_every_call") {
          dataSharingPolicy = "review_every_call"
          if unsafeActionPolicy == "never_ask" { unsafeActionPolicy = "reviewer_may_approve" }
        }
      } else {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("Who approves risky calls?").font(NoemaFont.sectionTitle)
          Text(dataSharingPolicy == "review_every_call" ? "This applies to every call because sharing always requires review." : "Risky calls can change, delete, or send information.")
            .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
        policyOption("Always me", "Risky call → You approve → Runs", selected: unsafeActionPolicy == "always_ask") { unsafeActionPolicy = "always_ask" }
        policyOption("Noema first", "Risky call → Noema checks → You if needed", selected: unsafeActionPolicy == "reviewer_may_approve") { unsafeActionPolicy = "reviewer_may_approve" }
        policyOption("Run automatically", "Risky call → Runs", selected: unsafeActionPolicy == "never_ask", disabled: dataSharingPolicy == "review_every_call", disabledReason: "Choose “Share when needed” first.") { unsafeActionPolicy = "never_ask" }
      }
    }
  }

  @ViewBuilder private func policyOption(_ title: String, _ path: String, selected: Bool, disabled: Bool = false, disabledReason: String? = nil, action: @escaping () -> Void) -> some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
      Button(action: action) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          HStack(spacing: NoemaSpacing.sm) {
            Image(systemName: title == "Share when needed" ? "message" : title == "Review every time" ? "checkmark.shield" : title == "Always me" ? "person" : title == "Noema first" ? "shield" : "bolt")
              .foregroundStyle(selected ? NoemaColor.accent : NoemaColor.contentSecondary)
            Text(title).font(NoemaFont.bodyEmphasized); Spacer(minLength: 0)
            if selected { Image(systemName: "checkmark").foregroundStyle(NoemaColor.accent) }
          }
          Text(path).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading).padding(NoemaSpacing.md)
        .background(selected ? NoemaColor.pine50 : NoemaColor.surfaceSecondary, in: RoundedRectangle(cornerRadius: NoemaRadius.inner))
        .overlay { RoundedRectangle(cornerRadius: NoemaRadius.inner).stroke(selected ? NoemaColor.accent : NoemaColor.separatorSubtle, lineWidth: 1) }
      }
      .buttonStyle(.plain).disabled(disabled || isWorking || isOffline || definition.superseded).opacity(disabled || isWorking || isOffline || definition.superseded ? 0.5 : 1)
      if let disabledReason, disabled { Text(disabledReason).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary) }
    }
  }

  @ViewBuilder private var policySheetActions: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Spacer(minLength: 0)
      if policyStep == .sharing {
        Button("Continue") { policyStep = .unsafeActions }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      } else {
        Button("Back") { policyStep = .sharing }
          .buttonStyle(NoemaActionButtonStyle(variant: .ghost)).disabled(isWorking || isOffline || definition.superseded)
        Button("Enable \(definition.displayName)") { Task { await savePolicy() } }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      }
    }
  }

  @ViewBuilder private var actions: some View {
    HStack(spacing: NoemaSpacing.sm) {
      if definition.reviewed, policyConnection == nil, !oauthSetupUnavailable, let setupURL = credentialSetup?.setupURL {
        Link("Developer Tools", destination: setupURL).font(NoemaFont.captionEmphasized)
      } else if definition.reviewed, let source = definition.sourceReference {
        Link("Open official source", destination: source).font(NoemaFont.captionEmphasized)
      }
      Spacer(minLength: 0)
      if definition.reviewed, policyConnection != nil {
        Button("Review permissions") { policyPresented = true }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      } else if definition.reviewed, nextAction?.kind == "attach_account", let nextAction {
        Button("Connect \(definition.displayName)") { Task { await attach(nextAction) } }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      } else if !oauthSetupUnavailable, definition.reviewed,
                ["add_account", "add_access", "reconnect_account"].contains(nextAction?.kind ?? ""), let nextAction {
        Button(authorizing ? "Opening…" : actionTitle(nextAction)) { Task { await authorize(nextAction) } }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || authorizing || definition.superseded)
      } else if definition.reviewed, !oauthSetupUnavailable, credentialSetup != nil {
        Button("Add credentials") { credentialSetupPresented = true }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary))
          .disabled(isWorking || isOffline || definition.superseded)
      } else if !definition.reviewed {
        Button("Cancel") { Task { await cancel() } }
          .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
          .disabled(isWorking || isOffline || definition.superseded)
        Button("Approve") { Task { await approve() } }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      }
    }
  }

  private func approve() async {
    isWorking = true; errorMessage = nil
    defer { isWorking = false }
    do { try await onApprove() } catch { errorMessage = error.localizedDescription }
  }

  private func cancel() async {
    isWorking = true; errorMessage = nil
    defer { isWorking = false }
    do { try await onCancel() } catch { errorMessage = error.localizedDescription }
  }

  private func authorize(_ action: AdapterNextActionModel) async {
    isWorking = true; errorMessage = nil; authorizationExpired = false
    defer { isWorking = false }
    do {
      let attempt = try await onStartOAuth(action)
      authorizationExpiresAt = attempt.expiresAt; authorizing = true; onOpenBrowser(attempt.authorizationURL)
      Task {
        do {
          let status = try await onWaitForOAuth(attempt, action)
          await MainActor.run {
            authorizing = false
            authorizationExpiresAt = nil
            if status != "completed" { errorMessage = oauthFailure(status) }
          }
        } catch {
          await MainActor.run { errorMessage = error.localizedDescription }
        }
      }
    } catch {
      authorizing = false; authorizationExpiresAt = nil; errorMessage = error.localizedDescription
    }
  }

  private func attach(_ action: AdapterNextActionModel) async {
    isWorking = true; errorMessage = nil
    defer { isWorking = false }
    do { try await onAttach(action) } catch { errorMessage = error.localizedDescription }
  }

  private func actionTitle(_ action: AdapterNextActionModel) -> String {
    switch action.kind {
    case "add_access": "Add access"
    case "reconnect_account": "Reconnect account"
    default: "Add account"
    }
  }

  private func oauthFailure(_ status: String) -> String {
    switch status {
    case "denied": "Access was not approved. Current account access did not change."
    case "expired": "Account authorization expired. Current account access did not change."
    case "superseded": "A newer account authorization replaced this attempt."
    default: "Account authorization failed. Current account access did not change."
    }
  }

  private func savePolicy() async {
    guard let connection = policyConnection else { return }
    isWorking = true; errorMessage = nil
    defer { isWorking = false }
    do {
      try await onSavePolicy(connection, dataSharingPolicy, unsafeActionPolicy)
      policyPresented = false
    }
    catch { errorMessage = error.localizedDescription }
  }

}
