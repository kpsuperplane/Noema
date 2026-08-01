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
  let workspaceId: String

  private(set) var workspace: TasksWorkspaceSnapshot?
  private(set) var projects: [TasksProjectSnapshot] = []
  private(set) var columns: [TasksColumnSnapshot] = []
  private(set) var tasks: [TasksTaskRow] = []
  private(set) var needsYou: [TasksAttentionRow] = []
  private(set) var detail: TasksDetailSnapshot?
  private(set) var runItems: [TasksRunItemSnapshot] = []
  private(set) var history: [TasksTaskRow] = []
  private(set) var selectedProjectId: String?
  private(set) var eventCursor: String?
  private(set) var isRefreshing = false
  private(set) var isConnected = true
  private(set) var lastError: String?

  private var eventSubscription: Cancellable?
  private var taskSubscription: Cancellable?
  private var runtimeSubscription: Cancellable?
  private var started = false

  init(client: ApolloClient, workspaceId: String = TasksModel.personalWorkspaceId) {
    self.client = client
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
      let projectsQuery = TasksProjectsQuery(workspaceId: workspaceId, includeArchived: false, first: .some(100), after: .none)
      let needsQuery = TasksNeedsYouQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), first: .some(50), after: .none)
      let listInput = WorkTasksInput(workspaceId: workspaceId, projectId: optional(selectedProjectId), scope: GraphQLEnum(.all))
      let listQuery = TasksListQuery(input: listInput, first: .some(100), after: .none)

      if let overview = try await fetch(overviewQuery).data { applyOverview(overview.workOverview) }
      if let projects = try await fetch(projectsQuery).data { applyProjects(projects.projects) }
      if let needsYou = try await fetch(needsQuery).data { applyNeedsYou(needsYou.needsYou) }
      if let allTasks = try await fetch(listQuery).data { tasks = allTasks.workTasks.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) } }
      isConnected = true
      lastError = nil
    } catch {
      isConnected = false
      lastError = error.localizedDescription
    }
  }

  func selectProject(_ projectId: String?) async {
    selectedProjectId = projectId
    await refresh()
  }

  func loadDetail(taskId: String) async {
    do {
      let query = TasksDetailQuery(taskId: taskId)
      if let result = try await fetch(query).data { detail = mapDetail(result.task) }
      isConnected = true
      lastError = nil
      subscribeToTask(taskId)
      subscribeToRuntime(taskId)
    } catch {
      isConnected = false
      lastError = error.localizedDescription
    }
  }

  func clearDetail() {
    taskSubscription?.cancel()
    taskSubscription = nil
    runtimeSubscription?.cancel()
    runtimeSubscription = nil
    runItems = []
    history = []
    detail = nil
  }

  func loadRunItems(runId: String) async {
    do {
      let query = TasksRunItemsQuery(runId: runId, first: .some(100), after: .none)
      if let result = try await fetch(query).data {
        runItems = result.taskRunItems.edges.map { item in
          let node = item.node
          return TasksRunItemSnapshot(id: node.itemId, runId: node.runId, sequence: node.sequenceIndex, round: node.roundIndex, kind: node.kind.rawValue, status: node.status.rawValue, content: node.contentText, createdAt: node.createdAt, updatedAt: node.updatedAt)
        }
      }
      isConnected = true
      lastError = nil
    } catch { record(error) }
  }

  func clearRunItems() {
    runItems = []
  }

  func loadHistory() async {
    do {
      let query = TasksHistoryQuery(workspaceId: workspaceId, projectId: optional(selectedProjectId), kind: .none, text: .none, first: .some(50), after: .none)
      if let result = try await fetch(query).data {
        history = result.taskHistory.edges.map { mapSummary($0.node.fragments.tasksTaskSummaryFields) }
      }
      isConnected = true
      lastError = nil
    } catch { record(error) }
  }

  func capture(title: String, description: String, projectId: String?) async {
    guard isConnected else { return }
    let input = CaptureTaskInput(workspaceId: workspaceId, projectId: optional(projectId), title: title, description: description, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksCaptureTaskMutation(input: input))
      eventCursor = result.captureTask.eventCursor
      detail = mergeCommand(result.captureTask.task.fragments.tasksCommandTaskFields, into: nil)
      await refresh()
    } catch { record(error) }
  }

  func updateInbox(task: TasksTaskRow, title: String, description: String, projectId: String?) async {
    await updateInbox(taskId: task.id, revision: task.revision, generation: task.generation, title: title, description: description, projectId: projectId)
  }

  func updateInbox(task: TasksDetailSnapshot, title: String, description: String, projectId: String?) async {
    await updateInbox(taskId: task.id, revision: task.revision, generation: task.generation, title: title, description: description, projectId: projectId)
  }

  private func updateInbox(taskId: String, revision: Int, generation: Int, title: String, description: String, projectId: String?) async {
    guard isConnected else { return }
    let input = UpdateInboxTaskInput(taskId: taskId, expectedRevision: Int32(revision), expectedGeneration: Int32(generation), title: .some(title), description: .some(description), projectId: optional(projectId), clearProject: projectId == nil ? .some(true) : .none, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksUpdateInboxTaskMutation(input: input))
      eventCursor = result.updateInboxTask.eventCursor
      detail = mergeCommand(result.updateInboxTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
    } catch { record(error) }
  }

  func queue(task: TasksTaskRow) async {
    await queue(taskId: task.id, revision: task.revision, generation: task.generation)
  }

  func queue(task: TasksDetailSnapshot) async {
    await queue(taskId: task.id, revision: task.revision, generation: task.generation)
  }

  private func queue(taskId: String, revision: Int, generation: Int) async {
    guard isConnected else { return }
    let input = QueueTaskInput(taskId: taskId, expectedRevision: Int32(revision), expectedGeneration: Int32(generation), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksQueueTaskMutation(input: input))
      eventCursor = result.queueTask.eventCursor
      detail = mergeCommand(result.queueTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
    } catch { record(error) }
  }

  func answer(task: TasksDetailSnapshot, answer: String, approval: ApprovalDecision? = nil) async {
    guard isConnected, let gate = task.activeGate else { return }
    let input = AnswerTaskInput(taskId: task.id, gateId: gate.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), answerMarkdown: answer, approvalDecision: approval.map(GraphQLEnum.init) ?? .none, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksAnswerTaskMutation(input: input))
      eventCursor = result.answerTask.eventCursor
      detail = mergeCommand(result.answerTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
    } catch { record(error) }
  }

  func retry(task: TasksDetailSnapshot, note: String? = nil) async {
    guard isConnected, let gate = task.activeGate else { return }
    let input = RetryTaskInput(taskId: task.id, gateId: gate.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), retryNote: optional(note), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksRetryTaskMutation(input: input))
      eventCursor = result.retryTask.eventCursor
      detail = mergeCommand(result.retryTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
    } catch { record(error) }
  }

  func cancel(task: TasksDetailSnapshot, reason: String? = nil) async {
    guard isConnected else { return }
    let input = CancelTaskInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), reason: optional(reason), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksCancelTaskMutation(input: input))
      eventCursor = result.cancelTask.eventCursor
      detail = mergeCommand(result.cancelTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
    } catch { record(error) }
  }

  func reopen(task: TasksDetailSnapshot, feedback: String, request: String? = nil) async {
    guard isConnected else { return }
    let input = ReopenTaskInput(taskId: task.id, expectedRevision: Int32(task.revision), expectedGeneration: Int32(task.generation), feedbackMarkdown: feedback, requestMarkdown: optional(request), replacementCriteria: .none, complexity: .none, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksReopenTaskMutation(input: input))
      eventCursor = result.reopenTask.eventCursor
      detail = mergeCommand(result.reopenTask.task.fragments.tasksCommandTaskFields, into: detail)
      await refresh()
    } catch { record(error) }
  }

  func createProject(name: String, description: String) async {
    guard isConnected else { return }
    let input = CreateProjectInput(workspaceId: workspaceId, name: name, description: description, clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksCreateProjectMutation(input: input))
      eventCursor = result.createProject.eventCursor
      await refresh()
    } catch { record(error) }
  }

  func updateProject(_ project: TasksProjectSnapshot, name: String, description: String) async {
    guard isConnected else { return }
    let input = UpdateProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), name: .some(name), description: .some(description), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksUpdateProjectMutation(input: input))
      eventCursor = result.updateProject.eventCursor
      await refresh()
    } catch { record(error) }
  }

  func archiveProject(_ project: TasksProjectSnapshot) async {
    guard isConnected else { return }
    let input = ArchiveProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksArchiveProjectMutation(input: input))
      eventCursor = result.archiveProject.eventCursor
      await refresh()
    } catch { record(error) }
  }

  func reopenProject(_ project: TasksProjectSnapshot) async {
    guard isConnected else { return }
    let input = ReopenProjectInput(projectId: project.id, expectedRevision: Int32(project.revision), clientMutationId: UUID().uuidString)
    do {
      let result = try await perform(TasksReopenProjectMutation(input: input))
      eventCursor = result.reopenProject.eventCursor
      await refresh()
    } catch { record(error) }
  }

  private func subscribeToWork() {
    eventSubscription?.cancel()
    eventSubscription = client.subscribe(subscription: TasksWorkEventsSubscription(workspaceId: workspaceId, after: optional(eventCursor))) { [weak self] result in
      Task { @MainActor in
        guard let self else { return }
        switch result {
        case .success(let value):
          self.isConnected = true
          self.eventCursor = value.data?.workEvents.cursor ?? self.eventCursor
          await self.refresh()
        case .failure(let error):
          self.record(error)
          try? await Task.sleep(for: .seconds(2))
          guard self.started else { return }
          await self.refresh()
          self.subscribeToWork()
        }
      }
    }
  }

  private func subscribeToTask(_ taskId: String) {
    taskSubscription?.cancel()
    taskSubscription = client.subscribe(subscription: TasksTaskEventsSubscription(taskId: taskId, after: optional(eventCursor))) { [weak self] result in
      Task { @MainActor in
        guard let self else { return }
        switch result {
        case .success(let value):
          self.isConnected = true
          self.eventCursor = value.data?.taskEvents.cursor ?? self.eventCursor
          await self.loadDetail(taskId: taskId)
        case .failure(let error):
          self.record(error)
        }
      }
    }
  }

  private func subscribeToRuntime(_ taskId: String) {
    runtimeSubscription?.cancel()
    runtimeSubscription = client.subscribe(subscription: TasksRuntimeEventsSubscription(taskId: taskId)) { [weak self] result in
      Task { @MainActor in
        guard let self else { return }
        switch result {
        case .success(let value):
          self.isConnected = true
          if let runId = value.data?.taskRuntimeEvents.runId {
            await self.loadRunItems(runId: runId)
          }
          await self.loadDetail(taskId: taskId)
        case .failure(let error):
          self.record(error)
        }
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
    TasksRunSnapshot(id: source.runId, kind: source.kind.rawValue, status: source.status.rawValue, attempt: source.attemptIndex, activity: source.activityLabel, startedAt: source.startedAt, endedAt: nil, error: nil)
  }

  private func mapRun(_ source: TasksRunFields) -> TasksRunSnapshot {
    TasksRunSnapshot(id: source.runId, kind: source.kind.rawValue, status: source.status.rawValue, attempt: source.attemptIndex, activity: source.errorMessage ?? "", startedAt: source.startedAt, endedAt: source.endedAt, error: source.errorMessage)
  }

  private func mapGate(_ source: TasksGateFields) -> TasksGateSnapshot {
    TasksGateSnapshot(id: source.gateId, kind: source.kind.rawValue, state: source.state.rawValue, prompt: source.prompt, context: source.contextMarkdown, suggestedAnswers: source.suggestedAnswers, recoveryReason: source.recoveryReason?.rawValue, retryRunKind: source.retryRunKind?.rawValue)
  }

  private func mapReview(_ source: TasksReviewSummaryFields) -> TasksReviewSnapshot {
    TasksReviewSnapshot(id: source.reviewId, verdict: source.verdict.rawValue, feedback: source.feedback, createdAt: source.createdAt)
  }

  private func mapDetail(_ source: TasksDetailQuery.Data.Task) -> TasksDetailSnapshot {
    let command = source.fragments.tasksCommandTaskFields
    return TasksDetailSnapshot(id: command.taskId, title: command.title, description: command.description, project: source.project.map { mapProject($0.fragments.tasksProjectFields) }, stage: mapStage(command.stage.fragments.tasksStageFields), revision: command.revision, generation: command.generation, updatedAt: command.updatedAt, completedAt: command.completedAt, currentContract: source.currentContract.map { $0.fragments.tasksContractFields.requestMarkdown }, currentRun: command.currentRun.map { mapRun($0.fragments.tasksCurrentRunFields) }, activeGate: command.activeGate.map { mapGate($0.fragments.tasksGateFields) }, latestSubmission: source.latestSubmission.map { mapSubmission($0.fragments.tasksSubmissionFields) }, completedResult: source.completedResult.map { mapSubmission($0.fragments.tasksSubmissionFields) }, latestReview: source.latestReview.map { mapReview($0.fragments.tasksReviewSummaryFields) }, messages: source.messages.map { TasksMessageSnapshot(id: $0.messageId, body: $0.bodyMarkdown, author: $0.author, createdAt: $0.createdAt) }, runs: source.runs.map { mapRun($0.fragments.tasksRunFields) }, validActions: Set(command.validActions.map(\.rawValue)))
  }

  private func mergeCommand(_ source: TasksCommandTaskFields, into previous: TasksDetailSnapshot?) -> TasksDetailSnapshot {
    var next = previous ?? TasksDetailSnapshot(id: source.taskId, title: source.title, description: source.description, project: nil, stage: mapStage(source.stage.fragments.tasksStageFields), revision: source.revision, generation: source.generation, updatedAt: source.updatedAt, completedAt: source.completedAt, currentContract: nil, currentRun: nil, activeGate: nil, latestSubmission: nil, completedResult: nil, latestReview: nil, messages: [], runs: [], validActions: [])
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

  private func mapSubmission(_ source: TasksSubmissionFields) -> TasksSubmissionSnapshot {
    TasksSubmissionSnapshot(id: source.submissionId, summary: source.summary, result: source.resultMarkdown, createdAt: source.createdAt, criteria: source.criteria.map { TasksCriterionSnapshot(id: $0.criterionId, ordinal: 0, description: "", evidence: $0.evidenceMarkdown) })
  }

  private func optional<T>(_ value: T?) -> GraphQLNullable<T> {
    value.map(GraphQLNullable.some) ?? .none
  }

  private func fetch<Query: GraphQLQuery>(_ query: Query) async throws -> GraphQLResponse<Query> where Query.ResponseFormat == SingleResponseFormat {
    try await client.fetch(query: query, cachePolicy: .networkFirst)
  }

  private func perform<Mutation: GraphQLMutation>(_ mutation: Mutation) async throws -> Mutation.Data where Mutation.ResponseFormat == SingleResponseFormat {
    guard let data = try await client.perform(mutation: mutation).data else {
      throw ApolloClient.Error.noResults
    }
    return data
  }

  private func record(_ error: Error) {
    isConnected = false
    lastError = error.localizedDescription
  }
}
