import Apollo
import Foundation
import NoemaAPI
import Observation

enum OnboardingStage: Equatable {
  case loading
  case chooseProvider
  case authenticate
  case localModel
  case models
  case failed(String)
}

struct ProviderAccountModel: Identifiable, Equatable {
  let id: String
  let kind: String
  let displayName: String
  let authMethod: String
  let status: String
  let isActive: Bool
}

struct ProviderCatalogModel: Identifiable, Equatable {
  let kind: String
  let displayName: String
  let authMethod: String

  var id: String { kind }
}

struct ProviderAuthModel: Equatable {
  let attemptID: String
  let providerAccountID: String
  let providerKind: String
  let status: String
  let method: String
  let verificationURL: URL?
  let userCode: String?
  let instructions: String?
  let errorMessage: String?
}

struct LocalModelSetupModel: Equatable {
  let isReady: Bool
  let runtimeStatus: String
  let modelID: String?
  let modelName: String?
  let file: String?
  let license: String?
  let downloadGB: Double?
  let backend: String?
  let hardwareExplanation: String?
  let installationID: String?
  let installationStatus: String?
  let completedBytes: Int
  let totalBytes: Int?
  let errorMessage: String?
}

struct ModelOption: Identifiable, Equatable {
  let id: String
  let label: String
  let disabledReason: String?
  let reasoningEfforts: [String]
  let defaultReasoningEffort: String?
}

struct ModelRecommendation: Equatable {
  let useCase: String
  let modelProfile: String
  let reasoningEffort: String?
  let disabledReason: String?
}

struct ModelSelectionDraft: Equatable {
  var mode: String
  var profile: String?
  var reasoning: String?
}

@MainActor
@Observable
final class OnboardingModel {
  private(set) var stage: OnboardingStage = .loading
  private(set) var accounts: [ProviderAccountModel] = []
  private(set) var catalog: [ProviderCatalogModel] = []
  private(set) var auth: ProviderAuthModel?
  private(set) var localModel: LocalModelSetupModel?
  private(set) var modelProviderName = ""
  private(set) var modelProviderKind = ""
  private(set) var modelProviderID = ""
  private(set) var modelSetupLoadingKind: String?
  private(set) var modelOptions: [ModelOption] = []
  private(set) var modelRecommendations: [ModelRecommendation] = []
  private(set) var errorMessage: String?
  private(set) var isSaving = false
  private(set) var localSaving = false
  var draft: [String: ModelSelectionDraft] = [:]

  let client: ApolloClient
  private var authSubscription: Task<Void, Never>?
  private var localSubscription: Task<Void, Never>?

  init(client: ApolloClient) { self.client = client }

  func start() async {
    stage = .loading
    do {
      try await refreshProviders()
      try await refreshLocalModel()
      if localModel?.isReady == false, localModel?.modelID != nil {
        stage = .localModel
        startLocalSubscription()
      } else {
        stage = .chooseProvider
      }
    } catch {
      fail(error)
    }
  }

  func choose(_ account: ProviderAccountModel) async {
    errorMessage = nil
    if account.status == "AUTHENTICATED" {
      do { try await loadModels(for: account) } catch { errorMessage = error.localizedDescription }
      return
    }
    do {
      let method = NoemaAPI.ProviderAuthMethod(rawValue: account.authMethod) ?? .oauthPkce
      let input = NoemaAPI.StartProviderAuthAttemptInput(
        providerKind: account.kind,
        providerAccountId: .some(account.id),
        method: GraphQLEnum(method)
      )
      let response = try await client.perform(mutation: NoemaAPI.StartProviderAuthAttemptMutation(input: input))
      guard let attempt = response.data?.startProviderAuthAttempt else { throw OnboardingError.emptyResponse }
      auth = ProviderAuthModel(attemptID: attempt.attemptId, providerAccountID: attempt.providerAccountId, providerKind: attempt.providerKind, status: attempt.status.rawValue, method: attempt.method.rawValue, verificationURL: URL(string: attempt.verificationUrl ?? ""), userCode: attempt.userCode, instructions: attempt.instructions, errorMessage: attempt.errorMessage)
      stage = .authenticate
      startAuthSubscription(attemptID: attempt.attemptId)
    } catch { errorMessage = error.localizedDescription }
  }

