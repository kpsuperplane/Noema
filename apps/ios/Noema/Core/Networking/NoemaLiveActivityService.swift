@preconcurrency import ActivityKit
import Apollo
import Foundation
import Observation
import UIKit
import NoemaAPI

struct ClientLiveActivityStatusModel: Equatable, Sendable {
  let available: Bool
  let blocker: String?
  let enabled: Bool
  let registered: Bool
  let environment: String?
}

@MainActor
@Observable
final class NoemaLiveActivityService {
  private(set) var status: ClientLiveActivityStatusModel?
  private(set) var isLoading = false
  private(set) var isWorking = false
  private(set) var errorMessage: String?
  private(set) var activitiesEnabled = ActivityAuthorizationInfo().areActivitiesEnabled
  private(set) var isEnabled = false
  private(set) var diagnosticMessage: String?
  private(set) var isRunningDiagnostic = false

  private var client: ApolloClient?
  private var profile: NoemaProfile?
  private var pushToStartToken: Data?
  private var pushToStartTask: Task<Void, Never>?
  private var registrationTask: Task<Void, Never>?
  private var registrationGeneration = 0
  private var registrationPending = false
  private var statusGeneration = 0
  private var activityUpdatesTask: Task<Void, Never>?
  private var updateTokenTasks: [String: Task<Void, Never>] = [:]
  private var activityStateTasks: [String: Task<Void, Never>] = [:]
  private var endingActivityIDs = Set<String>()
  private var diagnosticActivity: Activity<NoemaTasksActivityAttributes>?

  var settingsDetail: String {
    if let blocker = status?.blocker, !blocker.isEmpty { return blocker }
    if !activitiesEnabled { return "Live Activities are disabled in this device's system settings." }
    if status?.enabled == true {
      return "Noema Tasks can keep an active task visible on your Lock Screen and Dynamic Island."
    }
    if status?.available == false { return "Live Activities are not available for this server." }
    return "Show the active Noema task on your Lock Screen and Dynamic Island."
  }

  var canEnable: Bool {
    status?.available == true && status?.enabled == false && activitiesEnabled && !isWorking
  }

  var hasRunningActivity: Bool {
    guard let profile else { return false }
    return !matchingActivities(for: profile).isEmpty
  }

  var canRunDiagnostic: Bool {
    profile != nil && client != nil && activitiesEnabled && isEnabled
      && status?.available == true && !isWorking && !isRunningDiagnostic
  }

  var hasDiagnosticActivity: Bool { diagnosticActivity != nil }

  func configure(profile: NoemaProfile?, client: ApolloClient?) async {
    trace(
      "configured",
      fields: ["paired": String(profile != nil), "hasClient": String(client != nil)]
    )
    statusGeneration &+= 1
    await cancelRegistration()
    stopObservers()
    if let profile {
      await endActivitiesNotMatching(profile)
    } else {
      await endAllActivities()
    }
    self.profile = profile
    self.client = client
    status = nil
    errorMessage = nil
    diagnosticMessage = nil
    pushToStartToken = nil
    isEnabled = false
    guard profile != nil, client != nil else { return }
    await refresh()
  }

  func refresh() async {
    guard !isWorking else { return }
    statusGeneration &+= 1
    let generation = statusGeneration
    activitiesEnabled = ActivityAuthorizationInfo().areActivitiesEnabled
    trace(
      "authorization_snapshot",
      fields: [
        "activitiesEnabled": String(activitiesEnabled),
        "activityCount": String(Activity<NoemaTasksActivityAttributes>.activities.count)
      ]
    )
    guard let client else { return }
    isLoading = true
    defer { isLoading = false }
    do {
      let response = try await client.fetch(
        query: NoemaAPI.ClientLiveActivityStatusQuery(),
        cachePolicy: .networkOnly
      )
      if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
      guard let value = response.data?.clientLiveActivityStatus else { throw LiveActivityError.emptyResponse }
      guard generation == statusGeneration else { return }
      applyStatus(
        available: value.available,
        blocker: value.blocker,
        enabled: value.enabled,
        registered: value.registered,
        environment: value.environment?.rawValue
      )
      trace(
        "status_loaded",
        fields: [
          "available": String(value.available),
          "enabled": String(value.enabled),
          "registered": String(value.registered),
          "environment": value.environment?.rawValue ?? "none"
        ]
      )
      errorMessage = nil
      guard value.available, value.enabled, activitiesEnabled else {
        await cancelRegistration()
        stopObservers()
        await endActivities(for: profile, notifyServer: false)
        return
      }
      startActivityObserver()
      observeExistingActivities()
      readPushToStartToken()
      startTokenObserver()
      await registerPushToStartTokenIfPossible()
    } catch {
      guard generation == statusGeneration else { return }
      errorMessage = "Live Activity status could not be loaded."
      trace("status_failed", error: error)
    }
  }

