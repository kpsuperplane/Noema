import Foundation

struct TasksWorkspaceSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let name: String
  let description: String
  let isPersonal: Bool
}

struct TasksProjectSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let workspaceId: String
  var name: String
  var description: String
  var folder: String?
  var revision: Int
  let archivedAt: String?
}

struct TasksScheduleSnapshot: Hashable, Sendable {
  let scheduledFor: String
  let timeZone: String
  let missedRunPolicy: String
  let recurrenceId: String?
  let recurrenceRevision: Int?
  let recurrenceScheduledFor: String?

  var isRecurring: Bool { recurrenceId != nil }
}

struct TasksExecutorSnapshot: Hashable, Sendable {
  let agentId: String
  let backend: String
  let cwdOverride: String?
  let effectiveCwd: String?
  let effectiveCwdSource: String

  static let `default` = TasksExecutorSnapshot(
    agentId: "agent:task-executor",
    backend: "BUILT_IN",
    cwdOverride: nil,
    effectiveCwd: nil,
    effectiveCwdSource: "DEFAULT"
  )
}

struct TasksAcpAgentSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let displayName: String
  let enabled: Bool
  let authStatus: String
  let healthStatus: String
  let implementationName: String?
  let implementationVersion: String?
  let lastError: String?
}

struct TasksRecurrenceOccurrenceSnapshot: Identifiable, Hashable, Sendable {
  let recurrenceRevision: Int
  let scheduledFor: String
  let localSlot: String
  let trigger: String
  let resolution: String
  let taskId: String?
  let createdAt: String

  var id: String { "\(localSlot):\(recurrenceRevision)" }
}

struct TasksRecurrenceSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  var title: String
  var description: String
  var startsAt: String
  var cronExpression: String
  var timeZone: String
  var missedRunPolicy: String
  var overlapPolicy: String
  var lifecycle: String
  var revision: Int
  var nextRunAt: String?
  var pendingCoalescedAt: String?
  var occurrences: [TasksRecurrenceOccurrenceSnapshot]
}

enum TasksRecurrenceAction: Sendable {
  case pause
  case resume
  case skip
  case end
  case runNow
}

enum TasksStageBehavior: String, CaseIterable, Sendable {
  case intake = "INTAKE"
  case dispatch = "DISPATCH"
  case active = "ACTIVE"
  case humanGate = "HUMAN_GATE"
  case terminalSuccess = "TERMINAL_SUCCESS"
  case terminalCancelled = "TERMINAL_CANCELLED"
  case unknown

  init(_ value: String) {
    self = Self(rawValue: value) ?? .unknown
  }

  var title: String {
    switch self {
    case .intake: "Inbox"
    case .dispatch: "Queue"
    case .active: "Doing"
    case .humanGate: "Waiting"
    case .terminalSuccess: "Done"
    case .terminalCancelled: "Cancelled"
    case .unknown: "Task"
    }
  }
}

struct TasksStageSnapshot: Hashable, Sendable {
  let id: String
  let name: String
  let behavior: TasksStageBehavior
}

struct TasksRunSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let instanceName: String
  let kind: String
  let status: String
  let attempt: Int
  let activity: String
  let startedAt: String?
  let endedAt: String?
  let createdAt: String?
  let error: String?
}

struct TasksRunItemSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let runId: String
  let sequence: Int
  let round: Int
  let kind: String
  let status: String
  let correlationId: String?
  let parentItemId: String?
  let content: String?
  let payloadText: String
  let createdAt: String
  let updatedAt: String
}

struct TasksGateSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let kind: String
  let state: String
  let prompt: String
  let context: String
  let suggestedAnswers: [String]
  let recoveryReason: String?
  let retryRunKind: String?
}

struct TasksReviewSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let verdict: String
  let feedback: String
  let createdAt: String
}

struct TasksTaskRow: Identifiable, Hashable, Sendable {
  let id: String
  let workspaceId: String
  let projectId: String?
  let projectName: String?
  let title: String
  let summary: String
  let executor: TasksExecutorSnapshot
  let schedule: TasksScheduleSnapshot?
  let stage: TasksStageSnapshot
  let revision: Int
  let generation: Int
  let updatedAt: String
  let completedAt: String?
  let currentRun: TasksRunSnapshot?
  let activeGate: TasksGateSnapshot?
  let latestReview: TasksReviewSnapshot?
  let validActions: Set<String>
}

struct TasksAttentionRow: Identifiable, Hashable, Sendable {
  let id: String
  let kind: String
  let title: String
  let summary: String
  let task: TasksTaskRow
  let gate: TasksGateSnapshot?
  let review: TasksReviewSnapshot?
  let validActions: Set<String>
}

struct TasksMessageSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let body: String
  let author: String
  let createdAt: String
}

struct TasksCriterionSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let ordinal: Int
  let description: String
  let expectedEvidence: String?
  let evidence: String?
  let verdict: String
}

struct TasksArtifactSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let versionID: String
  let title: String
  let kind: String
  let storageKind: String
  let mediaType: String?
  let downloadURL: String?
  let externalURL: String?
}

struct TasksSubmissionSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let summary: String
  let result: String
  let createdAt: String
  let citations: [ProviderCitation]
  let criteria: [TasksCriterionSnapshot]
  let artifacts: [TasksArtifactSnapshot]
}

struct TasksDetailSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  var title: String
  var description: String
  var project: TasksProjectSnapshot?
  var executor: TasksExecutorSnapshot = .default
  var schedule: TasksScheduleSnapshot? = nil
  var stage: TasksStageSnapshot
  var revision: Int
  var generation: Int
  var updatedAt: String
  var completedAt: String?
  var createdAt: String
  var complexity: String?
  var maxReviewRounds: Int?
  var sourceLabel: String?
  var currentContract: String?
  var criteria: [TasksCriterionSnapshot]
  var currentRun: TasksRunSnapshot?
  var activeGate: TasksGateSnapshot?
  var latestSubmission: TasksSubmissionSnapshot?
  var completedResult: TasksSubmissionSnapshot?
  var latestReview: TasksReviewSnapshot?
  var messages: [TasksMessageSnapshot]
  var runs: [TasksRunSnapshot]
  var validActions: Set<String>
}

extension TasksTaskRow {
  func detailSnapshot(gate: TasksGateSnapshot?) -> TasksDetailSnapshot {
    TasksDetailSnapshot(
      id: id,
      title: title,
      description: summary,
      project: nil,
      stage: stage,
      revision: revision,
      generation: generation,
      updatedAt: updatedAt,
      completedAt: completedAt,
      createdAt: "",
      complexity: nil,
      maxReviewRounds: nil,
      sourceLabel: projectName,
      currentContract: nil,
      criteria: [],
      currentRun: currentRun,
      activeGate: gate,
      latestSubmission: nil,
      completedResult: nil,
      latestReview: latestReview,
      messages: [],
      runs: [],
      validActions: validActions
    )
  }
}

struct TasksColumnSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let title: String
  let behavior: TasksStageBehavior
  let count: Int
}
