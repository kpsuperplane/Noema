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

  private var client: ApolloClient?
  private var profile: NoemaProfile?
  private var pushToStartToken: Data?
  private var pushToStartTask: Task<Void, Never>?
  private var registrationTask: Task<Void, Never>?
  private var registrationGeneration = 0
  private var statusGeneration = 0
  private var desiredEnabled = false
  private var activityUpdatesTask: Task<Void, Never>?
  private var updateTokenTasks: [String: Task<Void, Never>] = [:]
  private var activityStateTasks: [String: Task<Void, Never>] = [:]
  private var endingActivityIDs = Set<String>()

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

  var isEnabled: Bool { desiredEnabled }

  var hasRunningActivity: Bool {
    guard let profile else { return false }
    return !matchingActivities(for: profile).isEmpty
  }

  func configure(profile: NoemaProfile?, client: ApolloClient?) async {
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
    pushToStartToken = nil
    desiredEnabled = false
    guard profile != nil, client != nil else { return }
    await refresh()
  }

  func refresh() async {
    statusGeneration &+= 1
    let generation = statusGeneration
    activitiesEnabled = ActivityAuthorizationInfo().areActivitiesEnabled
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
    }
  }

  func enable() async {
    guard canEnable else { return }
    statusGeneration &+= 1
    desiredEnabled = true
    isWorking = true
    defer { isWorking = false }
    errorMessage = nil
    activitiesEnabled = ActivityAuthorizationInfo().areActivitiesEnabled
    guard activitiesEnabled else {
      desiredEnabled = false
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
    desiredEnabled = false
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
        desiredEnabled = status?.enabled == true
        errorMessage = "Noema must reach the paired server before Live Activities can be removed."
      }
    } else {
      succeeded = false
      desiredEnabled = status?.enabled == true
      errorMessage = "Noema must reach the paired server before Live Activities can be removed."
    }
    await endActivities(for: profile, notifyServer: false)
    pushToStartToken = nil
    return succeeded
  }

  func clearLocalActivities() async {
    statusGeneration &+= 1
    desiredEnabled = false
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
    guard active else { return }
    Task { await refresh() }
  }

  private func applyStatus(
    available: Bool,
    blocker: String?,
    enabled: Bool,
    registered: Bool,
    environment: String?
  ) {
    desiredEnabled = enabled
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
    }
  }

  private func startTokenObserver() {
    guard pushToStartTask == nil else { return }
    pushToStartTask = Task { [weak self] in
      for await token in Activity<NoemaTasksActivityAttributes>.pushToStartTokenUpdates {
        guard !Task.isCancelled else { return }
        self?.pushToStartToken = token
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
    guard registrationTask == nil, client != nil, let token = pushToStartToken,
          desiredEnabled,
          status?.available == true else { return }
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
  }

  private func registrationEnded(generation: Int, token: Data) {
    guard generation == registrationGeneration else { return }
    registrationTask = nil
    if pushToStartToken != token { scheduleRegistration() }
  }

  private func performRegistration(token: Data, generation: Int) async {
    guard let client, let profile, desiredEnabled, status?.available == true else { return }
    do {
      let activeActivityIds = matchingActivities(for: profile)
        .filter { $0.activityState == .active || $0.activityState == .stale }
        .map(\.attributes.activityId)
        .sorted()
      let input = NoemaAPI.RegisterClientLiveActivitiesInput(
        pushToStartToken: token.base64URLEncoded,
        environment: GraphQLEnum(Self.apnsEnvironment),
        activeActivityIds: activeActivityIds
      )
      let response = try await client.perform(
        mutation: NoemaAPI.RegisterClientLiveActivitiesMutation(input: input)
      )
      if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
      guard let value = response.data?.registerClientLiveActivities else { throw LiveActivityError.emptyResponse }
      guard value.environment?.rawValue == Self.apnsEnvironment.rawValue else {
        throw LiveActivityError.server("The server expects a different APNs environment.")
      }
      guard desiredEnabled, generation == registrationGeneration else { return }
      applyStatus(
        available: value.available,
        blocker: value.blocker,
        enabled: value.enabled,
        registered: value.registered,
        environment: value.environment?.rawValue
      )
      errorMessage = nil
    } catch {
      guard desiredEnabled, generation == registrationGeneration else { return }
      desiredEnabled = status?.enabled == true
      errorMessage = "Noema could not register Live Activities on this device."
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
        await end(activity, notifyServer: false)
        return
      }
    } catch {
      errorMessage = "Noema could not register Live Activity updates."
    }
  }

  private func activityWasDismissed(_ activity: Activity<NoemaTasksActivityAttributes>) async {
    updateTokenTasks[activity.id]?.cancel()
    updateTokenTasks[activity.id] = nil
    activityStateTasks[activity.id] = nil
    guard !endingActivityIDs.contains(activity.id), let client else { return }
    do {
      let response = try await client.perform(
        mutation: NoemaAPI.DismissClientLiveActivityMutation(activityId: activity.attributes.activityId)
      )
      if let message = response.errors?.first?.message { throw LiveActivityError.server(message) }
    } catch {
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
    updateTokenTasks[localActivityID]?.cancel()
    updateTokenTasks[localActivityID] = nil
    activityStateTasks[localActivityID]?.cancel()
    activityStateTasks[localActivityID] = nil
    endingActivityIDs.remove(localActivityID)
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

  private static var apnsEnvironment: ApnsEnvironment {
    #if DEBUG
    .development
    #else
    .production
    #endif
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
