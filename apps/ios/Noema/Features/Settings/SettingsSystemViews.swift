import CoreImage.CIFilterBuiltins
import NoemaAPI
import SwiftUI
import UIKit

struct SettingsMetricRow: View {
  let label: String
  let value: String

  var body: some View {
    HStack(spacing: NoemaSpacing.sm) {
      Text(label)
        .font(NoemaFont.body)
        .foregroundStyle(NoemaColor.contentSecondary)
      Spacer(minLength: NoemaSpacing.sm)
      Text(value)
        .font(NoemaFont.mono)
        .foregroundStyle(NoemaColor.content)
    }
    .padding(.vertical, NoemaSpacing.xxs)
  }
}

private enum LocalModelSelection: Identifiable {
  case installation(String)
  case catalog(String)

  var id: String {
    switch self {
    case .installation(let id): "installation-\(id)"
    case .catalog(let id): "catalog-\(id)"
    }
  }
}

struct LocalModelsSettings: View {
  let settings: SettingsModel
  @State private var importPresented = false
  @State private var selection: LocalModelSelection?

  var body: some View {
    let installations = settings.snapshot?.localModelInstallations ?? []
    let installedModelIDs = Set(
      installations
        .filter { !["FAILED", "CANCELLED"].contains($0.status.rawValue) }
        .map(\.modelId)
    )
    let catalog = (settings.snapshot?.localModelCatalog ?? []).filter { !installedModelIDs.contains($0.modelId) }
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      if settings.isLoading && settings.snapshot == nil {
        NoemaInlineState(message: "Loading local models…", symbol: "arrow.triangle.2.circlepath")
      } else if settings.snapshot == nil {
        SettingsSectionCard("Local models") {
          SettingsEmpty(settings: settings, message: "Local models are unavailable.")
        }
      } else {
        VStack(alignment: .leading, spacing: NoemaSpacing.compact) {
          SettingsGroupTitle(
            "On this device",
            actionTitle: "Import model",
            actionDisabled: !settings.canMutate
          ) { importPresented = true }
          if installations.isEmpty {
            Text("No local models are on this device.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
              .padding(.horizontal, NoemaSpacing.xs)
          } else {
            ForEach(installations, id: \.installationId) { installation in
              SettingsListCard(
                title: installation.name,
                detail: installation.errorMessage,
                symbol: "cpu",
                status: installation.isActive ? "Active model" : installation.status.rawValue == "INSTALLED" ? nil : humanize(installation.status.rawValue),
                statusTone: installationTone(installation.status.rawValue)
              ) { selection = .installation(installation.installationId) }
            }
          }
        }
        VStack(alignment: .leading, spacing: NoemaSpacing.compact) {
          SettingsGroupTitle("Available")
          if catalog.isEmpty {
            Text("No other curated models are available.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
              .padding(.horizontal, NoemaSpacing.xs)
          } else {
            ForEach(catalog, id: \.modelId) { model in
              SettingsListCard(
                title: model.name,
                detail: model.selectedBuild == nil ? "Incompatible with this device" : model.hardwareFit?.explanation,
                symbol: "cpu",
                status: model.isRecommended ? "Recommended" : nil,
                statusTone: .success
              ) { selection = .catalog(model.modelId) }
            }
          }
        }
      }
    }
    .sheet(isPresented: $importPresented) { LocalModelImportEditor(settings: settings) }
    .sheet(item: $selection) { selection in
      switch selection {
      case .installation(let id):
        if let installation = installations.first(where: { $0.installationId == id }) {
          LocalModelInstallationSheet(
            installation: installation,
            model: settings.snapshot?.localModelCatalog.first { $0.modelId == installation.modelId },
            settings: settings,
            onClose: { self.selection = nil }
          )
        }
      case .catalog(let id):
        if let model = settings.snapshot?.localModelCatalog.first(where: { $0.modelId == id }) {
          LocalModelCatalogSheet(model: model, settings: settings, onClose: { self.selection = nil })
        }
      }
    }
  }

  private func installationTone(_ value: String) -> NoemaStatusToken.Tone {
    switch value {
    case "FAILED": .error
    case "INSTALLED": .success
    default: .neutral
    }
  }
}

private struct LocalModelInstallationSheet: View {
  let installation: NoemaAPI.SettingsSnapshotQuery.Data.LocalModelInstallation
  let model: NoemaAPI.SettingsSnapshotQuery.Data.LocalModelCatalog?
  let settings: SettingsModel
  let onClose: () -> Void
  @State private var removePresented = false