  /// Starts a fresh account auth flow from the provider catalog. Catalog rows
  /// intentionally carry no account id, so the server creates the account.
  func choose(_ provider: ProviderCatalogModel) async {
    errorMessage = nil
    do {
      let method = NoemaAPI.ProviderAuthMethod(rawValue: provider.authMethod) ?? .oauthPkce
      let input = NoemaAPI.StartProviderAuthAttemptInput(
        providerKind: provider.kind,
        providerAccountId: .none,
        method: GraphQLEnum(method)
      )
      let response = try await client.perform(mutation: NoemaAPI.StartProviderAuthAttemptMutation(input: input))
      guard let attempt = response.data?.startProviderAuthAttempt else { throw OnboardingError.emptyResponse }
      auth = ProviderAuthModel(attemptID: attempt.attemptId, providerAccountID: attempt.providerAccountId, providerKind: attempt.providerKind, status: attempt.status.rawValue, method: attempt.method.rawValue, verificationURL: URL(string: attempt.verificationUrl ?? ""), userCode: attempt.userCode, instructions: attempt.instructions, errorMessage: attempt.errorMessage)
      stage = .authenticate
      startAuthSubscription(attemptID: attempt.attemptId)
    } catch { errorMessage = error.localizedDescription }
  }

  /// Creates the write-only OpenRouter account used by the onboarding API-key
  /// fallback. Keep this on the onboarding model so the visual flow can stay
  /// inline with the provider card instead of routing through Settings.
  func connectOpenRouterAPIKey(_ secret: String) async {
    let value = secret.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !value.isEmpty, !isSaving else { return }
    isSaving = true
    errorMessage = nil
    defer { isSaving = false }
    do {
      let input = NoemaAPI.CreateProviderAccountInput(
        providerKind: "openrouter",
        displayName: .none,
        secret: value,
        authMethod: GraphQLEnum(.secretInput)
      )
      let response = try await client.perform(
        mutation: NoemaAPI.SettingsCreateProviderAccountMutation(input: input)
      )
      guard let account = response.data?.createProviderAccount else {
        throw OnboardingError.emptyResponse
      }
      try await refreshProviders()
      try await loadModels(for: ProviderAccountModel(
        id: account.providerAccountId,
        kind: account.providerKind,
        displayName: account.displayName,
        authMethod: account.authMethod,
        status: account.status.rawValue,
        isActive: account.isActive
      ))
    } catch {
      errorMessage = error.localizedDescription
    }
  }

  func cancelAuthentication() async {
    guard let auth else { return }
    do {
      let input = NoemaAPI.CancelProviderAuthAttemptInput(attemptId: auth.attemptID)
      _ = try await client.perform(mutation: NoemaAPI.CancelProviderAuthAttemptMutation(input: input))
      authSubscription?.cancel()
      self.auth = nil
      stage = .chooseProvider
    } catch { errorMessage = error.localizedDescription }
  }

  /// Refetches the durable attempt after Safari is dismissed. A browser return
  /// can race the subscription while the app is backgrounded, so this query is
  /// the reconciliation authority before showing the next onboarding stage.
  func refreshAuthentication() async {
    guard let auth else { return }
    do {
      let response = try await client.fetch(query: NoemaAPI.ProviderAuthAttemptQuery(attemptId: auth.attemptID), cachePolicy: .networkOnly)
      guard let value = response.data?.providerAuthAttempt else { return }
      guard let status = value.status.value else { return }
      await updateAuth(
        attemptID: value.attemptId,
        providerAccountID: value.providerAccountId,
        providerKind: value.providerKind,
        status: status.rawValue,
        method: value.method.rawValue,
        verificationURL: value.verificationUrl,
        userCode: value.userCode,
        instructions: value.instructions,
        errorMessage: value.errorMessage,
        statusValue: status
      )
    } catch { errorMessage = error.localizedDescription }
  }

  func installRecommendedLocalModel() async {
    guard let modelID = localModel?.modelID, !localSaving else { return }
    localSaving = true
    errorMessage = nil
    defer { localSaving = false }
    do {
      let input = NoemaAPI.InstallLocalModelInput(modelId: modelID, file: localModel?.file.map { .some($0) } ?? .none)
      let response = try await client.perform(mutation: NoemaAPI.InstallLocalModelMutation(input: input))
      if let installation = response.data?.installLocalModel { applyInstallation(installation) }
      stage = .localModel
      startLocalSubscription()
    } catch { errorMessage = error.localizedDescription }
  }