  func setEnabled(_ enabled: Bool) {
    guard enabled != isEnabled, !isWorking else { return }
    if enabled {
      guard canEnable else { return }
      isEnabled = true
      isWorking = true
      statusGeneration &+= 1
      Task { await finishEnabling() }
    } else {
      isEnabled = false
      isWorking = true
      statusGeneration &+= 1
      Task { await disable() }
    }
  }

  private func finishEnabling() async {
    statusGeneration &+= 1
    defer { isWorking = false }
    errorMessage = nil
    activitiesEnabled = ActivityAuthorizationInfo().areActivitiesEnabled
    guard activitiesEnabled else {
      isEnabled = false
      return
    }
    readPushToStartToken()
    startTokenObserver()
    startActivityObserver()
    await registerPushToStartTokenIfPossible()
  }

  /// Disable server registration first, then always end local activities and cancel token observers.
  @discardableResult
  func disable() async -> Bool {
    isWorking = true
    defer { isWorking = false }
    statusGeneration &+= 1
    isEnabled = false
    await cancelRegistration()
    stopObservers()
    var succeeded = true
    if let client {
      do {
        let response = try await client.perform(mutation: NoemaAPI.DisableClientLiveActivitiesMutation())
        if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
        guard let value = response.data?.disableClientLiveActivities else { throw LiveActivityError.emptyResponse }
        applyStatus(
          available: value.available,
          blocker: value.blocker,
          enabled: value.enabled,
          registered: value.registered,
          environment: value.environment?.rawValue
        )
        errorMessage = nil
      } catch {
        succeeded = false
        isEnabled = status?.enabled == true
        errorMessage = "Noema must reach the paired server before Live Activities can be removed."
      }
    } else {
      succeeded = false
      isEnabled = status?.enabled == true
      errorMessage = "Noema must reach the paired server before Live Activities can be removed."
    }
    await endActivities(for: profile, notifyServer: false)
    pushToStartToken = nil
    return succeeded
  }

  func clearLocalActivities() async {
    statusGeneration &+= 1
    isEnabled = false
    await cancelRegistration()
    stopObservers()
    await endAllActivities()
    pushToStartToken = nil
    status = nil
  }

  func openSystemSettings() {
    guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
    UIApplication.shared.open(url)
  }

  func scenePhaseChanged(_ active: Bool) {
    trace("scene_phase_changed", fields: ["active": String(active)])
    guard active else { return }
    Task { await refresh() }
  }

  func toggleDiagnostic() async {
    if let diagnosticActivity {
      isRunningDiagnostic = true
      defer { isRunningDiagnostic = false }
      await end(diagnosticActivity, notifyServer: false)
      self.diagnosticActivity = nil
      diagnosticMessage = "The test Live Activity ended."
      trace(
        "diagnostic_ended",
        fields: ["activityId": diagnosticActivity.attributes.activityId]
      )
      return
    }
    guard canRunDiagnostic, let profile else { return }
    isRunningDiagnostic = true
    defer { isRunningDiagnostic = false }
    let now = Date().timeIntervalSince1970
    let attributes = NoemaTasksActivityAttributes(
      activityId: "live_activity:diagnostic:\(UUID().uuidString.lowercased())",
      clientId: profile.clientId,
      serverOrigin: profile.origin.absoluteString
    )
    let state = NoemaTasksActivityAttributes.ContentState(
      focusTaskId: "task:diagnostic",
      focusTitle: "Noema Live Activity test",
      projectName: nil,
      agentName: "Noema",
      phase: .working,
      statusLabel: "Testing",
      activeTaskCount: 1,
      startedAtEpoch: now,
      updatedAtEpoch: now,
      updateLabel: "This test bypasses APNs."
    )
    do {
      let activity = try Activity.request(
        attributes: attributes,
        content: ActivityContent(state: state, staleDate: Date().addingTimeInterval(300)),
        pushType: nil
      )
      diagnosticActivity = activity
      diagnosticMessage = "A test Live Activity started. Check the Lock Screen or Dynamic Island."
      trace(
        "diagnostic_started",
        fields: [
          "activityId": attributes.activityId,
          "localActivityId": activity.id,
          "state": Self.activityStateName(activity.activityState)
        ]
      )
      observe(activity)
    } catch {
      diagnosticMessage = "The test Live Activity could not start."
      trace("diagnostic_failed", error: error)
    }
  }

