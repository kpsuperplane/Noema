import Foundation
import SwiftUI
import UniformTypeIdentifiers

/// Native counterpart of the mobile-web AdapterDefinition intervention card.
struct AdapterDefinitionInterventionCard: View {
  enum PolicyStep { case sharing, unsafeActions }

  let definition: AdapterDefinitionModel
  let isOffline: Bool
  let onOpenBrowser: (URL) -> Void
  let onRefresh: () async -> Void
  let onApprove: () async throws -> Void
  let onImportClientJSON: (Data) async throws -> Void
  let onStartOAuth: (AdapterConnectionModel) async throws -> AdapterOAuthSetupAttempt
  let onSavePolicy: (AdapterConnectionModel, String, String) async throws -> Void
  @State private var fileImporterPresented = false
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
  @State private var policyPresented = false

  private var connection: AdapterConnectionModel? { definition.connections.first { $0.status == "authentication_required" } }
  private var policyConnection: AdapterConnectionModel? { definition.connections.first { $0.status == "active" && !$0.policyConfigured } }
  private var oauthSetupUnavailable: Bool { definition.reviewed && definition.acceptsOauthClientJSON && definition.oauthRedirectURI == nil }
  private var operationCount: Int { definition.operationDetails.isEmpty ? definition.operations.count : definition.operationDetails.count }
  private var isReadOnly: Bool { definition.operationDetails.isEmpty || definition.operationDetails.allSatisfy { $0.readOnly == true } }
  private var operationSummary: String { definition.operationDetails.isEmpty ? definition.operations.joined(separator: "\n") : definition.operationDetails.map(\.summary).joined(separator: "\n") }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      HStack(spacing: NoemaSpacing.sm) {
        Text(eyebrow)
        NoemaStatusToken(text: isReadOnly ? "Read only" : "Can make changes")
      }
      .font(NoemaFont.captionEmphasized)
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text(title).font(NoemaFont.bodyEmphasized).foregroundStyle(NoemaColor.content)
        Text(context).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
      }
      if !definition.reviewed {
        Text("Approving this plan confirms the definition only. It does not connect your account or grant access yet.")
          .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentTertiary)
      }
      if definition.reviewed, connection == nil, let redirectURI = definition.oauthRedirectURI {
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
        Label("Review access details", systemImage: "chevron.right")
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
    .fileImporter(isPresented: $fileImporterPresented, allowedContentTypes: [.json], allowsMultipleSelection: false) { result in
      Task { await importClientJSON(result) }
    }
    .task(id: authorizationExpiresAt) {
      guard let expiry = authorizationExpiresAt else { return }
      let wait = expiry.timeIntervalSinceNow
      if wait > 0 { try? await Task.sleep(for: .seconds(wait)) }
      guard !Task.isCancelled else { return }
      authorizing = false; authorizationExpiresAt = nil; authorizationExpired = true
    }
    .sheet(isPresented: $detailsPresented) {
      ChatInterventionSheet(title: "Review access details", subtitle: definition.displayName, detents: [.medium, .large], onClose: { detailsPresented = false }) {
        accessDetails
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
    return connection != nil ? "Authorization" : "OAuth setup"
  }

  private var title: String {
    if !definition.reviewed { return "Review \(definition.displayName)" }
    if definition.reviewed, policyConnection != nil { return "Enable \(definition.displayName)" }
    return connection != nil ? "Connect \(definition.displayName)" : "Add credentials for \(definition.displayName)"
  }

  private var context: String {
    if definition.superseded { return "A newer definition is available. Review the latest revision before changing access." }
    if !definition.reviewed {
      let scopes = definition.scopes.isEmpty ? "without OAuth scopes" : "using \(definition.scopes.count) OAuth scope\(definition.scopes.count == 1 ? "" : "s")"
      return isReadOnly
        ? "Noema is proposing \(operationCount) API operation\(operationCount == 1 ? "" : "s") \(scopes)."
        : "Noema is proposing \(operationCount) API operation\(operationCount == 1 ? "" : "s"), including access that can make changes, \(scopes)."
    }
    if definition.reviewed, policyConnection != nil { return policyStep == .sharing ? "Your account is connected. Choose when Noema may share relevant conversation details." : "Choose who may approve calls that can change, delete, or send information." }
    if oauthSetupUnavailable { return "This connection's reviewed OAuth callback modes do not match this Noema app. Ask Noema to propose a compatible definition." }
    if connection != nil { return "Noema has the OAuth client details. Continue in your browser to grant the reviewed access." }
    return "Open the provider's developer tools in another tab, create an OAuth client, download its JSON, then choose that file here. Noema keeps only the declared client fields."
  }

  @ViewBuilder private var accessDetails: some View {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("OAuth access").font(NoemaFont.captionEmphasized)
          Text(definition.scopes.isEmpty ? "No OAuth scopes requested" : definition.scopes.joined(separator: "\n"))
        }
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text("API operations").font(NoemaFont.captionEmphasized)
          Text(operationCount == 1 ? "1 operation" : "\(operationCount) operations")
          Text(operationSummary.isEmpty ? "No operations requested" : operationSummary).font(NoemaFont.monoTiny).textSelection(.enabled)
          adapterReviewDetails
        }
        Text("Definition revision \(definition.definitionRevision) · \(definition.connectionCount) connection\(definition.connectionCount == 1 ? "" : "s")")
        if let source = definition.sourceReference { Link("Open source documentation in another tab", destination: source) }
        DisclosureGroup("Technical definition") {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            if !definition.origin.isEmpty { Text("API origin\n\(definition.origin)") }
            if let setupURL = definition.clientSetupURL { Text("OAuth client setup\n\(setupURL.absoluteString)") }
            Text("Revision\n\(definition.definitionRevision)")
            DisclosureGroup("Canonical manifest") {
              Text(definition.manifestJSON).font(NoemaFont.monoTiny).textSelection(.enabled)
            }
          }
        }
      }
      .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
  }

  @ViewBuilder private var adapterReviewDetails: some View {
    let identity = definition.operationDetails.first { $0.operationID == definition.accountIdentityOperationID }
    if let identity {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text("Account identification").font(NoemaFont.captionEmphasized)
        Text(identity.summary)
      }
    } else if definition.authenticationMode == "oauth2_authorization_code_pkce" {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text("Account identification").font(NoemaFont.captionEmphasized)
        Text("No recognizable account identifier is configured. Connections use a generated label.")
      }
    }
    let transformed = definition.operationDetails.filter { $0.responseTransform != nil }
    if !transformed.isEmpty {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        Text("Response transforms").font(NoemaFont.captionEmphasized)
        ForEach(transformed) { operation in
          if let transform = operation.responseTransform {
            DisclosureGroup("\(operation.operationID) · \(transform.language)") {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                Text("Source SHA-256\n\(transform.sourceDigest)")
                Text("Accepted media types\n\(transform.acceptedContentTypes.joined(separator: "\n"))")
                Text("Exact source").font(NoemaFont.captionEmphasized)
                Text(transform.source).font(NoemaFont.monoTiny).textSelection(.enabled)
                Text("Output schema").font(NoemaFont.captionEmphasized)
                Text(transform.outputSchemaJSON).font(NoemaFont.monoTiny).textSelection(.enabled)
              }
            }
          }
        }
      }
    }
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
      if definition.reviewed, policyConnection == nil, !oauthSetupUnavailable, let setupURL = definition.clientSetupURL {
        Link("Open developer tools", destination: setupURL).font(NoemaFont.captionEmphasized)
      } else if definition.reviewed, let source = definition.sourceReference {
        Link("Open official source", destination: source).font(NoemaFont.captionEmphasized)
      }
      Spacer(minLength: 0)
      if definition.reviewed, policyConnection != nil {
        Button("Review permissions") { policyPresented = true }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      } else if !oauthSetupUnavailable, definition.reviewed, let connection {
        Button(authorizing ? "Opening…" : "Continue in browser") { Task { await authorize(connection) } }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || authorizing || definition.superseded)
      } else if definition.reviewed, !oauthSetupUnavailable {
        Button("Choose OAuth client JSON") { fileImporterPresented = true }.buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      } else if !definition.reviewed {
        Button("Approve access plan") { Task { await approve() } }.buttonStyle(NoemaActionButtonStyle(variant: .primary)).disabled(isWorking || isOffline || definition.superseded)
      }
    }
  }

  private func approve() async {
    isWorking = true; errorMessage = nil
    defer { isWorking = false }
    do { try await onApprove() } catch { errorMessage = error.localizedDescription }
  }

  private func authorize(_ connection: AdapterConnectionModel) async {
    isWorking = true; errorMessage = nil; authorizationExpired = false
    defer { isWorking = false }
    do {
      let attempt = try await onStartOAuth(connection)
      authorizationExpiresAt = attempt.expiresAt; authorizing = true; onOpenBrowser(attempt.authorizationURL)
    } catch {
      authorizing = false; authorizationExpiresAt = nil; errorMessage = error.localizedDescription
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

  private func importClientJSON(_ result: Result<[URL], Error>) async {
    guard case let .success(urls) = result, let url = urls.first else {
      if case let .failure(error) = result { errorMessage = error.localizedDescription }
      return
    }
    isWorking = true
    defer { isWorking = false }
    let accessed = url.startAccessingSecurityScopedResource(); defer { if accessed { url.stopAccessingSecurityScopedResource() } }
    do {
      let values = try url.resourceValues(forKeys: [.fileSizeKey])
      if let size = values.fileSize, size > 32 * 1024 { throw ChatModelError.invalidOAuthClientJSON }
      try await onImportClientJSON(Data(contentsOf: url, options: .mappedIfSafe))
      errorMessage = nil
    } catch { errorMessage = error.localizedDescription }
  }
}