  func cancelLocalInstall() async {
    guard let id = localModel?.installationID, !localSaving else { return }
    localSaving = true
    errorMessage = nil
    defer { localSaving = false }
    do {
      let response = try await client.perform(mutation: NoemaAPI.CancelLocalModelInstallMutation(installationId: id))
      if let installation = response.data?.cancelLocalModelInstall { applyInstallation(installation) }
    } catch { errorMessage = error.localizedDescription }
  }

  func updateSelection(_ key: String, profile: String?) {
    guard var value = draft[key] else { return }
    value.mode = profile == nil ? "NOEMA_RECOMMENDED" : "EXPLICIT_PROFILE"
    value.profile = profile
    value.reasoning = profile.flatMap { id in
      modelOptions.first(where: { $0.id == id })?.defaultReasoningEffort
        ?? modelOptions.first(where: { $0.id == id })?.reasoningEfforts.first
    }
    draft[key] = value
  }

  func updateRecommended(_ key: String) {
    draft[key] = ModelSelectionDraft(
      mode: "NOEMA_RECOMMENDED",
      profile: nil,
      reasoning: recommendedReasoning(for: key)
    )
  }

  func updateReasoning(_ key: String, effort: String) {
    guard var value = draft[key] else { return }
    value.reasoning = effort
    draft[key] = value
  }

  func reasoningOptions(for key: String, draft: ModelSelectionDraft) -> [String] {
    if draft.mode == "NOEMA_RECOMMENDED" {
      guard let effort = recommendedReasoning(for: key) else { return [] }
      return [effort]
    }
    guard let profile = modelOptions.first(where: { $0.id == draft.profile }) else { return [] }
    return profile.reasoningEfforts
  }

  func recommendedReasoning(for key: String) -> String? {
    modelRecommendations.first(where: { $0.useCase == useCase(for: key) })?.reasoningEffort
  }

  func recommendation(for key: String) -> ModelRecommendation? {
    modelRecommendations.first { $0.useCase == useCase(for: key) }
  }

  var canConfirmModels: Bool {
    let keys = [
      "noema", "simpleTasks", "mediumTasks", "difficultTasks", "taskReviewer",
      "webFetchSummarizer", "toolProgressAudit", "actionReviewer", "memoryConsolidation"
    ]
    return keys.allSatisfy { key in
      key == "actionReviewer" && modelProviderKind == "local_models"
        ? true
        : isValidSelection(draft[key], for: key)
    }
  }

  func confirmModels() async {
    guard !isSaving, !modelProviderID.isEmpty else { return }
    errorMessage = nil
    guard canConfirmModels else {
      errorMessage = "Choose a valid model for each setup role."
      return
    }
    isSaving = true
    defer { isSaving = false }
    do {
      let input = NoemaAPI.ConfirmOnboardingModelSelectionsInput(
        providerAccountId: modelProviderID,
        noema: selectionInput("noema"),
        simpleTasks: selectionInput("simpleTasks"),
        mediumTasks: selectionInput("mediumTasks"),
        difficultTasks: selectionInput("difficultTasks"),
        taskReviewer: selectionInput("taskReviewer"),
        webFetchSummarizer: selectionInput("webFetchSummarizer"),
        toolProgressAudit: selectionInput("toolProgressAudit"),
        actionReviewer: draft["actionReviewer"] == nil ? .none : .some(selectionInput("actionReviewer", allowEmpty: true)),
        memoryConsolidation: selectionInput("memoryConsolidation")
      )
      let response = try await client.perform(mutation: NoemaAPI.ConfirmOnboardingModelSelectionsMutation(input: input))
      guard response.data?.confirmOnboardingModelSelections.isUserOnboarded == true else { throw OnboardingError.notReady }
      stage = .chooseProvider
    } catch { errorMessage = error.localizedDescription }
  }

  func retry() async { await start() }

  func chooseDifferentProvider() {
    authSubscription?.cancel()
    auth = nil
    errorMessage = nil
    stage = .chooseProvider
  }

