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

    public let focusTaskId: String
    public let focusTitle: String
    public let projectName: String?
    public let phase: Phase
    public let statusLabel: String
    public let activeTaskCount: Int
    public let startedAtEpoch: Double?
    public let updatedAtEpoch: Double
    public let requiresAttention: Bool?

    public init(
      focusTaskId: String,
      focusTitle: String,
      projectName: String?,
      phase: Phase,
      statusLabel: String,
      activeTaskCount: Int,
      startedAtEpoch: Double?,
      updatedAtEpoch: Double,
      requiresAttention: Bool? = nil
    ) {
      self.focusTaskId = focusTaskId
      self.focusTitle = focusTitle
      self.projectName = projectName
      self.phase = phase
      self.statusLabel = statusLabel
      self.activeTaskCount = activeTaskCount
      self.startedAtEpoch = startedAtEpoch
      self.updatedAtEpoch = updatedAtEpoch
      self.requiresAttention = requiresAttention
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
