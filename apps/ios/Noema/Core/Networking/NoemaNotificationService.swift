import Apollo
import Foundation
import Observation
import UIKit
import UserNotifications
import NoemaAPI

struct ClientNotificationStatusModel: Equatable, Sendable {
  let available: Bool
  let blocker: String?
  let enabled: Bool
  let environment: String?
}

private struct NotificationRoute {
  let eventKey: String
  let destination: Destination

  enum Destination {
    case chat
    case task(String)
  }
}

@MainActor
@Observable
final class NoemaNotificationService {
  private(set) var status: ClientNotificationStatusModel?
  private(set) var authorizationStatus: UNAuthorizationStatus = .notDetermined
  private(set) var isLoading = false
  private(set) var isWorking = false
  private(set) var errorMessage: String?

  var onChatTap: (@MainActor () -> Void)?
  var onTaskTap: (@MainActor (String) -> Void)?

  private var client: ApolloClient?
  private var profile: NoemaProfile?
  private var deviceToken: Data?
  private var isModelReady = false
  private var isSceneActive = false
  private var isChatVisible = false
  private var pendingRoute: NotificationRoute?
  private var pendingNotificationUserInfo: [AnyHashable: Any]?
  private var handledEventKeys = Set<String>()
  private var presenceTask: Task<Void, Never>?
  private var registrationTask: Task<Void, Never>?
  private var presenceGeneration = 0
  private var registrationGeneration = 0
  private var statusGeneration = 0
  private var desiredEnabled = false

  var chatPromptVisible: Bool {
    status?.available == true
      && status?.enabled == false
      && authorizationStatus == .notDetermined
      && !isWorking
      && !promptDismissed
  }

  var settingsDetail: String {
    if let blocker = status?.blocker, !blocker.isEmpty { return blocker }
    if authorizationStatus == .denied {
      return "Notifications are blocked in this device's system settings."
    }
    if status?.enabled == true {
      return "Noema will alert this device for primary chat replies and items that need you."
    }
    if status?.available == false {
      return "Notifications are not available for this server."
    }
    return "Enable alerts for primary chat replies and items that need you."
  }

  var canEnable: Bool {
    status?.available == true && status?.enabled == false && !isWorking
  }

  func configure(profile: NoemaProfile?, client: ApolloClient?) async {
    statusGeneration &+= 1
    await cancelRegistration()
    self.profile = profile
    self.client = client
    if profile == nil || client == nil {
      status = nil
      desiredEnabled = false
      deviceToken = nil
      pendingNotificationUserInfo = nil
      stopPresence()
      return
    }
    if let pendingNotificationUserInfo {
      self.pendingNotificationUserInfo = nil
      receiveTap(userInfo: pendingNotificationUserInfo)
    }
    await refreshPermission()
    guard await refreshStatus() else { return }
    registerForEligibleLaunch()
    reconcilePresence()
  }

  func markModelReady() {
    isModelReady = true
    deliverPendingRouteIfReady()
  }

  func markModelNotReady() {
    isModelReady = false
    pendingRoute = nil
    stopPresence()
  }

  func refresh() async {
    await refreshPermission()
    guard await refreshStatus() else { return }
    registerForEligibleLaunch()
    reconcilePresence()
  }

  func dismissPrompt() {
    guard let key = promptDismissalKey else { return }
    UserDefaults.standard.set(true, forKey: key)
  }

  func scenePhaseChanged(_ active: Bool) {
    isSceneActive = active
    if active {
      Task {
        await refreshPermission()
        guard await refreshStatus() else { return }
        registerForEligibleLaunch()
        reconcilePresence()
      }
    } else {
      stopPresence()
    }
  }

  func chatVisibilityChanged(_ visible: Bool) {
    isChatVisible = visible
    reconcilePresence()
  }