  private func refreshProviders() async throws {
    let response = try await client.fetch(query: NoemaAPI.ProviderAccountsQuery(), cachePolicy: .networkOnly)
    guard let data = response.data else { throw OnboardingError.emptyResponse }
    catalog = data.providerAccountCatalog.map { ProviderCatalogModel(kind: $0.providerKind, displayName: $0.displayName, authMethod: $0.preferredAuthMethod.rawValue) }
    accounts = data.providerAccounts.map { ProviderAccountModel(id: $0.providerAccountId, kind: $0.providerKind, displayName: $0.displayName, authMethod: $0.authMethod, status: $0.status.rawValue, isActive: $0.isActive) }
  }

  private func refreshLocalModel() async throws {
    let response = try await client.fetch(query: NoemaAPI.LocalModelSetupQuery(), cachePolicy: .networkOnly)
    guard let setup = response.data?.localModelSetup else { throw OnboardingError.emptyResponse }
    let model = setup.recommendedModel
    let installation = setup.installation
    localModel = LocalModelSetupModel(
      isReady: setup.isReady,
      runtimeStatus: setup.runtimeStatus.rawValue,
      modelID: model?.modelId,
      modelName: model?.name ?? installation?.name,
      file: model?.selectedBuild?.file ?? installation?.file,
      license: model?.license,
      downloadGB: model?.selectedBuild?.downloadGb,
      backend: model?.hardwareFit?.backend.rawValue ?? installation?.backend?.rawValue,
      hardwareExplanation: model?.hardwareFit?.explanation,
      installationID: installation?.installationId,
      installationStatus: installation?.status.rawValue,
      completedBytes: installation?.completedBytes ?? 0,
      totalBytes: installation?.totalBytes,
      errorMessage: installation?.errorMessage
    )
  }

  private func loadModels(for account: ProviderAccountModel) async throws {
    modelSetupLoadingKind = account.kind
    defer { modelSetupLoadingKind = nil }
    let response = try await client.fetch(query: NoemaAPI.OnboardingModelSetupQuery(providerAccountId: account.id), cachePolicy: .networkOnly)
    guard let setup = response.data?.onboardingModelSetup else { throw OnboardingError.emptyResponse }
    modelProviderName = setup.providerDisplayName
    modelProviderKind = setup.providerKind
    modelProviderID = setup.providerAccountId
    modelOptions = setup.profiles.map {
      ModelOption(
        id: $0.id,
        label: $0.label,
        disabledReason: $0.disabledReason,
        reasoningEfforts: $0.reasoningEfforts.map(\.rawValue),
        defaultReasoningEffort: $0.defaultReasoningEffort?.rawValue
      )
    }
    modelRecommendations = setup.recommendations.map {
      ModelRecommendation(
        useCase: $0.useCase.rawValue,
        modelProfile: $0.modelProfile,
        reasoningEffort: $0.reasoningEffort?.rawValue,
        disabledReason: $0.disabledReason
      )
    }
    let proposals: [(String, String, String?, String?)] = [
      ("noema", setup.proposedSelections.noema.selectionMode.rawValue, setup.proposedSelections.noema.modelProfile, setup.proposedSelections.noema.reasoningEffort?.rawValue),
      ("simpleTasks", setup.proposedSelections.simpleTasks.selectionMode.rawValue, setup.proposedSelections.simpleTasks.modelProfile, setup.proposedSelections.simpleTasks.reasoningEffort?.rawValue),
      ("mediumTasks", setup.proposedSelections.mediumTasks.selectionMode.rawValue, setup.proposedSelections.mediumTasks.modelProfile, setup.proposedSelections.mediumTasks.reasoningEffort?.rawValue),
      ("difficultTasks", setup.proposedSelections.difficultTasks.selectionMode.rawValue, setup.proposedSelections.difficultTasks.modelProfile, setup.proposedSelections.difficultTasks.reasoningEffort?.rawValue),
      ("taskReviewer", setup.proposedSelections.taskReviewer.selectionMode.rawValue, setup.proposedSelections.taskReviewer.modelProfile, setup.proposedSelections.taskReviewer.reasoningEffort?.rawValue),
      ("webFetchSummarizer", setup.proposedSelections.webFetchSummarizer.selectionMode.rawValue, setup.proposedSelections.webFetchSummarizer.modelProfile, setup.proposedSelections.webFetchSummarizer.reasoningEffort?.rawValue),
      ("toolProgressAudit", setup.proposedSelections.toolProgressAudit.selectionMode.rawValue, setup.proposedSelections.toolProgressAudit.modelProfile, setup.proposedSelections.toolProgressAudit.reasoningEffort?.rawValue),
      ("memoryConsolidation", setup.proposedSelections.memoryConsolidation.selectionMode.rawValue, setup.proposedSelections.memoryConsolidation.modelProfile, setup.proposedSelections.memoryConsolidation.reasoningEffort?.rawValue)
    ]
    draft = Dictionary(uniqueKeysWithValues: proposals.map { ($0.0, ModelSelectionDraft(mode: $0.1, profile: $0.2, reasoning: $0.3)) })
    if let action = setup.proposedSelections.actionReviewer { draft["actionReviewer"] = ModelSelectionDraft(mode: action.selectionMode.rawValue, profile: action.modelProfile, reasoning: action.reasoningEffort?.rawValue) }
    stage = .models
  }

