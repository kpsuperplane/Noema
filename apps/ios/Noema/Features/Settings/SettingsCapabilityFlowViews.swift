import ApolloAPI
import NoemaAPI
import SwiftUI

struct APIConnectionSheet: View {
  let integration: SettingsIntegration
  let settings: SettingsModel
  let onAttached: (String) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var browserURL: URL?
  @State private var attemptID: String?
  @State private var selectedAction: SettingsAdapterNextAction?
  @State private var authorizationTask: Task<Void, Never>?
  @State private var isWorking = false
  @State private var errorMessage: String?

  private var definition: SettingsAdapterDefinition? {
    settings.adapterDefinitions.first { $0.semanticDigest == integration.sourceRevision }
  }

  private var action: SettingsAdapterNextAction? {
    if let selectedAction { return selectedAction }
    guard definition?.connectionActions.count == 1 else { return nil }
    return definition?.connectionActions.first
  }

  var body: some View {
    Group {
      if let definition, definition.connectionActions.count > 1, action == nil {
        actionChoiceSheet(definition: definition)
      } else if let definition, let action,
         action.kind == "import_application",
         let profileDigest = definition.oauthProfileDigest,
         let setup = definition.credentialSetup {
        AdapterCredentialSetupSheet(
          serviceName: definition.displayName,
          title: "Import OAuth client",
          setup: setup,
          scopes: definition.scopes,
          introduction: "Import this provider client document once. You can reuse it for more accounts and compatible APIs.",
          submitTitle: "Import OAuth client",
          dismissAfterSubmit: false,
          onClose: { dismiss() },
          onSubmit: { submission in
            guard let document = submission.document,
                  await settings.importAdapterOAuthApplication(profileDigest: profileDigest, document: document) else {
              throw SettingsError.server(settings.errorMessage ?? "The OAuth client could not be imported.")
            }
            await refresh()
          }
        )
      } else if let definition, let action,
                action.kind == "set_up_credential", let setup = definition.credentialSetup {
        AdapterCredentialSetupSheet(
          serviceName: definition.displayName,
          setup: setup,
          scopes: definition.scopes,
          onClose: { dismiss() },
          onSubmit: { submission in
            guard await settings.setupAdapterConnection(definition.semanticDigest, submission: submission) else {
              throw SettingsError.server(settings.errorMessage ?? "The API connection could not be added.")
            }
          }
        )
      } else if let definition, let action {
        oauthActionSheet(definition: definition, action: action)
      } else if let definition {
        SettingsBottomSheet(
          title: "Add connection to \(definition.displayName)",
          subtitle: integration.sourceSummary,
          detent: .medium,
          onClose: { dismiss() }
        ) {
          NoemaInlineState(
            message: "This definition has no reviewed credential setup compatible with this Noema app. Ask Noema to propose a compatible definition.",
            symbol: "exclamationmark.triangle",
            tone: .warning
          )
        }
      } else {
        SettingsBottomSheet(title: "Add connection to \(integration.name)", subtitle: integration.sourceSummary, detent: .medium, onClose: { dismiss() }) {
          NoemaInlineState(message: "Definition details are unavailable.", symbol: "wifi.slash", tone: .warning)
        }
      }
    }
    .noemaSheet(isPresented: Binding(
      get: { browserURL != nil },
      set: { if !$0 { browserURL = nil } }
    ), onDismiss: recoverAttempt) {
      if let browserURL { SafariView(url: browserURL) }
    }
    .onDisappear { authorizationTask?.cancel() }
  }

