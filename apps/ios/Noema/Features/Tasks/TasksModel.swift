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
  private(set) var isConnected = true
  private(set) var lastError: String?
  private(set) var hasLoadedTasks = false
  private(set) var commandTaskIDs = Set<String>()
  private(set) var commandErrors: [String: String] = [:]
  private(set) var interventionErrors: [String: String] = [:]

  private var eventSubscription: Task<Void, Never>?
  private var taskSubscription: Task<Void, Never>?
  private var runtimeSubscription: Task<Void, Never>?
  private var detailRequestID = UUID()
  private var detailTaskID: String?
  private var runItemEndCursor: [String: String] = [:]
  private var runItemHasNextPage: [String: Bool] = [:]
  private var tasksEndCursor: String?
  private var historyEndCursor: String?
  private(set) var hasMoreTasks = false
  private(set) var hasMoreHistory = false
  private var started = false

  init(client: ApolloClient, profile: NoemaProfile? = nil, workspaceId: String = TasksModel.personalWorkspaceId) {
    self.client = client
    self.profile = profile
    self.workspaceId = workspaceId
  }

  func start() async {
    guard !started else { return }
    started = true
    await refresh()
    subscribeToWork()
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
    subscribeToWork()
    if let taskId = detail?.id {
      subscribeToTask(taskId)
      subscribeToRuntime(taskId)
    }
  }

  func refresh() async {
    guard !isRefreshing else { return }
    isRefreshing = true
    defer { isRefreshing = false }

    do {
      let overviewQuery = TasksOverviewQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId))
      let projectsQuery = TasksProjectsQuery(workspaceId: workspaceId, includeArchived: true, first: .some(100), after: .none)
      let needsQuery = TasksNeedsYouQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), first: .some(50), after: .none)
      let pendingQuery = NoemaAPI.PendingChatInterventionsQuery(
        conversationId: .none,
        taskId: .none,
        projectId: optional(selectedProjectId),
        first: 50
      )
      let listInput = WorkTasksInput(workspaceId: workspaceId, projectId: optional(selectedProjectId), scope: GraphQLEnum(.active))
      let listQuery = TasksListQuery(input: listInput, first: .some(100), after: .none)

      if let overview = try await fetch(overviewQuery).data { applyOverview(overview.workOverview) }
      if let projects = try await fetch(projectsQuery).data { applyProjects(projects.projects) }
      if let needsYou = try await fetch(needsQuery).data { applyNeedsYou(needsYou.needsYou) }
      if let allTasks = try await fetch(listQuery).data {
        tasks = allTasks.workTasks.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }
        tasksEndCursor = allTasks.workTasks.pageInfo.endCursor
        hasMoreTasks = allTasks.workTasks.pageInfo.hasNextPage && tasksEndCursor != nil
        hasLoadedTasks = true
      }
      if let pending = try await fetch(pendingQuery).data {
        pendingInterventions = pending.pendingHumanInterventions.compactMap(HumanIntervention.init).filter {
          if case .attention = $0 { return false }
          return true
        }
        let visibleIDs = Set(pendingInterventions.map(\.id))
        interventionErrors = interventionErrors.filter { visibleIDs.contains($0.key) }
      }
      await loadHistory()
      if isConnected { lastError = nil }
    } catch {
      record(error)
    }
  }

  func selectProject(_ projectId: String?) async {
    selectedProjectId = projectId
    await refresh()
  }

  func loadDetail(taskId: String) async {
    let requestID = UUID()
    if detailTaskID != taskId {
      taskSubscription?.cancel()
      taskSubscription = nil
      runtimeSubscription?.cancel()
      runtimeSubscription = nil
    }
    detailRequestID = requestID
    detailTaskID = taskId
    if detail?.id != taskId {
      isLoadingDetail = true
      detail = nil
      runItems = []
    }
    defer {
      if detailRequestID == requestID { isLoadingDetail = false }
    }
    do {
      let query = TasksDetailQuery(taskId: taskId)
      if let result = try await fetch(query).data {
        guard detailRequestID == requestID, detailTaskID == taskId, !Task.isCancelled else { return }
        let nextDetail = mapDetail(result.task)
        detail = nextDetail
        let runIDs = Set(nextDetail.runs.map(\.id))
        runItems.removeAll { !runIDs.contains($0.runId) }
        for run in nextDetail.runs {
          await loadRunItems(runId: run.id)
          guard detailRequestID == requestID, detailTaskID == taskId, !Task.isCancelled else { return }
        }
      }
      guard detailRequestID == requestID, detailTaskID == taskId, !Task.isCancelled else { return }
      if isConnected { lastError = nil }
      if taskSubscription == nil { subscribeToTask(taskId) }
      if runtimeSubscription == nil { subscribeToRuntime(taskId) }
    } catch {
      guard detailRequestID == requestID, detailTaskID == taskId, !Task.isCancelled else { return }
      record(error)
    }
  }

  func clearDetail(taskId: String) {
    guard detailTaskID == taskId else { return }
    detailRequestID = UUID()
    detailTaskID = nil
    isLoadingDetail = false
    taskSubscription?.cancel()
    taskSubscription = nil
    runtimeSubscription?.cancel()
    runtimeSubscription = nil
    runItems = []
    runItemEndCursor = [:]
    runItemHasNextPage = [:]
    detail = nil
  }

  func loadRunItems(runId: String, after: String? = nil) async {
    let expectedTaskID = detailTaskID
    do {
      let query = TasksRunItemsQuery(runId: runId, first: .some(50), after: optional(after))
      if let result = try await fetch(query).data {
        guard expectedTaskID == detailTaskID, detailTaskID != nil, !Task.isCancelled else { return }
        let nextItems = result.taskRunItems.edges.map { item in
          let node = item.node
          return TasksRunItemSnapshot(id: node.itemId, runId: node.runId, sequence: node.sequenceIndex, round: node.roundIndex, kind: node.kind.rawValue, status: node.status.rawValue, correlationId: node.correlationId, parentItemId: node.parentItemId, content: node.contentText, payloadText: node.payload.encodedString, createdAt: node.createdAt, updatedAt: node.updatedAt)
        }
        let nextIDs = Set(nextItems.map(\.id))
        runItems.removeAll { nextIDs.contains($0.id) }
        runItems.append(contentsOf: nextItems)
        runItems.sort { $0.runId == $1.runId ? $0.sequence < $1.sequence : $0.runId < $1.runId }
        if let cursor = result.taskRunItems.pageInfo.endCursor { runItemEndCursor[runId] = cursor }
        runItemHasNextPage[runId] = result.taskRunItems.pageInfo.hasNextPage
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
      let query = TasksHistoryQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), kind: .none, text: .none, first: .some(50), after: .none)
      if let result = try await fetch(query).data {
        history = result.taskHistory.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }
        historyEndCursor = result.taskHistory.pageInfo.endCursor
        hasMoreHistory = result.taskHistory.pageInfo.hasNextPage && historyEndCursor != nil
      }
      if isConnected { lastError = nil }
    } catch { record(error) }
  }

  func loadMoreTasks() async {
    guard !isLoadingMoreTasks, hasMoreTasks, let cursor = tasksEndCursor else { return }
    isLoadingMoreTasks = true
    defer { isLoadingMoreTasks = false }
    do {
      let input = WorkTasksInput(workspaceId: workspaceId, projectId: optional(selectedProjectId), scope: GraphQLEnum(.active))
      let query = TasksListQuery(input: input, first: .some(100), after: .some(cursor))
      guard let result = try await fetch(query).data else { return }
      appendUnique(result.workTasks.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }, to: &tasks)
      tasksEndCursor = result.workTasks.pageInfo.endCursor
      hasMoreTasks = result.workTasks.pageInfo.hasNextPage && tasksEndCursor != nil
      lastError = nil
    } catch { record(error) }
  }

  func loadMoreHistory() async {
    guard !isLoadingMoreHistory, hasMoreHistory, let cursor = historyEndCursor else { return }
    isLoadingMoreHistory = true
    defer { isLoadingMoreHistory = false }
    do {
      let query = TasksHistoryQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), kind: .none, text: .none, first: .some(50), after: .some(cursor))
      guard let result = try await fetch(query).data else { return }
      appendUnique(result.taskHistory.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }, to: &history)
      historyEndCursor = result.taskHistory.pageInfo.endCursor
      hasMoreHistory = result.taskHistory.pageInfo.hasNextPage && historyEndCursor != nil
      lastError = nil
    } catch { record(error) }
  }

  private func appendUnique(_ incoming: [TasksTaskRow], to rows: inout [TasksTaskRow]) {
    let known = Set(rows.map(\.id))
    rows.append(contentsOf: incoming.filter { !known.contains($0.id) })
  }

  func capture(title: String, description: String, projectId: String?) async -> Bool {
    guard isConnected else { return false }
    let input = CaptureTaskInput(workspaceId: workspaceId, projectId: optional(projectId), title: title, description: description, clientMutationId: UUID().uuidString)
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
  func updateInbox(task: TasksTaskRow, title: String, description: String, projectId: String?) async -> Bool {
    await updateInbox(taskId: task.id, revision: task.revision, generation: task.generation, title: title, description: description, projectId: projectId)
  }

  @discardableResult
  func updateInbox(task: TasksDetailSnapshot, title: String, description: String, projectId: String?) async -> Bool {
    await updateInbox(taskId: task.id, revision: task.revision, generation: task.generation, title: title, description: description, projectId: projectId)
  }

  private func updateInbox(taskId: String, revision: Int, generation: Int, title: String, description: String, projectId: String?) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = UpdateInboxTaskInput(taskId: taskId, expectedRevision: Int32(revision), expectedGeneration: Int32(generation), title: .some(title), description: .some(description), projectId: optional(projectId), clearProject: projectId == nil ? .some(true) : .none, clientMutationId: UUID().uuidString)
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

  func importAdapterClientJSON(_ definition: AdapterDefinitionModel, data: Data) async throws {
    guard isConnected else { throw ChatModelError.offline }
    try await HumanInterventionActions.importClientJSON(definition, data: data, client: client)
    await refresh()
  }

  func startAdapterOAuth(_ connection: AdapterConnectionModel) async throws -> AdapterOAuthSetupAttempt {
    guard isConnected else { throw ChatModelError.offline }
    let attempt = try await HumanInterventionActions.startOAuth(connection, client: client)
    await refresh()
    return attempt
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
    let input = ReopenTaskInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), feedbackMarkdown: feedback, requestMarkdown: optional(request), replacementCriteria: .none, complexity: .none, clientMutationId: UUID().uuidString)
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
  func createProject(name: String, description: String) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = CreateProjectInput(workspaceId: workspaceId, name: name, description: description, clientMutationId: UUID().uuidString)
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
  func updateProject(_ project: TasksProjectSnapshot, name: String, description: String) async -> Bool {
    guard isConnected else { return false }
    lastError = nil
    let input = UpdateProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), name: .some(name), description: .some(description), clientMutationId: UUID().uuidString)
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

  private func subscribeToWork() {
    eventSubscription?.cancel()
    eventSubscription = Task { [weak self] in
      guard let self else { return }
      do {
        let stream = try client.subscribe(
          subscription: TasksWorkEventsSubscription(
            workspaceId: workspaceId,
            after: optional(eventCursor)
          )
        )
        for try await value in stream {
          guard !Task.isCancelled else { return }
          isConnected = true
          eventCursor = value.data?.workEvents.cursor ?? eventCursor
          await refresh()
        }
      } catch {
        guard !Task.isCancelled else { return }
        record(error)
        try? await Task.sleep(for: .seconds(2))
        guard started, !Task.isCancelled else { return }
        await refresh()
        subscribeToWork()
      }
    }
  }

  private func subscribeToTask(_ taskId: String) {
    taskSubscription?.cancel()
    taskSubscription = Task { [weak self] in
      guard let self else { return }
      do {
        let stream = try client.subscribe(
          subscription: TasksTaskEventsSubscription(taskId: taskId, after: optional(eventCursor))
        )
        for try await value in stream {
          guard !Task.isCancelled else { return }
          isConnected = true
          eventCursor = value.data?.taskEvents.cursor ?? eventCursor
          await loadDetail(taskId: taskId)
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
        let stream = try client.subscribe(subscription: TasksRuntimeEventsSubscription(taskId: taskId))
        for try await value in stream {
          guard !Task.isCancelled else { return }
          isConnected = true
          if let runId = value.data?.taskRuntimeEvents.runId {
            await loadRunItems(runId: runId)
          }
          await loadDetail(taskId: taskId)
        }
      } catch {
        guard !Task.isCancelled else { return }
        record(error)
      }
    }
  }

  private func applyOverview(_ overview: TasksOverviewQuery.Data.WorkOverview) {
    workspace = TasksWorkspaceSnapshot(id: overview.workspace.workspaceId, name: overview.workspace.name, description: overview.workspace.description, isPersonal: overview.workspace.isPersonal)
    columns = overview.boardColumns.map { TasksColumnSnapshot(id: $0.stage.stageId, title: $0.stage.name, behavior: TasksStageBehavior($0.stage.behavior.rawValue), count: $0.taskCount) }
    tasks = overview.recentTasks.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }
  }

  private func applyProjects(_ page: TasksProjectsQuery.Data.Projects) {
    projects = page.edges.map { mapProject($0.node.fragments.tasksProjectFields) }
  }

  private func applyNeedsYou(_ page: TasksNeedsYouQuery.Data.NeedsYou) {
    needsYou = page.edges.map { item in
      let node = item.node
      let task = mapCard(node.task.fragments.tasksTaskCardFields)
      return TasksAttentionRow(id: task.id, kind: node.kind.rawValue, title: node.title, summary: node.summary, task: task, gate: node.gate.map { mapGate($0.fragments.tasksGateFields) }, review: node.review.map { TasksReviewSnapshot(id: $0.reviewId, verdict: $0.verdict.rawValue, feedback: $0.feedback, createdAt: $0.createdAt) }, validActions: Set(node.validActions.map(\.rawValue)))
    }
  }

  private func mapSummary(_ source: TasksTaskSummaryFields) -> TasksTaskRow {
    TasksTaskRow(id: source.taskId, workspaceId: source.workspace.workspaceId, projectId: source.project?.projectId, projectName: source.project?.name, title: source.title, summary: source.descriptionPreview, stage: mapStage(source.stage.fragments.tasksStageFields), revision: source.revision, generation: source.generation, updatedAt: source.updatedAt, completedAt: source.completedAt, currentRun: source.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) }, activeGate: source.activeGate.map { mapGate($0.fragments.tasksGateFields) }, latestReview: source.latestReview.map { mapReview($0.fragments.tasksReviewSummaryFields) }, validActions: Set(source.validActions.map(\.rawValue)))
  }

  private func mapCard(_ source: TasksTaskCardFields) -> TasksTaskRow {
    TasksTaskRow(id: source.taskId, workspaceId: source.workspace.workspaceId, projectId: source.project?.projectId, projectName: source.project?.name, title: source.title, summary: source.descriptionPreview, stage: mapStage(source.stage.fragments.tasksStageFields), revision: source.revision, generation: source.generation, updatedAt: source.updatedAt, completedAt: source.completedAt, currentRun: source.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) }, activeGate: source.activeGate.map { mapGate($0.fragments.tasksGateFields) }, latestReview: source.latestReview.map { mapReview($0.fragments.tasksReviewSummaryFields) }, validActions: Set(source.validActions.map(\.rawValue)))
  }

  private func mapProject(_ source: TasksProjectFields) -> TasksProjectSnapshot {
    TasksProjectSnapshot(id: source.projectId, workspaceId: source.workspaceId, name: source.name, description: source.description, revision: source.revision, archivedAt: source.archivedAt)
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

  private func mapReview(_ source: TasksReviewSummaryFields) -> TasksReviewSnapshot {
    TasksReviewSnapshot(id: source.reviewId, verdict: source.verdict.rawValue, feedback: source.feedback, createdAt: source.createdAt)
  }

  private func mapDetail(_ source: TasksDetailQuery.Data.Task) -> TasksDetailSnapshot {
    let command = source.fragments.tasksCommandTaskFields
    let contract = source.currentContract?.fragments.tasksContractFields
    let reviews = source.reviews.map { $0.fragments.tasksReviewFields }
    let submissions = source.submissions.map { $0.fragments.tasksSubmissionFields }
    var reviewedCriteria: [String: TasksReviewFields.Criterium] = [:]
    var reviewCriteriaBySubmission: [String: [String: TasksReviewFields.Criterium]] = [:]
    for review in reviews {
      var criteriaForSubmission = reviewCriteriaBySubmission[review.reviewedSubmissionId] ?? [:]
      for criterion in review.criteria {
        if reviewedCriteria[criterion.criterionId] == nil { reviewedCriteria[criterion.criterionId] = criterion }
        if criteriaForSubmission[criterion.criterionId] == nil { criteriaForSubmission[criterion.criterionId] = criterion }
      }
      reviewCriteriaBySubmission[review.reviewedSubmissionId] = criteriaForSubmission
    }
    var submittedEvidence: [String: String] = [:]
    for submission in submissions {
      for criterion in submission.criteria where submittedEvidence[criterion.criterionId] == nil {
        submittedEvidence[criterion.criterionId] = criterion.evidenceMarkdown
      }
    }
    let contractCriteria = (contract?.criteria ?? []).map {
      let reviewed = reviewedCriteria[$0.criterionId]
      return TasksCriterionSnapshot(
        id: $0.criterionId,
        ordinal: $0.ordinal,
        description: $0.description,
        expectedEvidence: $0.expectedEvidence,
        evidence: reviewed?.evidenceMarkdown ?? submittedEvidence[$0.criterionId],
        verdict: reviewed?.outcome.rawValue ?? "PENDING"
      )
    }
    let criteria = Dictionary(uniqueKeysWithValues: contractCriteria.map {
      ($0.id, ($0.ordinal, $0.description, $0.expectedEvidence))
    })
    return TasksDetailSnapshot(id: command.taskId, title: command.title, description: command.description, project: source.project.map { mapProject($0.fragments.tasksProjectFields) }, stage: mapStage(command.stage.fragments.tasksStageFields), revision: command.revision, generation: command.generation, updatedAt: command.updatedAt, completedAt: command.completedAt, createdAt: source.createdAt, complexity: contract?.complexity.rawValue, maxReviewRounds: contract?.executionPolicy.maxReviewRounds, sourceLabel: source.project?.name ?? (source.source.conversationId == nil ? nil : "Conversation"), currentContract: contract?.requestMarkdown, criteria: contractCriteria, currentRun: command.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) }, activeGate: command.activeGate.map { mapGate($0.fragments.tasksGateFields) }, latestSubmission: source.latestSubmission.map { mapSubmission($0.fragments.tasksSubmissionFields, contractCriteria: criteria, reviewedCriteria: reviewCriteriaBySubmission[$0.submissionId] ?? [:]) }, completedResult: source.completedResult.map { mapSubmission($0.fragments.tasksSubmissionFields, contractCriteria: criteria, reviewedCriteria: reviewCriteriaBySubmission[$0.submissionId] ?? [:]) }, latestReview: source.latestReview.map { mapReview($0.fragments.tasksReviewSummaryFields) }, messages: source.messages.map { TasksMessageSnapshot(id: $0.messageId, body: $0.bodyMarkdown, author: $0.author, createdAt: $0.createdAt) }, runs: source.runs.map { mapRun($0.fragments.tasksRunFields) }, validActions: Set(command.validActions.map(\.rawValue)))
  }

  private func mergeCommand(_ source: TasksCommandTaskFields, into previous: TasksDetailSnapshot?) -> TasksDetailSnapshot {
    var next = previous ?? TasksDetailSnapshot(id: source.taskId, title: source.title, description: source.description, project: nil, stage: mapStage(source.stage.fragments.tasksStageFields), revision: source.revision, generation: source.generation, updatedAt: source.updatedAt, completedAt: source.completedAt, createdAt: "", complexity: nil, maxReviewRounds: nil, sourceLabel: nil, currentContract: nil, criteria: [], currentRun: nil, activeGate: nil, latestSubmission: nil, completedResult: nil, latestReview: nil, messages: [], runs: [], validActions: [])
    next.title = source.title
    next.description = source.description
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

  private func mapSubmission(
    _ source: TasksSubmissionFields,
    contractCriteria: [String: (ordinal: Int, description: String, expectedEvidence: String?)],
    reviewedCriteria: [String: TasksReviewFields.Criterium]
  ) -> TasksSubmissionSnapshot {
    TasksSubmissionSnapshot(
      id: source.submissionId,
      summary: source.summary,
      result: source.resultMarkdown,
      createdAt: source.createdAt,
      criteria: source.criteria.map {
        let contract = contractCriteria[$0.criterionId]
        let reviewed = reviewedCriteria[$0.criterionId]
        return TasksCriterionSnapshot(id: $0.criterionId, ordinal: contract?.ordinal ?? 0, description: contract?.description ?? $0.criterionId, expectedEvidence: contract?.expectedEvidence, evidence: reviewed?.evidenceMarkdown ?? $0.evidenceMarkdown, verdict: reviewed?.outcome.rawValue ?? "PENDING")
      },
      artifacts: source.artifacts.map {
        TasksArtifactSnapshot(id: $0.artifactId, versionID: $0.artifactVersionId, title: $0.title, kind: $0.artifactKind, storageKind: $0.storageKind.rawValue, mediaType: $0.mediaType, downloadURL: $0.downloadUrl, externalURL: $0.externalUrl)
      }
    )
  }

  private func optional<T>(_ value: T?) -> GraphQLNullable<T> {
    value.map(GraphQLNullable.some) ?? .none
  }

  private func fetch<Query: GraphQLQuery>(_ query: Query) async throws -> GraphQLResponse<Query> where Query.ResponseFormat == SingleResponseFormat {
    let response = try await client.fetch(query: query, cachePolicy: .networkFirst)
    isConnected = response.source == .server
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
    if !(error is TasksGraphQLError) { isConnected = false }
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
