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
    VStack(alignment: .leading, spacing: NoemaSpacing.xxl) {
      if let setup = settings.snapshot?.localModelSetup {
        SettingsSectionCard("Runtime") {
          HStack {
            Text("Status").font(NoemaFont.body)
            Spacer(minLength: NoemaSpacing.sm)
            NoemaStatusToken(text: setup.isReady ? "Ready" : setup.runtimeStatus.rawValue, tone: setup.isReady ? .success : .warning)
          }
          if !setup.isReady {
            SettingsAction(title: "Retry runtime", symbol: "arrow.clockwise", role: nil, disabled: !settings.canMutate) {
              Task { await settings.retryLocalModelRuntime() }
            }
          }
          if let recommendation = setup.recommendedModel {
            SettingsRowDivider()
            Text("Recommended").font(NoemaFont.captionEmphasized)
            Text(recommendation.name).font(NoemaFont.bodyEmphasized)
            if let fit = recommendation.hardwareFit {
              Text(fit.explanation).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
            }
            if setup.installation == nil {
              SettingsAction(title: "Install \(recommendation.name)", symbol: "arrow.down.circle", role: nil, disabled: !settings.canMutate) {
                Task { await settings.installLocalModel(modelID: recommendation.modelId, file: recommendation.selectedBuild?.file) }
              }
            }
          }
        }
      } else if settings.isLoading {
        SettingsSectionCard { NoemaInlineState(message: "Loading local model runtime…", symbol: "arrow.triangle.2.circlepath") }
      }
      SettingsSectionCard("Installed") {
        if installations.isEmpty {
          NoemaInlineState(message: "No local models installed.", symbol: "cpu")
        } else {
          ForEach(Array(installations.enumerated()), id: \.element.installationId) { index, installation in
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(spacing: NoemaSpacing.sm) {
                  Text(installation.name).font(NoemaFont.bodyEmphasized)
                  Spacer(minLength: NoemaSpacing.sm)
                  if installation.isActive { NoemaStatusToken(text: "Active", tone: .success) }
                }
                Text("\(installation.status.rawValue) · \(installation.file)")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                HStack(spacing: NoemaSpacing.md) {
                  if !installation.isActive && installation.status.rawValue == "INSTALLED" {
                    SettingsAction(title: "Use", symbol: "checkmark.circle", role: nil, disabled: !settings.canMutate) {
                      Task { await settings.activateLocalModel(installationID: installation.installationId) }
                    }
                  }
                  if ["QUEUED", "DOWNLOADING", "VERIFYING"].contains(installation.status.rawValue) {
                    SettingsAction(title: "Cancel", symbol: "xmark.circle", role: .destructive, disabled: !settings.canMutate) {
                      Task { await settings.cancelLocalModelInstall(installationID: installation.installationId) }
                    }
                  }
                  SettingsAction(title: "Remove", symbol: "trash", role: .destructive, disabled: !settings.canMutate || installation.isActive) {
                    Task { await settings.removeLocalModel(installationID: installation.installationId) }
                  }
                }
              }
            }
          }
        }
      }
      let catalog = settings.snapshot?.localModelCatalog ?? []
      if !catalog.isEmpty {
        SettingsSectionCard("Curated models") {
          ForEach(Array(catalog.enumerated()), id: \.element.modelId) { index, model in
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(spacing: NoemaSpacing.sm) {
                  Text(model.name).font(NoemaFont.bodyEmphasized)
                  if model.isRecommended { NoemaStatusToken(text: "Recommended", tone: .success) }
                  Spacer(minLength: NoemaSpacing.sm)
                }
                if let fit = model.hardwareFit?.explanation {
                  Text(fit).font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
                }
                Text("License: \(model.license)").font(NoemaFont.caption).foregroundStyle(NoemaColor.contentSecondary)
                if let installation = installations.first(where: { $0.modelId == model.modelId }) {
                  Text(installation.status.rawValue.capitalized)
                    .font(NoemaFont.captionEmphasized)
                    .foregroundStyle(NoemaColor.contentSecondary)
                } else {
                  SettingsAction(title: "Install \(model.name)", symbol: "arrow.down.circle", role: nil, disabled: !settings.canMutate) {
                    Task { await settings.installLocalModel(modelID: model.modelId, file: model.selectedBuild?.file) }
                  }
                }
              }
            }
          }
        }
      }
      SettingsSectionCard {
        SettingsAction(title: "Import local model", symbol: "square.and.arrow.down", role: nil, disabled: !settings.canMutate) { importPresented = true }
      }
    }
    .sheet(isPresented: $importPresented) { LocalModelImportEditor(settings: settings) }
  }
}

struct ProvidersSettings: View {
  let settings: SettingsModel
  @State private var addPresented = false
  @State private var secretAccount: SettingsProviderAccount?
  @State private var clearAccount: SettingsProviderAccount?
  @State private var deleteAccount: SettingsProviderAccount?
  @State private var defaultEditor: SettingsPreferenceTarget?

