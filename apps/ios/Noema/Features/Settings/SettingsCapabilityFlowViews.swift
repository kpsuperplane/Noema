import ApolloAPI
import NoemaAPI
import SwiftUI
import UniformTypeIdentifiers

struct APIConnectionSheet: View {
  let integration: SettingsIntegration
  let settings: SettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var importing = false
  @State private var busy = false
  @State private var error: String?

  private var definition: SettingsAdapterDefinition? {
    settings.adapterDefinitions.first { $0.semanticDigest == integration.sourceRevision }
  }

  var body: some View {
    SettingsBottomSheet(title: "Add connection to \(integration.name)", subtitle: integration.sourceSummary, detent: .large, onClose: { dismiss() }) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        if let definition {
          Text("\(definition.authenticationMode.replacingOccurrences(of: "_", with: " ").capitalized) · \(definition.operations.count) operations")
            .font(NoemaFont.bodyEmphasized)
          if !definition.scopes.isEmpty { Text("Scopes: \(definition.scopes.joined(separator: ", "))").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary) }
          ForEach(definition.operations) { operation in
            Text("\(operation.method) \(operation.path)").font(NoemaFont.mono).foregroundStyle(NoemaColor.contentSecondary)
          }
          if let identity = definition.operations.first(where: { $0.id == definition.accountIdentityOperationID }) {
            SettingsSheetField("Account identification") { Text("\(identity.method) \(identity.path)").font(NoemaFont.mono) }
          } else if definition.authenticationMode == "oauth2_authorization_code_pkce" {
            NoemaInlineState(message: "No recognizable account identifier is configured. Connections use a generated label.", symbol: "person.crop.circle.badge.questionmark", tone: .warning)
          }
          let transformed = definition.operations.filter { $0.responseTransform != nil }
          if !transformed.isEmpty {
            SettingsSheetField("Response transforms") {
              VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
                ForEach(transformed) { operation in
                  if let transform = operation.responseTransform {
                    DisclosureGroup("\(operation.id) · \(transform.language)") {
                      Text("Source SHA-256\n\(transform.sourceDigest)")
                      Text("Accepted media types\n\(transform.acceptedContentTypes.joined(separator: "\n"))")
                      Text(transform.source).font(NoemaFont.monoTiny).textSelection(.enabled)
                      Text(transform.outputSchemaJSON).font(NoemaFont.monoTiny).textSelection(.enabled)
                    }
                  }
                }
              }
            }
          }
          DisclosureGroup("Technical definition") {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              Text("Revision\n\(definition.definitionRevision)")
              if let setupURL = definition.clientSetupURL { Text("OAuth client setup\n\(setupURL.absoluteString)") }
              DisclosureGroup("Canonical manifest") { Text(definition.manifestJSON).font(NoemaFont.monoTiny).textSelection(.enabled) }
            }
            .font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
          }
          if !definition.reviewed {
            SettingsAction(title: "Approve definition", symbol: "checkmark.shield", role: nil, disabled: busy || !settings.canMutate) {
              busy = true
              Task {
                if !(await settings.approveAdapterDefinition(definition.semanticDigest)) {
                  error = settings.errorMessage ?? "The definition could not be approved."
                }
                busy = false
              }
            }
          } else if definition.acceptsOAuthClientJSON {
            if let clientSetupURL = definition.clientSetupURL { Link("Open developer tools", destination: clientSetupURL).font(NoemaFont.captionEmphasized) }
            SettingsAction(title: "Choose OAuth client JSON", symbol: "doc.badge.plus", role: nil, disabled: busy || !settings.canMutate) { importing = true }
          } else {
            NoemaInlineState(message: "This definition does not accept an OAuth client document.", symbol: "info.circle", tone: .warning)
          }
          if let source = definition.sourceReference { Link("Open source documentation", destination: source).font(NoemaFont.captionEmphasized) }
        } else {
          NoemaInlineState(message: "Definition details are unavailable.", symbol: "wifi.slash", tone: .warning)
        }
        if let error { Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger) }
      }
    }
    .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
      guard let definition else { return }
      switch result {
      case .success(let url):
        busy = true
        Task {
          let accessed = url.startAccessingSecurityScopedResource()
          defer { if accessed { url.stopAccessingSecurityScopedResource() } }
          defer { busy = false }
          do {
            let values = try url.resourceValues(forKeys: [.fileSizeKey])
            guard let size = values.fileSize, size > 0, size <= 32 * 1024 else {
              error = "Choose a non-empty OAuth client JSON file smaller than 32 KB."
              return
            }
            let ok = await settings.importAdapterOAuthClientJSON(
              definition.semanticDigest,
              data: try Data(contentsOf: url, options: .mappedIfSafe)
            )
            if ok { dismiss() } else { error = settings.errorMessage ?? "The OAuth client could not be imported." }
          } catch { self.error = error.localizedDescription }
        }
      case .failure(let error): self.error = error.localizedDescription
      }
    }
    .interactiveDismissDisabled(busy)
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
    .sheet(isPresented: Binding(get: { browserURL != nil }, set: { if !$0 { browserURL = nil } }), onDismiss: { if let attemptID { check(attemptID) } }) { if let browserURL { SafariView(url: browserURL) } }
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
    .sheet(isPresented: Binding(get: { browserURL != nil }, set: { if !$0 { browserURL = nil } }), onDismiss: { if let attemptID { check(attemptID) } }) { if let browserURL { SafariView(url: browserURL) } }
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
