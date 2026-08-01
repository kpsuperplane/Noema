import Apollo
import ApolloAPI
import Foundation
import NoemaAPI

extension SettingsModel {
  func startAuthSubscription(attemptID: String) {
    authSubscription?.cancel()
    guard let client else { return }
    authSubscription = Task { [weak self] in
      do {
        let stream = try client.subscribe(
          subscription: NoemaAPI.ProviderAuthAttemptEventsSubscription(attemptId: attemptID)
        )
        for try await response in stream {
          guard let value = response.data?.providerAuthAttemptEvents else { continue }
          await self?.applyAuth(value)
        }
      } catch is CancellationError {
        return
      } catch {
        self?.setAuthError(error)
      }
    }
  }

  func applyAuth(
    _ value: NoemaAPI.ProviderAuthAttemptEventsSubscription.Data.ProviderAuthAttemptEvents
  ) async {
    auth = ProviderAuthModel(
      attemptID: value.attemptId,
      providerAccountID: value.providerAccountId,
      providerKind: value.providerKind,
      status: value.status.rawValue,
      method: value.method.rawValue,
      verificationURL: URL(string: value.verificationUrl ?? ""),
      userCode: value.userCode,
      instructions: value.instructions,
      errorMessage: value.errorMessage
    )
    if value.status == .completed {
      authSubscription?.cancel()
      auth = nil
      await load(client: client)
    } else if value.status == .failed || value.status == .expired || value.status == .cancelled {
      errorMessage = value.errorMessage ?? "Provider sign-in did not complete."
    }
  }

  func setAuthError(_ error: Error) {
    errorMessage = error.localizedDescription
  }

  static func selectionMode(from value: String) -> NoemaAPI.ModelPreferenceSelectionMode {
    NoemaAPI.ModelPreferenceSelectionMode(rawValue: value) ?? .noemaRecommended
  }

  static func reasoning(from value: String?) -> GraphQLNullable<GraphQLEnum<NoemaAPI.ReasoningEffort>> {
    guard let value, let effort = NoemaAPI.ReasoningEffort(rawValue: value) else { return .none }
    return .some(GraphQLEnum(effort))
  }

  static func optional<T>(_ value: T?) -> GraphQLNullable<T> {
    value.map(GraphQLNullable.some) ?? .none
  }

  static func modelOptions(from values: [NoemaAPI.SettingsSnapshotQuery.Data.Agent.ModelOption]) -> [SettingsModelOption] {
    modelOptions(values, providerKind: \.providerKind, providerAccountID: \.providerAccountId, providerDisplayName: \.providerDisplayName, status: { $0.status.rawValue }, disabledReason: \.disabledReason, profiles: \.profiles, profileID: \.id, profileLabel: \.label, profileDisabledReason: \.disabledReason, profileReasoningEfforts: { $0.reasoningEfforts.map(\.rawValue) }, profileDefaultReasoningEffort: { $0.defaultReasoningEffort?.rawValue })
  }

  static func modelOptions(from values: [NoemaAPI.SettingsSnapshotQuery.Data.MemorySettings.ModelOption]) -> [SettingsModelOption] {
    modelOptions(values, providerKind: \.providerKind, providerAccountID: \.providerAccountId, providerDisplayName: \.providerDisplayName, status: { $0.status.rawValue }, disabledReason: \.disabledReason, profiles: \.profiles, profileID: \.id, profileLabel: \.label, profileDisabledReason: \.disabledReason, profileReasoningEfforts: { $0.reasoningEfforts.map(\.rawValue) }, profileDefaultReasoningEffort: { $0.defaultReasoningEffort?.rawValue })
  }

  static func modelOptions(from values: [NoemaAPI.SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.ModelOption]) -> [SettingsModelOption] {
    modelOptions(values, providerKind: \.providerKind, providerAccountID: \.providerAccountId, providerDisplayName: \.providerDisplayName, status: { $0.status.rawValue }, disabledReason: \.disabledReason, profiles: \.profiles, profileID: \.id, profileLabel: \.label, profileDisabledReason: \.disabledReason, profileReasoningEfforts: { $0.reasoningEfforts.map(\.rawValue) }, profileDefaultReasoningEffort: { $0.defaultReasoningEffort?.rawValue })
  }