  func requestAuthorizationAndEnable() async {
    guard canEnable, profile != nil, client != nil else { return }
    statusGeneration &+= 1
    desiredEnabled = true
    errorMessage = nil
    isWorking = true
    defer { isWorking = false }

    let granted: Bool
    do {
      granted = try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound])
    } catch {
      desiredEnabled = false
      errorMessage = "Noema could not request notification permission."
      return
    }
    await refreshPermission()
    guard granted else {
      desiredEnabled = false
      errorMessage = settingsDetail
      return
    }

    UIApplication.shared.registerForRemoteNotifications()
    scheduleRegistration()
  }

  @discardableResult
  func disable() async -> Bool {
    isWorking = true
    defer { isWorking = false }
    statusGeneration &+= 1
    desiredEnabled = false
    await cancelRegistration()
    stopPresence()
    guard let client else {
      errorMessage = "Noema must reach the paired server before notifications can be removed."
      return false
    }
    do {
      let response = try await client.perform(mutation: NoemaAPI.DisableClientNotificationsMutation())
      if let message = response.errors?.first?.message { throw NotificationError.server(message) }
      guard let value = response.data?.disableClientNotifications else { throw NotificationError.emptyResponse }
      applyStatus(
        available: value.available,
        blocker: value.blocker,
        enabled: value.enabled,
        environment: value.environment?.rawValue
      )
      errorMessage = nil
      clearLocalRegistration()
      return true
    } catch {
      errorMessage = "Noema must reach the paired server before notifications can be removed."
      return false
    }
  }

  func clearLocalRegistration() {
    UIApplication.shared.unregisterForRemoteNotifications()
    deviceToken = nil
  }

  func openSystemSettings() {
    guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
    UIApplication.shared.open(url)
  }

  func didRegister(deviceToken: Data) {
    self.deviceToken = deviceToken
    scheduleRegistration()
  }

  func didFailToRegister(error: Error) {
    deviceToken = nil
    errorMessage = "Noema could not register this device for notifications."
  }

  func presentationOptions(for userInfo: [AnyHashable: Any]) -> UNNotificationPresentationOptions {
    guard let route = validatePayload(userInfo) else { return [] }
    guard case .chat = route.destination, isSceneActive, isChatVisible else { return [.banner, .sound] }
    return []
  }

  func receiveTap(userInfo: [AnyHashable: Any]) {
    guard profile != nil else {
      guard validatePayload(userInfo, requireClient: false) != nil else { return }
      pendingNotificationUserInfo = userInfo
      return
    }
    guard let route = validatePayload(userInfo), handledEventKeys.insert(route.eventKey).inserted else { return }
    pendingRoute = route
    deliverPendingRouteIfReady()
  }

  private func refreshStatus() async -> Bool {
    guard let client else { return false }
    statusGeneration &+= 1
    let generation = statusGeneration
    isLoading = true
    defer { isLoading = false }
    do {
      let response = try await client.fetch(
        query: NoemaAPI.ClientNotificationStatusQuery(),
        cachePolicy: .networkOnly
      )
      if let message = response.errors?.first?.message { throw NotificationError.server(message) }
      guard let value = response.data?.clientNotificationStatus else { throw NotificationError.emptyResponse }
      guard generation == statusGeneration else { return false }
      applyStatus(
        available: value.available,
        blocker: value.blocker,
        enabled: value.enabled,
        environment: value.environment?.rawValue
      )
      errorMessage = nil
      return true
    } catch {
      guard generation == statusGeneration else { return false }
      errorMessage = status == nil
        ? "Notification status could not be loaded."
        : "Notification status could not be refreshed."
      return true
    }
  }

  private func refreshPermission() async {
    authorizationStatus = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
  }

  private func registerForEligibleLaunch() {
    guard status?.available == true,
          status?.enabled == true,
          authorizationStatus == .authorized
    else { return }
    UIApplication.shared.registerForRemoteNotifications()
    scheduleRegistration()
  }

  private func scheduleRegistration() {
    guard registrationTask == nil, client != nil, let token = deviceToken,
          desiredEnabled,
          authorizationStatus == .authorized,
          status?.available == true else { return }
    registrationGeneration &+= 1
    let generation = registrationGeneration
    registrationTask = Task { [weak self] in
      await self?.registerDeviceTokenIfPossible(token: token, generation: generation)
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
    if deviceToken != token { scheduleRegistration() }
  }

  private func registerDeviceTokenIfPossible(token: Data, generation: Int) async {
    guard let client, desiredEnabled, status?.available == true
    else { return }
    let input = NoemaAPI.RegisterClientNotificationsInput(
      deviceToken: token.base64URLEncoded,
      environment: GraphQLEnum(Self.apnsEnvironment)
    )
    do {
      let response = try await client.perform(
        mutation: NoemaAPI.RegisterClientNotificationsMutation(input: input)
      )
      if let message = response.errors?.first?.message { throw NotificationError.server(message) }
      guard let value = response.data?.registerClientNotifications else { throw NotificationError.emptyResponse }
      guard value.environment?.rawValue == Self.apnsEnvironment.rawValue else {
        throw NotificationError.server("The server expects a different APNs environment.")
      }
      guard desiredEnabled, generation == registrationGeneration else { return }
      applyStatus(
        available: value.available,
        blocker: value.blocker,
        enabled: value.enabled,
        environment: value.environment?.rawValue
      )
      errorMessage = nil
      reconcilePresence()
    } catch {
      errorMessage = "Noema could not enable notifications on this device."
    }
  }

  private func applyStatus(available: Bool, blocker: String?, enabled: Bool, environment: String?) {
    desiredEnabled = enabled
    status = ClientNotificationStatusModel(
      available: available,
      blocker: blocker,
      enabled: enabled,
      environment: environment
    )
    reconcilePresence()
  }

  private func reconcilePresence() {
    guard isModelReady, isSceneActive, isChatVisible, status?.enabled == true,
          client != nil, profile != nil
    else {
      stopPresence()
      return
    }
    guard presenceTask == nil else { return }
    guard let client, let profile else { return }
    presenceGeneration &+= 1
    let generation = presenceGeneration
    presenceTask = Task { [weak self] in
      do {
        let stream = try client.subscribe(subscription: NoemaAPI.ClientNotificationPresenceSubscription())
        for try await response in stream {
          guard !Task.isCancelled else { return }
          guard let presence = response.data?.clientNotificationPresence,
                presence.clientId == profile.clientId,
                presence.ready else { continue }
        }
      } catch {
        // Presence is an optimization for suppressing local foreground alerts. Chat remains usable.
      }
      self?.presenceEnded(generation: generation)
    }
  }

  private func presenceEnded(generation: Int) {
    guard generation == presenceGeneration else { return }
    presenceTask = nil
  }

  private func stopPresence() {
    presenceGeneration &+= 1
    presenceTask?.cancel()
    presenceTask = nil
  }

  private func deliverPendingRouteIfReady() {
    guard isModelReady, let route = pendingRoute else { return }
    pendingRoute = nil
    switch route.destination {
    case .chat:
      onChatTap?()
    case let .task(taskID):
      onTaskTap?(taskID)
    }
  }

  private func validatePayload(_ userInfo: [AnyHashable: Any], requireClient: Bool = true) -> NotificationRoute? {
    let version: Int?
    if let value = userInfo["version"] as? NSNumber { version = value.intValue }
    else if let value = userInfo["version"] as? String { version = Int(value) }
    else { version = nil }
    guard version == 1,
          let eventKey = userInfo["eventKey"] as? String,
          !eventKey.isEmpty,
          eventKey.utf8.count <= 256,
          let clientId = userInfo["clientId"] as? String,
          let route = userInfo["route"] as? String
    else { return nil }
    if requireClient, clientId != profile?.clientId { return nil }
    switch route {
    case "chat":
      return NotificationRoute(eventKey: eventKey, destination: .chat)
    case "task":
      guard let taskID = userInfo["taskId"] as? String,
            (1...256).contains(taskID.utf8.count),
            taskID.hasPrefix("task:") else { return nil }
      return NotificationRoute(eventKey: eventKey, destination: .task(taskID))
    default:
      return nil
    }
  }

  private static var apnsEnvironment: NoemaAPI.ApnsEnvironment {
    #if DEBUG
    .development
    #else
    .production
    #endif
  }

  private var promptDismissalKey: String? {
    profile.map { "dev.noema.app.ios.notifications-prompt.\($0.origin.absoluteString).\($0.clientId)" }
  }

  private var promptDismissed: Bool {
    guard let key = promptDismissalKey else { return false }
    return UserDefaults.standard.bool(forKey: key)
  }
}

private enum NotificationError: LocalizedError {
  case emptyResponse
  case server(String)

  var errorDescription: String? {
    switch self {
    case .emptyResponse: "The notification server returned no status."
    case let .server(message): message
    }
  }
}

@MainActor
final class NoemaApplicationDelegate: NSObject, UIApplicationDelegate, @preconcurrency UNUserNotificationCenterDelegate {
  nonisolated(unsafe) static weak var notifications: NoemaNotificationService?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    UNUserNotificationCenter.current().delegate = self
    return true
  }

  func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
    Self.notifications?.didRegister(deviceToken: deviceToken)
  }

  func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
    Self.notifications?.didFailToRegister(error: error)
  }

  func userNotificationCenter(
    _ center: UNUserNotificationCenter,
    willPresent notification: UNNotification,
    withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
  ) {
    completionHandler(Self.notifications?.presentationOptions(for: notification.request.content.userInfo) ?? [])
  }

  func userNotificationCenter(
    _ center: UNUserNotificationCenter,
    didReceive response: UNNotificationResponse,
    withCompletionHandler completionHandler: @escaping () -> Void
  ) {
    Self.notifications?.receiveTap(userInfo: response.notification.request.content.userInfo)
    completionHandler()
  }
}