  private func actionChoiceSheet(definition: SettingsAdapterDefinition) -> some View {
    SettingsBottomSheet(
      title: "Connect \(definition.displayName)",
      subtitle: "Choose the account or provider setup for this API.",
      detent: .medium,
      onClose: { dismiss() }
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
        ForEach(Array(definition.connectionActions.enumerated()), id: \.offset) { index, candidate in
          if index > 0 { SettingsRowDivider() }
          Button {
            selectedAction = candidate
          } label: {
            HStack(spacing: NoemaSpacing.sm) {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                Text(choiceTitle(definition, action: candidate)).font(NoemaFont.bodyEmphasized)
                Text(actionDescription(definition, action: candidate))
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
              Spacer(minLength: NoemaSpacing.sm)
              Image(systemName: "chevron.right").foregroundStyle(NoemaColor.contentTertiary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
          }
          .buttonStyle(.plain)
          .disabled(!settings.canMutate)
        }
      }
    }
  }

  private func oauthActionSheet(
    definition: SettingsAdapterDefinition,
    action: SettingsAdapterNextAction
  ) -> some View {
    SettingsBottomSheet(
      title: actionTitle(definition, action: action),
      subtitle: integration.sourceSummary,
      detent: .medium,
      onClose: { dismiss() }
    ) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        Text(actionDescription(definition, action: action))
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.contentSecondary)
        if !action.missingScopes.isEmpty {
          DisclosureGroup("Technical details") {
            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              Text("Operations").font(NoemaFont.captionEmphasized)
              ForEach(action.operationIDs, id: \.self) { operation in
                Text(operation).font(NoemaFont.monoTiny).textSelection(.enabled)
              }
              Text("Dependent APIs").font(NoemaFont.captionEmphasized)
              ForEach(dependentAPINames(action), id: \.self) { name in
                Text(name).font(NoemaFont.caption)
              }
              Text("New OAuth scopes").font(NoemaFont.captionEmphasized)
              ForEach(action.missingScopes, id: \.self) { scope in
                Text(scope).font(NoemaFont.monoTiny).textSelection(.enabled)
              }
            }
            .padding(.top, NoemaSpacing.sm)
          }
        }
        if let errorMessage {
          NoemaInlineState(message: errorMessage, symbol: "exclamationmark.triangle", tone: .error)
        }
        HStack(spacing: NoemaSpacing.sm) {
          Spacer(minLength: 0)
          Button("Cancel") { dismiss() }
            .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
            .disabled(isWorking)
          Button(actionTitle(definition, action: action)) {
            Task { await run(action) }
          }
          .buttonStyle(NoemaActionButtonStyle(variant: .primary))
          .disabled(isWorking || !settings.canMutate)
        }
      }
    }
  }

  private func run(_ action: SettingsAdapterNextAction) async {
    guard !isWorking else { return }
    isWorking = true
    errorMessage = nil
    defer { isWorking = false }
    if action.kind == "attach_account" {
      if let connectionID = await settings.attachAdapterGrant(action) {
        onAttached(connectionID)
        dismiss()
      } else { errorMessage = settings.errorMessage ?? "The account could not be attached." }
      return
    }
    guard let attempt = await settings.startAdapterOAuth(action) else {
      errorMessage = settings.errorMessage ?? "Account authorization could not start."
      return
    }
    attemptID = attempt.attemptID
    browserURL = attempt.authorizationURL
    authorizationTask?.cancel()
    authorizationTask = Task {
      guard let result = await settings.waitForAdapterOAuth(attemptID: attempt.attemptID) else { return }
      await finish(result, action: action)
    }
  }

  private func recoverAttempt() {
    guard let attemptID, let action else { return }
    Task {
      guard let result = await settings.adapterOAuthAttempt(attemptID: attemptID),
            result.status != "authorizing" else { return }
      await finish(result, action: action)
    }
  }

  @MainActor
  private func finish(
    _ result: (status: String, grantID: String?, grantRevision: Int?),
    action: SettingsAdapterNextAction
  ) async {
    guard result.status == "completed" else {
      errorMessage = oauthFailure(result.status)
      return
    }
    if action.connectionID == nil {
      guard let grantID = result.grantID, let grantRevision = result.grantRevision else {
        errorMessage = "The account was authorized, but Noema did not return its exact revision."
        return
      }
      guard let connectionID = await settings.attachAdapterGrant(
        action,
        grantID: grantID,
        grantRevision: grantRevision
      ) else {
        errorMessage = settings.errorMessage ?? "The account was authorized, but the API could not be attached."
        return
      }
      onAttached(connectionID)
    }
    await refresh()
    dismiss()
  }

  private func refresh() async {
    await settings.loadAdapterDefinitions()
    await settings.loadAdapterOAuthState()
  }

  private func actionTitle(_ definition: SettingsAdapterDefinition, action: SettingsAdapterNextAction) -> String {
    switch action.kind {
    case "attach_account": "Connect \(definition.displayName)"
    case "add_access": "Add \(definition.displayName) access"
    case "reconnect_account": "Reconnect account"
    case "add_account":
      settings.adapterOAuthState?.applications
        .first { $0.applicationID == action.applicationID }
        .map { "Add \($0.providerName) account" } ?? "Add account"
    default: "Continue"
    }
  }

  private func choiceTitle(_ definition: SettingsAdapterDefinition, action: SettingsAdapterNextAction) -> String {
    let grant = settings.adapterOAuthState?.grants.first { $0.grantID == action.grantID }
    let account = grant?.accountLabel ?? (grant == nil ? nil : "Unlabeled account")
    return switch action.kind {
    case "attach_account": account.map { "Connect \($0)" } ?? "Connect \(definition.displayName)"
    case "add_access": account.map { "Add access for \($0)" } ?? "Add \(definition.displayName) access"
    case "reconnect_account": account.map { "Reconnect \($0)" } ?? "Reconnect account"
    case "add_account":
      if let application = settings.adapterOAuthState?.applications
        .first(where: { $0.applicationID == action.applicationID }) {
        application.projectLabel.map { "Add account with \($0)" }
          ?? "Add \(application.providerName) account"
      } else {
        "Add account"
      }
    case "import_application": "Set up \(definition.displayName)"
    default: actionTitle(definition, action: action)
    }
  }

  private func actionDescription(_ definition: SettingsAdapterDefinition, action: SettingsAdapterNextAction) -> String {
    switch action.kind {
    case "attach_account": "Use an account that already has the required access."
    case "add_access": "This enables \(action.operationIDs.count) additional \(definition.displayName) operations. Current access stays available if you cancel or deny consent."
    case "reconnect_account": "Reconnect this account. API definitions and local policy stay unchanged."
    default: "Use the existing provider setup. No new client document is required."
    }
  }

  private func dependentAPINames(_ action: SettingsAdapterNextAction) -> [String] {
    guard let grant = settings.adapterOAuthState?.grants.first(where: { $0.grantID == action.grantID }) else {
      return ["No connected APIs"]
    }
    let connectionIDs = Set(grant.connectionIDs)
    let names = Set(settings.adapterDefinitions.flatMap { definition in
      definition.connections.contains { connectionIDs.contains($0.id) } ? [definition.displayName] : []
    })
    return names.isEmpty ? ["No connected APIs"] : names.sorted()
  }

  private func oauthFailure(_ status: String) -> String {
    switch status {
    case "denied": "Access was not approved. Current account access did not change."
    case "expired": "Account authorization expired. Current account access did not change."
    case "superseded": "A newer account authorization replaced this attempt."
    default: "Account authorization failed. Current account access did not change."
    }
  }
}