  private var transferActive: Bool {
    ["QUEUED", "DOWNLOADING", "VERIFYING"].contains(installation.status.rawValue)
  }

  var body: some View {
    SettingsBottomSheet(title: installation.name, detent: .large, onClose: onClose) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        if let error = installation.errorMessage {
          NoemaInlineState(message: error, symbol: "exclamationmark.triangle", tone: .error)
        }
        SettingsSectionCard("Model") {
          SettingsMetricRow(label: "Hardware fit", value: model?.hardwareFit?.explanation ?? "Not reported")
          SettingsRowDivider()
          SettingsMetricRow(label: "License", value: model?.license ?? "Not reported")
          SettingsRowDivider()
          SettingsMetricRow(label: "Disk usage", value: formatBytes(installation.diskBytes))
        }
        SettingsSectionCard("Installation") {
          SettingsMetricRow(label: "State", value: humanize(installation.status.rawValue))
          if let total = installation.totalBytes, total > 0, transferActive {
            ProgressView(value: Double(installation.completedBytes), total: Double(total))
            Text("\(formatBytes(installation.completedBytes)) of \(formatBytes(total))")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          }
          HStack(spacing: NoemaSpacing.md) {
            if transferActive {
              SettingsAction(title: "Cancel", symbol: "xmark.circle", role: .destructive, disabled: !settings.canMutate) {
                Task { await settings.cancelLocalModelInstall(installationID: installation.installationId) }
              }
            } else if !installation.isActive && installation.status.rawValue == "INSTALLED" {
              SettingsAction(title: "Use this model", symbol: "play", role: nil, disabled: !settings.canMutate) {
                Task { await settings.activateLocalModel(installationID: installation.installationId) }
              }
            }
          }
          DisclosureGroup("Technical details") {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              SettingsMetricRow(label: "Installation ID", value: installation.installationId)
              SettingsMetricRow(label: "Model ID", value: installation.modelId)
              SettingsMetricRow(label: "Filename", value: installation.file)
              if let sha256 = installation.sha256 { SettingsMetricRow(label: "Digest", value: sha256) }
            }
            .padding(.top, NoemaSpacing.xs)
          }
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        }
        if let setup = settings.snapshot?.localModelSetup,
           installation.isActive || setup.installation?.installationId == installation.installationId {
          SettingsSectionCard("Runtime") {
            SettingsMetricRow(label: "State", value: humanize(setup.runtimeStatus.rawValue))
            SettingsRowDivider()
            SettingsMetricRow(label: "System default", value: settings.snapshot?.defaultModelPreference?.modelProfile ?? "Not selected")
            if setup.runtimeStatus.rawValue == "FAILED" {
              SettingsAction(title: "Retry runtime", symbol: "arrow.clockwise", role: nil, disabled: !settings.canMutate) {
                Task { await settings.retryLocalModelRuntime() }
              }
            }
          }
        }
        if !transferActive && !installation.isActive {
          SettingsSectionCard("Lifecycle") {
            Text("Removing this installation deletes its local model data.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
            SettingsAction(title: "Remove model", symbol: "trash", role: .destructive, disabled: !settings.canMutate) {
              removePresented = true
            }
          }
        }
      }
    }
    .sheet(isPresented: $removePresented) {
      SettingsMutationConfirmationSheet(
        title: "Remove \(installation.name)?",
        message: "This removes \(formatBytes(installation.diskBytes)) from this device. You can install it again later.",
        confirmTitle: "Remove model"
      ) {
        let removed = await settings.removeLocalModel(installationID: installation.installationId)
        if removed { onClose() }
        return removed
      }
    }
  }
}

private struct LocalModelCatalogSheet: View {
  let model: NoemaAPI.SettingsSnapshotQuery.Data.LocalModelCatalog
  let settings: SettingsModel
  let onClose: () -> Void

