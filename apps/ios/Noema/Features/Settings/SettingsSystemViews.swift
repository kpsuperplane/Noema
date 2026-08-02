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

struct LocalModelsSettings: View {
  let settings: SettingsModel
  @State private var importPresented = false

  var body: some View {
    let installations = settings.snapshot?.localModelInstallations ?? []
    let totalDiskBytes = totalDiskBytes(for: installations)
    let installedModelIDs = Set(
      installations
        .filter { !["FAILED", "CANCELLED"].contains($0.status.rawValue) }
        .map(\.modelId)
    )
    let catalog = (settings.snapshot?.localModelCatalog ?? []).filter { !installedModelIDs.contains($0.modelId) }
    VStack(alignment: .leading, spacing: NoemaSpacing.xxl) {
      if let setup = settings.snapshot?.localModelSetup {
        SettingsSectionCard {
          HStack(spacing: NoemaSpacing.sm) {
            Text("Local runtime").font(NoemaFont.sectionTitle)
            Spacer(minLength: NoemaSpacing.sm)
            NoemaStatusToken(
              text: setup.isReady ? "Ready" : humanize(setup.runtimeStatus.rawValue),
              tone: setup.isReady ? .success : .warning
            )
          }
          SettingsMetricRow(label: "Active model", value: setup.installation?.name ?? "No active local model")
          SettingsRowDivider()
          SettingsMetricRow(label: "Disk usage", value: formatBytes(totalDiskBytes))
          SettingsRowDivider()
          SettingsMetricRow(
            label: "System default",
            value: settings.snapshot?.defaultModelPreference?.modelProfile ?? ""
          )
          if setup.runtimeStatus.rawValue == "FAILED" {
            SettingsAction(title: "Retry runtime", symbol: "arrow.clockwise", role: nil, disabled: !settings.canMutate) {
              Task { await settings.retryLocalModelRuntime() }
            }
          }
        }
      } else if settings.isLoading {
        SettingsSectionCard { NoemaInlineState(message: "Loading local model runtime…", symbol: "arrow.triangle.2.circlepath") }
      } else {
        SettingsSectionCard("Local runtime") {
          NoemaInlineState(message: "Local model runtime could not be loaded.", symbol: "exclamationmark.triangle", tone: .warning)
          SettingsAction(title: "Retry", symbol: "arrow.clockwise", role: nil, disabled: settings.isOffline) {
            Task { await settings.load(client: settings.client) }
          }
        }
      }
      SettingsSectionCard("Installed models") {
        HStack {
          Spacer(minLength: NoemaSpacing.sm)
          Text("\(formatBytes(totalDiskBytes)) on disk")
            .font(NoemaFont.mono)
            .foregroundStyle(NoemaColor.contentSecondary)
        }
        if installations.isEmpty {
          Text("No local models are installed yet.")
            .font(NoemaFont.body)
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          ForEach(Array(installations.enumerated()), id: \.element.installationId) { index, installation in
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(spacing: NoemaSpacing.sm) {
                  Text(installation.name).font(NoemaFont.bodyEmphasized)
                  if installation.isActive { NoemaStatusToken(text: "Active", tone: .success) }
                  NoemaStatusToken(text: humanize(installation.status.rawValue), tone: installationTone(installation.status.rawValue))
                  Spacer(minLength: NoemaSpacing.sm)
                }
                Text(installation.file).font(NoemaFont.mono).foregroundStyle(NoemaColor.contentSecondary)
                if let total = installation.totalBytes, total > 0, installation.status.rawValue != "INSTALLED" {
                  ProgressView(value: Double(installation.completedBytes), total: Double(total))
                  Text("\(formatBytes(installation.completedBytes)) of \(formatBytes(total))")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
                Text([
                  installation.backend?.rawValue.uppercased(),
                  formatBytes(installation.diskBytes),
                  humanize(installation.sourceKind.rawValue)
                ].compactMap { $0 }.joined(separator: " · "))
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                if let error = installation.errorMessage {
                  Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.danger)
                }
                HStack(spacing: NoemaSpacing.md) {
                  if !installation.isActive && installation.status.rawValue == "INSTALLED" {
                    SettingsAction(title: "Use this model", symbol: "checkmark.circle", role: nil, disabled: !settings.canMutate) {
                      Task { await settings.activateLocalModel(installationID: installation.installationId) }
                    }
                  }
                  if ["QUEUED", "DOWNLOADING", "VERIFYING"].contains(installation.status.rawValue) {
                    SettingsAction(title: "Cancel", symbol: "xmark.circle", role: .destructive, disabled: !settings.canMutate) {
                      Task { await settings.cancelLocalModelInstall(installationID: installation.installationId) }
                    }
                  }
                  if !installation.isActive && !["QUEUED", "DOWNLOADING", "VERIFYING"].contains(installation.status.rawValue) {
                    SettingsAction(title: "Remove", symbol: "trash", role: .destructive, disabled: !settings.canMutate) {
                      Task { await settings.removeLocalModel(installationID: installation.installationId) }
                    }
                  }
                }
              }
            }
          }
        }
      }
      SettingsSectionCard("Curated models") {
        if catalog.isEmpty {
          NoemaInlineState(message: "Every compatible curated model is installed.", symbol: "checkmark.circle")
        } else {
          ForEach(Array(catalog.enumerated()), id: \.element.modelId) { index, model in
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(spacing: NoemaSpacing.sm) {
                  Text(model.name).font(NoemaFont.bodyEmphasized)
                  if model.isRecommended { NoemaStatusToken(text: "Recommended", tone: .success) }
                  NoemaStatusToken(text: model.license, tone: .neutral)
                  Spacer(minLength: NoemaSpacing.sm)
                }
                if let fit = model.hardwareFit?.explanation {
                  Text(fit).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
                }
                Text([
                  model.compatibleBackend?.rawValue.uppercased(),
                  model.selectedBuild.map { String(format: "%.1f GB download", $0.downloadGb) },
                  model.selectedBuild?.file
                ].compactMap { $0 }.joined(separator: " · "))
                  .font(NoemaFont.mono)
                  .foregroundStyle(NoemaColor.contentSecondary)
                SettingsAction(title: "Install \(model.name)", symbol: "arrow.down.circle", role: nil, disabled: !settings.canMutate || model.selectedBuild == nil) {
                  Task { await settings.installLocalModel(modelID: model.modelId, file: model.selectedBuild?.file) }
                }
              }
            }
          }
        }
      }
      SettingsSectionCard("Manual imports") {
        SettingsAction(title: "Import GGUF", symbol: "square.and.arrow.down", role: nil, disabled: !settings.canMutate) { importPresented = true }
      }
    }
    .sheet(isPresented: $importPresented) { LocalModelImportEditor(settings: settings) }
  }

  private func totalDiskBytes(for installations: [NoemaAPI.SettingsSnapshotQuery.Data.LocalModelInstallation]) -> Int {
    var byDigest: [String: Int] = [:]
    for installation in installations {
      let key = installation.sha256 ?? installation.installationId
      byDigest[key] = max(byDigest[key] ?? 0, installation.diskBytes)
    }
    return byDigest.values.reduce(0, +)
  }

  private func formatBytes(_ bytes: Int) -> String {
    guard bytes > 0 else { return "0 bytes" }
    return ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
  }

  private func humanize(_ value: String) -> String {
    value.replacingOccurrences(of: "_", with: " ").lowercased().capitalized
  }

  private func installationTone(_ value: String) -> NoemaStatusToken.Tone {
    switch value {
    case "FAILED": .error
    case "INSTALLED": .success
    default: .neutral
    }
  }
}