  static func modelOptions(from values: [NoemaAPI.SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.ModelOption]) -> [SettingsModelOption] {
    modelOptions(values, providerKind: \.providerKind, providerAccountID: \.providerAccountId, providerDisplayName: \.providerDisplayName, status: { $0.status.rawValue }, disabledReason: \.disabledReason, profiles: \.profiles, profileID: \.id, profileLabel: \.label, profileDisabledReason: \.disabledReason, profileReasoningEfforts: { $0.reasoningEfforts.map(\.rawValue) }, profileDefaultReasoningEffort: { $0.defaultReasoningEffort?.rawValue })
  }

  static func modelOptions(from values: [NoemaAPI.SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.ModelOption]) -> [SettingsModelOption] {
    modelOptions(values, providerKind: \.providerKind, providerAccountID: \.providerAccountId, providerDisplayName: \.providerDisplayName, status: { $0.status.rawValue }, disabledReason: \.disabledReason, profiles: \.profiles, profileID: \.id, profileLabel: \.label, profileDisabledReason: \.disabledReason, profileReasoningEfforts: { $0.reasoningEfforts.map(\.rawValue) }, profileDefaultReasoningEffort: { $0.defaultReasoningEffort?.rawValue })
  }

  private static func modelOptions<Option, Profile>(
    _ values: [Option], providerKind: (Option) -> String, providerAccountID: (Option) -> String,
    providerDisplayName: (Option) -> String, status: (Option) -> String, disabledReason: (Option) -> String?,
    profiles: (Option) -> [Profile], profileID: (Profile) -> String, profileLabel: (Profile) -> String,
    profileDisabledReason: (Profile) -> String?, profileReasoningEfforts: (Profile) -> [String],
    profileDefaultReasoningEffort: (Profile) -> String?
  ) -> [SettingsModelOption] {
    values.map { value in
      SettingsModelOption(
        providerKind: providerKind(value), providerAccountId: providerAccountID(value),
        providerDisplayName: providerDisplayName(value), status: status(value), disabledReason: disabledReason(value),
        profiles: profiles(value).map { profile in
          SettingsModelProfile(id: profileID(profile), label: profileLabel(profile), disabledReason: profileDisabledReason(profile), reasoningEfforts: profileReasoningEfforts(profile), defaultReasoningEffort: profileDefaultReasoningEffort(profile))
        }
      )
    }
  }

  static func preference(from value: NoemaAPI.SettingsSnapshotQuery.Data.Agent.ModelPreference?) -> SettingsPreference? {
    value.map { SettingsPreference(providerKind: $0.providerKind, providerAccountId: $0.providerAccountId, modelProfile: $0.modelProfile, reasoningEffort: $0.reasoningEffort?.rawValue, selectionMode: $0.selectionMode.rawValue) }
  }

  static func preference(from value: NoemaAPI.SettingsSnapshotQuery.Data.MemorySettings.ModelPreference?) -> SettingsPreference? {
    value.map { SettingsPreference(providerKind: $0.providerKind, providerAccountId: $0.providerAccountId, modelProfile: $0.modelProfile, reasoningEffort: $0.reasoningEffort?.rawValue, selectionMode: $0.selectionMode.rawValue) }
  }

  static func preference(from value: NoemaAPI.SettingsSnapshotQuery.Data.WebFetchSettings.Summarizer.ModelPreference?) -> SettingsPreference? {
    value.map { SettingsPreference(providerKind: $0.providerKind, providerAccountId: $0.providerAccountId, modelProfile: $0.modelProfile, reasoningEffort: $0.reasoningEffort?.rawValue, selectionMode: $0.selectionMode.rawValue) }
  }

  static func preference(from value: NoemaAPI.SettingsSnapshotQuery.Data.PrivacySettings.Reviewer.ModelPreference?) -> SettingsPreference? {
    value.map { SettingsPreference(providerKind: $0.providerKind, providerAccountId: $0.providerAccountId, modelProfile: $0.modelProfile, reasoningEffort: $0.reasoningEffort?.rawValue, selectionMode: $0.selectionMode.rawValue) }
  }

  static func preference(from value: NoemaAPI.SettingsSnapshotQuery.Data.UsageSettings.ProgressAudit.ModelPreference?) -> SettingsPreference? {
    value.map { SettingsPreference(providerKind: $0.providerKind, providerAccountId: $0.providerAccountId, modelProfile: $0.modelProfile, reasoningEffort: $0.reasoningEffort?.rawValue, selectionMode: $0.selectionMode.rawValue) }
  }