  var body: some View {
    SettingsBottomSheet(title: model.name, detent: .large, onClose: onClose) {
      SettingsSectionCard("Model") {
        SettingsMetricRow(label: "Hardware fit", value: model.hardwareFit?.explanation ?? "No compatible build")
        SettingsRowDivider()
        SettingsMetricRow(label: "License", value: model.license)
        SettingsRowDivider()
        SettingsMetricRow(label: "Download", value: model.selectedBuild.map { String(format: "%.1f GB", $0.downloadGb) } ?? "Unavailable")
        SettingsAction(title: "Install", symbol: "arrow.down.circle", role: nil, disabled: !settings.canMutate || model.selectedBuild == nil) {
          Task {
            if await settings.installLocalModel(modelID: model.modelId, file: model.selectedBuild?.file) {
              onClose()
            }
          }
        }
        DisclosureGroup("Technical details") {
          VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
            SettingsMetricRow(label: "Model ID", value: model.modelId)
            SettingsMetricRow(label: "Repository", value: model.repo)
            SettingsMetricRow(label: "Revision", value: model.revision)
            if let build = model.selectedBuild {
              SettingsMetricRow(label: "Filename", value: build.file)
              SettingsMetricRow(label: "Digest", value: build.sha256)
            }
          }
          .padding(.top, NoemaSpacing.xs)
        }
        .font(NoemaFont.caption)
        .foregroundStyle(NoemaColor.contentSecondary)
      }
    }
  }
}

private func formatBytes(_ bytes: Int) -> String {
  guard bytes > 0 else { return "0 bytes" }
  return ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
}

private func humanize(_ value: String) -> String {
  value.replacingOccurrences(of: "_", with: " ").lowercased().capitalized
}

struct ProvidersSettings: View {
  let settings: SettingsModel
  @State private var addPresented = false
  @State private var selectedAccount: SettingsProviderAccount?

  var body: some View {
    let accounts = settings.snapshot?.providerAccounts ?? []
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsGroupTitle(
        "Provider accounts",
        actionTitle: "Add provider",
        actionDisabled: !settings.canMutate || providerCatalog.isEmpty
      ) { addPresented = true }
      if settings.isLoading && settings.snapshot == nil {
        NoemaInlineState(message: "Loading provider accounts…", symbol: "arrow.triangle.2.circlepath")
      } else if settings.snapshot == nil {
        SettingsSectionCard { SettingsEmpty(settings: settings, message: "Provider accounts are unavailable.") }
      } else if accounts.isEmpty {
        Text("Add a provider account to make models and web tools available.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
          .padding(.horizontal, NoemaSpacing.xs)
      } else {
        ForEach(groupedProviderAccounts(accounts), id: \.providerKind) { group in
          VStack(alignment: .leading, spacing: NoemaSpacing.compact) {
            Text(group.name)
              .font(NoemaFont.taskTitle)
              .foregroundStyle(NoemaColor.content)
              .padding(.horizontal, NoemaSpacing.xs)
            ForEach(group.accounts, id: \.providerAccountId) { account in
              SettingsListCard(
                title: account.displayName,
                detail: account.lastErrorMessage,
                symbol: "server.rack",
                status: account.isDefault ? "Default" : account.isActive ? nil : humanize(account.status.rawValue),
                statusTone: account.isActive ? .neutral : .warning
              ) { selectedAccount = providerAccount(account) }
            }
          }
        }
      }
    }
    .sheet(isPresented: $addPresented) {
      ProviderAccountEditor(settings: settings, catalog: providerCatalog)
    }
    .sheet(item: $selectedAccount) { selected in
      if let account = accounts.first(where: { $0.providerAccountId == selected.providerAccountID }) {
        ProviderAccountDetailSheet(
          account: account,
          catalog: settings.snapshot?.providerAccountCatalog.first { $0.providerKind == account.providerKind },
          local: selected,
          settings: settings,
          onClose: { selectedAccount = nil }
        )
      }
    }
  }

  private var providerCatalog: [SettingsProviderCatalog] {
    (settings.snapshot?.providerAccountCatalog ?? []).map {
      SettingsProviderCatalog(providerKind: $0.providerKind, displayName: $0.displayName, preferredAuthMethod: $0.preferredAuthMethod.rawValue, supportedAuthMethods: $0.supportedAuthMethods.map(\.rawValue))
    }
  }

  private func providerAccount(_ value: NoemaAPI.SettingsSnapshotQuery.Data.ProviderAccount) -> SettingsProviderAccount {
    SettingsProviderAccount(providerAccountID: value.providerAccountId, providerKind: value.providerKind, displayName: value.displayName, authMethod: value.authMethod, status: value.status.rawValue, isActive: value.isActive, isDefault: value.isDefault, lastError: value.lastErrorMessage)
  }