  var body: some View {
    let accounts = settings.snapshot?.providerAccounts ?? []
    VStack(alignment: .leading, spacing: NoemaSpacing.xxl) {
      SettingsSectionCard("Connected accounts") {
        if accounts.isEmpty {
          NoemaInlineState(message: "No provider accounts are connected.", symbol: "server.rack")
        } else {
          ForEach(Array(accounts.enumerated()), id: \.element.providerAccountId) { index, account in
            let local = providerAccount(account)
            if index > 0 { SettingsRowDivider() }
            SettingsRow {
              VStack(alignment: .leading, spacing: NoemaSpacing.xs) {
                HStack(spacing: NoemaSpacing.sm) {
                  Text(account.displayName).font(NoemaFont.bodyEmphasized)
                  Spacer(minLength: NoemaSpacing.sm)
                  NoemaStatusToken(text: account.status.rawValue, tone: account.isActive ? .success : .warning)
                }
                Text("\(account.providerKind) · \(account.authMethod) · \(account.accountKey)")
                  .font(NoemaFont.caption)
                  .foregroundStyle(NoemaColor.contentSecondary)
                if let error = account.lastErrorMessage {
                  Text(error).font(NoemaFont.caption).foregroundStyle(NoemaColor.warning)
                }
                HStack(spacing: NoemaSpacing.md) {
                  if account.authMethod == NoemaAPI.ProviderAuthMethod.secretInput.rawValue {
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
      if let snapshot = settings.snapshot {
        let options = defaultModelOptions(snapshot)
        SettingsSectionCard("Default model") {
          if let preference = snapshot.defaultModelPreference {
            PreferenceSummary(provider: preference.providerKind, account: preference.providerAccountId, profile: preference.modelProfile, mode: preference.selectionMode.rawValue)
          } else {
            NoemaInlineState(message: "No default model selected.", symbol: "circle.dashed")
          }
          let target = SettingsPreferenceTarget(id: "default-model", title: "Default model", kind: .defaultModel, preference: SettingsModel.preference(from: snapshot.defaultModelPreference), options: options)
          SettingsAction(title: "Edit default model", symbol: "slider.horizontal.3", role: nil, disabled: !settings.canMutate || options.isEmpty) { defaultEditor = target }
        }
      }
      SettingsSectionCard("Available providers") {
        SettingsAction(title: "Add provider account", symbol: "plus", role: nil, disabled: !settings.canMutate) { addPresented = true }
        let providers = settings.snapshot?.providerAccountCatalog ?? []
        if providers.isEmpty {
          NoemaInlineState(message: "Provider catalog is unavailable.", symbol: "server.rack")
        } else {
          ForEach(Array(providers.enumerated()), id: \.element.providerKind) { index, provider in
            if index > 0 { SettingsRowDivider() }
            SettingsMetricRow(label: provider.displayName, value: provider.preferredAuthMethod.rawValue)
          }
        }
      }
    }
    .sheet(isPresented: $addPresented) {
      ProviderAccountEditor(settings: settings, catalog: providerCatalog)
    }
    .sheet(item: $secretAccount) { account in ProviderSecretEditor(account: account, settings: settings) }
    .sheet(item: $defaultEditor) { target in SettingsPreferenceEditor(target: target, settings: settings) }
    .sheet(item: $clearAccount) { account in
      SettingsConfirmationSheet(
        title: "Clear provider secret?",
        message: "The stored secret will be removed from this account.",
        confirmTitle: "Clear secret"
      ) {
        Task { await settings.clearProviderSecret(providerAccountID: account.providerAccountID) }
      }
    }
    .sheet(item: $deleteAccount) { account in
      SettingsConfirmationSheet(
        title: "Delete provider account?",
        message: "Delete \(account.displayName), its stored secrets, and any web tool selections using it. This cannot be undone from Settings.",
        confirmTitle: "Delete account"
      ) {
        Task { await settings.deleteProviderAccount(providerAccountID: account.providerAccountID) }
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

  private func defaultModelOptions(_ snapshot: NoemaAPI.SettingsSnapshotQuery.Data) -> [SettingsModelOption] {
    var values = snapshot.agents.flatMap { SettingsModel.modelOptions(from: $0.modelOptions) }
    if values.isEmpty { values = SettingsModel.modelOptions(from: snapshot.memorySettings.modelOptions) }
    var seen = Set<String>()
    return values.filter { seen.insert($0.providerAccountId).inserted }
  }
}

struct ClientsSettings: View {
  let settings: SettingsModel
  let profile: NoemaProfile?
  let onRevoke: (PairedClient) -> Void

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
                Image(uiImage: image)
                  .interpolation(.none)
                  .resizable()
                  .scaledToFit()
                  .frame(maxWidth: 200)
                  .frame(maxWidth: .infinity)
                  .accessibilityLabel("Scan this QR code to pair a Noema client")
              }
              Text(pairing.uri)
                .font(NoemaFont.mono)
                .foregroundStyle(NoemaColor.contentSecondary)
                .textSelection(.enabled)
                .lineLimit(3)
              HStack(spacing: NoemaSpacing.sm) {
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
                  Label("Start pairing", systemImage: "arrow.clockwise")
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
        if settings.isLoading && settings.clients.isEmpty {
          NoemaInlineState(message: "Loading paired clients…", symbol: "arrow.triangle.2.circlepath")
        } else if let error = settings.errorMessage, settings.clients.isEmpty {
          NoemaInlineState(message: error, symbol: "wifi.slash", tone: .warning)
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
          if client.isCurrent { NoemaStatusToken(text: "Current", tone: .success) }
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