  private func selectionInput(_ key: String, allowEmpty: Bool = false) -> NoemaAPI.OnboardingModelSelectionInput {
    let value = draft[key] ?? ModelSelectionDraft(mode: "NOEMA_RECOMMENDED", profile: nil, reasoning: nil)
    let mode = NoemaAPI.ModelPreferenceSelectionMode(rawValue: value.mode) ?? .noemaRecommended
    let reasoning = value.reasoning.flatMap { NoemaAPI.ReasoningEffort(rawValue: $0) }.map(GraphQLEnum.init)
    return NoemaAPI.OnboardingModelSelectionInput(selectionMode: GraphQLEnum(mode), modelProfile: value.profile.map { .some($0) } ?? (allowEmpty ? .none : .none), reasoningEffort: reasoning.map { .some($0) } ?? .none)
  }

  private func useCase(for key: String) -> String {
    switch key {
    case "noema": "PRIMARY"
    case "simpleTasks": "TASK_SIMPLE"
    case "mediumTasks": "TASK_MEDIUM"
    case "difficultTasks": "TASK_DIFFICULT"
    case "taskReviewer": "TASK_REVIEWER"
    case "webFetchSummarizer": "WEB_FETCH_SUMMARIZER"
    case "toolProgressAudit": "TOOL_PROGRESS_AUDIT"
    case "actionReviewer": "ACTION_REVIEWER"
    case "memoryConsolidation": "MEMORY_CONSOLIDATION"
    default: "PRIMARY"
    }
  }

  private func isValidSelection(_ selection: ModelSelectionDraft?, for key: String) -> Bool {
    guard let selection else { return false }
    if selection.mode == "NOEMA_RECOMMENDED" {
      return modelRecommendations.contains {
        $0.useCase == useCase(for: key) && $0.disabledReason == nil
      }
    }
    guard let profile = modelOptions.first(where: { $0.id == selection.profile && $0.disabledReason == nil }) else {
      return false
    }
    if profile.reasoningEfforts.isEmpty {
      return selection.reasoning == nil
    }
    return selection.reasoning.map(profile.reasoningEfforts.contains) ?? false
  }

  private func startAuthSubscription(attemptID: String) {
    authSubscription?.cancel()
    authSubscription = Task { [weak self] in
      do {
        let stream = try self?.client.subscribe(subscription: NoemaAPI.ProviderAuthAttemptEventsSubscription(attemptId: attemptID))
        if let stream {
          for try await response in stream {
            guard let value = response.data?.providerAuthAttemptEvents else { continue }
            await self?.apply(value)
          }
        }
      } catch { self?.errorMessage = error.localizedDescription }
    }
  }

  private func apply(_ value: NoemaAPI.ProviderAuthAttemptEventsSubscription.Data.ProviderAuthAttemptEvents) async {
    guard let status = value.status.value else { return }
    await updateAuth(
      attemptID: value.attemptId,
      providerAccountID: value.providerAccountId,
      providerKind: value.providerKind,
      status: status.rawValue,
      method: value.method.rawValue,
      verificationURL: value.verificationUrl,
      userCode: value.userCode,
      instructions: value.instructions,
      errorMessage: value.errorMessage,
      statusValue: status
    )
  }