struct SettingsAdapterDefinitionReview: View {
  let definition: SettingsAdapterDefinition
  let settings: SettingsModel
  @State private var detailsExpanded = false
  @State private var isWorking = false
  @State private var errorMessage: String?

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.md) {
      HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          Text(definition.displayName).font(NoemaFont.bodyEmphasized)
          Text("\(definition.operations.count) operation\(definition.operations.count == 1 ? "" : "s") · \(definition.scopes.count) OAuth scope\(definition.scopes.count == 1 ? "" : "s")")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        Spacer(minLength: NoemaSpacing.sm)
        NoemaStatusToken(text: definition.superseded ? "Superseded" : "Needs review", tone: .warning)
      }

      DisclosureGroup("Review API details", isExpanded: $detailsExpanded) {
        VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
          authenticationDetails
          operationDetails
          responseDetails
          definitionDetails
        }
        .padding(.top, NoemaSpacing.sm)
      }
      .font(NoemaFont.captionEmphasized)

      if let errorMessage {
        Text(errorMessage)
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.danger)
          .fixedSize(horizontal: false, vertical: true)
      }

      HStack(spacing: NoemaSpacing.sm) {
        Spacer(minLength: 0)
        Button("Cancel") { cancel() }
          .buttonStyle(NoemaActionButtonStyle(variant: .ghost))
          .disabled(isWorking || !settings.canMutate)
        Button {
          approve()
        } label: {
          HStack(spacing: NoemaSpacing.xs) {
            if isWorking { ProgressView().controlSize(.small) }
            Text("Approve")
          }
        }
        .buttonStyle(NoemaActionButtonStyle(variant: .primary))
        .disabled(isWorking || !settings.canMutate)
      }
    }
  }

  private var authenticationDetails: some View {
    SettingsDefinitionDetailGroup(title: "Authentication") {
      SettingsDefinitionMetadataRow(label: "Method", value: authenticationLabel(definition.authenticationMode))
      SettingsDefinitionMetadataRow(
        label: "Credential",
        value: definition.credentialSetup?.credentialType ?? credentialLabel(definition.authenticationMode)
      )
      if let setup = definition.credentialSetup, let setupURL = setup.setupURL {
        SettingsDefinitionMetadataRow(label: "Setup page") {
          Link(setupURL.absoluteString, destination: setupURL)
            .foregroundStyle(NoemaColor.accent)
        }
      }
      SettingsDefinitionMetadataRow(label: "Scopes", value: definition.scopes.isEmpty ? "No scopes requested" : definition.scopes.joined(separator: ", "))
      if let identity = definition.operations.first(where: { $0.id == definition.accountIdentityOperationID }) {
        SettingsDefinitionMetadataRow(label: "Account label", value: "\(identity.method) \(identity.path)")
      } else {
        SettingsDefinitionMetadataRow(
          label: "Account label",
          value: definition.authenticationMode == "oauth2_authorization_code_pkce" ? "Generated connection label" : "Not configured"
        )
      }
    }
  }

  private var operationDetails: some View {
    SettingsDefinitionDetailGroup(title: "API operations") {
      Text("\(definition.operations.count) operation\(definition.operations.count == 1 ? "" : "s")")
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentTertiary)
      ForEach(Array(definition.operations.enumerated()), id: \.element.id) { index, operation in
        if index > 0 { SettingsRowDivider(verticalPadding: NoemaSpacing.xs) }
        VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
          HStack(alignment: .firstTextBaseline, spacing: NoemaSpacing.sm) {
            Text(humanize(operation.id)).font(NoemaFont.bodyEmphasized)
            Text(operation.id).font(NoemaFont.monoTiny).foregroundStyle(NoemaColor.contentTertiary)
          }
          Text("\(operation.method) \(operation.path)")
            .font(NoemaFont.monoTiny)
            .foregroundStyle(NoemaColor.content)
            .textSelection(.enabled)
          Text(operationBehavior(operation))
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          Text(operation.arguments.isEmpty ? "No arguments" : "\(operation.arguments.count) argument\(operation.arguments.count == 1 ? "" : "s"): \(operation.arguments.joined(separator: ", "))")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
          if operation.responseTransform != nil {
            Text("Response is normalized before it reaches the agent")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
        }
      }
    }
  }

  @ViewBuilder
  private var responseDetails: some View {
    let transformed = definition.operations.filter { $0.responseTransform != nil }
    if !transformed.isEmpty {
      SettingsDefinitionDetailGroup(title: "Response handling") {
        ForEach(transformed) { operation in
          if let transform = operation.responseTransform {
            DisclosureGroup(humanize(operation.id)) {
              VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
                SettingsDefinitionMetadataRow(label: "Language", value: transform.language)
                SettingsDefinitionMetadataRow(label: "Accepted responses", value: transform.acceptedContentTypes.joined(separator: ", "))
                SettingsDefinitionMetadataRow(label: "Source SHA-256", value: transform.sourceDigest)
                SettingsDefinitionMetadataRow(label: "Transform source", value: transform.source, monospace: true)
                SettingsDefinitionMetadataRow(label: "Output schema", value: transform.outputSchemaJSON, monospace: true)
              }
              .padding(.top, NoemaSpacing.xs)
            }
          }
        }
      }
    }
  }

  private var definitionDetails: some View {
    SettingsDefinitionDetailGroup(title: "Definition") {
      SettingsDefinitionMetadataRow(label: "Review status", value: definition.reviewed ? "Reviewed" : "Pending review")
      SettingsDefinitionMetadataRow(label: "API origin", value: definition.origin, monospace: true)
      SettingsDefinitionMetadataRow(label: "Revision", value: definition.definitionRevision)
      SettingsDefinitionMetadataRow(label: "Adapter ID", value: definition.adapterId, monospace: true)
      SettingsDefinitionMetadataRow(label: "Definition ID", value: definition.definitionId, monospace: true)
      SettingsDefinitionMetadataRow(label: "Definition SHA-256", value: definition.semanticDigest, monospace: true)
      if let source = definition.sourceReference {
        SettingsDefinitionMetadataRow(label: "Source") {
          Link(source.absoluteString, destination: source)
            .foregroundStyle(NoemaColor.accent)
        }
      }
      SettingsDefinitionMetadataRow(label: "Canonical manifest", value: definition.manifestJSON, monospace: true)
    }
  }

  private func approve() {
    runMutation { await settings.approveAdapterDefinition(definition.semanticDigest) }
  }

  private func cancel() {
    runMutation { await settings.cancelAdapterDefinition(definition.semanticDigest) }
  }

  private func runMutation(_ operation: @escaping () async -> Bool) {
    guard !isWorking, settings.canMutate else { return }
    isWorking = true
    errorMessage = nil
    Task {
      if !(await operation()) {
        errorMessage = settings.errorMessage ?? "The definition could not be updated."
      }
      isWorking = false
    }
  }

  private func authenticationLabel(_ value: String) -> String {
    switch value {
    case "oauth2_authorization_code_pkce": "OAuth 2.0 authorization code with PKCE"
    case "credential": "Provider credential"
    case "none": "No authentication"
    default: humanize(value)
    }
  }

  private func credentialLabel(_ value: String) -> String {
    switch value {
    case "oauth2_authorization_code_pkce": "OAuth client"
    case "credential": "Provider credential"
    default: "None"
    }
  }

  private func operationBehavior(_ operation: SettingsAdapterOperation) -> String {
    [
      hint(operation.readOnly, yes: "Read only", no: "Can change data", unknown: "Read behavior unknown"),
      hint(operation.idempotent, yes: "Idempotent", no: "Not idempotent", unknown: "Retry behavior unknown"),
      hint(operation.destructive, yes: "Destructive", no: "Non-destructive", unknown: "Destructive behavior unknown"),
      hint(operation.openWorld, yes: "External interaction", no: "No external interaction", unknown: "External behavior unknown")
    ].joined(separator: " · ")
  }

  private func hint(_ value: Bool?, yes: String, no: String, unknown: String) -> String {
    value == true ? yes : value == false ? no : unknown
  }

  private func humanize(_ value: String) -> String {
    let words = value.replacingOccurrences(of: "_", with: " ")
      .replacingOccurrences(of: "-", with: " ")
      .replacingOccurrences(of: ".", with: " ")
      .replacingOccurrences(of: ":", with: " ")
      .trimmingCharacters(in: .whitespacesAndNewlines)
      .lowercased()
    return words.prefix(1).uppercased() + words.dropFirst()
  }

}