struct ProvidersSettings: View {
  let settings: SettingsModel
  @State private var addPresented = false
  @State private var secretAccount: SettingsProviderAccount?
  @State private var clearAccount: SettingsProviderAccount?
  @State private var deleteAccount: SettingsProviderAccount?

  var body: some View {
    let accounts = settings.snapshot?.providerAccounts ?? []
    VStack(alignment: .leading, spacing: NoemaSpacing.xxl) {
      SettingsSectionCard("Provider accounts") {
        if settings.isLoading && settings.snapshot == nil {
          NoemaInlineState(message: "Loading provider accounts…", symbol: "arrow.triangle.2.circlepath")
        } else if settings.snapshot == nil {
          SettingsEmpty(settings: settings, message: "Provider accounts are unavailable.")
        } else if accounts.isEmpty {
          HStack {
            Spacer(minLength: NoemaSpacing.sm)
            SettingsAction(title: "Add provider", symbol: "plus", role: nil, disabled: !settings.canMutate || providerCatalog.isEmpty) { addPresented = true }
          }
          NoemaInlineState(message: "No provider accounts have been added yet.", symbol: "server.rack")
        } else {
          HStack {
            Spacer(minLength: NoemaSpacing.sm)
            SettingsAction(title: "Add provider", symbol: "plus", role: nil, disabled: !settings.canMutate || providerCatalog.isEmpty) { addPresented = true }
          }
          ForEach(Array(accounts.enumerated()), id: \.element.providerAccountId) { index, account in
            let local = providerAccount(account)
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(spacing: NoemaSpacing.sm) {
                  Text(account.displayName).font(NoemaFont.bodyEmphasized)
                  NoemaStatusToken(text: humanize(account.status.rawValue), tone: account.isActive ? .success : .warning)
                  if account.isDefault { NoemaStatusToken(text: "Default", tone: .neutral) }
                  Spacer(minLength: NoemaSpacing.sm)
                }
                Text("\(account.providerKind) · \(humanize(account.authMethod))")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                if account.providerKind == "foundation_local" {
                  Text("Local Apple model support is managed by this machine; agent model choices stay in Agents.")
                    .font(NoemaFont.caption)
                    .foregroundStyle(NoemaColor.contentSecondary)
                }
                DisclosureGroup("Technical details") {
                  VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                    SettingsMetricRow(label: "Account", value: account.accountKey)
                    SettingsMetricRow(label: "Status", value: humanize(account.status.rawValue))
                    if let checked = account.lastCheckedAt { SettingsMetricRow(label: "Last checked", value: checked) }
                    if let authenticated = account.lastAuthenticatedAt { SettingsMetricRow(label: "Last authenticated", value: authenticated) }
                  }
                  .padding(.top, NoemaSpacing.xs)
                }
                .font(NoemaFont.caption)
                .foregroundStyle(NoemaColor.contentSecondary)
                if let error = account.lastErrorMessage {
                  Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.warning)
                }
                HStack(spacing: NoemaSpacing.md) {
                  if account.authMethod == NoemaAPI.ProviderAuthMethod.secretInput.rawValue || account.providerKind == "openrouter" {
                    SettingsAction(title: "Replace key", symbol: "key", role: nil, disabled: !settings.canMutate) { secretAccount = local }
                    SettingsAction(title: "Clear key", symbol: "trash", role: .destructive, disabled: !settings.canMutate) { clearAccount = local }
                  }
                  if !account.isDefault {
                    SettingsAction(title: "Delete", symbol: "trash", role: .destructive, disabled: !settings.canMutate) { deleteAccount = local }
                  }
                }
              }
            }
          }
        }
      }
    }
    .sheet(isPresented: $addPresented) {
      ProviderAccountEditor(settings: settings, catalog: providerCatalog)
    }
    .sheet(item: $secretAccount) { account in ProviderSecretEditor(account: account, settings: settings) }
    .sheet(item: $clearAccount) { account in
      SettingsMutationConfirmationSheet(
        title: "Clear provider secret?",
        message: "The stored secret will be removed from this account.",
        confirmTitle: "Clear secret"
      ) { await settings.clearProviderSecret(providerAccountID: account.providerAccountID) }
    }
    .sheet(item: $deleteAccount) { account in
      SettingsMutationConfirmationSheet(
        title: "Delete provider account?",
        message: "Delete \(account.displayName), its stored secrets, and any web tool selections using it. This cannot be undone from Settings.",
        confirmTitle: "Delete account"
      ) { await settings.deleteProviderAccount(providerAccountID: account.providerAccountID) }
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

  private func humanize(_ value: String) -> String {
    value.replacingOccurrences(of: "_", with: " ").lowercased().capitalized
  }
}