  private func applyStatus(
    available: Bool,
    blocker: String?,
    enabled: Bool,
    registered: Bool,
    environment: String?
  ) {
    isEnabled = enabled
    status = ClientLiveActivityStatusModel(
      available: available,
      blocker: blocker,
      enabled: enabled,
      registered: registered,
      environment: environment
    )
  }

  private func readPushToStartToken() {
    if let token = Activity<NoemaTasksActivityAttributes>.pushToStartToken {
      pushToStartToken = token
      trace("push_to_start_token_read", fields: ["available": "true"])
    } else {
      trace("push_to_start_token_read", fields: ["available": "false"])
    }
  }

  private func startTokenObserver() {
    guard pushToStartTask == nil else { return }
    pushToStartTask = Task { [weak self] in
      for await token in Activity<NoemaTasksActivityAttributes>.pushToStartTokenUpdates {
        guard !Task.isCancelled else { return }
        self?.pushToStartToken = token
        self?.trace("push_to_start_token_changed", fields: ["available": "true"])
        self?.scheduleRegistration()
      }
    }
  }

  private func stopObservers() {
    let tasks = [pushToStartTask, activityUpdatesTask]
      .compactMap { $0 } + Array(updateTokenTasks.values) + Array(activityStateTasks.values)
    tasks.forEach { $0.cancel() }
    pushToStartTask = nil
    activityUpdatesTask = nil
    updateTokenTasks.removeAll()
    activityStateTasks.removeAll()
  }

  private func registerPushToStartTokenIfPossible() async {
    scheduleRegistration()
    await registrationTask?.value
  }

  private func scheduleRegistration() {
    guard client != nil, let token = pushToStartToken, isEnabled,
          status?.available == true else { return }
    guard registrationTask == nil else {
      registrationPending = true
      return
    }
    registrationPending = false
    registrationGeneration &+= 1
    let generation = registrationGeneration
    registrationTask = Task { [weak self] in
      await self?.performRegistration(token: token, generation: generation)
      self?.registrationEnded(generation: generation, token: token)
    }
  }

  private func cancelRegistration() async {
    let task = registrationTask
    registrationGeneration &+= 1
    task?.cancel()
    await task?.value
    registrationTask = nil
    registrationPending = false
  }

  private func registrationEnded(generation: Int, token: Data) {
    guard generation == registrationGeneration else { return }
    registrationTask = nil
    if registrationPending || pushToStartToken != token {
      registrationPending = false
      scheduleRegistration()
    }
  }

  private func performRegistration(token: Data, generation: Int) async {
    guard let client, let profile, isEnabled, status?.available == true else { return }
    guard let environment = NoemaAPNSEnvironment.current else {
      errorMessage = "Noema could not read this app's APNs environment."
      trace("registration_failed", fields: ["reason": "missing_apns_environment"])
      return
    }
    do {
      let activeActivityIds = matchingActivities(for: profile)
        .filter {
          ($0.activityState == .active || $0.activityState == .stale)
            && !Self.isDiagnosticActivity($0)
        }
        .map(\.attributes.activityId)
        .sorted()
      trace(
        "registration_started",
        fields: [
          "activeActivityCount": String(activeActivityIds.count),
          "activeActivityIds": activeActivityIds.joined(separator: ",")
        ]
      )
      let input = NoemaAPI.RegisterClientLiveActivitiesInput(
        pushToStartToken: token.base64URLEncoded,
        environment: GraphQLEnum(environment),
        activeActivityIds: activeActivityIds
      )
      let response = try await client.perform(
        mutation: NoemaAPI.RegisterClientLiveActivitiesMutation(input: input)
      )
      if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
      guard let value = response.data?.registerClientLiveActivities else { throw LiveActivityError.emptyResponse }
      guard value.environment?.rawValue == environment.rawValue else {
        throw LiveActivityError.server("The server expects a different APNs environment.")
      }
      guard isEnabled, generation == registrationGeneration else { return }
      applyStatus(
        available: value.available,
        blocker: value.blocker,
        enabled: value.enabled,
        registered: value.registered,
        environment: value.environment?.rawValue
      )
      errorMessage = nil
      trace(
        "registration_finished",
        fields: [
          "registered": String(value.registered),
          "activeActivityCount": String(activeActivityIds.count)
        ]
      )
    } catch {
      guard isEnabled, generation == registrationGeneration else { return }
      isEnabled = status?.enabled == true
      errorMessage = "Noema could not register Live Activities on this device."
      trace("registration_failed", error: error)
    }
  }