private struct SettingsDefinitionDetailGroup<Content: View>: View {
  let title: String
  private let content: Content

  init(title: String, @ViewBuilder content: () -> Content) {
    self.title = title
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
      Text(title)
        .font(NoemaFont.captionEmphasized)
        .foregroundStyle(NoemaColor.content)
      content
    }
  }
}

private struct SettingsDefinitionMetadataRow<Content: View>: View {
  let label: String
  let value: String?
  let monospace: Bool
  private let content: Content?

  init(label: String, value: String, monospace: Bool = false) where Content == EmptyView {
    self.label = label
    self.value = value
    self.monospace = monospace
    self.content = nil
  }

  init(label: String, monospace: Bool = false, @ViewBuilder content: () -> Content) {
    self.label = label
    self.value = nil
    self.monospace = monospace
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: NoemaSpacing.xxs) {
      Text(label)
        .font(NoemaFont.metadata)
        .foregroundStyle(NoemaColor.contentSecondary)
      if let value {
        Text(value)
          .font(monospace ? NoemaFont.monoTiny : NoemaFont.caption)
          .foregroundStyle(NoemaColor.content)
          .textSelection(.enabled)
          .fixedSize(horizontal: false, vertical: true)
      } else if let content {
        content
          .font(NoemaFont.caption)
          .fixedSize(horizontal: false, vertical: true)
      }
    }
  }
}