  private func groupedProviderAccounts(
    _ accounts: [NoemaAPI.SettingsSnapshotQuery.Data.ProviderAccount]
  ) -> [(providerKind: String, name: String, accounts: [NoemaAPI.SettingsSnapshotQuery.Data.ProviderAccount])] {
    let grouped = Dictionary(grouping: accounts, by: \.providerKind)
    return grouped.map { providerKind, accounts in
      let name = settings.snapshot?.providerAccountCatalog.first { $0.providerKind == providerKind }?.displayName ?? providerKind
      return (providerKind, name, accounts)
    }
    .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
  }
}

private struct ProviderAccountDetailSheet: View {
  let account: NoemaAPI.SettingsSnapshotQuery.Data.ProviderAccount
  let catalog: NoemaAPI.SettingsSnapshotQuery.Data.ProviderAccountCatalog?
  let local: SettingsProviderAccount
  let settings: SettingsModel
  let onClose: () -> Void
  @State private var replaceKeyPresented = false
  @State private var clearKeyPresented = false
  @State private var deletePresented = false

  private var usesSecret: Bool {
    account.authMethod == NoemaAPI.ProviderAuthMethod.secretInput.rawValue || account.providerKind == "openrouter"
  }

  private var capabilityRows: [(id: String, status: String)] {
    if !account.capabilities.isEmpty {
      return account.capabilities.map { ($0.capabilityId, $0.status) }
    }
    return (catalog?.capabilities ?? []).map { ($0.capabilityId, $0.status) }
  }

  var body: some View {
    SettingsBottomSheet(title: account.displayName, detent: .large, onClose: onClose) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        if let error = account.lastErrorMessage {
          NoemaInlineState(message: error, symbol: "exclamationmark.triangle", tone: .warning)
        }
        SettingsSectionCard("Access") {
          SettingsMetricRow(label: "Authentication", value: humanize(account.authMethod))
          SettingsRowDivider()
          SettingsMetricRow(label: "State", value: humanize(account.status.rawValue))
          if usesSecret {
            HStack(spacing: NoemaSpacing.md) {
              SettingsAction(title: "Replace key", symbol: "key", role: nil, disabled: !settings.canMutate) {
                replaceKeyPresented = true
              }
              SettingsAction(title: "Clear key", symbol: "trash", role: .destructive, disabled: !settings.canMutate) {
                clearKeyPresented = true
              }
            }
          }
          DisclosureGroup("Technical details") {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              SettingsMetricRow(label: "Account", value: account.accountKey)
              if let checked = account.lastCheckedAt { SettingsMetricRow(label: "Last checked", value: checked) }
              if let authenticated = account.lastAuthenticatedAt { SettingsMetricRow(label: "Last authenticated", value: authenticated) }
            }
            .padding(.top, NoemaSpacing.xs)
          }
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        }
        SettingsSectionCard("Capabilities") {
          if capabilityRows.isEmpty {
            Text("No capabilities are reported.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
          } else {
            ForEach(Array(capabilityRows.enumerated()), id: \.offset) { index, capability in
              if index > 0 { SettingsRowDivider() }
              SettingsMetricRow(label: humanize(capability.id), value: humanize(capability.status))
            }
          }
        }
        if !account.isDefault {
          SettingsSectionCard("Lifecycle") {
            Text("Deleting this account removes its stored access and web tool selections.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
            SettingsAction(title: "Delete account", symbol: "trash", role: .destructive, disabled: !settings.canMutate) {
              deletePresented = true
            }
          }
        }
      }
    }
    .sheet(isPresented: $replaceKeyPresented) { ProviderSecretEditor(account: local, settings: settings) }
    .sheet(isPresented: $clearKeyPresented) {
      SettingsMutationConfirmationSheet(
        title: "Clear API key for \(account.displayName)?",
        message: "Noema cannot use this account until you add a key again.",
        confirmTitle: "Clear key"
      ) { await settings.clearProviderSecret(providerAccountID: account.providerAccountId) }
    }
    .sheet(isPresented: $deletePresented) {
      SettingsMutationConfirmationSheet(
        title: "Delete \(account.displayName)?",
        message: "This removes the account, its stored access, and web tool selections.",
        confirmTitle: "Delete account"
      ) {
        let deleted = await settings.deleteProviderAccount(providerAccountID: account.providerAccountId)
        if deleted { onClose() }
        return deleted
      }
    }
  }
}