  private func updateAuth(
    attemptID: String,
    providerAccountID: String,
    providerKind: String,
    status: String,
    method: String,
    verificationURL: String?,
    userCode: String?,
    instructions: String?,
    errorMessage: String?,
    statusValue: NoemaAPI.ProviderAuthAttemptStatus
  ) async {
    auth = ProviderAuthModel(attemptID: attemptID, providerAccountID: providerAccountID, providerKind: providerKind, status: status, method: method, verificationURL: URL(string: verificationURL ?? ""), userCode: userCode, instructions: instructions, errorMessage: errorMessage)
    if statusValue == .completed {
      authSubscription?.cancel()
      do {
        try await refreshProviders()
        if let account = accounts.first(where: { $0.id == providerAccountID }) { try await loadModels(for: account) }
      } catch { self.errorMessage = error.localizedDescription }
    } else if statusValue == .failed || statusValue == .expired || statusValue == .cancelled {
      self.errorMessage = errorMessage ?? "Provider sign-in did not complete."
    }
  }

  private func startLocalSubscription() {
    localSubscription?.cancel()
    localSubscription = Task { [weak self] in
      do {
        let stream = try self?.client.subscribe(subscription: NoemaAPI.LocalModelEventsSubscription(after: .none))
        if let stream {
          for try await response in stream {
            if let installation = response.data?.localModelEvents.installation { self?.applyInstallation(installation) }
          }
        }
      } catch { self?.errorMessage = error.localizedDescription }
    }
  }

  private func applyInstallation(_ value: NoemaAPI.InstallLocalModelMutation.Data.InstallLocalModel) {
    updateLocalModel(modelID: value.modelId, modelName: value.name, file: value.file, status: value.status, installationID: value.installationId, completedBytes: value.completedBytes, totalBytes: value.totalBytes, isActive: value.isActive, errorMessage: value.errorMessage)
  }

  private func applyInstallation(_ value: NoemaAPI.CancelLocalModelInstallMutation.Data.CancelLocalModelInstall) {
    updateLocalModel(modelID: value.modelId, modelName: value.name, file: value.file, status: value.status, installationID: value.installationId, completedBytes: value.completedBytes, totalBytes: value.totalBytes, isActive: value.isActive, errorMessage: value.errorMessage)
  }

  private func applyInstallation(_ value: NoemaAPI.LocalModelEventsSubscription.Data.LocalModelEvents.Installation) {
    updateLocalModel(modelID: value.modelId, modelName: value.name, file: value.file, status: value.status, installationID: value.installationId, completedBytes: value.completedBytes, totalBytes: value.totalBytes, isActive: value.isActive, errorMessage: value.errorMessage)
  }

  private func updateLocalModel(
    modelID: String,
    modelName: String,
    file: String,
    status: GraphQLEnum<NoemaAPI.LocalModelInstallationStatus>,
    installationID: String,
    completedBytes: Int,
    totalBytes: Int?,
    isActive: Bool,
    errorMessage: String?
  ) {
    localModel = LocalModelSetupModel(
      isReady: isActive && status == .installed,
      runtimeStatus: localModel?.runtimeStatus ?? "STARTING",
      modelID: modelID,
      modelName: modelName,
      file: file,
      license: localModel?.license,
      downloadGB: localModel?.downloadGB,
      backend: localModel?.backend,
      hardwareExplanation: localModel?.hardwareExplanation,
      installationID: installationID,
      installationStatus: status.rawValue,
      completedBytes: completedBytes,
      totalBytes: totalBytes,
      errorMessage: errorMessage
    )
    if isActive && status == .installed { stage = .localModel }
  }

  private func fail(_ error: Error) {
    errorMessage = error.localizedDescription
    stage = .failed(errorMessage ?? "Setup could not continue.")
  }
}

enum OnboardingError: LocalizedError {
  case emptyResponse
  case notReady
  var errorDescription: String? {
    switch self {
    case .emptyResponse: "Noema returned an empty setup response."
    case .notReady: "Noema did not accept the model selections yet."
    }
  }
}
