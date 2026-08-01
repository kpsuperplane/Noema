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
  var revision: Int
  let archivedAt: String?
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
    case .unknown: "Work"
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
  let evidence: String?
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
  let criteria: [TasksCriterionSnapshot]
  let artifacts: [TasksArtifactSnapshot]
}

struct TasksDetailSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  var title: String
  var description: String
  var project: TasksProjectSnapshot?
  var stage: TasksStageSnapshot
  var revision: Int
  var generation: Int
  var updatedAt: String
  var completedAt: String?
  var currentContract: String?
  var currentRun: TasksRunSnapshot?
  var activeGate: TasksGateSnapshot?
  var latestSubmission: TasksSubmissionSnapshot?
  var completedResult: TasksSubmissionSnapshot?
  var latestReview: TasksReviewSnapshot?
  var messages: [TasksMessageSnapshot]
  var runs: [TasksRunSnapshot]
  var validActions: Set<String>
}

struct TasksColumnSnapshot: Identifiable, Hashable, Sendable {
  let id: String
  let title: String
  let behavior: TasksStageBehavior
  let count: Int
}