private extension String {
  var nilIfBlank: String? {
    let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? nil : trimmed
  }
}

struct MCPConnectionSheet: View {
  let integration: SettingsIntegration
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var label = ""
  @State private var secretEnv = ""
  @State private var secretHeaders = ""
  @State private var clientID = ""
  @State private var clientSecret = ""
  @State private var scopes = ""
  @State private var busy = false
  @State private var error: String?

  var body: some View {
    SettingsBottomSheet(title: "Add connection to \(integration.name)", subtitle: integration.sourceSummary, detent: .large, onClose: { dismiss() }) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Account or installation label") { TextField("Optional", text: $label).settingsSheetControl() }
        SettingsSheetField("Private environment variables") { TextEditor(text: $secretEnv).frame(minHeight: 72).settingsSheetControl() }
        SettingsSheetField("Private request headers") { TextEditor(text: $secretHeaders).frame(minHeight: 72).settingsSheetControl() }
        HStack(spacing: NoemaSpacing.sm) {
          TextField("OAuth client ID", text: $clientID).settingsSheetControl()
          SecureField("Client secret", text: $clientSecret).settingsSheetControl()
        }
        SettingsSheetField("OAuth scopes") { TextField("scope.one scope.two", text: $scopes).settingsSheetControl() }
        if let error { Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger) }
        SettingsSheetActions(primaryTitle: "Add connection", isSaving: busy, primaryDisabled: busy || !settings.canMutate, onCancel: { dismiss() }) {
          guard let env = settingsMap(secretEnv), let headers = settingsMap(secretHeaders) else { error = "Use KEY=value on each line."; return }
          busy = true
          Task {
            let result = await settings.addMCPConnection(definitionID: integration.id, sourceRevision: integration.sourceRevision, label: label, secretEnv: env, secretHeaders: headers, oauthClientID: clientID, oauthClientSecret: clientSecret, oauthScopes: scopes.split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init))
            if let result, result.setupStatus == "ready_for_policy", result.server != nil {
              dismiss()
            } else {
              error = result?.setupError ?? settings.errorMessage ?? "The MCP connection still needs authentication."
            }
            busy = false
          }
        }
      }
    }
  }
}