struct ClientsSettings: View {
  let settings: SettingsModel
  let profile: NoemaProfile?
  let appModel: NoemaAppModel
  let onDisconnect: () -> Void
  @State private var copiedPairingURI: String?
  @State private var selectedClient: PairedClient?

  var body: some View {
    let activeClients = settings.clients.filter { !$0.isRevoked }
    let revokedClients = settings.clients.filter(\.isRevoked)
    VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
      SettingsSectionCard("Pair a client") {
        Text("Create a short-lived link for a Noema client. The link stays in this page's memory and expires after 10 minutes.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        TimelineView(.periodic(from: .now, by: 1)) { context in
          if let pairing = settings.pairingLink, pairing.expiresAt > context.date {
            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              if let image = pairingQRCode(pairing.uri) {
                VStack(spacing: NoemaSpacing.xs) {
                  Image(uiImage: image)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: 200)
                    .accessibilityLabel("Scan this QR code to pair a Noema client")
                  Text("Scan with the Noema client")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
                .frame(maxWidth: .infinity)
              }
              Text(pairing.uri)
                .font(NoemaFont.mono)
                .foregroundStyle(NoemaColor.contentSecondary)
                .textSelection(.enabled)
                .lineLimit(3)
              HStack(spacing: NoemaSpacing.sm) {
                Button {
                  UIPasteboard.general.string = pairing.uri
                  copiedPairingURI = pairing.uri
                } label: {
                  Label(copiedPairingURI == pairing.uri ? "Copied" : "Copy link", systemImage: copiedPairingURI == pairing.uri ? "checkmark" : "doc.on.doc")
                }
                .buttonStyle(.plain)
                .font(NoemaFont.captionEmphasized)
                ShareLink(item: pairing.uri) {
                  Label("Share link", systemImage: "square.and.arrow.up")
                }
                .font(NoemaFont.captionEmphasized)
                Text("Expires in \(remaining(pairing.expiresAt, at: context.date))")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
              }
            }
          } else {
            VStack(alignment: .leading, spacing: NoemaSpacing.sm) {
              if settings.pairingLink != nil {
                NoemaInlineState(message: "This pairing link expired. Start a new link to continue.", symbol: "clock.badge.exclamationmark", tone: .warning)
              }
              if let error = settings.pairingErrorMessage {
                Text(error)
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.danger)
              }
              Button {
                Task { await settings.startClientPairing(profile: profile) }
              } label: {
                if settings.isStartingPairing {
                  ProgressView().controlSize(.small)
                } else {
                  Label(settings.pairingLink == nil && settings.pairingErrorMessage == nil ? "Start pairing" : "Retry pairing", systemImage: "arrow.clockwise")
                }
              }
              .font(NoemaFont.captionEmphasized)
              .foregroundStyle(NoemaColor.content)
              .frame(maxWidth: .infinity, minHeight: 32)
              .background(NoemaColor.controlFill, in: NoemaSuperellipse(cornerRadius: NoemaRadius.element))
              .buttonStyle(.plain)
              .disabled(profile == nil || settings.isStartingPairing || settings.isOffline)
            }
            .padding(.top, NoemaSpacing.sm)
            .padding(.bottom, NoemaSpacing.xs)
          }
        }
      }
      SettingsSectionCard("Paired clients") {
        if settings.isLoadingClients && !settings.hasLoadedClients {
          NoemaInlineState(message: "Loading paired clients…", symbol: "arrow.triangle.2.circlepath")
        } else if let error = settings.clientsErrorMessage, settings.clients.isEmpty {
          NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
          SettingsAction(title: "Retry", symbol: "arrow.clockwise", role: nil, disabled: settings.isOffline) {
            Task { await settings.loadClients() }
          }
        } else {
          Text("Active")
            .font(NoemaFont.taskTitle)
            .padding(.top, NoemaSpacing.compact)
          if activeClients.isEmpty {
            NoemaInlineState(message: "No active clients are paired yet.", symbol: "iphone")
          } else {
            VStack(spacing: 0) {
              ForEach(Array(activeClients.enumerated()), id: \.element.id) { index, client in
                if index > 0 { SettingsRowDivider(verticalPadding: 0) }
                clientRow(client)
              }
            }
            .padding(.top, NoemaSpacing.xs)
          }
          if !revokedClients.isEmpty {
            SettingsRowDivider(verticalPadding: 0)
            Text("Revoked")
              .font(NoemaFont.taskTitle)
            VStack(spacing: 0) {
              ForEach(Array(revokedClients.enumerated()), id: \.element.id) { index, client in
                if index > 0 { SettingsRowDivider(verticalPadding: 0) }
                clientRow(client)
              }
            }
          }
        }
      }
      SettingsSectionCard("This app") {
        Text("Remove this device's saved connection to pair it with another Noema server. This does not revoke the client on the current server.")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        SettingsAction(
          title: "Unpair this app",
          symbol: "rectangle.portrait.and.arrow.right",
          role: .destructive,
          disabled: false,
          action: onDisconnect
        )
      }
    }
    .sheet(item: $selectedClient) { client in
      PairedClientDetailSheet(
        client: client,
        settings: settings,
        appModel: appModel,
        onClose: { selectedClient = nil }
      )
    }
  }

  @ViewBuilder
  private func clientRow(_ client: PairedClient) -> some View {
    SettingsListCard(
      title: client.displayName,
      detail: "Added \(formatted(client.createdAt))",
      symbol: "iphone",
      status: client.isCurrent ? "Current client" : nil,
      statusTone: .success
    ) { selectedClient = client }
  }

  private func pairingQRCode(_ value: String) -> UIImage? {
    let filter = CIFilter.qrCodeGenerator()
    filter.message = Data(value.utf8)
    filter.correctionLevel = "M"
    guard let output = filter.outputImage?.transformed(by: CGAffineTransform(scaleX: 8, y: 8)) else { return nil }
    return UIImage(ciImage: output)
  }

  private func remaining(_ expiration: Date, at now: Date) -> String {
    let seconds = max(0, Int(expiration.timeIntervalSince(now)))
    return String(format: "%d:%02d", seconds / 60, seconds % 60)
  }

  private func formatted(_ value: String) -> String {
    let parser = ISO8601DateFormatter()
    parser.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    guard let date = parser.date(from: value) ?? ISO8601DateFormatter().date(from: value) else { return value }
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "MMM d, yyyy, h:mm a"
    return formatter.string(from: date)
  }
}