struct ClientsSettings: View {
  let settings: SettingsModel
  let profile: NoemaProfile?
  let onRevoke: (PairedClient) -> Void
  @State private var copiedPairingURI: String?

  var body: some View {
    let activeClients = settings.clients.filter { !$0.isRevoked }
    let revokedClients = settings.clients.filter(\.isRevoked)
    VStack(alignment: .leading, spacing: NoemaSpacing.xxl) {
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
              .frame(maxWidth: .infinity, minHeight: 34)
              .background(NoemaColor.controlFill, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
              .buttonStyle(.plain)
              .disabled(profile == nil || settings.isStartingPairing || settings.isOffline)
            }
          }
        }
      }
      SettingsSectionCard("Paired clients") {
        if settings.isLoadingClients && settings.clients.isEmpty {
          NoemaInlineState(message: "Loading paired clients…", symbol: "arrow.triangle.2.circlepath")
        } else if let error = settings.clientsErrorMessage, settings.clients.isEmpty {
          NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
          SettingsAction(title: "Retry", symbol: "arrow.clockwise", role: nil, disabled: settings.isOffline) {
            Task { await settings.loadClients() }
          }
        } else {
          Text("Active")
            .font(NoemaFont.bodyEmphasized)
            .padding(.top, NoemaSpacing.compact)
          if activeClients.isEmpty {
            NoemaInlineState(message: "No active clients are paired yet.", symbol: "iphone")
          } else {
            VStack(spacing: 0) {
              ForEach(Array(activeClients.enumerated()), id: \.element.id) { index, client in
                if index > 0 { SettingsRowDivider() }
                clientRow(client)
              }
            }
            .padding(.top, NoemaSpacing.xs)
          }
          if !revokedClients.isEmpty {
            SettingsRowDivider()
            Text("Revoked")
              .font(NoemaFont.bodyEmphasized)
            VStack(spacing: 0) {
              ForEach(Array(revokedClients.enumerated()), id: \.element.id) { index, client in
                if index > 0 { SettingsRowDivider() }
                clientRow(client)
              }
            }
          }
        }
      }
    }
  }

  @ViewBuilder
  private func clientRow(_ client: PairedClient) -> some View {
    SettingsRow {
      VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
        HStack(spacing: NoemaSpacing.sm) {
          Text(client.displayName).font(NoemaFont.bodyEmphasized)
          if client.isCurrent { NoemaStatusToken(text: "Current client", tone: .success) }
          if client.isRevoked { NoemaStatusToken(text: "Revoked", tone: .neutral) }
          Spacer(minLength: NoemaSpacing.sm)
        }
        Text("Added \(formatted(client.createdAt))")
          .font(NoemaFont.caption)
          .foregroundStyle(NoemaColor.contentSecondary)
        if let revokedAt = client.revokedAt {
          Text("Revoked \(formatted(revokedAt))")
            .font(NoemaFont.caption)
            .foregroundStyle(NoemaColor.contentSecondary)
        } else {
          Button {
            onRevoke(client)
          } label: {
            Label("Revoke", systemImage: "trash")
              .padding(.horizontal, NoemaSpacing.md)
              .frame(minHeight: 28)
          }
          .font(NoemaFont.body)
          .foregroundStyle(NoemaColor.white)
          .background(NoemaColor.red700, in: RoundedRectangle(cornerRadius: NoemaRadius.element))
          .buttonStyle(.plain)
          .disabled(!settings.canMutate)
        }
      }
    }
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