struct MCPSetupSheet: View {
  let settings: SettingsModel
  let appModel: NoemaAppModel
  @Environment(\.dismiss) private var dismiss
  @State private var name = ""
  @State private var transport = "streamable_http"
  @State private var url = ""
  @State private var command = ""
  @State private var args = ""
  @State private var cwd = ""
  @State private var env = ""
  @State private var secretEnv = ""
  @State private var headers = ""
  @State private var secretHeaders = ""
  @State private var busy = false
  @State private var error: String?
  @State private var attemptID: String?
  @State private var browserURL: URL?
  @State private var result: SettingsMCPSetupResult?
  @State private var oauthClientID = ""
  @State private var oauthClientSecret = ""
  @State private var oauthScopes = ""

  var body: some View {
    SettingsBottomSheet(title: "Connect a service", subtitle: "Check the MCP endpoint, then sign in or provide credentials if it asks.", detent: .large, onClose: { dismiss() }) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSheetField("Name in Noema") { TextField("Service name", text: $name).settingsSheetControl() }
        SettingsSheetField("Connection type") { Picker("Connection type", selection: $transport) { Text("Web address").tag("streamable_http"); Text("Local command").tag("stdio") }.pickerStyle(.segmented) }
        if transport == "streamable_http" {
          SettingsSheetField("Server address") { TextField("https://example.com/mcp", text: $url).keyboardType(.URL).settingsSheetControl() }
          SettingsSheetField("Request headers") { TextEditor(text: $headers).frame(minHeight: 72).settingsSheetControl() }
          SettingsSheetField("Private request headers") { TextEditor(text: $secretHeaders).frame(minHeight: 72).settingsSheetControl() }
        } else {
          SettingsSheetField("Command") { TextField("Command", text: $command).settingsSheetControl() }
          SettingsSheetField("Command options") { TextEditor(text: $args).frame(minHeight: 56).settingsSheetControl() }
          SettingsSheetField("Working directory") { TextField("Optional", text: $cwd).settingsSheetControl() }
          SettingsSheetField("Environment variables") { TextEditor(text: $env).frame(minHeight: 72).settingsSheetControl() }
          SettingsSheetField("Private environment variables") { TextEditor(text: $secretEnv).frame(minHeight: 72).settingsSheetControl() }
        }
        if let result {
          Text(result.setupError ?? "\(result.setupStatus.replacingOccurrences(of: "_", with: " ").capitalized) · \(result.discoveredToolCount) tools discovered")
            .font(NoemaFont.caption).foregroundStyle(result.setupError == nil ? NoemaColor.contentSecondary : NoemaColor.danger)
          if result.setupStatus == "needs_auth", result.oauthClientCredentialsSupported {
            SettingsSheetField("OAuth client ID") { TextField("Client ID", text: $oauthClientID).settingsSheetControl() }
            SettingsSheetField("OAuth client secret") { SecureField("Client secret", text: $oauthClientSecret).settingsSheetControl() }
            SettingsSheetField("OAuth scopes") { TextField(result.scopes.joined(separator: " "), text: $oauthScopes).settingsSheetControl() }
            SettingsAction(title: "Try credentials", symbol: "key", role: nil, disabled: busy || oauthClientID.nilIfBlank == nil) { retryWithCredentials() }
          }
          if result.setupStatus == "authentication_available" {
            SettingsAction(title: "Use public tools only", symbol: "globe", role: nil, disabled: busy) { usePublicTools() }
          }
          if result.oauthAuthorizationSupported { SettingsAction(title: "Continue in browser", symbol: "safari", role: nil, disabled: busy) { startOAuth() } }
          if let attemptID { SettingsAction(title: "Check sign-in status", symbol: "arrow.clockwise", role: nil, disabled: busy) { check(attemptID) } }
        }
        if let error { Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger) }
        SettingsSheetActions(primaryTitle: "Check connection", isSaving: busy, primaryDisabled: busy || name.nilIfBlank == nil || !settings.canMutate, onCancel: { dismiss() }) { create() }
      }
    }
    .noemaSheet(isPresented: Binding(get: { browserURL != nil }, set: { if !$0 { browserURL = nil } }), onDismiss: { if let attemptID { check(attemptID) } }) { if let browserURL { SafariView(url: browserURL) } }
  }

  private func input(
    authPreference: NoemaAPI.McpSetupAuthPreference? = nil,
    includeOAuthCredentials: Bool = false
  ) -> NoemaAPI.CreateMcpServerInput? {
    guard let name = name.nilIfBlank else { return nil }
    let preference = authPreference.map { GraphQLNullable.some(GraphQLEnum($0)) } ?? .none
    if transport == "stdio" {
      guard let command = command.nilIfBlank, let env = settingsMap(env), let secret = settingsMap(secretEnv) else { return nil }
      return NoemaAPI.CreateMcpServerInput(displayName: name, transportKind: transport, stdio: .some(NoemaAPI.McpStdioConfigInput(command: command, args: args.split(separator: "\n").map(String.init), cwd: SettingsModel.optional(cwd.nilIfBlank), env: SettingsModel.json(env), secretEnv: SettingsModel.json(secret))), http: .none, authPreference: preference)
    }
    guard let url = url.nilIfBlank, let headers = settingsMap(headers), let secret = settingsMap(secretHeaders) else { return nil }
    let credentials = includeOAuthCredentials ? oauthClientID.nilIfBlank.map {
      NoemaAPI.McpOAuthClientCredentialsInput(clientId: $0, clientSecret: oauthClientSecret, scopes: oauthScopes.split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init))
    } : nil
    return NoemaAPI.CreateMcpServerInput(displayName: name, transportKind: transport, stdio: .none, http: .some(NoemaAPI.McpHttpConfigInput(url: url, headers: SettingsModel.json(headers), secretHeaders: SettingsModel.json(secret), oauthClientCredentials: credentials.map { .some($0) } ?? .none)), authPreference: preference)
  }

  private func create() {
    guard let input = input() else { error = "Complete the connection fields using KEY=value lines."; return }
    busy = true; error = nil
    Task {
      result = await settings.createMCPServer(input: input)
      busy = false
      if let result, result.setupStatus == "ready_for_policy", result.server != nil { dismiss() }
      else if result == nil { error = settings.errorMessage ?? "MCP setup failed." }
    }
  }

  private func startOAuth() {
    guard let input = input(), let origin = appModel.profile?.origin else { return }
    busy = true
    Task {
      let attempt = await settings.startMCPServerOAuth(input: input, redirectURI: origin.appending(path: "mcp/oauth/callback").absoluteString)
      attemptID = attempt?.attemptID; result = attempt?.setupResult ?? result; browserURL = attempt?.authorizationURL; busy = false
      if attempt?.errorMessage != nil { error = attempt?.errorMessage }
    }
  }

  private func retryWithCredentials() {
    guard let input = input(includeOAuthCredentials: true) else { return }
    busy = true
    Task {
      result = await settings.createMCPServer(input: input)
      busy = false
      if result?.setupStatus == "ready_for_policy" { dismiss() }
      else if result == nil { error = settings.errorMessage ?? "MCP setup could not continue with those credentials." }
    }
  }

  private func usePublicTools() {
    guard let input = input(authPreference: .useAnonymous) else { return }
    busy = true
    Task {
      result = await settings.createMCPServer(input: input)
      busy = false
      if result?.setupStatus == "ready_for_policy" { dismiss() }
      else if result == nil { error = settings.errorMessage ?? "The public tools could not be connected." }
    }
  }

  private func check(_ id: String) {
    busy = true
    Task {
      let attempt = await settings.loadMCPAuthAttempt(id)
      result = attempt?.setupResult ?? result
      busy = false
      if attempt?.setupResult?.setupStatus == "ready_for_policy" {
        await settings.load(client: settings.client)
        dismiss()
      }
    }
  }
}

