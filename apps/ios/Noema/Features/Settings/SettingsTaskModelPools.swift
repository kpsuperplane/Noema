import Apollo
import ApolloAPI
import NoemaAPI

struct SettingsTaskModelPool: Identifiable, Hashable {
  var id: String { poolEntryID }
  let poolEntryID: String
  let complexity: NoemaAPI.TaskComplexity
  let label: String?
  let providerKind: String
  let providerAccountID: String
  let modelProfile: String?
  let reasoningEffort: String?
  let selectionMode: String
  let enabled: Bool
  let sortOrder: Int

  var displayName: String {
    complexity == .difficult ? "High" : complexity.rawValue.capitalized
  }

  var preference: SettingsPreference {
    SettingsPreference(
      providerKind: providerKind,
      providerAccountId: providerAccountID,
      modelProfile: modelProfile,
      reasoningEffort: reasoningEffort,
      selectionMode: selectionMode
    )
  }
}

extension SettingsModel {
  func loadTaskModelPools(client: ApolloClient? = nil) async {
    guard let client = client ?? self.client else { return }
    isLoadingTaskModelPools = true
    taskModelPoolsErrorMessage = nil
    defer { isLoadingTaskModelPools = false }
    do {
      let stream = try client.fetch(query: NoemaAPI.NativeTaskModelPoolsQuery(), cachePolicy: .cacheAndNetwork)
      for try await response in stream {
        if let values = response.data?.taskModelPools {
          taskModelPools = values.map(Self.taskModelPool(from:))
          hasLoadedTaskModelPools = true
        }
        if let message = response.errors?.first?.message { taskModelPoolsErrorMessage = message }
      }
    } catch {
      if taskModelPools.isEmpty { taskModelPoolsErrorMessage = "Task model pools could not be loaded." }
    }
  }

  func updateTaskModelPool(
    _ pool: SettingsTaskModelPool,
    preference: SettingsPreference? = nil,
    enabled: Bool? = nil
  ) async -> Bool {
    guard canMutate, let client else { return false }
    let next = preference ?? pool.preference
    let providerKind = snapshot?.agents
      .first(where: { $0.isPrimary })?.modelOptions
      .first(where: { $0.providerAccountId == next.providerAccountId })?.providerKind ?? next.providerKind
    let input = NoemaAPI.TaskModelPoolEntryInput(
      complexity: GraphQLEnum(pool.complexity),
      label: SettingsModel.optional(pool.label),
      providerKind: providerKind,
      providerAccountId: next.providerAccountId,
      selectionMode: GraphQLEnum(SettingsModel.selectionMode(from: next.selectionMode)),
      modelProfile: SettingsModel.optional(next.modelProfile),
      reasoningEffort: SettingsModel.reasoning(from: next.reasoningEffort),
      enabled: enabled ?? pool.enabled,
      sortOrder: Int32(clamping: pool.sortOrder)
    )
    isMutating = true
    defer { isMutating = false }
    do {
      let response = try await client.perform(
        mutation: NoemaAPI.NativeUpdateTaskModelPoolEntryMutation(
          poolEntryId: pool.poolEntryID,
          input: input
        )
      )
      if let message = response.errors?.first?.message { throw SettingsError.server(message) }
      guard let value = response.data?.updateTaskModelPoolEntry else { throw SettingsError.unavailable }
      let updated = Self.taskModelPool(from: value)
      taskModelPools = taskModelPools.map { $0.id == updated.id ? updated : $0 }
      taskModelPoolsErrorMessage = nil
      return true
    } catch {
      taskModelPoolsErrorMessage = error.localizedDescription
      return false
    }
  }
}
