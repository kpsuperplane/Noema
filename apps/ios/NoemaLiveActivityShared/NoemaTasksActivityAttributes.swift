import ActivityKit
import Foundation

/// The data shared by the Noema app and its Live Activity widget.
///
/// This type intentionally contains only display state. The widget never receives
/// the paired credential or an Apollo client, and the app owns all server work.
public struct NoemaTasksActivityAttributes: ActivityAttributes, Codable, Hashable, Sendable {
  public struct ContentState: Codable, Hashable, Sendable {
    public enum Phase: String, Codable, Hashable, Sendable {
      case inProgress
      case planning
      case working
      case reviewing
      case completed
      case cancelled
    }

    public struct TaskSummary: Codable, Hashable, Sendable, Identifiable {
      public let taskId: String
      public let title: String
      public let phase: Phase
      public let statusLabel: String
      public let startedAtEpoch: Double?
      public let requiresAttention: Bool?

      public var id: String { taskId }

      public init(
        taskId: String,
        title: String,
        phase: Phase,
        statusLabel: String,
        startedAtEpoch: Double?,
        requiresAttention: Bool? = nil
      ) {
        self.taskId = taskId
        self.title = title
        self.phase = phase
        self.statusLabel = statusLabel
        self.startedAtEpoch = startedAtEpoch
        self.requiresAttention = requiresAttention
      }
    }

    public let focusTaskId: String
    public let focusTitle: String
    public let projectName: String?
    public let agentName: String?
    public let phase: Phase
    public let statusLabel: String
    public let activeTaskCount: Int
    public let startedAtEpoch: Double?
    public let updatedAtEpoch: Double
    public let requiresAttention: Bool?
    public let updateLabel: String?
    public let updateAtEpoch: Double?
    public let completedOutputCount: Int?
    public let taskSummaries: [TaskSummary]?

    public init(
      focusTaskId: String,
      focusTitle: String,
      projectName: String?,
      agentName: String? = nil,
      phase: Phase,
      statusLabel: String,
      activeTaskCount: Int,
      startedAtEpoch: Double?,
      updatedAtEpoch: Double,
      requiresAttention: Bool? = nil,
      updateLabel: String? = nil,
      updateAtEpoch: Double? = nil,
      completedOutputCount: Int? = nil,
      taskSummaries: [TaskSummary]? = nil
    ) {
      self.focusTaskId = focusTaskId
      self.focusTitle = focusTitle
      self.projectName = projectName
      self.agentName = agentName
      self.phase = phase
      self.statusLabel = statusLabel
      self.activeTaskCount = activeTaskCount
      self.startedAtEpoch = startedAtEpoch
      self.updatedAtEpoch = updatedAtEpoch
      self.requiresAttention = requiresAttention
      self.updateLabel = updateLabel
      self.updateAtEpoch = updateAtEpoch
      self.completedOutputCount = completedOutputCount
      self.taskSummaries = taskSummaries
    }
  }

  /// The server-visible activity identifier used for update-token registration.
  public let activityId: String
  public let clientId: String
  public let serverOrigin: String

  public init(activityId: String, clientId: String, serverOrigin: String) {
    self.activityId = activityId
    self.clientId = clientId
    self.serverOrigin = serverOrigin
  }
}