struct MCPReauthenticationSheet: View {
  let serverID: String
  let usesBrowserOAuth: Bool
  let isHTTP: Bool
  let settings: SettingsModel
  let appModel: NoemaAppModel
  @Environment(\.dismiss) private var dismiss
  @State private var busy = false
  @State private var attemptID: String?
  @State private var browserURL: URL?
  @State private var error: String?
  @State private var result: SettingsMCPSetupResult?
  @State private var oauthClientID = ""
  @State private var oauthClientSecret = ""
  @State private var oauthScopes = ""
  @State private var secretEnvironment = ""
  @State private var secretHeaders = ""

  var body: some View {
    SettingsBottomSheet(title: "Reconnect MCP", subtitle: "Sign in again to refresh this server's credentials.", onClose: { dismiss() }) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        if let error { Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger) }
        if !usesBrowserOAuth {
          if !isHTTP {
            SettingsSheetField("Private environment variables") { TextEditor(text: $secretEnvironment).frame(minHeight: 72).settingsSheetControl() }
          }
          if isHTTP {
            SettingsSheetField("Private request headers") { TextEditor(text: $secretHeaders).frame(minHeight: 72).settingsSheetControl() }
          }
          if isHTTP {
            SettingsSheetField("OAuth client ID") { TextField("Client ID", text: $oauthClientID).settingsSheetControl() }
            SettingsSheetField("OAuth client secret") { SecureField("Client secret", text: $oauthClientSecret).settingsSheetControl() }
            SettingsSheetField("OAuth scopes") { TextField(result?.scopes.joined(separator: " ") ?? "", text: $oauthScopes).settingsSheetControl() }
          }
        }
        SettingsSheetActions(primaryTitle: usesBrowserOAuth ? "Continue in browser" : "Reconnect", isSaving: busy, primaryDisabled: busy || !settings.canMutate, onCancel: { dismiss() }) {
          if usesBrowserOAuth { start() } else { continueWithCredentials() }
        }
        if let attemptID { SettingsAction(title: "Check sign-in status", symbol: "arrow.clockwise", role: nil, disabled: busy) { check(attemptID) } }
      }
    }
    .task { if usesBrowserOAuth { start() } }
    .noemaSheet(isPresented: Binding(get: { browserURL != nil }, set: { if !$0 { browserURL = nil } }), onDismiss: { if let attemptID { check(attemptID) } }) { if let browserURL { SafariView(url: browserURL) } }
  }

  private func start() {
    guard let origin = appModel.profile?.origin else { return }
    busy = true
    Task { let attempt = await settings.startMCPReauthentication(serverID: serverID, redirectURI: origin.appending(path: "mcp/oauth/callback").absoluteString); attemptID = attempt?.attemptID; browserURL = attempt?.authorizationURL; error = attempt?.errorMessage; busy = false }
  }

  private func check(_ id: String) {
    busy = true
    Task {
      let attempt = await settings.loadMCPAuthAttempt(id)
      result = attempt?.setupResult
      error = attempt?.errorMessage
      busy = false
      if result?.setupStatus == "ready_for_policy" {
        await settings.load(client: settings.client)
        dismiss()
      }
    }
  }

  private func continueWithCredentials() {
    guard let privateEnvironment = settingsMap(secretEnvironment),
          let privateHeaders = settingsMap(secretHeaders) else {
      error = "Use KEY=value on each line."
      return
    }
    busy = true
    Task {
      result = await settings.continueMCPServerSetup(serverID: serverID, secretEnv: privateEnvironment, secretHeaders: privateHeaders, oauthClientID: oauthClientID, oauthClientSecret: oauthClientSecret, oauthScopes: oauthScopes.split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init))
      busy = false
      if result?.setupStatus == "ready_for_policy" { dismiss() }
      else if result == nil { error = settings.errorMessage ?? "MCP reauthentication could not continue." }
    }
  }
}

private func settingsMap(_ text: String) -> [String: String]? {
  var result: [String: String] = [:]
  for line in text.split(whereSeparator: \.isNewline).map(String.init) {
    let parts = line.split(separator: "=", maxSplits: 1).map(String.init)
    if parts.count == 1 && line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
    guard parts.count == 2, let key = parts.first?.trimmingCharacters(in: .whitespaces), !key.isEmpty else { return nil }
    guard result.updateValue(parts[1].trimmingCharacters(in: .whitespaces), forKey: key) == nil else { return nil }
  }
  return result
}