private struct PairedClientDetailSheet: View {
  let client: PairedClient
  let settings: SettingsModel
  let appModel: NoemaAppModel
  let onClose: () -> Void
  @State private var revokePresented = false

  var body: some View {
    SettingsBottomSheet(title: client.displayName, detent: .large, onClose: onClose) {
      VStack(alignment: .leading, spacing: NoemaSpacing.lg) {
        SettingsSectionCard("Access") {
          SettingsMetricRow(label: "State", value: client.isRevoked ? "Revoked" : "Active")
          SettingsRowDivider()
          SettingsMetricRow(label: "Paired", value: formatted(client.createdAt))
          DisclosureGroup("Technical details") {
            VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
              SettingsMetricRow(label: "Client ID", value: client.id)
              if let revokedAt = client.revokedAt {
                SettingsMetricRow(label: "Revoked", value: formatted(revokedAt))
              }
            }
            .padding(.top, NoemaSpacing.xs)
          }
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        }
        if !client.isRevoked {
          SettingsSectionCard("Lifecycle") {
            Text("Revoking this client stops new authenticated requests. Its audit history remains.")
              .font(NoemaFont.caption)
              .foregroundStyle(NoemaColor.contentSecondary)
            SettingsAction(title: "Revoke client", symbol: "trash", role: .destructive, disabled: !settings.canMutate) {
              revokePresented = true
            }
          }
        }
      }
    }
    .sheet(isPresented: $revokePresented, onDismiss: {
      if settings.clients.first(where: { $0.id == client.id })?.isRevoked == true { onClose() }
    }) {
      ClientRevocationSheet(client: client, settings: settings, appModel: appModel)
    }
  }

  private func formatted(_ value: String) -> String {
    let parser = ISO8601DateFormatter()
    parser.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    guard let date = parser.date(from: value) ?? ISO8601DateFormatter().date(from: value) else { return value }
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "MMM d, yyyy, h:mm a"
    return formatter.string(from: date)
  }
}
