import Apollo
import ApolloAPI
import Foundation
import NoemaAPI
import Observation

@MainActor
@Observable
final class TasksModel {
  static let personalWorkspaceId = "workspace:personal"

  let client: ApolloClient
  let profile: NoemaProfile?
  let workspaceId: String

  private(set) var workspace: TasksWorkspaceSnapshot?
  private(set) var projects: [TasksProjectSnapshot] = []
  private(set) var acpAgents: [TasksAcpAgentSnapshot] = []
  private(set) var columns: [TasksColumnSnapshot] = []
  private(set) var tasks: [TasksTaskRow] = []
  private(set) var needsYou: [TasksAttentionRow] = []
  private(set) var pendingInterventions: [HumanIntervention] = []
  private(set) var detail: TasksDetailSnapshot?
  private(set) var runItems: [TasksRunItemSnapshot] = []
  private(set) var history: [TasksTaskRow] = []
  private(set) var selectedProjectId: String?
  private(set) var eventCursor: String?
  private(set) var isRefreshing = false
  private(set) var isLoadingDetail = false
  private(set) var isLoadingOlderRunItems = false
  private(set) var isLoadingMoreTasks = false
  private(set) var isLoadingMoreHistory = false
  var isConnected: Bool { !connectionStatus.isDisconnected }
  private(set) var lastError: String?
  private(set) var hasLoadedTasks = false
  private(set) var commandTaskIDs = Set<String>()
  private(set) var commandErrors: [String: String] = [:]
  private(set) var interventionErrors: [String: String] = [:]
  private(set) var tasksErrorMessage: String?
  private(set) var interventionsErrorMessage: String?
  private(set) var projectsErrorMessage: String?
  private(set) var historyErrorMessage: String?

  private var eventSubscription: Task<Void, Never>?
  private var taskSubscription: Task<Void, Never>?
  private var runtimeSubscription: Task<Void, Never>?
  private var detailCoreWatcher: GraphQLQueryWatcher<TasksDetailCoreQuery>?
  private var detailActivityWatcher: GraphQLQueryWatcher<TasksDetailActivityQuery>?
  private var detailOutcomeWatcher: GraphQLQueryWatcher<TasksDetailOutcomeQuery>?
  private var detailRequestID = UUID()
  private var detailTaskID: String?
  private var hydratedRunIDs = Set<String>()
  private var runItemEndCursor: [String: String] = [:]
  private var runItemHasNextPage: [String: Bool] = [:]
  private var tasksEndCursor: String?
  private var historyEndCursor: String?
  private var recurrenceCache: [String: TasksRecurrenceSnapshot] = [:]
  private var hasLoadedAcpAgents = false
  private(set) var hasMoreTasks = false
  private(set) var hasMoreHistory = false
  private var started = false
  private let connectionStatus: NoemaConnectionStatus

  init(
    client: ApolloClient,
    profile: NoemaProfile? = nil,
    connectionStatus: NoemaConnectionStatus,
    workspaceId: String = TasksModel.personalWorkspaceId
  ) {
    self.client = client
    self.profile = profile
    self.connectionStatus = connectionStatus
    self.workspaceId = workspaceId
  }

  func start() async {
    guard !started else { return }
    started = true
    await restoreCachedSnapshot()
    await refresh()
    subscribeToTasks()
  }

  /// Re-read the authoritative task snapshots before accepting new live events.
  ///
  /// A reconnect can span a gap in the event stream, so the cursor is kept while
  /// the subscriptions are torn down and rebuilt only after a cache/network read.
  func recoverConnection() async {
    started = true
    eventSubscription?.cancel()
    taskSubscription?.cancel()
    runtimeSubscription?.cancel()
    eventSubscription = nil
    taskSubscription = nil
    runtimeSubscription = nil
    await refresh()
    guard isConnected else { return }
    subscribeToTasks()
    if let taskId = detailTaskID {
      subscribeToTask(taskId)
      subscribeToRuntime(taskId)
    }
  }