  private func observeExistingActivities() {
    guard let profile else { return }
    for activity in matchingActivities(for: profile) { observe(activity) }
  }

  private func startActivityObserver() {
    guard activityUpdatesTask == nil else { return }
    activityUpdatesTask = Task { [weak self] in
      for await activity in Activity<NoemaTasksActivityAttributes>.activityUpdates {
        guard !Task.isCancelled else { return }
        guard let self, let profile = self.profile,
              activity.attributes.clientId == profile.clientId,
              activity.attributes.serverOrigin == profile.origin.absoluteString else { continue }
        self.observe(activity)
      }
    }
  }

  private func observe(_ activity: Activity<NoemaTasksActivityAttributes>) {
    let duplicates = Activity<NoemaTasksActivityAttributes>.activities.filter {
      $0.attributes.activityId == activity.attributes.activityId
        && $0.attributes.clientId == activity.attributes.clientId
        && $0.attributes.serverOrigin == activity.attributes.serverOrigin
        && !endingActivityIDs.contains($0.id)
    }
    guard let canonical = duplicates.min(by: { $0.id < $1.id }) else { return }
    for duplicate in duplicates where duplicate.id != canonical.id {
      endingActivityIDs.insert(duplicate.id)
      Task { await end(duplicate, notifyServer: false) }
    }
    guard canonical.id == activity.id else {
      observe(canonical)
      return
    }
    trace(
      "activity_observed",
      fields: [
        "activityId": activity.attributes.activityId,
        "localActivityId": activity.id,
        "state": Self.activityStateName(activity.activityState)
      ]
    )
    if !Self.isDiagnosticActivity(activity) {
      scheduleRegistration()
    }
    let activityID = activity.id
    if updateTokenTasks[activityID] == nil {
      updateTokenTasks[activityID] = Task { [weak self] in
        if let token = activity.pushToken {
          await self?.registerUpdateToken(token, activity: activity)
        }
        for await token in activity.pushTokenUpdates {
          guard !Task.isCancelled else { return }
          await self?.registerUpdateToken(token, activity: activity)
        }
        self?.updateTokenTasks[activityID] = nil
      }
    }
    guard activityStateTasks[activityID] == nil else { return }
    activityStateTasks[activityID] = Task { [weak self] in
      for await state in activity.activityStateUpdates {
        guard !Task.isCancelled else { return }
        self?.trace(
          "activity_state_changed",
          fields: [
            "activityId": activity.attributes.activityId,
            "localActivityId": activity.id,
            "state": Self.activityStateName(state)
          ]
        )
        if state == .dismissed {
          await self?.activityWasDismissed(activity)
          return
        }
      }
      self?.activityStateTasks[activityID] = nil
    }
  }

  private func registerUpdateToken(
    _ token: Data,
    activity: Activity<NoemaTasksActivityAttributes>
  ) async {
    trace(
      "update_token_observed",
      fields: [
        "activityId": activity.attributes.activityId,
        "localActivityId": activity.id
      ]
    )
    guard let client, status?.available == true, status?.enabled == true else { return }
    do {
      let input = NoemaAPI.RegisterClientLiveActivityUpdateInput(
        activityId: activity.attributes.activityId,
        updateToken: token.base64URLEncoded
      )
      let response = try await client.perform(
        mutation: NoemaAPI.RegisterClientLiveActivityUpdateMutation(input: input)
      )
      if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
      guard response.data?.registerClientLiveActivityUpdate == true else {
        trace(
          "update_token_rejected",
          fields: ["activityId": activity.attributes.activityId]
        )
        await end(activity, notifyServer: false)
        return
      }
      trace(
        "update_token_registered",
        fields: ["activityId": activity.attributes.activityId]
      )
    } catch {
      errorMessage = "Noema could not register Live Activity updates."
      trace(
        "update_token_registration_failed",
        error: error,
        fields: ["activityId": activity.attributes.activityId]
      )
    }
  }