  static func preference(from value: NoemaAPI.SettingsSnapshotQuery.Data.DefaultModelPreference?) -> SettingsPreference? {
    value.map { SettingsPreference(providerKind: $0.providerKind, providerAccountId: $0.providerAccountId, modelProfile: $0.modelProfile, reasoningEffort: $0.reasoningEffort, selectionMode: $0.selectionMode.rawValue) }
  }

  static func capabilityDetail(from data: NoemaAPI.CapabilityConnectionQuery.Data) -> SettingsCapabilityDetail? {
    guard let connection = data.capabilityConnection else { return nil }
    return SettingsCapabilityDetail(
      kind: NoemaAPI.CapabilityIntegrationKind(rawValue: connection.kind.rawValue) ?? .api,
      definitionId: connection.definitionId, connectionId: connection.connectionId, name: connection.name,
      connectionLabel: connection.connectionLabel, sourceRevision: connection.sourceRevision,
      connectionRevision: connection.connectionRevision, policyRevision: connection.policyRevision,
      status: connection.status, healthStatus: connection.healthStatus, authStatus: connection.authStatus,
      dataSharingPolicy: connection.dataSharingPolicy, unsafeActionPolicy: connection.unsafeActionPolicy,
      toolCount: connection.toolCount, availableToolCount: connection.availableToolCount,
      pendingToolCount: connection.pendingToolCount, defaultedToolCount: connection.defaultedToolCount,
      disabledToolCount: connection.disabledToolCount,
      tools: data.capabilityTools.map { tool in
        SettingsCapabilityTool(
          connectionId: tool.connectionId, toolId: tool.toolId, name: tool.name,
          description: tool.description, enabled: tool.enabled, readOnly: tool.readOnly.value,
          idempotent: tool.idempotent.value, destructive: tool.destructive.value, openWorld: tool.openWorld.value,
          status: tool.status, policyRevision: tool.policyRevision, sourceRevision: tool.sourceRevision,
          decisionPreview: tool.decisionPreview
        )
      }
    )
  }

  static func client(from value: NoemaAPI.ClientsQuery.Data.Client) -> PairedClient {
    PairedClient(id: value.clientId, displayName: value.displayName, createdAt: value.createdAt, revokedAt: value.revokedAt, isCurrent: value.isCurrent)
  }

  static func taskModelPool(
    from value: NoemaAPI.NativeTaskModelPoolsQuery.Data.TaskModelPool
  ) -> SettingsTaskModelPool {
    SettingsTaskModelPool(
      poolEntryID: value.poolEntryId,
      complexity: NoemaAPI.TaskComplexity(rawValue: value.complexity.rawValue) ?? .medium,
      label: value.label,
      providerKind: value.providerKind,
      providerAccountID: value.providerAccountId,
      modelProfile: value.modelProfile,
      reasoningEffort: value.reasoningEffort?.rawValue,
      selectionMode: value.selectionMode.rawValue,
      enabled: value.enabled,
      sortOrder: value.sortOrder
    )
  }

  static func taskModelPool(
    from value: NoemaAPI.NativeUpdateTaskModelPoolEntryMutation.Data.UpdateTaskModelPoolEntry
  ) -> SettingsTaskModelPool {
    SettingsTaskModelPool(
      poolEntryID: value.poolEntryId,
      complexity: NoemaAPI.TaskComplexity(rawValue: value.complexity.rawValue) ?? .medium,
      label: value.label,
      providerKind: value.providerKind,
      providerAccountID: value.providerAccountId,
      modelProfile: value.modelProfile,
      reasoningEffort: value.reasoningEffort?.rawValue,
      selectionMode: value.selectionMode.rawValue,
      enabled: value.enabled,
      sortOrder: value.sortOrder
    )
  }
}

enum SettingsError: LocalizedError {
  case unavailable
  case mutationsUnavailable
  case server(String)

  var errorDescription: String? {
    switch self {
    case .unavailable: "Settings are unavailable."
    case .mutationsUnavailable: "Changes are unavailable while Noema is offline."
    case .server(let message): message
    }
  }
}