  func refresh() async {
    guard !isRefreshing else { return }
    isRefreshing = true
    defer { isRefreshing = false }

    let overviewQuery = TasksOverviewQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId))
    let projectsQuery = TasksProjectsQuery(workspaceId: workspaceId, includeArchived: true, first: .some(100), after: .none)
    let needsQuery = TasksNeedsYouQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), first: .some(50), after: .none)
    let pendingQuery = NoemaAPI.PendingChatInterventionsQuery(
      conversationId: .none,
      taskId: .none,
      projectId: optional(selectedProjectId),
      first: 50
    )
    let listInput = TaskListInput(workspaceId: workspaceId, projectId: optional(selectedProjectId), scope: GraphQLEnum(.active))
    let listQuery = TasksListQuery(input: listInput, first: .some(100), after: .none)
    var refreshFailed = false

    do {
      if let overview = try await fetch(overviewQuery).data { applyOverview(overview.tasksOverview) }
    } catch {
      refreshFailed = true
      record(error)
    }
    do {
      if let projects = try await fetch(projectsQuery).data { applyProjects(projects.projects) }
      projectsErrorMessage = nil
    } catch {
      refreshFailed = true
      projectsErrorMessage = error.localizedDescription
      record(error)
    }
    if !hasLoadedAcpAgents {
      do {
        if let agents = try await fetch(SettingsAcpAgentsQuery()).data {
          acpAgents = agents.acpAgents.map {
            TasksAcpAgentSnapshot(
              id: $0.agentId,
              displayName: $0.displayName,
              enabled: $0.enabled,
              authStatus: $0.authStatus.rawValue,
              healthStatus: $0.healthStatus.rawValue,
              implementationName: $0.implementationName,
              implementationVersion: $0.implementationVersion,
              lastError: $0.lastError
            )
          }
          hasLoadedAcpAgents = true
        }
      } catch {
        // ACP setup is optional; task list remains usable with the built-in executor.
      }
    }
    do {
      if let needsYou = try await fetch(needsQuery).data { applyNeedsYou(needsYou.needsYou) }
      interventionsErrorMessage = nil
    } catch {
      refreshFailed = true
      interventionsErrorMessage = error.localizedDescription
      record(error)
    }
    do {
      if let allTasks = try await fetch(listQuery).data {
        applyTaskList(allTasks)
      }
      tasksErrorMessage = nil
    } catch {
      refreshFailed = true
      tasksErrorMessage = error.localizedDescription
      record(error)
    }
    do {
      if let pending = try await fetch(pendingQuery).data {
        pendingInterventions = pending.pendingHumanInterventions.compactMap(HumanIntervention.init).filter {
          if case .attention = $0 { return false }
          return true
        }
        let visibleIDs = Set(pendingInterventions.map(\.id))
        interventionErrors = interventionErrors.filter { visibleIDs.contains($0.key) }
      }
    } catch {
      refreshFailed = true
      interventionsErrorMessage = error.localizedDescription
      record(error)
    }
    await loadHistory()
    if historyErrorMessage != nil { refreshFailed = true }
    if isConnected, !refreshFailed {
      lastError = nil
    }
  }

  func selectProject(_ projectId: String?) async {
    selectedProjectId = projectId
    hasLoadedTasks = false
    tasks = []
    needsYou = []
    pendingInterventions = []
    history = []
    await restoreCachedSnapshot()
    await refresh()
  }

  func loadDetail(taskId: String) async {
    if detailTaskID == taskId,
       detailCoreWatcher != nil,
       detailActivityWatcher != nil,
       detailOutcomeWatcher != nil {
      await refreshDetail()
      return
    }
    let requestID = UUID()
    if detailTaskID != taskId {
      taskSubscription?.cancel()
      taskSubscription = nil
      runtimeSubscription?.cancel()
      runtimeSubscription = nil
    }
    cancelDetailWatchers()
    detailRequestID = requestID
    detailTaskID = taskId
    if detail?.id != taskId {
      detail = detailSeed(taskId: taskId)
      isLoadingDetail = detail == nil
      runItems = []
      hydratedRunIDs = []
    }
    let coreWatcher = await client.watch(
      query: TasksDetailCoreQuery(taskId: taskId),
      cachePolicy: .cacheAndNetwork
    ) { [weak self] result in
      Task { @MainActor in self?.receiveDetailCore(result, taskId: taskId, requestID: requestID) }
    }
    guard detailRequestID == requestID, detailTaskID == taskId else {
      coreWatcher.cancel()
      return
    }
    detailCoreWatcher = coreWatcher
    let activityWatcher = await client.watch(
      query: TasksDetailActivityQuery(taskId: taskId),
      cachePolicy: .cacheAndNetwork
    ) { [weak self] result in
      Task { @MainActor in await self?.receiveDetailActivity(result, taskId: taskId, requestID: requestID) }
    }
    guard detailRequestID == requestID, detailTaskID == taskId else {
      activityWatcher.cancel()
      return
    }
    detailActivityWatcher = activityWatcher
    let outcomeWatcher = await client.watch(
      query: TasksDetailOutcomeQuery(taskId: taskId),
      cachePolicy: .cacheAndNetwork
    ) { [weak self] result in
      Task { @MainActor in self?.receiveDetailOutcome(result, taskId: taskId, requestID: requestID) }
    }
    guard detailRequestID == requestID, detailTaskID == taskId else {
      outcomeWatcher.cancel()
      return
    }
    detailOutcomeWatcher = outcomeWatcher
    if taskSubscription == nil { subscribeToTask(taskId) }
    if runtimeSubscription == nil { subscribeToRuntime(taskId) }
  }

  func clearDetail(taskId: String) {
    guard detailTaskID == taskId else { return }
    detailRequestID = UUID()
    detailTaskID = nil
    isLoadingDetail = false
    cancelDetailWatchers()
    taskSubscription?.cancel()
    taskSubscription = nil
    runtimeSubscription?.cancel()
    runtimeSubscription = nil
  }

  func workspaceFileText(taskId: String, path: String) async throws -> String {
    let response = try await client.fetchNetworkFirst(
      query: TasksTaskWorkspaceFileQuery(taskId: taskId, path: path)
    )
    if let message = response.errors?.first?.message { throw TasksGraphQLError.server(message) }
    guard let file = response.data?.taskWorkspaceFile, file.path == path else {
      throw ApolloClient.Error.noResults
    }
    return file.content
  }

  func loadRunItems(runId: String, after: String? = nil) async {
    let expectedTaskID = detailTaskID
    do {
      let query = TasksRunItemsQuery(runId: runId, first: .some(50), after: optional(after))
      let stream = try client.fetch(query: query, cachePolicy: .cacheAndNetwork)
      for try await response in stream {
        if let message = response.errors?.first?.message { throw TasksGraphQLError.server(message) }
        if let result = response.data {
          guard expectedTaskID == detailTaskID, detailTaskID != nil, !Task.isCancelled else { return }
          applyRunItems(result.taskRunItems, runId: runId)
        }
      }
      if isConnected { lastError = nil }
    } catch {
      guard expectedTaskID == detailTaskID, detailTaskID != nil, !Task.isCancelled else { return }
      record(error)
    }
  }

  var hasMoreRunItems: Bool {
    detail?.runs.contains { runItemHasNextPage[$0.id] == true } ?? false
  }

  func loadOlderRunItems() async {
    guard !isLoadingOlderRunItems,
          let run = detail?.runs.first(where: { runItemHasNextPage[$0.id] == true }),
          let cursor = runItemEndCursor[run.id] else { return }
    isLoadingOlderRunItems = true
    defer { isLoadingOlderRunItems = false }
    await loadRunItems(runId: run.id, after: cursor)
  }

  func loadHistory() async {
    do {
      let query = TasksHistoryQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), kind: .none, first: .some(50), after: .none)
      if let result = try await fetch(query).data {
        applyHistory(result)
      }
      historyErrorMessage = nil
    } catch {
      historyErrorMessage = error.localizedDescription
      record(error)
    }
  }

  func loadMoreTasks() async {
    guard !isLoadingMoreTasks, hasMoreTasks, let cursor = tasksEndCursor else { return }
    isLoadingMoreTasks = true
    defer { isLoadingMoreTasks = false }
    do {
      let input = TaskListInput(workspaceId: workspaceId, projectId: optional(selectedProjectId), scope: GraphQLEnum(.active))
      let query = TasksListQuery(input: input, first: .some(100), after: .some(cursor))
      guard let result = try await fetch(query).data else { return }
      appendUnique(result.tasks.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }, to: &tasks)
      tasksEndCursor = result.tasks.pageInfo.endCursor
      hasMoreTasks = result.tasks.pageInfo.hasNextPage && tasksEndCursor != nil
      lastError = nil
    } catch { record(error) }
  }

  func loadMoreHistory() async {
    guard !isLoadingMoreHistory, hasMoreHistory, let cursor = historyEndCursor else { return }
    isLoadingMoreHistory = true
    defer { isLoadingMoreHistory = false }
    do {
      let query = TasksHistoryQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), kind: .none, first: .some(50), after: .some(cursor))
      guard let result = try await fetch(query).data else { return }
      appendUnique(result.taskHistory.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }, to: &history)
      historyEndCursor = result.taskHistory.pageInfo.endCursor
      hasMoreHistory = result.taskHistory.pageInfo.hasNextPage && historyEndCursor != nil
      historyErrorMessage = nil
      lastError = nil
    } catch {
      historyErrorMessage = error.localizedDescription
      record(error)
    }
  }

  private func appendUnique(_ incoming: [TasksTaskRow], to rows: inout [TasksTaskRow]) {
    let known = Set(rows.map(\.id))
    rows.append(contentsOf: incoming.filter { !known.contains($0.id) })
  }

  func capture(
    title: String,
    taskDocument: String,
    projectId: String?,
    schedule: NewTaskScheduleInput? = nil,
    executorAgentId: String = "agent:task-executor",
    cwdOverride: String? = nil
  ) async -> Bool {
    guard isConnected else { return false }
    let input = CaptureTaskInput(
      workspaceId: workspaceId,
      projectId: optional(projectId),
      title: title,
      taskDocument: taskDocument,
      schedule: schedule.map(GraphQLNullable.some) ?? .none,
      executorAgentId: optional(executorAgentId),
      cwdOverride: optional(cwdOverride),
      clientMutationId: UUID().uuidString
    )
    do {
      let result = try await perform(TasksCaptureTaskMutation(input: input))
      eventCursor = result.captureTask.eventCursor
      detail = mergeCommand(result.captureTask.task.fragments.tasksCommandTaskFields, into: nil)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func updateInbox(
    task: TasksDetailSnapshot,
    title: String,
    taskDocument: String,
    projectId: String?,
    executorAgentId: String? = nil,
    cwdOverride: String? = nil,
    clearCwdOverride: Bool = false
  ) async -> Bool {
    await updateInbox(taskId: task.id, revision: task.revision, generation: task.generation, title: title, taskDocument: taskDocument, taskDocumentDigest: task.taskDocumentDigest, projectId: projectId, executorAgentId: executorAgentId, cwdOverride: cwdOverride, clearCwdOverride: clearCwdOverride)
  }

  private func updateInbox(
    taskId: String,
    revision: Int,
    generation: Int,
    title: String,
    taskDocument: String,
    taskDocumentDigest: String,
    projectId: String?,
    executorAgentId: String?,
    cwdOverride: String?,
    clearCwdOverride: Bool
  ) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = UpdateInboxTaskInput(
      taskId: taskId,
      expectedRevision: Int32(revision),
      expectedGeneration: Int32(generation),
      title: .some(title),
      taskDocument: .some(taskDocument),
      expectedTaskDocumentDigest: .some(taskDocumentDigest),
      projectId: optional(projectId),
      clearProject: projectId == nil ? .some(true) : .none,
      executorAgentId: executorAgentId.map(GraphQLNullable.some) ?? .none,
      cwdOverride: cwdOverride.map(GraphQLNullable.some) ?? .none,
      clearCwdOverride: clearCwdOverride ? .some(true) : .none,
      clientMutationId: UUID().uuidString
    )
    do {
      let result = try await perform(TasksUpdateInboxTaskMutation(input: input))
      eventCursor = result.updateInboxTask.eventCursor
      detail = mergeCommand(result.updateInboxTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func queue(task: TasksTaskRow) async -> Bool {
    await queue(taskId: task.id, revision: task.revision, generation: task.generation)
  }

  @discardableResult
  func queue(task: TasksDetailSnapshot) async -> Bool {
    await queue(taskId: task.id, revision: task.revision, generation: task.generation)
  }

  private func queue(taskId: String, revision: Int, generation: Int) async -> Bool {
    guard isConnected else { return false }
    let input = QueueTaskInput(taskId: taskId, expectedRevision: Int32(revision), expectedGeneration: Int32(generation), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksQueueTaskMutation(input: input))
      eventCursor = result.queueTask.eventCursor
      detail = mergeCommand(result.queueTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func schedule(task: TasksDetailSnapshot, input schedule: NewTaskScheduleInput) async -> Bool {
    await scheduleTask(taskId: task.id, revision: task.revision, generation: task.generation, schedule: schedule, reschedule: false)
  }

  @discardableResult
  func reschedule(task: TasksDetailSnapshot, input schedule: NewTaskScheduleInput) async -> Bool {
    await scheduleTask(taskId: task.id, revision: task.revision, generation: task.generation, schedule: schedule, reschedule: true)
  }

  private func scheduleTask(taskId: String, revision: Int, generation: Int, schedule: NewTaskScheduleInput, reschedule: Bool) async -> Bool {
    guard isConnected else { return false }
    let input = ScheduleTaskInput(taskId: taskId, expectedRevision: Int32(revision), expectedGeneration: Int32(generation), schedule: schedule, clientMutationId: UUID().uuidString)
    do {
      if reschedule {
        let result = try await perform(TasksRescheduleTaskMutation(input: input))
        eventCursor = result.rescheduleTask.eventCursor
        detail = mergeCommand(result.rescheduleTask.task.fragments.tasksCommandTaskFields, into: detail)
      } else {
        let result = try await perform(TasksScheduleTaskMutation(input: input))
        eventCursor = result.scheduleTask.eventCursor
        detail = mergeCommand(result.scheduleTask.task.fragments.tasksCommandTaskFields, into: detail)
      }
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func unschedule(task: TasksDetailSnapshot) async -> Bool {
    guard isConnected else { return false }
    let input = UnscheduleTaskInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksUnscheduleTaskMutation(input: input))
      eventCursor = result.unscheduleTask.eventCursor
      detail = mergeCommand(result.unscheduleTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func runScheduledNow(task: TasksDetailSnapshot) async -> Bool {
    guard isConnected else { return false }
    let input = RunScheduledTaskNowInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksRunScheduledTaskNowMutation(input: input))
      eventCursor = result.runScheduledTaskNow.eventCursor
      detail = mergeCommand(result.runScheduledTaskNow.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  func loadRecurrence(recurrenceId: String) async -> TasksRecurrenceSnapshot? {
    if let cached = recurrenceCache[recurrenceId] { return cached }
    do {
      guard let recurrence = try await fetch(TasksTaskRecurrenceQuery(recurrenceId: recurrenceId, first: .some(30))).data?.taskRecurrence else { return nil }
      let snapshot = mapRecurrence(recurrence)
      recurrenceCache[recurrenceId] = snapshot
      return snapshot
    } catch {
      // Recurrence detail decorates an already-readable task card. Keep the
      // work surface online when only this optional expansion cannot load.
      return nil
    }
  }

  func refreshRecurrence(_ recurrence: TasksRecurrenceSnapshot) async -> TasksRecurrenceSnapshot? {
    recurrenceCache.removeValue(forKey: recurrence.id)
    return await loadRecurrence(recurrenceId: recurrence.id)
  }

  @discardableResult
  func updateRecurrence(
    _ recurrence: TasksRecurrenceSnapshot,
    startsAt: String,
    cronExpression: String,
    timeZone: String,
    missedRunPolicy: String,
    overlapPolicy: String
  ) async -> Bool {
    guard isConnected else { return false }
    let input = UpdateTaskRecurrenceInput(
      recurrenceId: recurrence.id,
      expectedRevision: Int32(recurrence.revision),
      title: .none,
      taskDocument: .none,
      expectedTaskDocumentDigest: .none,
      projectId: .none,
      clearProject: .none,
      startsAt: .some(startsAt),
      cronExpression: .some(cronExpression),
      timeZone: .some(timeZone),
      missedRunPolicy: .some(GraphQLEnum(rawValue: missedRunPolicy)),
      overlapPolicy: .some(GraphQLEnum(rawValue: overlapPolicy)),
      clientMutationId: UUID().uuidString
    )
    do {
      let result = try await perform(TasksUpdateTaskRecurrenceMutation(input: input))
      eventCursor = result.updateTaskRecurrence.eventCursor
      detail = mergeCommand(result.updateTaskRecurrence.task.fragments.tasksCommandTaskFields, into: detail)
      recurrenceCache.removeValue(forKey: recurrence.id)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func updateRecurrenceTemplate(_ recurrence: TasksRecurrenceSnapshot, title: String, taskDocument: String) async -> Bool {
    guard isConnected else { return false }
    let input = UpdateTaskRecurrenceInput(
      recurrenceId: recurrence.id,
      expectedRevision: Int32(recurrence.revision),
      title: .some(title),
      taskDocument: .some(taskDocument),
      expectedTaskDocumentDigest: .some(recurrence.taskDocumentDigest),
      projectId: .none,
      clearProject: .none,
      startsAt: .none,
      cronExpression: .none,
      timeZone: .none,
      missedRunPolicy: .none,
      overlapPolicy: .none,
      clientMutationId: UUID().uuidString
    )
    do {
      let result = try await perform(TasksUpdateTaskRecurrenceMutation(input: input))
      eventCursor = result.updateTaskRecurrence.eventCursor
      recurrenceCache.removeValue(forKey: recurrence.id)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func changeRecurrence(_ recurrence: TasksRecurrenceSnapshot, action: TasksRecurrenceAction) async -> Bool {
    guard isConnected else { return false }
    let input = TaskRecurrenceCommandInput(recurrenceId: recurrence.id, expectedRevision: Int32(recurrence.revision), clientMutationId: UUID().uuidString)
    do {
      let cursor: String
      switch action {
      case .pause:
        cursor = try await perform(TasksPauseTaskRecurrenceMutation(input: input)).pauseTaskRecurrence.eventCursor
      case .resume:
        cursor = try await perform(TasksResumeTaskRecurrenceMutation(input: input)).resumeTaskRecurrence.eventCursor
      case .skip:
        cursor = try await perform(TasksSkipTaskRecurrenceNextMutation(input: input)).skipTaskRecurrenceNext.eventCursor
      case .end:
        cursor = try await perform(TasksEndTaskRecurrenceMutation(input: input)).endTaskRecurrence.eventCursor
      case .runNow:
        let result = try await perform(TasksRunTaskRecurrenceNowMutation(input: input))
        cursor = result.runTaskRecurrenceNow.eventCursor
        detail = mergeCommand(result.runTaskRecurrenceNow.task.fragments.tasksCommandTaskFields, into: detail)
      }
      eventCursor = cursor
      recurrenceCache.removeValue(forKey: recurrence.id)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  func schedulePreview(startsAt: String, timeZone: String, cronExpression: String?) async -> [String] {
    do {
      let input = TaskSchedulePreviewInput(startsAt: startsAt, timeZone: timeZone, cronExpression: optional(cronExpression))
      return try await fetch(TasksSchedulePreviewQuery(input: input)).data?.taskSchedulePreview.occurrences ?? []
    } catch {
      return []
    }
  }

  func answer(task: TasksDetailSnapshot, answer: String, approval: ApprovalDecision? = nil) async -> Bool {
    guard isConnected, let gate = task.activeGate, !commandTaskIDs.contains(task.id) else { return false }
    commandTaskIDs.insert(task.id)
    commandErrors[task.id] = nil
    defer { commandTaskIDs.remove(task.id) }
    let input = AnswerTaskInput(taskId: task.id, gateId: gate.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), answerMarkdown: answer, approvalDecision: approval.map(GraphQLEnum.init) ?? .none, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksAnswerTaskMutation(input: input))
      eventCursor = result.answerTask.eventCursor
      detail = mergeCommand(result.answerTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      recordCommandError(error, taskID: task.id)
      return false
    }
  }

  func retry(task: TasksDetailSnapshot, note: String? = nil) async -> Bool {
    guard isConnected, let gate = task.activeGate, !commandTaskIDs.contains(task.id) else { return false }
    commandTaskIDs.insert(task.id)
    commandErrors[task.id] = nil
    defer { commandTaskIDs.remove(task.id) }
    let input = RetryTaskInput(taskId: task.id, gateId: gate.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), retryNote: optional(note), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksRetryTaskMutation(input: input))
      eventCursor = result.retryTask.eventCursor
      detail = mergeCommand(result.retryTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      recordCommandError(error, taskID: task.id)
      return false
    }
  }

  func commandIsPending(taskID: String) -> Bool { commandTaskIDs.contains(taskID) }
  func commandError(taskID: String) -> String? { commandErrors[taskID] }
  func interventionError(id: String) -> String? { interventionErrors[id] }

  func resolve(_ intervention: HumanIntervention, decision: String) async {
    guard isConnected, case let .governed(action) = intervention else { return }
    interventionErrors[action.actionID] = nil
    do {
      try await HumanInterventionActions.resolve(action, decision: decision, client: client)
      await refresh()
    } catch { recordIntervention(error, id: action.actionID) }
  }

  func approveAdapterDefinition(_ definition: AdapterDefinitionModel) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.approve(definition, client: client)
    await refresh()
  }

  func cancelAdapterDefinition(_ definition: AdapterDefinitionModel) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.cancel(definition, client: client)
    await refresh()
  }

  func setupAdapterConnection(_ definition: AdapterDefinitionModel, submission: AdapterCredentialSubmission) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.setup(definition, submission: submission, client: client)
    await refresh()
  }

  func importAdapterOauthClient(_ setup: AdapterOauthClientSetupModel, submission: AdapterCredentialSubmission) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.importOauthClient(setup, submission: submission, client: client)
    await refresh()
  }

  func startAdapterOAuth(_ action: AdapterNextActionModel) async throws -> AdapterOAuthSetupAttempt {
    guard isConnected else { throw ChatModelError.offline }
    let attempt = try await HumanInterventionActions.startOAuth(action, client: client)
    await refresh()
    return attempt
  }

  func attachAdapterGrant(_ action: AdapterNextActionModel) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.attach(action, client: client)
    await refresh()
  }

  func completeAdapterOAuth(_ attempt: AdapterOAuthSetupAttempt, action: AdapterNextActionModel) async throws -> String {
    guard isConnected else { throw ChatModelError.offline }
    let status = try await HumanInterventionActions.waitForOAuth(attempt, action: action, client: client)
    await refresh()
    return status
  }

  func saveAdapterPolicy(_ connection: AdapterConnectionModel, sharing: String, unsafeActions: String) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.savePolicy(connection, sharing: sharing, unsafeActions: unsafeActions, client: client)
    await refresh()
  }

  func startMcpAuthentication(_ auth: McpAuthModel) async -> URL? {
    guard isConnected else { return nil }
    interventionErrors[auth.requestID] = nil
    do {
      let url = try await HumanInterventionActions.startMcpAuthentication(auth, client: client, profile: profile)
      await refresh()
      return url
    } catch {
      recordIntervention(error, id: auth.requestID)
      return nil
    }
  }

  func skipMcpAuthentication(_ auth: McpAuthModel) async {
    guard isConnected else { return }
    interventionErrors[auth.requestID] = nil
    do {
      try await HumanInterventionActions.skipMcpAuthentication(auth, client: client)
      await refresh()
    } catch { recordIntervention(error, id: auth.requestID) }
  }

  func startAdapterAuthentication(_ auth: AdapterAuthModel) async -> URL? {
    guard isConnected else { return nil }
    interventionErrors[auth.requestID] = nil
    do {
      let url = try await HumanInterventionActions.startAdapterAuthentication(auth, client: client)
      await refresh()
      return url
    } catch {
      recordIntervention(error, id: auth.requestID)
      return nil
    }
  }

  func skipAdapterAuthentication(_ auth: AdapterAuthModel) async {
    guard isConnected else { return }
    interventionErrors[auth.requestID] = nil
    do {
      try await HumanInterventionActions.skipAdapterAuthentication(auth, client: client)
      await refresh()
    } catch { recordIntervention(error, id: auth.requestID) }
  }

  func resolveMcpSetup(_ setup: McpSetupModel, mcpServerID: String) async {
    guard isConnected, !mcpServerID.isEmpty else { return }
    do {
      try await HumanInterventionActions.resolveMcpSetup(setup, mcpServerID: mcpServerID, client: client)
      await refresh()
    } catch { record(error) }
  }

  func connectMcpPublicly(_ setup: McpSetupModel) async throws -> McpSetupServerModel {
    guard isConnected else { throw ChatModelError.offline }
    let server = try await HumanInterventionActions.createPublicMcpServer(setup, client: client)
    await refresh()
    return server
  }

  func startMcpSetupOAuth(_ setup: McpSetupModel) async throws -> URL {
    guard isConnected, let profile else { throw ChatModelError.offline }
    return try await HumanInterventionActions.startMcpSetupOAuth(
      setup,
      redirectURI: profile.origin.appending(path: "mcp/oauth/callback").absoluteString,
      client: client
    )
  }

  func saveMcpPolicy(_ server: McpSetupServerModel, sharing: String, unsafeActions: String) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.saveMcpPolicy(server: server, sharing: sharing, unsafeActions: unsafeActions, client: client)
  }

  func resolveMcpSetup(_ setup: McpSetupModel, server: McpSetupServerModel) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.resolveMcpSetup(setup, mcpServerID: server.serverID, client: client)
    await refresh()
  }

  @discardableResult
  func cancel(task: TasksDetailSnapshot, reason: String? = nil) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = CancelTaskInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), reason: optional(reason), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksCancelTaskMutation(input: input))
      eventCursor = result.cancelTask.eventCursor
      detail = mergeCommand(result.cancelTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func reopen(task: TasksDetailSnapshot, feedback: String, request: String? = nil) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = ReopenTaskInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), feedbackMarkdown: feedback, requestMarkdown: optional(request), complexity: .none, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksReopenTaskMutation(input: input))
      eventCursor = result.reopenTask.eventCursor
      detail = mergeCommand(result.reopenTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func createProject(name: String, description: String, folder: String? = nil) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = CreateProjectInput(workspaceId: workspaceId, name: name, description: description, folder: optional(folder), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksCreateProjectMutation(input: input))
      eventCursor = result.createProject.eventCursor
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func updateProject(_ project: TasksProjectSnapshot, name: String, description: String, folder: String? = nil, clearFolder: Bool = false) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = UpdateProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), name: .some(name), description: .some(description), folder: optional(folder), clearFolder: clearFolder ? .some(true) : .none, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksUpdateProjectMutation(input: input))
      eventCursor = result.updateProject.eventCursor
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func archiveProject(_ project: TasksProjectSnapshot) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = ArchiveProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksArchiveProjectMutation(input: input))
      eventCursor = result.archiveProject.eventCursor
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  @discardableResult
  func reopenProject(_ project: TasksProjectSnapshot) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = ReopenProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksReopenProjectMutation(input: input))
      eventCursor = result.reopenProject.eventCursor
      await refresh()
      return true
    } catch {
      record(error)
      return false
    }
  }

  private func subscribeToTasks() {
    eventSubscription?.cancel()
    eventSubscription = Task { [weak self] in
      guard let self else { return }
      do {
        let stream = try client.recoveringSubscribe(
          subscription: TasksEventsSubscription(
            workspaceId: workspaceId,
            after: optional(eventCursor)
          )
        )
        for try await value in stream {
          guard !Task.isCancelled else { return }
          eventCursor = value.data?.tasksEvents.cursor ?? eventCursor
          await refresh()
        }
      } catch {
        guard !Task.isCancelled else { return }
        record(error)
      }
    }
  }

  private func subscribeToTask(_ taskId: String) {
    taskSubscription?.cancel()
    taskSubscription = Task { [weak self] in
      guard let self else { return }
      do {
        let stream = try client.recoveringSubscribe(
          subscription: TasksTaskEventsSubscription(taskId: taskId, after: optional(eventCursor))
        )
        for try await value in stream {
          guard !Task.isCancelled else { return }
          eventCursor = value.data?.taskEvents.cursor ?? eventCursor
          await refreshDetail(for: value.data?.taskEvents.kind)
        }
      } catch {
        guard !Task.isCancelled else { return }
        record(error)
      }
    }
  }

  private func subscribeToRuntime(_ taskId: String) {
    runtimeSubscription?.cancel()
    runtimeSubscription = Task { [weak self] in
      guard let self else { return }
      do {
        let stream = try client.recoveringSubscribe(subscription: TasksRuntimeEventsSubscription(taskId: taskId))
        for try await value in stream {
          guard !Task.isCancelled else { return }
          if let runId = value.data?.taskRuntimeEvents.runId {
            await refreshRunItems(runId: runId)
          }
        }
      } catch {
        guard !Task.isCancelled else { return }
        record(error)
      }
    }
  }

  private func cancelDetailWatchers() {
    detailCoreWatcher?.cancel()
    detailActivityWatcher?.cancel()
    detailOutcomeWatcher?.cancel()
    detailCoreWatcher = nil
    detailActivityWatcher = nil
    detailOutcomeWatcher = nil
  }

  private func refreshDetail(for eventKind: String? = nil) async {
    switch eventKind {
    case "run.heartbeat":
      await detailCoreWatcher?.fetch(cachePolicy: .networkOnly)
    case "task.message_appended", "task.message_consumed":
      await detailActivityWatcher?.fetch(cachePolicy: .networkOnly)
    case "notification.queued", "notification.delivered", "notification.failed":
      return
    default:
      await detailCoreWatcher?.fetch(cachePolicy: .networkOnly)
      await detailActivityWatcher?.fetch(cachePolicy: .networkOnly)
      await detailOutcomeWatcher?.fetch(cachePolicy: .networkOnly)
    }
  }

  private func refreshRunItems(runId: String) async {
    let expectedTaskID = detailTaskID
    do {
      let result = try await client.fetch(
        query: TasksRunItemsQuery(runId: runId, first: .some(50), after: .none),
        cachePolicy: .networkOnly
      )
      guard expectedTaskID == detailTaskID, detailTaskID != nil, !Task.isCancelled else { return }
      if let message = result.errors?.first?.message { throw TasksGraphQLError.server(message) }
      if let data = result.data { applyRunItems(data.taskRunItems, runId: runId) }
      if isConnected { lastError = nil }
    } catch {
      guard expectedTaskID == detailTaskID, detailTaskID != nil, !Task.isCancelled else { return }
      record(error)
    }
  }

  private func applyRunItems(
    _ page: TasksRunItemsQuery.Data.TaskRunItems,
    runId: String
  ) {
    let incoming = page.edges.map { item in
      let node = item.node
      return TasksRunItemSnapshot(id: node.itemId, runId: node.runId, sequence: node.sequenceIndex, round: node.roundIndex, kind: node.kind.rawValue, status: node.status.rawValue, correlationId: node.correlationId, parentItemId: node.parentItemId, content: node.contentText, payloadText: node.payload.encodedString, createdAt: node.createdAt, updatedAt: node.updatedAt)
    }
    var mergedByID = Dictionary(uniqueKeysWithValues: runItems.lazy.filter { $0.runId == runId }.map { ($0.id, $0) })
    for item in incoming { mergedByID[item.id] = item }
    let merged = mergedByID.values.sorted { $0.sequence < $1.sequence }
    let first = runItems.firstIndex { $0.runId == runId }
    let last = runItems.lastIndex { $0.runId == runId }
    if let first, let last {
      if Array(runItems[first...last]) != merged {
        runItems.replaceSubrange(first...last, with: merged)
      }
    } else if !merged.isEmpty {
      let insertion = runItems.firstIndex { $0.runId > runId } ?? runItems.endIndex
      runItems.insert(contentsOf: merged, at: insertion)
    }
    if let cursor = page.pageInfo.endCursor { runItemEndCursor[runId] = cursor }
    runItemHasNextPage[runId] = page.pageInfo.hasNextPage
  }

  private func detailSeed(taskId: String) -> TasksDetailSnapshot? {
    if let attention = needsYou.first(where: { $0.task.id == taskId }) {
      return attention.task.detailSnapshot(gate: attention.gate)
    }
    return (tasks + history).first(where: { $0.id == taskId })?.detailSnapshot(gate: nil)
  }

  private func receiveDetailCore(
    _ result: Result<GraphQLResponse<TasksDetailCoreQuery>, any Error>,
    taskId: String,
    requestID: UUID
  ) {
    guard detailRequestID == requestID, detailTaskID == taskId else { return }
    switch result {
    case .success(let response):
      if let message = response.errors?.first?.message {
        isLoadingDetail = false
        record(TasksGraphQLError.server(message))
        return
      }
      guard let source = response.data?.task.fragments.tasksDetailCoreFields else { return }
      detail = mergeDetailCore(source, into: detail)
      isLoadingDetail = false
      if response.source == .server {
        lastError = nil
      }
    case .failure(let error):
      isLoadingDetail = false
      record(error)
    }
  }

  private func receiveDetailActivity(
    _ result: Result<GraphQLResponse<TasksDetailActivityQuery>, any Error>,
    taskId: String,
    requestID: UUID
  ) async {
    guard detailRequestID == requestID, detailTaskID == taskId else { return }
    switch result {
    case .success(let response):
      if let message = response.errors?.first?.message {
        record(TasksGraphQLError.server(message))
        return
      }
      guard let source = response.data?.task.fragments.tasksDetailActivityFields else { return }
      detail = mergeDetailActivity(source, into: detail)
      if response.source == .server {
        lastError = nil
      }
      let runIDs = Set(source.runs.map(\.runId))
      runItems.removeAll { !runIDs.contains($0.runId) }
      hydratedRunIDs.formIntersection(runIDs)
      let missingRunIDs = runIDs.subtracting(hydratedRunIDs)
      hydratedRunIDs.formUnion(missingRunIDs)
      await withTaskGroup(of: Void.self) { group in
        for runID in missingRunIDs {
          group.addTask { [weak self] in await self?.loadRunItems(runId: runID) }
        }
      }
    case .failure(let error):
      record(error)
    }
  }

  private func receiveDetailOutcome(
    _ result: Result<GraphQLResponse<TasksDetailOutcomeQuery>, any Error>,
    taskId: String,
    requestID: UUID
  ) {
    guard detailRequestID == requestID, detailTaskID == taskId else { return }
    switch result {
    case .success(let response):
      if let message = response.errors?.first?.message {
        record(TasksGraphQLError.server(message))
        return
      }
      guard let source = response.data?.task.fragments.tasksDetailOutcomeFields else { return }
      detail = mergeDetailOutcome(source, into: detail)
      if response.source == .server {
        lastError = nil
      }
    case .failure(let error):
      record(error)
    }
  }

  private func applyOverview(_ overview: TasksOverviewQuery.Data.TasksOverview) {
    workspace = TasksWorkspaceSnapshot(id: overview.workspace.workspaceId, name: overview.workspace.name, description: overview.workspace.description, isPersonal: overview.workspace.isPersonal)
    columns = overview.boardColumns.map { TasksColumnSnapshot(id: $0.stage.stageId, title: $0.stage.name, behavior: TasksStageBehavior($0.stage.behavior.rawValue), count: $0.taskCount) }
  }

  private func applyProjects(_ page: TasksProjectsQuery.Data.Projects) {
    projects = page.edges.map { mapProject($0.node.fragments.tasksProjectFields) }
  }

  private func applyNeedsYou(_ page: TasksNeedsYouQuery.Data.NeedsYou) {
    needsYou = page.edges.map { item in
      let node = item.node
      let task = mapCard(node.task.fragments.tasksTaskCardFields)
      return TasksAttentionRow(id: task.id, kind: node.kind.rawValue, title: node.title, summary: node.summary, task: task, gate: node.gate.map { mapGate($0.fragments.tasksGateFields) }, validActions: Set(node.validActions.map(\.rawValue)))
    }
  }

  private func mapSummary(_ source: TasksTaskSummaryFields) -> TasksTaskRow {
    TasksTaskRow(
      id: source.taskId,
      workspaceId: source.workspace.workspaceId,
      projectId: source.project?.projectId,
      projectName: source.project?.name,
      title: source.title,
      summary: source.taskDocumentPreview,
      executor: mapExecutor(agentId: source.executorAgentId, backend: source.executorBackend, cwdOverride: source.cwdOverride, effectiveCwd: source.effectiveCwd, effectiveCwdSource: source.effectiveCwdSource),
      schedule: source.schedule.map { mapSchedule(scheduledFor: $0.scheduledFor, timeZone: $0.timeZone, missedRunPolicy: $0.missedRunPolicy.rawValue, recurrenceId: $0.recurrenceId, recurrenceRevision: $0.recurrenceRevision, recurrenceScheduledFor: $0.recurrenceScheduledFor) },
      stage: mapStage(source.stage.fragments.tasksStageFields),
      revision: source.revision,
      generation: source.generation,
      updatedAt: source.updatedAt,
      completedAt: source.completedAt,
      currentRun: source.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) },
      activeGate: source.activeGate.map { mapGate($0.fragments.tasksGateFields) },
      validActions: Set(source.validActions.map(\.rawValue))
    )
  }

  private func mapCard(_ source: TasksTaskCardFields) -> TasksTaskRow {
    TasksTaskRow(
      id: source.taskId,
      workspaceId: source.workspace.workspaceId,
      projectId: source.project?.projectId,
      projectName: source.project?.name,
      title: source.title,
      summary: source.taskDocumentPreview,
      executor: mapExecutor(agentId: source.executorAgentId, backend: source.executorBackend, cwdOverride: source.cwdOverride, effectiveCwd: source.effectiveCwd, effectiveCwdSource: source.effectiveCwdSource),
      schedule: source.schedule.map { mapSchedule(scheduledFor: $0.scheduledFor, timeZone: $0.timeZone, missedRunPolicy: $0.missedRunPolicy.rawValue, recurrenceId: $0.recurrenceId, recurrenceRevision: $0.recurrenceRevision, recurrenceScheduledFor: $0.recurrenceScheduledFor) },
      stage: mapStage(source.stage.fragments.tasksStageFields),
      revision: source.revision,
      generation: source.generation,
      updatedAt: source.updatedAt,
      completedAt: source.completedAt,
      currentRun: source.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) },
      activeGate: source.activeGate.map { mapGate($0.fragments.tasksGateFields) },
      validActions: Set(source.validActions.map(\.rawValue))
    )
  }

  private func mapExecutor(agentId: String, backend: String, cwdOverride: String?, effectiveCwd: String?, effectiveCwdSource: String) -> TasksExecutorSnapshot {
    TasksExecutorSnapshot(agentId: agentId, backend: backend, cwdOverride: cwdOverride, effectiveCwd: effectiveCwd, effectiveCwdSource: effectiveCwdSource)
  }

  private func mapSchedule(scheduledFor: String, timeZone: String, missedRunPolicy: String, recurrenceId: String?, recurrenceRevision: Int?, recurrenceScheduledFor: String?) -> TasksScheduleSnapshot {
    TasksScheduleSnapshot(scheduledFor: scheduledFor, timeZone: timeZone, missedRunPolicy: missedRunPolicy, recurrenceId: recurrenceId, recurrenceRevision: recurrenceRevision, recurrenceScheduledFor: recurrenceScheduledFor)
  }

  private func mapProject(_ source: TasksProjectFields) -> TasksProjectSnapshot {
    TasksProjectSnapshot(id: source.projectId, workspaceId: source.workspaceId, name: source.name, description: source.description, folder: source.folder, revision: source.revision, archivedAt: source.archivedAt)
  }

  private func mapStage(_ source: TasksStageFields) -> TasksStageSnapshot {
    TasksStageSnapshot(id: source.stageId, name: source.name, behavior: TasksStageBehavior(source.behavior.rawValue))
  }

  private func mapRun(_ source: TasksCurrentRunFields) -> TasksRunSnapshot {
    TasksRunSnapshot(id: source.runId, instanceName: source.instanceName, kind: source.kind.rawValue, status: source.status.rawValue, attempt: source.attemptIndex, activity: source.activityLabel, startedAt: source.startedAt, endedAt: nil, createdAt: nil, error: nil)
  }

  private func mapRun(_ source: TasksRunFields) -> TasksRunSnapshot {
    TasksRunSnapshot(id: source.runId, instanceName: source.instanceName, kind: source.kind.rawValue, status: source.status.rawValue, attempt: source.attemptIndex, activity: source.errorMessage ?? "", startedAt: source.startedAt, endedAt: source.endedAt, createdAt: source.createdAt, error: source.errorMessage)
  }

  private func mapGate(_ source: TasksGateFields) -> TasksGateSnapshot {
    TasksGateSnapshot(id: source.gateId, kind: source.kind.rawValue, state: source.state.rawValue, prompt: source.prompt, context: source.contextMarkdown, suggestedAnswers: source.suggestedAnswers, recoveryReason: source.recoveryReason?.rawValue, retryRunKind: source.retryRunKind?.rawValue)
  }

  private func mergeDetailCore(
    _ source: TasksDetailCoreFields,
    into previous: TasksDetailSnapshot?
  ) -> TasksDetailSnapshot {
    var next = mergeCommand(source.fragments.tasksCommandTaskFields, into: previous)
    next.project = source.project.map { mapProject($0.fragments.tasksProjectFields) }
    next.createdAt = source.createdAt
    next.sourceLabel = source.project?.name ?? (source.source.conversationId == nil ? nil : "Conversation")
    return next
  }

  private func mergeDetailActivity(
    _ source: TasksDetailActivityFields,
    into previous: TasksDetailSnapshot?
  ) -> TasksDetailSnapshot {
    var next = previous ?? emptyDetail(taskId: source.taskId)
    next.messages = source.messages.map {
      TasksMessageSnapshot(id: $0.messageId, body: $0.bodyMarkdown, author: $0.author, createdAt: $0.createdAt)
    }
    next.runs = source.runs.map { mapRun($0.fragments.tasksRunFields) }
    next.contributorInstanceNames = source.contributorInstanceNames
    return next
  }

  private func mergeDetailOutcome(
    _ source: TasksDetailOutcomeFields,
    into previous: TasksDetailSnapshot?
  ) -> TasksDetailSnapshot {
    var next = previous ?? emptyDetail(taskId: source.taskId)
    next.taskDocument = source.taskDocument
    next.taskDocumentDigest = source.taskDocumentDigest
    next.resultDocument = source.resultDocument
    next.resultCitations = ProviderCitation.from(metadata: source.resultMetadata.encodedString)
    next.reviewDocument = source.reviewDocument
    next.workspaceFiles = source.workspaceFiles.map {
      TasksWorkspaceFileSnapshot(path: $0.path, isDirectory: $0.isDirectory)
    }
    next.workspaceFilesTruncated = source.workspaceFilesTruncated
    return next
  }

  private func emptyDetail(taskId: String) -> TasksDetailSnapshot {
    TasksDetailSnapshot(
      id: taskId,
      title: "Task",
      project: nil,
      stage: TasksStageSnapshot(id: "", name: "Task", behavior: .unknown),
      revision: 0,
      generation: 0,
      updatedAt: "",
      completedAt: nil,
      createdAt: "",
      sourceLabel: nil,
      taskDocument: "",
      taskDocumentDigest: "",
      resultDocument: nil,
      resultCitations: [],
      reviewDocument: nil,
      workspaceFiles: [],
      workspaceFilesTruncated: false,
      currentRun: nil,
      activeGate: nil,
      messages: [],
      runs: [],
      contributorInstanceNames: [],
      validActions: []
    )
  }

  private func mergeCommand(_ source: TasksCommandTaskFields, into previous: TasksDetailSnapshot?) -> TasksDetailSnapshot {
    var next = previous ?? TasksDetailSnapshot(id: source.taskId, title: source.title, project: nil, executor: TasksExecutorSnapshot.default, schedule: nil, stage: mapStage(source.stage.fragments.tasksStageFields), revision: source.revision, generation: source.generation, updatedAt: source.updatedAt, completedAt: source.completedAt, createdAt: "", sourceLabel: nil, taskDocument: source.taskDocument, taskDocumentDigest: source.taskDocumentDigest, resultDocument: nil, resultCitations: [], reviewDocument: nil, workspaceFiles: [], workspaceFilesTruncated: false, currentRun: nil, activeGate: nil, messages: [], runs: [], contributorInstanceNames: [], validActions: [])
    next.title = source.title
    next.taskDocument = source.taskDocument
    next.taskDocumentDigest = source.taskDocumentDigest
    next.executor = mapExecutor(agentId: source.executorAgentId, backend: source.executorBackend, cwdOverride: source.cwdOverride, effectiveCwd: source.effectiveCwd, effectiveCwdSource: source.effectiveCwdSource)
    next.schedule = source.schedule.map { mapSchedule(scheduledFor: $0.scheduledFor, timeZone: $0.timeZone, missedRunPolicy: $0.missedRunPolicy.rawValue, recurrenceId: $0.recurrenceId, recurrenceRevision: $0.recurrenceRevision, recurrenceScheduledFor: $0.recurrenceScheduledFor) }
    next.stage = mapStage(source.stage.fragments.tasksStageFields)
    next.revision = source.revision
    next.generation = source.generation
    next.updatedAt = source.updatedAt
    next.completedAt = source.completedAt
    next.currentRun = source.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) }
    next.activeGate = source.activeGate.map { mapGate($0.fragments.tasksGateFields) }
    next.validActions = Set(source.validActions.map(\.rawValue))
    return next
  }

  private func mapRecurrence(_ source: TasksTaskRecurrenceQuery.Data.TaskRecurrence) -> TasksRecurrenceSnapshot {
    TasksRecurrenceSnapshot(
      id: source.recurrenceId,
      title: source.title,
      taskDocument: source.taskDocument,
      taskDocumentDigest: source.taskDocumentDigest,
      startsAt: source.startsAt,
      cronExpression: source.cronExpression,
      timeZone: source.timeZone,
      missedRunPolicy: source.missedRunPolicy.rawValue,
      overlapPolicy: source.overlapPolicy.rawValue,
      lifecycle: source.lifecycle.rawValue,
      revision: source.revision,
      nextRunAt: source.nextRunAt,
      pendingCoalescedAt: source.pendingCoalescedAt,
      occurrences: source.occurrences.map {
        TasksRecurrenceOccurrenceSnapshot(
          recurrenceRevision: $0.recurrenceRevision,
          scheduledFor: $0.scheduledFor,
          localSlot: $0.localSlot,
          trigger: $0.trigger.rawValue,
          resolution: $0.resolution.rawValue,
          taskId: $0.taskId,
          createdAt: $0.createdAt
        )
      }
    )
  }

  private func optional<T>(_ value: T?) -> GraphQLNullable<T> {
    value.map(GraphQLNullable.some) ?? .none
  }

  private func restoreCachedSnapshot() async {
    let project = optional(selectedProjectId)
    let overviewQuery = TasksOverviewQuery(workspaceId: workspaceId, projectId: project)
    let projectsQuery = TasksProjectsQuery(workspaceId: workspaceId, includeArchived: true, first: .some(100), after: .none)
    let needsQuery = TasksNeedsYouQuery(workspaceId: workspaceId, projectId: project, first: .some(50), after: .none)
    let pendingQuery = NoemaAPI.PendingChatInterventionsQuery(
      conversationId: .none,
      taskId: .none,
      projectId: project,
      first: 50
    )
    let listQuery = TasksListQuery(
      input: TaskListInput(workspaceId: workspaceId, projectId: project, scope: GraphQLEnum(.active)),
      first: .some(100),
      after: .none
    )
    let historyQuery = TasksHistoryQuery(
      workspaceId: workspaceId,
      projectId: project,
      kind: .none,
      first: .some(50),
      after: .none
    )
    if let data = await cachedData(overviewQuery) { applyOverview(data.tasksOverview) }
    if let data = await cachedData(projectsQuery) { applyProjects(data.projects) }
    if let data = await cachedData(needsQuery) { applyNeedsYou(data.needsYou) }
    if let data = await cachedData(listQuery) { applyTaskList(data) }
    if let data = await cachedData(pendingQuery) {
      pendingInterventions = data.pendingHumanInterventions.compactMap(HumanIntervention.init).filter {
        if case .attention = $0 { return false }
        return true
      }
    }
    if let data = await cachedData(historyQuery) { applyHistory(data) }
  }

  private func cachedData<Query: GraphQLQuery>(_ query: Query) async -> Query.Data? where Query.ResponseFormat == SingleResponseFormat {
    do {
      return try await client.fetch(query: query, cachePolicy: .cacheOnly)?.data
    } catch {
      return nil
    }
  }

  private func applyTaskList(_ data: TasksListQuery.Data) {
    tasks = data.tasks.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }
    tasksEndCursor = data.tasks.pageInfo.endCursor
    hasMoreTasks = data.tasks.pageInfo.hasNextPage && tasksEndCursor != nil
    hasLoadedTasks = true
  }

  private func applyHistory(_ data: TasksHistoryQuery.Data) {
    history = data.taskHistory.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }
    historyEndCursor = data.taskHistory.pageInfo.endCursor
    hasMoreHistory = data.taskHistory.pageInfo.hasNextPage && historyEndCursor != nil
  }

  private func fetch<Query: GraphQLQuery>(_ query: Query) async throws -> GraphQLResponse<Query> where Query.ResponseFormat == SingleResponseFormat {
    let response = try await client.fetchNetworkFirst(query: query)
    if let message = response.errors?.first?.message { throw TasksGraphQLError.server(message) }
    guard response.data != nil else { throw ApolloClient.Error.noResults }
    return response
  }

  private func perform<Mutation: GraphQLMutation>(_ mutation: Mutation) async throws -> Mutation.Data where Mutation.ResponseFormat == SingleResponseFormat {
    let response = try await client.perform(mutation: mutation)
    if let message = response.errors?.first?.message { throw TasksGraphQLError.server(message) }
    guard let data = response.data else {
      throw ApolloClient.Error.noResults
    }
    return data
  }

  private func record(_ error: Error) {
    lastError = error.localizedDescription
  }

  private func recordCommandError(_ error: Error, taskID: String) {
    record(error)
    commandErrors[taskID] = error.localizedDescription
  }

  private func recordIntervention(_ error: Error, id: String) {
    record(error)
    interventionErrors[id] = error.localizedDescription
  }
}

private enum TasksGraphQLError: LocalizedError {
  case server(String)

  var errorDescription: String? {
    switch self {
    case let .server(message): message
    }
  }
}