  private func activityWasDismissed(_ activity: Activity<NoemaTasksActivityAttributes>) async {
    updateTokenTasks[activity.id]?.cancel()
    updateTokenTasks[activity.id] = nil
    activityStateTasks[activity.id] = nil
    if diagnosticActivity?.id == activity.id {
      diagnosticActivity = nil
      diagnosticMessage = "The test Live Activity was dismissed."
      return
    }
    guard !endingActivityIDs.contains(activity.id), let client else { return }
    do {
      let response = try await client.perform(
        mutation: NoemaAPI.DismissClientLiveActivityMutation(activityId: activity.attributes.activityId)
      )
      if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
      trace(
        "dismissal_reported",
        fields: ["activityId": activity.attributes.activityId]
      )
    } catch {
      trace(
        "dismissal_report_failed",
        error: error,
        fields: ["activityId": activity.attributes.activityId]
      )
      // The activity is already dismissed locally; the next status refresh reconciles the server.
    }
  }

  private func endActivities(for profile: NoemaProfile?, notifyServer: Bool) async {
    guard let profile else { return }
    for activity in matchingActivities(for: profile) {
      await end(activity, notifyServer: notifyServer)
    }
  }

  private func end(
    _ activity: Activity<NoemaTasksActivityAttributes>,
    notifyServer: Bool
  ) async {
    let localActivityID = activity.id
    let serverActivityID = activity.attributes.activityId
    endingActivityIDs.insert(localActivityID)
    if notifyServer, let client {
      _ = try? await client.perform(
        mutation: NoemaAPI.DismissClientLiveActivityMutation(activityId: serverActivityID)
      )
    }
    await activity.end(nil, dismissalPolicy: .immediate)
    trace(
      "activity_ended_locally",
      fields: [
        "activityId": serverActivityID,
        "localActivityId": localActivityID,
        "reported": String(notifyServer)
      ]
    )
    updateTokenTasks[localActivityID]?.cancel()
    updateTokenTasks[localActivityID] = nil
    activityStateTasks[localActivityID]?.cancel()
    activityStateTasks[localActivityID] = nil
    endingActivityIDs.remove(localActivityID)
    if diagnosticActivity?.id == localActivityID {
      diagnosticActivity = nil
      diagnosticMessage = "The test Live Activity ended."
    }
  }

  private func matchingActivities(for profile: NoemaProfile) -> [Activity<NoemaTasksActivityAttributes>] {
    Activity<NoemaTasksActivityAttributes>.activities.filter {
      $0.attributes.clientId == profile.clientId
        && $0.attributes.serverOrigin == profile.origin.absoluteString
    }
  }

  private func endActivitiesNotMatching(_ profile: NoemaProfile) async {
    for activity in Activity<NoemaTasksActivityAttributes>.activities
    where activity.attributes.clientId != profile.clientId
      || activity.attributes.serverOrigin != profile.origin.absoluteString {
      await end(activity, notifyServer: false)
    }
  }

  private func endAllActivities() async {
    for activity in Activity<NoemaTasksActivityAttributes>.activities {
      await end(activity, notifyServer: false)
    }
  }

  private func trace(_ event: String, fields: [String: String] = [:]) {
    NoemaDiagnosticTrace.shared.record(category: "live_activity", event: event, fields: fields)
  }

  private func trace(
    _ event: String,
    error: Error,
    fields: [String: String] = [:]
  ) {
    NoemaDiagnosticTrace.shared.record(
      category: "live_activity",
      event: event,
      error: error,
      fields: fields
    )
  }

  private static func activityStateName(_ state: ActivityState) -> String {
    switch state {
    case .active: "active"
    case .dismissed: "dismissed"
    case .ended: "ended"
    case .pending: "pending"
    case .stale: "stale"
    @unknown default: "unknown"
    }
  }

  private static func isDiagnosticActivity(
    _ activity: Activity<NoemaTasksActivityAttributes>
  ) -> Bool {
    activity.attributes.activityId.hasPrefix("live_activity:diagnostic:")
  }
}

private enum LiveActivityError: LocalizedError {
  case emptyResponse
  case server(String)

  var errorDescription: String? {
    switch self {
    case .emptyResponse: "The Live Activity server returned no status."
    case let .server(message): message
    }
  }
}
